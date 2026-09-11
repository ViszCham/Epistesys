use lc631_receipt_kernel::UntrustedReceipt;
use lc631_tl::CanonicalTranslationEnvelope;
use serde::{Deserialize, Serialize};

use crate::process::BoundedToolResult;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LanguageKind {
    Rust,
    Python,
    Assembly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceState {
    Observed,
    NotApplicable,
    Candidate,
    NeedsEvidence,
    NotAuthorized,
    Unavailable,
    Refuted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClosureState {
    Open,
    Candidate,
    Satisfied,
    Blocked,
    NotApplicable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CorpusReceipt {
    pub corpus_id: String,
    pub file_name: String,
    pub sha256: String,
    pub explicit_records: usize,
    pub authority_class: String,
    pub count_is_coverage: bool,
    pub content_packaged: bool,
    pub claim_boundary: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AnalysisTargetProfile {
    pub languages: Vec<LanguageKind>,
    pub source_name: Option<String>,
    pub source_revision: Option<String>,
    pub selection_owner: String,
    pub selection_authoritative: bool,
    pub selection_conflicts: Vec<String>,
    pub target_triple: Option<String>,
    pub toolchain_revision: Option<String>,
    pub unknowns: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StageReceipt {
    pub stage_id: String,
    pub group: String,
    pub dependencies: Vec<String>,
    pub state: EvidenceState,
    pub closure_state: ClosureState,
    pub evidence_digests: Vec<String>,
    pub blockers: Vec<String>,
    pub creates_authority: bool,
    pub creates_output_commit: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DirectionalLossReceipt {
    pub from: String,
    pub to: String,
    pub observed_defects: Vec<String>,
    pub unknowns: Vec<String>,
    pub scalar_aggregate_used: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompilerArtifactKind {
    RustcVersion,
    ExpandedSource,
    Hir,
    Thir,
    Mir,
    LlvmIr,
    Assembly,
    Object,
    RustAnalyzerVersion,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CompilerArtifactReceipt {
    pub kind: CompilerArtifactKind,
    pub state: EvidenceState,
    pub artifact_digest: Option<String>,
    pub output_bytes: usize,
    pub process: BoundedToolResult,
    pub claim_boundary: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct RustSourceMetrics {
    pub unsafe_markers: usize,
    pub extern_markers: usize,
    pub inline_asm_markers: usize,
    pub async_markers: usize,
    pub atomic_markers: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RustAnalysisSurface {
    pub schema_version: String,
    pub source_revision: String,
    pub source_bytes: usize,
    pub complete_source: bool,
    pub toolchain_identity: Option<String>,
    pub artifacts: Vec<CompilerArtifactReceipt>,
    pub metrics: RustSourceMetrics,
    pub compiler_accepted: bool,
    pub creates_authority: bool,
    pub creates_output_commit: bool,
    pub unknowns: Vec<String>,
    pub claim_boundary: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CargoWorldReceipt {
    pub state: EvidenceState,
    pub cargo_version: Option<String>,
    pub metadata_digest: Option<String>,
    pub package_count: usize,
    pub workspace_member_count: usize,
    pub target_count: usize,
    pub build_script_targets: usize,
    pub proc_macro_targets: usize,
    pub process: BoundedToolResult,
    pub source_bound: bool,
    pub checked_package: Option<String>,
    pub crate_check_state: EvidenceState,
    pub crate_check_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crate_check_process: Option<BoundedToolResult>,
    pub build_execution_observed: bool,
    pub claim_boundary: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct PythonMetrics {
    pub ast_nodes: usize,
    pub scopes: usize,
    pub imports: usize,
    pub functions: usize,
    pub classes: usize,
    pub async_constructs: usize,
    pub annotations: usize,
    pub top_level_effects: usize,
    pub bytecode_instructions: usize,
    pub distinct_opcodes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PythonInterpreterReceipt {
    pub implementation: String,
    pub version: String,
    pub cache_tag: Option<String>,
    pub platform: String,
    pub machine: String,
    pub abi_flags: String,
    pub sysconfig_platform: String,
    pub virtual_environment: bool,
    pub free_threaded_build: Option<bool>,
    pub gil_enabled: Option<bool>,
    pub jit_enabled: Option<bool>,
    pub environment_fingerprint: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PythonToolCapability {
    pub tool: String,
    pub state: EvidenceState,
    pub version_digest: Option<String>,
    pub process: BoundedToolResult,
    pub executed_on_candidate: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PythonAnalysisSurface {
    pub schema_version: String,
    pub source_revision: String,
    pub source_bytes: usize,
    pub interpreter: Option<PythonInterpreterReceipt>,
    pub parsed: bool,
    pub compiled: bool,
    pub syntax_error: Option<String>,
    pub code_object_digest: Option<String>,
    pub opcode_digest: Option<String>,
    pub metrics: PythonMetrics,
    pub imports: Vec<String>,
    pub risk_indicators: Vec<String>,
    pub tool_capabilities: Vec<PythonToolCapability>,
    pub candidate_executed: bool,
    pub creates_authority: bool,
    pub creates_output_commit: bool,
    pub unknowns: Vec<String>,
    pub claim_boundary: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AssemblyTargetProfile {
    pub architecture: Option<String>,
    pub isa_revision: Option<String>,
    pub features: Vec<String>,
    pub dialect: Option<String>,
    pub object_format: Option<String>,
    pub abi: Option<String>,
    pub privilege: Option<String>,
    pub ambiguous: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct AssemblyMetrics {
    pub source_lines: usize,
    pub labels: usize,
    pub directives: usize,
    pub instructions: usize,
    pub branches: usize,
    pub memory_operands: usize,
    pub atomic_or_fence: usize,
    pub privileged_candidates: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AssemblyToolCapability {
    pub tool: String,
    pub state: EvidenceState,
    pub version_digest: Option<String>,
    pub process: BoundedToolResult,
    pub executed_on_candidate: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AssemblyAnalysisSurface {
    pub schema_version: String,
    pub source_revision: String,
    pub source_origin: String,
    pub target: AssemblyTargetProfile,
    pub metrics: AssemblyMetrics,
    pub mnemonics: Vec<String>,
    pub risk_indicators: Vec<String>,
    pub tool_capabilities: Vec<AssemblyToolCapability>,
    pub object_digest: Option<String>,
    pub source_assembled: bool,
    pub differential_observed: bool,
    pub creates_authority: bool,
    pub creates_output_commit: bool,
    pub unknowns: Vec<String>,
    pub claim_boundary: String,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationKind {
    Rustfmt,
    RustcCheck,
    Clippy,
    Tests,
    Coverage,
    Mutation,
    Miri,
    Property,
    Fuzz,
    Differential,
    ConcurrencyModel,
    FormalVerifier,
    SupplyChain,
    SemverMsrv,
    Performance,
    HumanReview,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptBinding {
    SelfAttested,
    LocalCommand,
    HostBound,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ValidationReceipt {
    pub kind: ValidationKind,
    pub binding: ReceiptBinding,
    pub state: EvidenceState,
    pub source_revision: String,
    pub tool_revision: String,
    pub invocation_digest: String,
    pub output_digest: String,
    pub scope: String,
    pub unsupported: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attestation: Option<UntrustedReceipt>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct ValidationEvidenceBundle {
    pub bundle_version: String,
    pub receipts: Vec<ValidationReceipt>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ValidationSummary {
    pub accepted_receipts: usize,
    pub rejected_receipts: usize,
    pub duplicate_kinds: Vec<ValidationKind>,
    pub all_receipts_non_authorizing: bool,
    pub authenticated_receipts: usize,
    pub legacy_host_bound_rejected: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StageCompletionReceipt {
    pub stage_id: String,
    pub binding: ReceiptBinding,
    pub source_revision: String,
    pub evidence_digest: String,
    pub scope: String,
    pub unsupported: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attestation: Option<UntrustedReceipt>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReleaseEvidenceBundle {
    pub bundle_version: String,
    pub stage_evidence: Vec<StageCompletionReceipt>,
    pub evaluation_corpus_digest: String,
    pub evaluation_result_digest: String,
    pub semantic_reversal_observed: bool,
    pub directional_loss_observed: bool,
    pub dead_fixture_count: usize,
    pub v630_baseline_digest: String,
    pub exact_host_output_digest: String,
    pub package_manifest_digest: String,
    pub remote_ci_digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evaluation_attestation: Option<UntrustedReceipt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub v630_baseline_attestation: Option<UntrustedReceipt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exact_host_output_attestation: Option<UntrustedReceipt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_manifest_attestation: Option<UntrustedReceipt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_ci_attestation: Option<UntrustedReceipt>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReleaseSummary {
    pub accepted_stage_receipts: usize,
    pub rejected_stage_receipts: usize,
    pub evaluation_complete: bool,
    pub v630_baseline_bound: bool,
    pub host_output_bound: bool,
    pub package_bound: bool,
    pub remote_ci_bound: bool,
    pub release_complete: bool,
    pub authenticity_complete: bool,
    pub blockers: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProgramAnalysisReport {
    pub schema_version: String,
    pub version: String,
    pub shadow_only: bool,
    pub corpora: Vec<CorpusReceipt>,
    pub languages: Vec<LanguageKind>,
    pub target: AnalysisTargetProfile,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rust: Option<RustAnalysisSurface>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cargo: Option<CargoWorldReceipt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub python: Option<PythonAnalysisSurface>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assembly: Option<AssemblyAnalysisSurface>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation: Option<ValidationSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release: Option<ReleaseSummary>,
    pub stages: Vec<StageReceipt>,
    pub directional_loss: Vec<DirectionalLossReceipt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical_translation: Option<CanonicalTranslationEnvelope>,
    pub dependency_order_valid: bool,
    pub wiring_complete: bool,
    pub observed_count: usize,
    pub unresolved_count: usize,
    pub satisfied_count: usize,
    pub blocked_count: usize,
    pub release_complete: bool,
    pub creates_authority: bool,
    pub creates_output_commit: bool,
    pub residual_risks: Vec<String>,
    pub claim_boundary: String,
}

impl ProgramAnalysisReport {
    pub fn stage(&self, id: &str) -> Option<&StageReceipt> {
        self.stages.iter().find(|stage| stage.stage_id == id)
    }
}
