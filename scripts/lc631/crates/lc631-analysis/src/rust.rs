use lc631_core::stable_sha256;
use std::sync::{Mutex, OnceLock};

use crate::process::{run_bounded_tool, BoundedToolRequest, ToolRunState};
use crate::{
    CompilerArtifactKind, CompilerArtifactReceipt, EvidenceState, RustAnalysisSurface,
    RustSourceMetrics, StageReceipt,
};

const MAX_RUST_SOURCE_BYTES: usize = 64 * 1024;
const NIGHTLY_TOOLCHAIN: &str = "+nightly-2026-07-01";

pub(crate) fn analyze_rust_source(source: &str) -> RustAnalysisSurface {
    let _compiler_guard = compiler_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let source_revision = stable_sha256(source);
    let crate_name = format!("lc631_unit_{}", &source_revision[7..19]);
    if source.len() > MAX_RUST_SOURCE_BYTES {
        return RustAnalysisSurface {
            schema_version: "lc631-rust-analysis.v1".to_string(),
            source_revision,
            source_bytes: source.len(),
            complete_source: true,
            toolchain_identity: None,
            artifacts: Vec::new(),
            metrics: source_metrics(source),
            compiler_accepted: false,
            creates_authority: false,
            creates_output_commit: false,
            unknowns: vec!["Rust source exceeds the 64 KiB bounded compiler limit".to_string()],
            claim_boundary: claim_boundary(),
        };
    }

    let artifacts = vec![
        run_artifact(
            CompilerArtifactKind::RustcVersion,
            &["--version", "--verbose"],
            None,
            None,
        ),
        run_artifact(
            CompilerArtifactKind::Mir,
            &[
                "--crate-name",
                "lc631_unit",
                "--crate-type=lib",
                "--edition=2021",
                "--emit=mir=-",
                "-",
            ],
            Some(source),
            Some(&crate_name),
        ),
        run_artifact(
            CompilerArtifactKind::LlvmIr,
            &[
                "--crate-name",
                "lc631_unit",
                "--crate-type=lib",
                "--edition=2021",
                "--emit=llvm-ir=-",
                "-",
            ],
            Some(source),
            Some(&crate_name),
        ),
        run_artifact(
            CompilerArtifactKind::Assembly,
            &[
                "--crate-name",
                "lc631_unit",
                "--crate-type=lib",
                "--edition=2021",
                "--emit=asm=-",
                "-",
            ],
            Some(source),
            Some(&crate_name),
        ),
        run_artifact(
            CompilerArtifactKind::Object,
            &[
                "--crate-name",
                "lc631_unit",
                "--crate-type=lib",
                "--edition=2021",
                "--emit=obj=-",
                "-",
            ],
            Some(source),
            Some(&crate_name),
        ),
        run_nightly_artifact(
            CompilerArtifactKind::ExpandedSource,
            "expanded",
            source,
            &crate_name,
        ),
        run_nightly_artifact(CompilerArtifactKind::Hir, "hir-tree", source, &crate_name),
        run_nightly_artifact(CompilerArtifactKind::Thir, "thir-tree", source, &crate_name),
        run_artifact(
            CompilerArtifactKind::RustAnalyzerVersion,
            &["--version"],
            None,
            None,
        ),
    ];
    let toolchain_identity = artifacts
        .iter()
        .find(|artifact| artifact.kind == CompilerArtifactKind::RustcVersion)
        .filter(|artifact| artifact.state == EvidenceState::Observed)
        .map(|artifact| bounded_utf8(&artifact.process.stdout, 512));
    let compiler_accepted = [
        CompilerArtifactKind::Mir,
        CompilerArtifactKind::LlvmIr,
        CompilerArtifactKind::Object,
    ]
    .into_iter()
    .all(|kind| {
        artifacts
            .iter()
            .any(|artifact| artifact.kind == kind && artifact.state == EvidenceState::Observed)
    });
    let mut unknowns = vec![
        "Cargo workspace, features, cfg, build scripts, proc macros, dependencies, tests, runtime behavior, and host output are unobserved"
            .to_string(),
        "compiler acceptance and emitted artifacts do not establish unsafe soundness or user intent"
            .to_string(),
    ];
    if artifacts
        .iter()
        .any(|artifact| artifact.state == EvidenceState::Unavailable)
    {
        unknowns
            .push("one or more optional Rust compiler/analyzer views are unavailable".to_string());
    }

    RustAnalysisSurface {
        schema_version: "lc631-rust-analysis.v1".to_string(),
        source_revision,
        source_bytes: source.len(),
        complete_source: true,
        toolchain_identity,
        artifacts,
        metrics: source_metrics(source),
        compiler_accepted,
        creates_authority: false,
        creates_output_commit: false,
        unknowns,
        claim_boundary: claim_boundary(),
    }
}

fn compiler_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

pub(crate) fn apply_rust_stages(stages: &mut [StageReceipt], rust: &RustAnalysisSurface) {
    let digest = |kind| artifact_digest(rust, kind);
    update(
        stages,
        "RPA-09",
        state_for(rust, CompilerArtifactKind::Mir),
        digest(CompilerArtifactKind::Mir),
        &["rustc MIR is compiler observation, not lossless source or full macro provenance"],
    );
    update(
        stages,
        "RPA-10",
        state_for(rust, CompilerArtifactKind::ExpandedSource),
        digest(CompilerArtifactKind::ExpandedSource),
        &["expanded source is nightly/version specific and proc-macro execution is not sandboxed"],
    );
    let ir_state = if [
        CompilerArtifactKind::Hir,
        CompilerArtifactKind::Thir,
        CompilerArtifactKind::Mir,
    ]
    .into_iter()
    .all(|kind| state_for(rust, kind) == EvidenceState::Observed)
    {
        EvidenceState::Observed
    } else {
        EvidenceState::NeedsEvidence
    };
    let ir_digests = [
        CompilerArtifactKind::Hir,
        CompilerArtifactKind::Thir,
        CompilerArtifactKind::Mir,
    ]
    .into_iter()
    .flat_map(&digest)
    .collect::<Vec<_>>();
    update(stages, "RPA-12", ir_state, ir_digests, &["IR outputs are human/debug formats and mapping across compiler revisions remains unstable"]);
    update(
        stages,
        "RPA-13",
        if rust.compiler_accepted {
            EvidenceState::Observed
        } else {
            EvidenceState::Refuted
        },
        digest(CompilerArtifactKind::Mir),
        &["compiler acceptance does not establish semantic intent or unsafe validity"],
    );
    update(stages, "RPA-14", EvidenceState::Candidate, vec![rust.source_revision.clone()], &["unsafe markers are an inventory; all-safe-caller soundness and provenance remain unproved"]);
    let backend_digests = [
        CompilerArtifactKind::LlvmIr,
        CompilerArtifactKind::Assembly,
        CompilerArtifactKind::Object,
    ]
    .into_iter()
    .flat_map(&digest)
    .collect::<Vec<_>>();
    let backend_state = if backend_digests.len() == 3 {
        EvidenceState::Observed
    } else {
        EvidenceState::NeedsEvidence
    };
    update(
        stages,
        "RPA-15",
        backend_state,
        backend_digests.clone(),
        &["layout, ABI, FFI, unwind, callback, and allocator obligations remain target-specific"],
    );
    update(
        stages,
        "RPA-16",
        if rust.metrics.async_markers > 0 {
            EvidenceState::Candidate
        } else {
            EvidenceState::NeedsEvidence
        },
        vec![rust.source_revision.clone()],
        &["async marker inventory does not establish Pin, drop, wake, or cancellation behavior"],
    );
    update(
        stages,
        "RPA-17",
        if rust.metrics.atomic_markers > 0 {
            EvidenceState::Candidate
        } else {
            EvidenceState::NeedsEvidence
        },
        vec![rust.source_revision.clone()],
        &["atomic marker inventory does not explore weak memory, races, or schedules"],
    );
    update(stages, "RPA-18", backend_state, backend_digests, &["emitted assembly/object digest is not disassembly validation, runtime behavior, or performance evidence"]);
    update(
        stages,
        "RPA-11",
        state_for(rust, CompilerArtifactKind::RustAnalyzerVersion),
        digest(CompilerArtifactKind::RustAnalyzerVersion),
        &["rust-analyzer capability/version is not a crate-graph HIR observation"],
    );
}

fn run_nightly_artifact(
    kind: CompilerArtifactKind,
    unpretty: &str,
    source: &str,
    crate_name: &str,
) -> CompilerArtifactReceipt {
    let capability = nightly_capability();
    if capability.state != ToolRunState::Observed
        || !String::from_utf8_lossy(&capability.stdout)
            .lines()
            .any(|line| line.starts_with(NIGHTLY_TOOLCHAIN.trim_start_matches('+')))
    {
        return CompilerArtifactReceipt {
            kind,
            state: if capability.state == ToolRunState::Unavailable {
                EvidenceState::Unavailable
            } else {
                EvidenceState::NeedsEvidence
            },
            artifact_digest: None,
            output_bytes: 0,
            process: capability.clone(),
            claim_boundary: "pinned nightly toolchain is unavailable; no Expanded/HIR/THIR compiler claim is produced"
                .into(),
        };
    }
    let flag = format!("unpretty={unpretty}");
    let args = [
        NIGHTLY_TOOLCHAIN,
        "--crate-name",
        "lc631_unit",
        "--crate-type=lib",
        "--edition=2021",
        "-Z",
        flag.as_str(),
        "-",
    ];
    run_artifact(kind, &args, Some(source), Some(crate_name))
}

fn run_artifact(
    kind: CompilerArtifactKind,
    args: &[&str],
    source: Option<&str>,
    crate_name: Option<&str>,
) -> CompilerArtifactReceipt {
    let program = if kind == CompilerArtifactKind::RustAnalyzerVersion {
        "rust-analyzer"
    } else {
        "rustc"
    };
    let mut owned_args = args
        .iter()
        .map(|arg| (*arg).to_string())
        .collect::<Vec<_>>();
    if let Some(crate_name) = crate_name {
        if let Some(index) = owned_args.iter().position(|arg| arg == "--crate-name") {
            if let Some(value) = owned_args.get_mut(index + 1) {
                *value = crate_name.to_string();
            }
        }
    }
    let borrowed_args = owned_args.iter().map(String::as_str).collect::<Vec<_>>();
    let result = run_bounded_tool(BoundedToolRequest {
        program,
        args: &borrowed_args,
        stdin: source.map(str::as_bytes),
        timeout_ms: 5_000,
        stdout_limit: 2 * 1024 * 1024,
        stderr_limit: 64 * 1024,
    });
    let state = match result.state {
        ToolRunState::Observed if !result.stdout_truncated => EvidenceState::Observed,
        ToolRunState::Unavailable => EvidenceState::Unavailable,
        ToolRunState::TimedOut => EvidenceState::NeedsEvidence,
        ToolRunState::Failed
            if matches!(
                kind,
                CompilerArtifactKind::ExpandedSource
                    | CompilerArtifactKind::Hir
                    | CompilerArtifactKind::Thir
            ) && nightly_toolchain_unavailable(&result) =>
        {
            EvidenceState::Unavailable
        }
        ToolRunState::Failed
            if matches!(
                kind,
                CompilerArtifactKind::RustcVersion | CompilerArtifactKind::RustAnalyzerVersion
            ) =>
        {
            EvidenceState::Unavailable
        }
        ToolRunState::Failed | ToolRunState::Observed => EvidenceState::Refuted,
    };
    let artifact_digest = (state == EvidenceState::Observed).then(|| result.stdout_digest.clone());
    CompilerArtifactReceipt {
        kind,
        state,
        artifact_digest,
        output_bytes: result.stdout.len(),
        process: result,
        claim_boundary: "bounded compiler/analyzer output for one complete virtual Rust unit; no Cargo graph, candidate execution, runtime safety, proof, authority, or host commit"
            .to_string(),
    }
}

fn nightly_capability() -> &'static crate::BoundedToolResult {
    static CAPABILITY: std::sync::OnceLock<crate::BoundedToolResult> = std::sync::OnceLock::new();
    CAPABILITY.get_or_init(|| {
        run_bounded_tool(BoundedToolRequest {
            program: "rustup",
            args: &["toolchain", "list"],
            stdin: None,
            timeout_ms: 2_000,
            stdout_limit: 64 * 1024,
            stderr_limit: 64 * 1024,
        })
    })
}

fn nightly_toolchain_unavailable(result: &crate::BoundedToolResult) -> bool {
    let diagnostic = format!(
        "{} {}",
        result.diagnostic.as_deref().unwrap_or_default(),
        String::from_utf8_lossy(&result.stderr)
    )
    .to_ascii_lowercase();
    [
        "toolchain is not installed",
        "toolchain 'nightly",
        "no release found",
        "could not download",
        "rustup could not choose",
    ]
    .iter()
    .any(|marker| diagnostic.contains(marker))
}

fn state_for(rust: &RustAnalysisSurface, kind: CompilerArtifactKind) -> EvidenceState {
    rust.artifacts
        .iter()
        .find(|artifact| artifact.kind == kind)
        .map(|artifact| artifact.state)
        .unwrap_or(EvidenceState::Unavailable)
}

fn artifact_digest(rust: &RustAnalysisSurface, kind: CompilerArtifactKind) -> Vec<String> {
    rust.artifacts
        .iter()
        .find(|artifact| artifact.kind == kind)
        .and_then(|artifact| artifact.artifact_digest.clone())
        .into_iter()
        .collect()
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

fn source_metrics(source: &str) -> RustSourceMetrics {
    RustSourceMetrics {
        unsafe_markers: source.matches("unsafe").count(),
        extern_markers: source.matches("extern ").count(),
        inline_asm_markers: source.matches("asm!").count() + source.matches("global_asm!").count(),
        async_markers: source.matches("async ").count() + source.matches(".await").count(),
        atomic_markers: source.matches("Atomic").count() + source.matches("Ordering::").count(),
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

fn claim_boundary() -> String {
    "bounded rustc/rust-analyzer observations over one complete virtual unit; compiler views are revision-specific and do not establish Cargo configuration coverage, unsafe soundness, runtime behavior, performance, proof, authority, or host output commit"
        .to_string()
}
