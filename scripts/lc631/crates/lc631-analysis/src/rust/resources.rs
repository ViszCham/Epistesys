//! Resource-aware compiler observations. No global cached capability or legacy
//! runner is used; every fresh probe stays on the injected common owner.
use super::*;
use crate::assembly::{capture_codegen_snapshot, AssemblySnapshot};
use crate::process::{with_tool_result, ToolProcessRunner};
use lc631_process_supervisor::ResourceError;

struct Artifact {
    receipt: CompilerArtifactReceipt,
    version: Option<String>,
    assembly: Option<AssemblySnapshot>,
}

pub(crate) fn analyze_rust_source_with_resources(
    source: &str,
    prompt: &str,
    runner: &mut dyn ToolProcessRunner,
) -> Result<(RustAnalysisSurface, Option<AssemblySnapshot>), ResourceError> {
    let revision = stable_sha256(source);
    if source.len() > MAX_RUST_SOURCE_BYTES {
        return Ok((
            RustAnalysisSurface {
                schema_version: "lc631-rust-analysis.v1".into(),
                source_revision: revision,
                source_bytes: source.len(),
                complete_source: true,
                toolchain_identity: None,
                artifacts: Vec::new(),
                metrics: source_metrics(source),
                compiler_accepted: false,
                creates_authority: false,
                creates_output_commit: false,
                unknowns: vec!["Rust source exceeds the 64 KiB bounded compiler limit".into()],
                claim_boundary: claim_boundary(),
            },
            None,
        ));
    }
    let name = format!("lc631_unit_{}", &revision[7..19]);
    let mut artifacts = Vec::new();
    let mut assembly = None;
    let mut version = None;
    for (kind, args, input) in [
        (
            CompilerArtifactKind::RustcVersion,
            vec!["--version", "--verbose"],
            None,
        ),
        (
            CompilerArtifactKind::Mir,
            vec![
                "--crate-name",
                "lc631_unit",
                "--crate-type=lib",
                "--edition=2021",
                "--emit=mir=-",
                "-",
            ],
            Some(source),
        ),
        (
            CompilerArtifactKind::LlvmIr,
            vec![
                "--crate-name",
                "lc631_unit",
                "--crate-type=lib",
                "--edition=2021",
                "--emit=llvm-ir=-",
                "-",
            ],
            Some(source),
        ),
        (
            CompilerArtifactKind::Assembly,
            vec![
                "--crate-name",
                "lc631_unit",
                "--crate-type=lib",
                "--edition=2021",
                "--emit=asm=-",
                "-",
            ],
            Some(source),
        ),
        (
            CompilerArtifactKind::Object,
            vec![
                "--crate-name",
                "lc631_unit",
                "--crate-type=lib",
                "--edition=2021",
                "--emit=obj=-",
                "-",
            ],
            Some(source),
        ),
    ] {
        let observed = artifact(kind, &args, input, &name, source, prompt, runner)?;
        if observed.version.is_some() {
            version = observed.version;
        }
        if observed.assembly.is_some() {
            assembly = observed.assembly;
        }
        artifacts.push(observed.receipt);
    }
    let (nightly, nightly_report) = with_tool_result(
        runner,
        source,
        BoundedToolRequest {
            program: "rustup",
            args: &["toolchain", "list"],
            stdin: None,
            timeout_ms: 2000,
            stdout_limit: 65_536,
            stderr_limit: 65_536,
        },
        |report, stdout, _| {
            (
                report.state == ToolRunState::Observed
                    && String::from_utf8_lossy(stdout)
                        .lines()
                        .any(|line| line.starts_with(NIGHTLY_TOOLCHAIN.trim_start_matches('+'))),
                report.clone(),
            )
        },
    )?;
    for (kind, unpretty) in [
        (CompilerArtifactKind::ExpandedSource, "expanded"),
        (CompilerArtifactKind::Hir, "hir-tree"),
        (CompilerArtifactKind::Thir, "thir-tree"),
    ] {
        if !nightly {
            artifacts.push(CompilerArtifactReceipt {kind,state:if nightly_report.state==ToolRunState::Unavailable {EvidenceState::Unavailable} else {EvidenceState::NeedsEvidence},
                artifact_digest:None,output_bytes:0,process:nightly_report.clone(),claim_boundary:"pinned nightly toolchain is unavailable; no Expanded/HIR/THIR compiler claim is produced".into()});
        } else {
            let flag = format!("unpretty={unpretty}");
            let args = [
                NIGHTLY_TOOLCHAIN,
                "--crate-name",
                "lc631_unit",
                "--crate-type=lib",
                "--edition=2021",
                "-Z",
                &flag,
                "-",
            ];
            artifacts
                .push(artifact(kind, &args, Some(source), &name, source, prompt, runner)?.receipt);
        }
    }
    artifacts.push(
        artifact(
            CompilerArtifactKind::RustAnalyzerVersion,
            &["--version"],
            None,
            &name,
            source,
            prompt,
            runner,
        )?
        .receipt,
    );
    let compiler_accepted = [
        CompilerArtifactKind::Mir,
        CompilerArtifactKind::LlvmIr,
        CompilerArtifactKind::Object,
    ]
    .iter()
    .all(|kind| {
        artifacts
            .iter()
            .any(|a| a.kind == *kind && a.state == EvidenceState::Observed)
    });
    let mut unknowns=vec!["Cargo workspace, features, cfg, build scripts, proc macros, dependencies, tests, runtime behavior, and host output are unobserved".into(),
        "compiler acceptance and emitted artifacts do not establish unsafe soundness or user intent".into()];
    if artifacts
        .iter()
        .any(|a| a.state == EvidenceState::Unavailable)
    {
        unknowns.push("one or more optional Rust compiler/analyzer views are unavailable".into());
    }
    Ok((
        RustAnalysisSurface {
            schema_version: "lc631-rust-analysis.v1".into(),
            source_revision: revision,
            source_bytes: source.len(),
            complete_source: true,
            toolchain_identity: version,
            artifacts,
            metrics: source_metrics(source),
            compiler_accepted,
            creates_authority: false,
            creates_output_commit: false,
            unknowns,
            claim_boundary: claim_boundary(),
        },
        assembly,
    ))
}

fn artifact(
    kind: CompilerArtifactKind,
    args: &[&str],
    input: Option<&str>,
    name: &str,
    task_source: &str,
    prompt: &str,
    runner: &mut dyn ToolProcessRunner,
) -> Result<Artifact, ResourceError> {
    let program = if kind == CompilerArtifactKind::RustAnalyzerVersion {
        "rust-analyzer"
    } else {
        "rustc"
    };
    let mut owned = args.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    if let Some(index) = owned.iter().position(|s| s == "--crate-name") {
        if let Some(value) = owned.get_mut(index + 1) {
            *value = name.into();
        }
    }
    let args = owned.iter().map(String::as_str).collect::<Vec<_>>();
    with_tool_result(
        runner,
        task_source,
        BoundedToolRequest {
            program,
            args: &args,
            stdin: input.map(str::as_bytes),
            timeout_ms: 5000,
            stdout_limit: 2 * 1024 * 1024,
            stderr_limit: 65_536,
        },
        |report, stdout, stderr| {
            let state = classify(kind, report, stderr);
            let assembly =
                if kind == CompilerArtifactKind::Assembly && state == EvidenceState::Observed {
                    Some(capture_codegen_snapshot(stdout, prompt)?)
                } else {
                    None
                };
            Ok(Artifact {version:(kind==CompilerArtifactKind::RustcVersion && state==EvidenceState::Observed).then(||bounded_utf8(stdout,512)),assembly,
            receipt:CompilerArtifactReceipt {kind,state,artifact_digest:(state==EvidenceState::Observed).then(||report.stdout_digest.clone()),
                output_bytes:stdout.len(),process:report.clone(),claim_boundary:"bounded compiler/analyzer output for one complete virtual Rust unit; no Cargo graph, candidate execution, runtime safety, proof, authority, or host commit".into()}})
        },
    )?
}

fn classify(
    kind: CompilerArtifactKind,
    report: &crate::BoundedToolResult,
    stderr: &[u8],
) -> EvidenceState {
    match report.state {
        ToolRunState::Observed if !report.stdout_truncated => EvidenceState::Observed,
        ToolRunState::Unavailable => EvidenceState::Unavailable,
        ToolRunState::TimedOut => EvidenceState::NeedsEvidence,
        ToolRunState::Failed
            if matches!(
                kind,
                CompilerArtifactKind::ExpandedSource
                    | CompilerArtifactKind::Hir
                    | CompilerArtifactKind::Thir
            ) =>
        {
            let text = format!(
                "{} {}",
                report.diagnostic.as_deref().unwrap_or_default(),
                String::from_utf8_lossy(stderr)
            )
            .to_ascii_lowercase();
            if [
                "toolchain is not installed",
                "toolchain 'nightly",
                "no release found",
                "could not download",
                "rustup could not choose",
            ]
            .iter()
            .any(|s| text.contains(s))
            {
                EvidenceState::Unavailable
            } else {
                EvidenceState::Refuted
            }
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
    }
}
