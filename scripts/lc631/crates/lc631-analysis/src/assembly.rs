use std::collections::BTreeSet;

use lc631_core::stable_sha256;

use crate::process::{run_bounded_tool, BoundedToolRequest, ToolRunState};
use crate::{
    AssemblyAnalysisSurface, AssemblyMetrics, AssemblyTargetProfile, AssemblyToolCapability,
    CompilerArtifactKind, EvidenceState, RustAnalysisSurface, StageReceipt,
};

const MAX_ASSEMBLY_SOURCE_BYTES: usize = 1024 * 1024;

pub(crate) fn from_explicit_source(source: &str, prompt: &str) -> AssemblyAnalysisSurface {
    analyze(source, prompt, "explicit_assembly_source", None)
}

pub(crate) fn from_rust_codegen(
    rust: &RustAnalysisSurface,
    prompt: &str,
) -> Option<AssemblyAnalysisSurface> {
    let assembly = rust.artifacts.iter().find(|artifact| {
        artifact.kind == CompilerArtifactKind::Assembly && artifact.state == EvidenceState::Observed
    })?;
    let object_digest = rust
        .artifacts
        .iter()
        .find(|artifact| {
            artifact.kind == CompilerArtifactKind::Object
                && artifact.state == EvidenceState::Observed
        })
        .and_then(|artifact| artifact.artifact_digest.clone());
    let source = String::from_utf8_lossy(&assembly.process.stdout);
    Some(analyze(
        &source,
        prompt,
        "rustc_codegen_assembly",
        object_digest,
    ))
}

pub(crate) fn apply_assembly_stages(
    stages: &mut [StageReceipt],
    assembly: &AssemblyAnalysisSurface,
) {
    let source = vec![assembly.source_revision.clone()];
    let target_observed = assembly.target.architecture.is_some()
        && assembly.target.dialect.is_some()
        && !assembly.target.ambiguous;
    update(
        stages,
        "RPA-32",
        if target_observed {
            EvidenceState::Observed
        } else {
            EvidenceState::NeedsEvidence
        },
        source.clone(),
        &["ISA revision, features, ABI, object format, or privilege may remain unbound"],
    );
    update(stages, "RPA-33", if assembly.metrics.instructions > 0 { EvidenceState::Candidate } else { EvidenceState::NeedsEvidence }, source.clone(), &["instruction inventory is not assembler acceptance, encoding, pseudo-op expansion, or relaxation evidence"]);
    update(stages, "RPA-34", if assembly.metrics.instructions > 0 { EvidenceState::Candidate } else { EvidenceState::NeedsEvidence }, source.clone(), &["register, memory, branch, atomic, and privileged inventory is not formal instruction-state semantics"]);
    update(stages, "RPA-35", if assembly.object_digest.is_some() { EvidenceState::Candidate } else { EvidenceState::NeedsEvidence }, assembly.object_digest.iter().cloned().collect(), &["object digest is observed when supplied by rustc; sections, symbols, relocations, link, unwind, debug, and loader are not decoded"]);
    update(stages, "RPA-36", if assembly.differential_observed { EvidenceState::Observed } else { EvidenceState::NeedsEvidence }, assembly.tool_capabilities.iter().filter_map(|tool| tool.version_digest.clone()).collect(), &["assembler/decoder version capabilities are not candidate execution or multi-tool differential evidence"]);
    update(stages, "RPA-37", if assembly.source_origin == "rustc_codegen_assembly" && assembly.object_digest.is_some() { EvidenceState::Candidate } else { EvidenceState::NeedsEvidence }, source, &["Rust source-to-assembly/object linkage is observed; inline-asm contracts, Python native extensions, GPU ISA, and runtime ABI remain unclosed"]);
}

fn analyze(
    source: &str,
    prompt: &str,
    origin: &str,
    object_digest: Option<String>,
) -> AssemblyAnalysisSurface {
    if source.len() > MAX_ASSEMBLY_SOURCE_BYTES {
        return AssemblyAnalysisSurface {
            schema_version: "lc631-assembly-analysis.v1".to_string(),
            source_revision: stable_sha256(source),
            source_origin: origin.to_string(),
            target: AssemblyTargetProfile {
                architecture: None,
                isa_revision: None,
                features: Vec::new(),
                dialect: None,
                object_format: None,
                abi: None,
                privilege: None,
                ambiguous: true,
            },
            metrics: AssemblyMetrics::default(),
            mnemonics: Vec::new(),
            risk_indicators: vec!["source_size_limit_exceeded".to_string()],
            tool_capabilities: Vec::new(),
            object_digest,
            source_assembled: false,
            differential_observed: false,
            creates_authority: false,
            creates_output_commit: false,
            unknowns: vec![format!(
                "Assembly source exceeds the {MAX_ASSEMBLY_SOURCE_BYTES}-byte analysis limit"
            )],
            claim_boundary: "source rejected by bounded assembly analysis; no assembler execution, instruction-semantics proof, ABI safety, runtime behavior, performance, authority, or output commit"
                .to_string(),
        };
    }
    let lower = format!("{}\n{}", prompt, source).to_ascii_lowercase();
    let target = infer_target(&lower);
    let (metrics, mnemonics, risks) = inventory(source);
    let tools = probe_tools();
    AssemblyAnalysisSurface {
        schema_version: "lc631-assembly-analysis.v1".to_string(),
        source_revision: stable_sha256(source),
        source_origin: origin.to_string(),
        target,
        metrics,
        mnemonics,
        risk_indicators: risks,
        tool_capabilities: tools,
        object_digest,
        source_assembled: origin == "rustc_codegen_assembly",
        differential_observed: false,
        creates_authority: false,
        creates_output_commit: false,
        unknowns: vec![
            "assembler acceptance, exact encoding, relocation/link/load, unwind, fault, memory-order, runtime, and performance evidence remain unobserved"
                .to_string(),
        ],
        claim_boundary: "static assembly inventory and tool capability receipts, optionally bound to rustc-emitted assembly/object digests; not assembler execution, instruction-semantics proof, ABI safety, runtime behavior, performance, authority, or output commit"
            .to_string(),
    }
}

fn infer_target(value: &str) -> AssemblyTargetProfile {
    let mut architectures = BTreeSet::new();
    if [
        "x86_64",
        "amd64",
        "%rax",
        " rax",
        " eax",
        ".intel_syntax",
        "movq",
    ]
    .iter()
    .any(|marker| value.contains(marker))
    {
        architectures.insert("x86_64");
    }
    if ["aarch64", "arm64", " x0", " w0", " nzcv", ".arch arm"]
        .iter()
        .any(|marker| value.contains(marker))
    {
        architectures.insert("aarch64");
    }
    if ["risc-v", "riscv", ".option", " a0", " jal "]
        .iter()
        .any(|marker| value.contains(marker))
    {
        architectures.insert("riscv");
    }
    if ["ptx", ".target sm_", "%tid", "ld.global"]
        .iter()
        .any(|marker| value.contains(marker))
    {
        architectures.insert("ptx");
    }
    if ["amdgpu", "amdgcn", "s_load_", "v_add_"]
        .iter()
        .any(|marker| value.contains(marker))
    {
        architectures.insert("amdgpu");
    }
    let dialect = if value.contains(".intel_syntax") {
        Some("gas_intel")
    } else if value.contains("%rax") || value.contains("%eax") {
        Some("gas_att")
    } else if value.contains("section .text") || value.contains("global ") {
        Some("nasm")
    } else if value.contains(" proc") || value.contains(" endp") {
        Some("masm")
    } else if value.contains(".def") || value.contains(".globl") {
        Some("llvm_gas")
    } else {
        None
    };
    let object_format = if value.contains(".def") || value.contains("coff") {
        Some("coff")
    } else if value.contains(".type") || value.contains("elf") {
        Some("elf")
    } else if value.contains("mach-o") || value.contains(".subsections_via_symbols") {
        Some("mach_o")
    } else {
        None
    };
    AssemblyTargetProfile {
        architecture: if architectures.len() == 1 {
            architectures
                .iter()
                .next()
                .map(|value| (*value).to_string())
        } else {
            None
        },
        isa_revision: None,
        features: Vec::new(),
        dialect: dialect.map(str::to_string),
        object_format: object_format.map(str::to_string),
        abi: None,
        privilege: None,
        ambiguous: architectures.len() != 1,
    }
}

fn inventory(source: &str) -> (AssemblyMetrics, Vec<String>, Vec<String>) {
    let mut metrics = AssemblyMetrics::default();
    let mut mnemonics = BTreeSet::new();
    let mut risks = BTreeSet::new();
    for raw in source.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        metrics.source_lines += 1;
        if line.ends_with(':') {
            metrics.labels += 1;
            continue;
        }
        if line.starts_with('.') {
            metrics.directives += 1;
            continue;
        }
        let mnemonic = line
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_end_matches(':')
            .to_ascii_lowercase();
        if mnemonic.is_empty() {
            continue;
        }
        metrics.instructions += 1;
        if [
            "jmp", "je", "jne", "call", "ret", "b", "bl", "br", "jal", "jalr",
        ]
        .contains(&mnemonic.as_str())
        {
            metrics.branches += 1;
        }
        if line.contains('[') || line.contains('(') {
            metrics.memory_operands += 1;
        }
        if mnemonic.contains("lock") || mnemonic.contains("fence") || mnemonic.contains("atomic") {
            metrics.atomic_or_fence += 1;
            risks.insert("memory_order_or_atomic_semantics".to_string());
        }
        if ["syscall", "sysenter", "hlt", "cli", "sti", "msr", "mrs"].contains(&mnemonic.as_str()) {
            metrics.privileged_candidates += 1;
            risks.insert("privileged_or_system_instruction".to_string());
        }
        mnemonics.insert(mnemonic);
    }
    (
        metrics,
        mnemonics.into_iter().collect(),
        risks.into_iter().collect(),
    )
}

fn probe_tools() -> Vec<AssemblyToolCapability> {
    [
        ("llvm-mc", &["--version"][..]),
        ("llvm-objdump", &["--version"][..]),
        ("nasm", &["-v"][..]),
        ("objdump", &["--version"][..]),
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
        AssemblyToolCapability {
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
