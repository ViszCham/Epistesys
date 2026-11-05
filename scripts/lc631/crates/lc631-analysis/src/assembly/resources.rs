//! Static codegen payload consumption before its borrowed process claim drops.
use super::*;
use crate::process::{with_tool_result, ToolProcessRunner};
use lc631_process_supervisor::ResourceError;

pub(crate) struct AssemblySnapshot(AssemblyAnalysisSurface);

pub(crate) fn capture_codegen_snapshot(
    bytes: &[u8],
    prompt: &str,
) -> Result<AssemblySnapshot, ResourceError> {
    let source = std::str::from_utf8(bytes)
        .map_err(|_| ResourceError::Io(std::io::ErrorKind::InvalidData))?;
    Ok(AssemblySnapshot(static_inventory(
        source,
        prompt,
        "rustc_codegen_assembly",
        None,
    )))
}

pub(crate) fn from_explicit_source_with_resources(
    source: &str,
    prompt: &str,
    runner: &mut dyn ToolProcessRunner,
) -> Result<AssemblyAnalysisSurface, ResourceError> {
    let mut result = static_inventory(source, prompt, "explicit_assembly_source", None);
    if source.len() <= MAX_ASSEMBLY_SOURCE_BYTES {
        result.tool_capabilities = probe_tools_resource(source, runner)?;
    }
    Ok(result)
}

pub(crate) fn finish_codegen_snapshot(
    snapshot: Option<AssemblySnapshot>,
    rust: &RustAnalysisSurface,
    source: &str,
    runner: &mut dyn ToolProcessRunner,
) -> Result<Option<AssemblyAnalysisSurface>, ResourceError> {
    let Some(AssemblySnapshot(mut result)) = snapshot else {
        return Ok(None);
    };
    result.object_digest = rust
        .artifacts
        .iter()
        .find(|a| a.kind == CompilerArtifactKind::Object && a.state == EvidenceState::Observed)
        .and_then(|a| a.artifact_digest.clone());
    result.tool_capabilities = probe_tools_resource(source, runner)?;
    Ok(Some(result))
}

fn probe_tools_resource(
    source: &str,
    runner: &mut dyn ToolProcessRunner,
) -> Result<Vec<AssemblyToolCapability>, ResourceError> {
    let mut tools = Vec::new();
    for (tool, args) in [
        ("llvm-mc", &["--version"][..]),
        ("llvm-objdump", &["--version"][..]),
        ("nasm", &["-v"][..]),
        ("objdump", &["--version"][..]),
    ] {
        tools.push(with_tool_result(
            runner,
            source,
            BoundedToolRequest {
                program: tool,
                args,
                stdin: None,
                timeout_ms: 2000,
                stdout_limit: 4096,
                stderr_limit: 4096,
            },
            |report, _, _| {
                let state = if report.state == ToolRunState::Observed && !report.stdout_truncated {
                    EvidenceState::Observed
                } else {
                    EvidenceState::Unavailable
                };
                AssemblyToolCapability {
                    tool: tool.into(),
                    state,
                    version_digest: (state == EvidenceState::Observed)
                        .then(|| report.stdout_digest.clone()),
                    process: report.clone(),
                    executed_on_candidate: false,
                }
            },
        )?);
    }
    Ok(tools)
}
