//! Cargo's resource-aware path. File input and each child use the injected run.
use super::*;
use crate::process::{with_input_result, with_tool_result, BoundedToolResult, ToolProcessRunner};

const MANIFEST_CAP: usize = 1024 * 1024;

struct Metadata {
    parsed: Option<Value>,
    process: BoundedToolResult,
}

pub(crate) fn observe_cargo_world_with_runner(
    repo: &str,
    source_path: Option<&Path>,
    source: Option<&str>,
    execute: bool,
    runner: &mut dyn ToolProcessRunner,
) -> Result<CargoWorldReceipt, ResourceError> {
    if repo.len() > 4096 {
        return Err(ResourceError::InvalidConfig);
    }
    let manifest = Path::new(repo).join("Cargo.toml");
    // This descriptor scopes the bounded ingress read; it is not a content
    // witness. Only the actual bytes below bind the Cargo process callbacks.
    let descriptor = format!("cargo-manifest-ingress:{}", manifest.display());
    let manifest_source = with_input_result(runner, &descriptor, MANIFEST_CAP, |view| {
        let input = crate::bounded_input::read_resource_text_file(&manifest, MANIFEST_CAP, view)?;
        Ok(input.as_str()?.to_string())
    })?;
    let tag = manifest_source.as_str();
    let version = with_tool_result(
        runner,
        tag,
        BoundedToolRequest {
            program: "cargo",
            args: &["--version"],
            stdin: None,
            timeout_ms: 2_000,
            stdout_limit: 4096,
            stderr_limit: 4096,
        },
        |process, stdout, _| {
            (process.state == ToolRunState::Observed).then(|| bounded_utf8(stdout, 256))
        },
    )?;
    let manifest_path = manifest
        .to_str()
        .ok_or(ResourceError::Io(std::io::ErrorKind::InvalidInput))?;
    let args = [
        "metadata",
        "--format-version=1",
        "--no-deps",
        "--offline",
        "--manifest-path",
        manifest_path,
    ];
    let metadata = with_tool_result(
        runner,
        tag,
        BoundedToolRequest {
            program: "cargo",
            args: &args,
            stdin: None,
            timeout_ms: 10_000,
            stdout_limit: 2 * 1024 * 1024,
            stderr_limit: 64 * 1024,
        },
        |process, stdout, _| Metadata {
            parsed: (process.state == ToolRunState::Observed)
                .then(|| serde_json::from_slice(stdout).ok())
                .flatten(),
            process: process.clone(),
        },
    )?;
    let state = if metadata.parsed.is_some() {
        EvidenceState::Observed
    } else if metadata.process.state == ToolRunState::Unavailable {
        EvidenceState::Unavailable
    } else {
        EvidenceState::Refuted
    };
    let package = match (metadata.parsed.as_ref(), source_path, source) {
        (Some(parsed), Some(path), Some(source)) => {
            source_package(parsed, repo, path, source, runner)?
        }
        _ => None,
    };
    let revision = source.map(stable_sha256);
    let check = if let Some(package) = package.as_deref().filter(|_| execute) {
        let args = [
            "check",
            "--locked",
            "--offline",
            "--manifest-path",
            manifest_path,
            "--package",
            package,
        ];
        Some(with_tool_result(
            runner,
            tag,
            BoundedToolRequest {
                program: "cargo",
                args: &args,
                stdin: None,
                timeout_ms: 120_000,
                stdout_limit: 2 * 1024 * 1024,
                stderr_limit: 2 * 1024 * 1024,
            },
            |process, _, _| process.clone(),
        )?)
    } else {
        None
    };
    let unchanged = with_input_result(runner, tag, MANIFEST_CAP, |view| {
        let input = crate::bounded_input::read_resource_text_file(&manifest, MANIFEST_CAP, view)?;
        Ok(input.as_str()? == tag)
    })?;
    if !unchanged {
        return Err(ResourceError::SourceMismatch);
    }
    if let (Some(parsed), Some(path), Some(source)) =
        (metadata.parsed.as_ref(), source_path, source)
    {
        if source_package(parsed, repo, path, source, runner)? != package {
            return Err(ResourceError::SourceMismatch);
        }
    }
    let check_state = if !execute {
        EvidenceState::NotAuthorized
    } else if package.is_none() {
        EvidenceState::NeedsEvidence
    } else {
        match check.as_ref().map(|p| p.state) {
            Some(ToolRunState::Observed) => EvidenceState::Observed,
            Some(ToolRunState::Unavailable) => EvidenceState::Unavailable,
            _ => EvidenceState::NeedsEvidence,
        }
    };
    let check_digest = check
        .as_ref()
        .filter(|p| p.state == ToolRunState::Observed)
        .map(|p| {
            stable_sha256(&format!(
                "{}:{}:{}:{}",
                revision.as_deref().unwrap_or_default(),
                p.args_digest,
                p.stdout_digest,
                p.stderr_digest
            ))
        });
    let packages = metadata
        .parsed
        .as_ref()
        .and_then(|v| v["packages"].as_array());
    let targets = packages
        .into_iter()
        .flatten()
        .flat_map(|p| p["targets"].as_array().into_iter().flatten());
    let kinds = targets
        .clone()
        .flat_map(|t| t["kind"].as_array().into_iter().flatten());
    let build_execution_observed = check
        .as_ref()
        .is_some_and(|p| p.state != ToolRunState::Unavailable);
    Ok(CargoWorldReceipt {
        state,cargo_version:version,metadata_digest:(state==EvidenceState::Observed).then(||metadata.process.stdout_digest.clone()),
        package_count:packages.map_or(0,Vec::len),
        workspace_member_count:metadata.parsed.as_ref().and_then(|v|v["workspace_members"].as_array()).map_or(0,Vec::len),
        target_count:targets.count(),build_script_targets:kinds.clone().filter(|v|v.as_str()==Some("custom-build")).count(),
        proc_macro_targets:kinds.filter(|v|v.as_str()==Some("proc-macro")).count(),
        process:metadata.process,source_bound:package.is_some(),checked_package:package,
        crate_check_state:check_state,crate_check_digest:check_digest,crate_check_process:check,build_execution_observed,
        claim_boundary:"cargo metadata is always read-only; explicit --execute may add one source-bound cargo check --locked --offline package receipt, which may execute build scripts/proc macros but does not establish feature-matrix coverage, tests, runtime behavior, proof, authority, or output commit".into(),
    })
}

fn source_package(
    metadata: &Value,
    repo: &str,
    path: &Path,
    source: &str,
    runner: &mut dyn ToolProcessRunner,
) -> Result<Option<String>, ResourceError> {
    let root = fs::canonicalize(repo).map_err(|e| ResourceError::Io(e.kind()))?;
    let path = fs::canonicalize(path).map_err(|e| ResourceError::Io(e.kind()))?;
    if !path.starts_with(&root) {
        return Ok(None);
    }
    let matches = with_input_result(runner, source, 262_144, |view| {
        let input = crate::bounded_input::read_resource_text_file(&path, 262_144, view)?;
        Ok(input.as_str()? == source)
    })?;
    if !matches {
        return Ok(None);
    }
    Ok(metadata["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|package| {
            let manifest = Path::new(package["manifest_path"].as_str()?);
            let root = fs::canonicalize(manifest.parent()?).ok()?;
            path.starts_with(&root).then(|| {
                (
                    root.components().count(),
                    package["name"].as_str().map(str::to_owned),
                )
            })
        })
        .filter_map(|(depth, name)| name.map(|name| (depth, name)))
        .max_by_key(|(depth, _)| *depth)
        .map(|(_, name)| name))
}
