use lc631_core::stable_sha256;
use serde::Deserialize;

use crate::process::{run_bounded_tool, BoundedToolRequest, ToolRunState};
use crate::python_script::SCRIPT;
use crate::{
    EvidenceState, PythonAnalysisSurface, PythonInterpreterReceipt, PythonMetrics,
    PythonToolCapability, StageReceipt,
};

const MAX_PYTHON_SOURCE_BYTES: usize = 1024 * 1024;

#[derive(Deserialize)]
struct RawObservation {
    schema: String,
    interpreter: RawInterpreter,
    parsed: bool,
    compiled: bool,
    syntax_error: Option<String>,
    code_object_digest: Option<String>,
    opcode_digest: Option<String>,
    #[serde(default)]
    metrics: PythonMetrics,
    imports: Vec<String>,
    risks: Vec<String>,
}

#[derive(Deserialize)]
struct RawInterpreter {
    implementation: String,
    version: String,
    cache_tag: Option<String>,
    platform: String,
    machine: String,
    abi_flags: String,
    sysconfig_platform: String,
    virtual_environment: bool,
    free_threaded_build: Option<bool>,
    gil_enabled: Option<bool>,
    jit_enabled: Option<bool>,
}

pub(crate) fn analyze_python_source(source: &str) -> PythonAnalysisSurface {
    let source_revision = stable_sha256(source);
    if source.len() > MAX_PYTHON_SOURCE_BYTES {
        return PythonAnalysisSurface {
            schema_version: "lc631-python-analysis.v1".to_string(),
            source_revision,
            source_bytes: source.len(),
            interpreter: None,
            parsed: false,
            compiled: false,
            syntax_error: None,
            code_object_digest: None,
            opcode_digest: None,
            metrics: PythonMetrics::default(),
            imports: Vec::new(),
            risk_indicators: vec!["source_size_limit_exceeded".to_string()],
            tool_capabilities: Vec::new(),
            candidate_executed: false,
            creates_authority: false,
            creates_output_commit: false,
            unknowns: vec![format!(
                "Python source exceeds the {MAX_PYTHON_SOURCE_BYTES}-byte analysis limit"
            )],
            claim_boundary: claim_boundary(),
        };
    }
    let mut observation = None;
    let mut process_unknowns = Vec::new();
    for (program, prefix) in [
        ("python", &[][..]),
        ("python3", &[][..]),
        ("py", &["-3"][..]),
    ] {
        let mut args = prefix.to_vec();
        args.extend(["-X", "utf8", "-I", "-S", "-c", SCRIPT]);
        let result = run_bounded_tool(BoundedToolRequest {
            program,
            args: &args,
            stdin: Some(source.as_bytes()),
            timeout_ms: 3_000,
            stdout_limit: 128 * 1024,
            stderr_limit: 16 * 1024,
        });
        if result.state == ToolRunState::Observed && !result.stdout_truncated {
            match serde_json::from_slice::<RawObservation>(&result.stdout) {
                Ok(raw) if raw.schema == "lc631-python-compiler-observation.v1" => {
                    observation = Some((program.to_string(), result, raw));
                    break;
                }
                Ok(_) => process_unknowns.push(format!("{program} returned an unsupported schema")),
                Err(error) => {
                    process_unknowns.push(format!("{program} returned invalid JSON: {error}"))
                }
            }
        } else {
            process_unknowns.push(format!("{program} backend state: {:?}", result.state));
        }
    }
    let capabilities = probe_typing_tools();
    match observation {
        Some((program, _process, raw)) => {
            let interpreter = interpreter_receipt(&program, &raw.interpreter);
            let parsed = raw.parsed;
            let compiled = raw.compiled;
            PythonAnalysisSurface {
                schema_version: "lc631-python-analysis.v1".to_string(),
                source_revision,
                source_bytes: source.len(),
                interpreter: Some(interpreter),
                parsed,
                compiled,
                syntax_error: raw.syntax_error,
                code_object_digest: raw.code_object_digest.map(prefix_digest),
                opcode_digest: raw.opcode_digest.map(prefix_digest),
                metrics: raw.metrics,
                imports: raw.imports,
                risk_indicators: raw.risks,
                tool_capabilities: capabilities,
                candidate_executed: false,
                creates_authority: false,
                creates_output_commit: false,
                unknowns: vec![
                    "project environment, import execution, checker results, tests, runtime behavior, and native safety remain unobserved".to_string(),
                    "candidate Python source was compiled but never executed".to_string(),
                ],
                claim_boundary: claim_boundary(),
            }
        }
        None => PythonAnalysisSurface {
            schema_version: "lc631-python-analysis.v1".to_string(),
            source_revision,
            source_bytes: source.len(),
            interpreter: None,
            parsed: false,
            compiled: false,
            syntax_error: None,
            code_object_digest: None,
            opcode_digest: None,
            metrics: PythonMetrics::default(),
            imports: Vec::new(),
            risk_indicators: Vec::new(),
            tool_capabilities: capabilities,
            candidate_executed: false,
            creates_authority: false,
            creates_output_commit: false,
            unknowns: process_unknowns,
            claim_boundary: claim_boundary(),
        },
    }
}

pub(crate) fn apply_python_stages(stages: &mut [StageReceipt], python: &PythonAnalysisSurface) {
    let source = vec![python.source_revision.clone()];
    update(
        stages,
        "RPA-28",
        if python.parsed && python.compiled {
            EvidenceState::Observed
        } else if python.syntax_error.is_some() {
            EvidenceState::Refuted
        } else {
            EvidenceState::Unavailable
        },
        source.clone(),
        &["AST/symtable/compile does not execute imports or establish project environment"],
    );
    let available_checkers = python
        .tool_capabilities
        .iter()
        .filter(|tool| tool.state == EvidenceState::Observed)
        .map(|tool| tool.process.stdout_digest.clone())
        .collect::<Vec<_>>();
    update(stages, "RPA-29", if available_checkers.is_empty() { EvidenceState::NeedsEvidence } else { EvidenceState::Candidate }, available_checkers, &["type-checker capability/version is observed; no checker was executed on candidate source or project configuration"]);
    let code = [
        python.code_object_digest.clone(),
        python.opcode_digest.clone(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    update(
        stages,
        "RPA-30",
        if code.len() == 2 {
            EvidenceState::Observed
        } else {
            EvidenceState::NeedsEvidence
        },
        code,
        &["code-object/opcode digest is CPython-version specific and runtime trace is unobserved"],
    );
    update(stages, "RPA-31", if python.interpreter.is_some() { EvidenceState::Candidate } else { EvidenceState::Unavailable }, source, &["dynamic effects, async cancellation, GIL schedules, free-threading races, and native-extension ABI remain unexecuted"]);
}

fn probe_typing_tools() -> Vec<PythonToolCapability> {
    [
        ("mypy", &["--version"][..]),
        ("pyright", &["--version"][..]),
        ("pyrefly", &["--version"][..]),
        ("ty", &["--version"][..]),
    ]
    .into_iter()
    .map(|(tool, args)| {
        let process = run_bounded_tool(BoundedToolRequest {
            program: tool,
            args,
            stdin: None,
            timeout_ms: 2_000,
            stdout_limit: 4096,
            stderr_limit: 4096,
        });
        let state = if process.state == ToolRunState::Observed && !process.stdout_truncated {
            EvidenceState::Observed
        } else {
            EvidenceState::Unavailable
        };
        PythonToolCapability {
            tool: tool.to_string(),
            state,
            version_digest: (state == EvidenceState::Observed)
                .then(|| process.stdout_digest.clone()),
            process,
            executed_on_candidate: false,
        }
    })
    .collect()
}

fn interpreter_receipt(program: &str, raw: &RawInterpreter) -> PythonInterpreterReceipt {
    let fingerprint = stable_sha256(&format!(
        "{program}|{}|{}|{}|{}|{}|{}|{}|{:?}|{:?}|{:?}",
        raw.implementation,
        raw.version,
        raw.cache_tag.as_deref().unwrap_or(""),
        raw.platform,
        raw.machine,
        raw.sysconfig_platform,
        raw.virtual_environment,
        raw.free_threaded_build,
        raw.gil_enabled,
        raw.jit_enabled
    ));
    PythonInterpreterReceipt {
        implementation: raw.implementation.clone(),
        version: raw.version.clone(),
        cache_tag: raw.cache_tag.clone(),
        platform: raw.platform.clone(),
        machine: raw.machine.clone(),
        abi_flags: raw.abi_flags.clone(),
        sysconfig_platform: raw.sysconfig_platform.clone(),
        virtual_environment: raw.virtual_environment,
        free_threaded_build: raw.free_threaded_build,
        gil_enabled: raw.gil_enabled,
        jit_enabled: raw.jit_enabled,
        environment_fingerprint: fingerprint,
    }
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

fn prefix_digest(value: String) -> String {
    format!("sha256:{value}")
}

fn claim_boundary() -> String {
    "isolated CPython AST, symtable, compile, code-object/opcode digest, static risk inventory, and checker capability probes only; candidate code and imports are never executed, and no runtime safety, proof, authority, or output commit is created"
        .to_string()
}
