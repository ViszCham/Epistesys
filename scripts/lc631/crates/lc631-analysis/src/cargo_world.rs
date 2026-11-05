use std::fs;
use std::path::{Path, PathBuf};

use lc631_core::stable_sha256;
use serde_json::Value;

use crate::bounded_input::read_text_file;
use crate::process::{capture_tool, not_started_request, BoundedToolRequest, ToolRunState};
use crate::{CargoWorldReceipt, EvidenceState, StageReceipt};
use lc631_process_supervisor::{ResourceError, ResourceReservationView};

mod resources;
pub(crate) use resources::observe_cargo_world_with_runner;

pub(crate) fn observe_cargo_world(
    repo: &str,
    source_path: Option<&Path>,
    source_revision: Option<&str>,
    execute: bool,
) -> CargoWorldReceipt {
    match observe_cargo_world_read(repo, source_path, source_revision, execute, None) {
        Ok(receipt) => receipt,
        Err(error) => unavailable_cargo_world(error),
    }
}

/// Resource-aware Rust API. Network/execute authority stays with the caller;
/// operational resources do not authorize Cargo build scripts or proc macros.
pub fn observe_cargo_world_with_resources(
    repo: &str,
    source_path: Option<&Path>,
    source_revision: Option<&str>,
    execute: bool,
    resources: &ResourceReservationView<'_>,
) -> Result<CargoWorldReceipt, ResourceError> {
    resources.check()?;
    if let Some(revision) = source_revision {
        let mut expected = String::from("sha256:");
        for byte in resources.source_revision().as_bytes() {
            use std::fmt::Write as _;
            write!(expected, "{byte:02x}").map_err(|_| ResourceError::InvalidConfig)?;
        }
        if revision != expected {
            return Err(ResourceError::SourceMismatch);
        }
    }
    observe_cargo_world_read(repo, source_path, source_revision, execute, Some(resources))
}

fn observe_cargo_world_read(
    repo: &str,
    source_path: Option<&Path>,
    source_revision: Option<&str>,
    execute: bool,
    resources: Option<&ResourceReservationView<'_>>,
) -> Result<CargoWorldReceipt, ResourceError> {
    let manifest = Path::new(repo).join("Cargo.toml");
    let manifest_text = manifest.to_string_lossy().to_string();
    let version = capture_tool(
        BoundedToolRequest {
            program: "cargo",
            args: &["--version"],
            stdin: None,
            timeout_ms: 2_000,
            stdout_limit: 4096,
            stderr_limit: 4096,
        },
        resources,
    )?;
    let cargo_version = (version.reported().state == ToolRunState::Observed
        && !version.reported().stdout_truncated)
        .then(|| bounded_utf8(version.stdout(), 256));
    drop(version);
    let args = [
        "metadata",
        "--format-version=1",
        "--no-deps",
        "--offline",
        "--manifest-path",
        manifest_text.as_str(),
    ];
    let capture = capture_tool(
        BoundedToolRequest {
            program: "cargo",
            args: &args,
            stdin: None,
            timeout_ms: 10_000,
            stdout_limit: 2 * 1024 * 1024,
            stderr_limit: 64 * 1024,
        },
        resources,
    )?;
    let process = capture.reported();
    let parsed = (process.state == ToolRunState::Observed && !process.stdout_truncated)
        .then(|| serde_json::from_slice::<Value>(capture.stdout()).ok())
        .flatten();
    let packages = parsed
        .as_ref()
        .and_then(|value| value["packages"].as_array())
        .cloned()
        .unwrap_or_default();
    let target_kinds = packages
        .iter()
        .flat_map(|package| package["targets"].as_array().into_iter().flatten())
        .flat_map(|target| target["kind"].as_array().into_iter().flatten())
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    let state = if parsed.is_some() {
        EvidenceState::Observed
    } else if process.state == ToolRunState::Unavailable {
        EvidenceState::Unavailable
    } else {
        EvidenceState::Refuted
    };
    let crate_target = match parsed.as_ref() {
        Some(metadata) => {
            locate_source_package(metadata, repo, source_path, source_revision, resources)?
        }
        None => None,
    };
    let process = capture.into_report();
    let crate_check_process = if let Some(package) = crate_target.as_deref().filter(|_| execute) {
        let args = [
            "check",
            "--locked",
            "--offline",
            "--manifest-path",
            manifest_text.as_str(),
            "--package",
            package,
        ];
        Some(
            capture_tool(
                BoundedToolRequest {
                    program: "cargo",
                    args: &args,
                    stdin: None,
                    timeout_ms: 120_000,
                    stdout_limit: 2 * 1024 * 1024,
                    stderr_limit: 2 * 1024 * 1024,
                },
                resources,
            )?
            .into_report(),
        )
    } else {
        None
    };
    let crate_check_state = if !execute {
        EvidenceState::NotAuthorized
    } else if crate_target.is_none() {
        EvidenceState::NeedsEvidence
    } else {
        match crate_check_process.as_ref().map(|result| result.state) {
            Some(ToolRunState::Observed) => EvidenceState::Observed,
            Some(ToolRunState::Unavailable) => EvidenceState::Unavailable,
            Some(ToolRunState::TimedOut) => EvidenceState::NeedsEvidence,
            Some(ToolRunState::Failed) => EvidenceState::NeedsEvidence,
            None => EvidenceState::NeedsEvidence,
        }
    };
    let crate_check_digest = crate_check_process
        .as_ref()
        .filter(|result| result.state == ToolRunState::Observed)
        .map(|result| {
            stable_sha256(&format!(
                "{}:{}:{}:{}",
                source_revision.unwrap_or_default(),
                result.args_digest,
                result.stdout_digest,
                result.stderr_digest
            ))
        });
    let build_execution_observed = crate_check_process
        .as_ref()
        .is_some_and(|result| result.state != ToolRunState::Unavailable);
    Ok(CargoWorldReceipt {
        state,
        cargo_version,
        metadata_digest: (state == EvidenceState::Observed)
            .then(|| process.stdout_digest.clone()),
        package_count: packages.len(),
        workspace_member_count: parsed
            .as_ref()
            .and_then(|value| value["workspace_members"].as_array())
            .map_or(0, Vec::len),
        target_count: packages
            .iter()
            .map(|package| package["targets"].as_array().map_or(0, Vec::len))
            .sum(),
        build_script_targets: target_kinds
            .iter()
            .filter(|kind| **kind == "custom-build")
            .count(),
        proc_macro_targets: target_kinds
            .iter()
            .filter(|kind| **kind == "proc-macro")
            .count(),
        process,
        source_bound: crate_target.is_some(),
        checked_package: crate_target,
        crate_check_state,
        crate_check_digest,
        crate_check_process,
        build_execution_observed,
        claim_boundary: "cargo metadata is always read-only; explicit --execute may add one source-bound cargo check --locked --offline package receipt, which may execute build scripts/proc macros but does not establish feature-matrix coverage, tests, runtime behavior, proof, authority, or output commit"
            .to_string(),
    })
}

fn unavailable_cargo_world(error: ResourceError) -> CargoWorldReceipt {
    let mut process = not_started_request("cargo", &[], "cargo_resource_input_unavailable");
    process.diagnostic = Some(format!("{error:?}"));
    CargoWorldReceipt { state:EvidenceState::Unavailable,cargo_version:None,metadata_digest:None,
        package_count:0,workspace_member_count:0,target_count:0,build_script_targets:0,proc_macro_targets:0,
        process,source_bound:false,checked_package:None,crate_check_state:EvidenceState::NeedsEvidence,
        crate_check_digest:None,crate_check_process:None,build_execution_observed:false,
        claim_boundary:"resource failure; no Cargo observation, source acceptance, authority or completion is established".into() }
}

pub(crate) fn apply_cargo_stages(stages: &mut [StageReceipt], cargo: &CargoWorldReceipt) {
    let evidence = cargo.metadata_digest.iter().cloned().collect::<Vec<_>>();
    update(
        stages,
        "RPA-06",
        cargo.state,
        evidence.clone(),
        &["metadata is not build execution, feature-matrix coverage, or dependency provenance"],
    );
    update(
        stages,
        "RPA-07",
        if cargo.state == EvidenceState::Observed {
            EvidenceState::Candidate
        } else {
            cargo.state
        },
        evidence.clone(),
        &[
            "host/target, profile, features, cfg, resolver, MSRV, panic, LTO, and linker matrix are not enumerated",
        ],
    );
    update(
        stages,
        "RPA-08",
        EvidenceState::NotAuthorized,
        evidence.clone(),
        &[
            "build.rs and proc macros were discovered but not executed; sandboxed execution authority is required",
        ],
    );
    update(
        stages,
        "RPA-11",
        if cargo.state == EvidenceState::Observed {
            EvidenceState::Candidate
        } else {
            cargo.state
        },
        evidence.clone(),
        &[
            "Cargo crate graph is observed; rust-analyzer semantic crate-instance evidence remains separate",
        ],
    );
    update(
        stages,
        "RPA-25",
        if cargo.state == EvidenceState::Observed {
            EvidenceState::Candidate
        } else {
            cargo.state
        },
        evidence,
        &[
            "registry checksums, advisories, license policy, SemVer, API, and MSRV evidence are unbound",
        ],
    );
    if cargo.build_execution_observed {
        update(
            stages,
            "RPA-08",
            EvidenceState::Candidate,
            cargo.crate_check_digest.iter().cloned().collect(),
            &[
                "cargo check was explicitly executed and may have run build.rs or proc macros without an OS-enforced sandbox",
            ],
        );
    }
    if cargo.crate_check_state != EvidenceState::NotAuthorized {
        update(
            stages,
            "RPA-13",
            cargo.crate_check_state,
            cargo.crate_check_digest.iter().cloned().collect(),
            &[
                "source-bound Cargo package acceptance does not establish feature-matrix coverage, semantic intent, unsafe validity, tests, or runtime behavior",
            ],
        );
    }
}

fn locate_source_package(
    metadata: &Value,
    repo: &str,
    source_path: Option<&Path>,
    source_revision: Option<&str>,
    resources: Option<&ResourceReservationView<'_>>,
) -> Result<Option<String>, ResourceError> {
    let Some((source_path, revision)) = source_path.zip(source_revision) else {
        return Ok(None);
    };
    let Some(repo) = fs::canonicalize(repo).ok() else {
        return Ok(None);
    };
    let Some(source) = canonical_source_path(source_path) else {
        return Ok(None);
    };
    if !source.starts_with(&repo) {
        return Ok(None);
    }
    let input = match read_text_file(&source, 262_144, resources) {
        Ok(input) => input,
        Err(error) if resources.is_some() => return Err(ResourceError::Io(error.kind())),
        Err(_) => return Ok(None),
    };
    let text = input.as_str().map_err(|e| ResourceError::Io(e.kind()))?;
    if stable_sha256(text) != revision {
        return Ok(None);
    }
    Ok(metadata["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|package| {
            let manifest = PathBuf::from(package["manifest_path"].as_str()?);
            let root = fs::canonicalize(manifest.parent()?).ok()?;
            let name = package["name"].as_str()?;
            source
                .starts_with(&root)
                .then(|| (root.components().count(), name.to_string()))
        })
        .max_by_key(|(depth, _)| *depth)
        .map(|(_, package)| package))
}

fn canonical_source_path(path: &Path) -> Option<PathBuf> {
    fs::canonicalize(path).ok()
}

fn update(
    stages: &mut [StageReceipt],
    id: &str,
    state: EvidenceState,
    evidence: Vec<String>,
    blockers: &[&str],
) {
    if let Some(stage) = stages.iter_mut().find(|stage| stage.stage_id == id) {
        stage.state = state;
        stage.evidence_digests = evidence;
        stage.blockers = blockers.iter().map(|value| (*value).to_string()).collect();
    }
}

fn bounded_utf8(bytes: &[u8], limit: usize) -> String {
    String::from_utf8_lossy(bytes)
        .chars()
        .take(limit)
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect()
}
