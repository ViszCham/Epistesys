//! Python source is compiled/inspected, never executed. Every capability probe
//! uses the injected common owner, not the legacy process entrypoint.
use super::*;
use crate::process::{with_tool_result, ToolProcessRunner};
use lc631_process_supervisor::ResourceError;

pub(crate) fn analyze_python_source_with_resources(
    source: &str,
    runner: &mut dyn ToolProcessRunner,
) -> Result<PythonAnalysisSurface, ResourceError> {
    let revision = stable_sha256(source);
    if source.len() > MAX_PYTHON_SOURCE_BYTES {
        let mut surface = finish_python_surface(
            source,
            revision,
            None,
            Vec::new(),
            vec![format!(
                "Python source exceeds the {MAX_PYTHON_SOURCE_BYTES}-byte analysis limit"
            )],
        );
        surface
            .risk_indicators
            .push("source_size_limit_exceeded".into());
        return Ok(surface);
    }
    let mut observation = None;
    let mut unknowns = Vec::new();
    for (program, prefix) in [
        ("python", &[][..]),
        ("python3", &[][..]),
        ("py", &["-3"][..]),
    ] {
        let mut args = prefix.to_vec();
        args.extend(["-X", "utf8", "-I", "-S", "-B", "-c", SCRIPT]);
        let observed = with_tool_result(
            runner,
            source,
            BoundedToolRequest {
                program,
                args: &args,
                stdin: Some(source.as_bytes()),
                timeout_ms: 3000,
                stdout_limit: 128 * 1024,
                stderr_limit: 16 * 1024,
            },
            |report, stdout, _| {
                if report.state == ToolRunState::Observed && !report.stdout_truncated {
                    serde_json::from_slice::<RawObservation>(stdout)
                        .map_err(|e| format!("{program} returned invalid JSON: {e}"))
                        .and_then(|raw| {
                            if raw.schema == "lc631-python-compiler-observation.v1" {
                                Ok(raw)
                            } else {
                                Err(format!("{program} returned an unsupported schema"))
                            }
                        })
                } else {
                    Err(format!("{program} backend state: {:?}", report.state))
                }
            },
        )?;
        match observed {
            Ok(raw) => {
                observation = Some((program.into(), raw));
                break;
            }
            Err(reason) => unknowns.push(reason),
        }
    }
    let mut capabilities = Vec::new();
    for tool in ["mypy", "pyright", "pyrefly", "ty"] {
        capabilities.push(with_tool_result(
            runner,
            source,
            BoundedToolRequest {
                program: tool,
                args: &["--version"],
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
                PythonToolCapability {
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
    Ok(finish_python_surface(
        source,
        revision,
        observation,
        capabilities,
        unknowns,
    ))
}
