use crate::{
    build_dgcl_implementation_closure, verify_dgcl_completion_plan, CodingEvidenceClaim,
    DgclImplementationClosureContext, DgclImplementationStatus, DgclPipelineReport,
    ImplementationGapState,
};
use lc631_core::stable_sha256;
use serde::Serialize;
use serde_json::Value;

pub const DGCL_STANDALONE_CANDIDATE_SCHEMA: &str = "epistesys-dgcl-standalone-candidate.v1";
const MAX_STANDALONE_CANDIDATE_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DgclStandaloneCandidateState {
    ExactCandidateCaptured,
    NoAction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DgclOutputRealizationState {
    NotObserved,
    ContractLedgerRendered,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostObservationState {
    Pending,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DgclRequirementOutputRealization {
    pub requirement_id: String,
    pub closure_status: DgclImplementationStatus,
    pub open_gap_ids: Vec<String>,
    pub output_realization: DgclOutputRealizationState,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DgclHostObservationStages {
    pub precommit: HostObservationState,
    pub post_send: HostObservationState,
    pub sink_delivery: HostObservationState,
    pub durable_replay: HostObservationState,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DgclStandaloneCandidate {
    pub schema_version: &'static str,
    pub source_revision: String,
    pub program_ir_digest: String,
    pub completion_plan_digest: String,
    pub implementation_closure_status: DgclImplementationStatus,
    pub implementation_complete_candidate: bool,
    pub candidate_state: DgclStandaloneCandidateState,
    pub candidate_digest: String,
    pub candidate_byte_len: usize,
    pub expected_schema_version: String,
    pub observed_schema_version: String,
    pub schema_version_matches: bool,
    pub candidate_text: String,
    pub requirement_realizations: Vec<DgclRequirementOutputRealization>,
    pub host_observation: DgclHostObservationStages,
    pub host_send_authorized: bool,
    pub output_commit_allowed: bool,
    pub authority_created: bool,
    pub claim_boundary: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum DgclStandaloneCandidateError {
    SourceRevisionMismatch,
    PipelineBindingMismatch,
    CompletionPlanInvalid,
    CandidateTooLarge,
    CandidateEmpty,
    CandidateUtf8Invalid,
    CandidateJsonInvalid,
    CandidateSchemaMismatch,
    CandidateSchemaExpectationInvalid,
    ClosureVerificationFailed,
}

/// Reverify the whole per-requirement evidence set, then render that exact
/// checked ledger. This observes ledger realization, not arbitrary prose
/// semantics or host delivery. Caller-constructed pipeline flags are ignored.
pub fn build_verified_dgcl_completion_candidate(
    source: &str,
    pipeline: &DgclPipelineReport,
    claims: &[CodingEvidenceClaim],
    context: DgclImplementationClosureContext<'_>,
) -> Result<DgclStandaloneCandidate, DgclStandaloneCandidateError> {
    if !crate::verify_dgcl_pipeline_identity(source, pipeline) {
        return Err(DgclStandaloneCandidateError::PipelineBindingMismatch);
    }
    let closure = build_dgcl_implementation_closure(
        source,
        &pipeline.program_ir,
        &pipeline.translation_projection,
        &pipeline.completion_plan,
        claims,
        context,
    )
    .map_err(|_| DgclStandaloneCandidateError::ClosureVerificationFailed)?;
    let output = serde_json::json!({
        "schema_version": "epistesys-dgcl-completion-output.v1",
        "source_revision": closure.source_revision,
        "program_ir_digest": closure.program_ir_digest,
        "completion_plan_digest": closure.completion_plan_digest,
        "requirements": closure.requirements,
        "gaps": closure.gaps,
        "blockers": closure.blockers,
        "status": closure.status,
        "authority_created": false,
        "output_commit_allowed": false
    });
    let bytes = serde_json::to_vec(&output)
        .map_err(|_| DgclStandaloneCandidateError::CandidateJsonInvalid)?;
    let mut checked_pipeline = pipeline.clone();
    checked_pipeline.implementation_closure = closure;
    let mut candidate = build_dgcl_standalone_candidate(
        source,
        &checked_pipeline,
        &bytes,
        "epistesys-dgcl-completion-output.v1",
    )?;
    candidate.implementation_complete_candidate = checked_pipeline
        .implementation_closure
        .implementation_complete_candidate;
    candidate.implementation_closure_status = checked_pipeline.implementation_closure.status;
    for realization in &mut candidate.requirement_realizations {
        realization.output_realization = DgclOutputRealizationState::ContractLedgerRendered;
        let checked = checked_pipeline
            .implementation_closure
            .requirements
            .iter()
            .find(|requirement| requirement.requirement_id == realization.requirement_id)
            .ok_or(DgclStandaloneCandidateError::PipelineBindingMismatch)?;
        realization.closure_status = checked.status;
        realization.open_gap_ids = checked
            .gap_ids
            .iter()
            .filter(|gap_id| {
                checked_pipeline
                    .implementation_closure
                    .gaps
                    .iter()
                    .any(|gap| {
                        &gap.gap_id == *gap_id
                            && !matches!(
                                gap.state,
                                ImplementationGapState::VerifiedReceipt
                                    | ImplementationGapState::NotApplicable
                            )
                    })
            })
            .cloned()
            .collect();
    }
    candidate.claim_boundary = "all per-task receipts were reverified against the current lifecycle and exact source/IR/plan; the checked requirement/gap ledger was rendered into exact candidate bytes; ledger realization is not general semantic correctness, send authority, host delivery, or research evidence";
    Ok(candidate)
}

pub fn build_dgcl_standalone_candidate(
    source: &str,
    pipeline: &DgclPipelineReport,
    candidate_bytes: &[u8],
    expected_schema_version: &str,
) -> Result<DgclStandaloneCandidate, DgclStandaloneCandidateError> {
    let source_revision = stable_sha256(source);
    if source_revision != pipeline.source_revision
        || source_revision != pipeline.program_ir.source_revision
        || source_revision != pipeline.translation_projection.source_revision
        || source_revision != pipeline.completion_plan.source_revision
        || source_revision != pipeline.implementation_closure.source_revision
    {
        return Err(DgclStandaloneCandidateError::SourceRevisionMismatch);
    }
    if pipeline.translation_projection.program_ir_digest != pipeline.program_ir.program_digest
        || pipeline.completion_plan.program_ir_digest != pipeline.program_ir.program_digest
        || pipeline.implementation_closure.program_ir_digest != pipeline.program_ir.program_digest
        || pipeline.implementation_closure.completion_plan_digest
            != pipeline.completion_plan.plan_digest
    {
        return Err(DgclStandaloneCandidateError::PipelineBindingMismatch);
    }
    verify_dgcl_completion_plan(
        source,
        &pipeline.program_ir,
        &pipeline.translation_projection,
        &pipeline.completion_plan,
    )
    .map_err(|_| DgclStandaloneCandidateError::CompletionPlanInvalid)?;
    if !crate::verify_dgcl_pipeline_identity(source, pipeline) {
        return Err(DgclStandaloneCandidateError::PipelineBindingMismatch);
    }
    let diagnostic_closure = build_dgcl_implementation_closure(
        source,
        &pipeline.program_ir,
        &pipeline.translation_projection,
        &pipeline.completion_plan,
        &[],
        DgclImplementationClosureContext {
            current_lifecycle: None,
            verifier: None,
            replay: &mut lc631_receipt_kernel::ReplayGuard::default(),
            now_epoch: 0,
        },
    )
    .map_err(|_| DgclStandaloneCandidateError::ClosureVerificationFailed)?;
    if expected_schema_version.trim().is_empty()
        || expected_schema_version.len() > 128
        || expected_schema_version.chars().any(char::is_control)
    {
        return Err(DgclStandaloneCandidateError::CandidateSchemaExpectationInvalid);
    }
    if candidate_bytes.is_empty() {
        return Err(DgclStandaloneCandidateError::CandidateEmpty);
    }
    if candidate_bytes.len() > MAX_STANDALONE_CANDIDATE_BYTES {
        return Err(DgclStandaloneCandidateError::CandidateTooLarge);
    }
    let candidate_text = std::str::from_utf8(candidate_bytes)
        .map_err(|_| DgclStandaloneCandidateError::CandidateUtf8Invalid)?
        .to_string();
    let candidate_json: Value = serde_json::from_slice(candidate_bytes)
        .map_err(|_| DgclStandaloneCandidateError::CandidateJsonInvalid)?;
    let observed_schema_version = candidate_json
        .get("schema_version")
        .and_then(Value::as_str)
        .ok_or(DgclStandaloneCandidateError::CandidateSchemaMismatch)?;
    if observed_schema_version != expected_schema_version {
        return Err(DgclStandaloneCandidateError::CandidateSchemaMismatch);
    }
    let mut requirement_realizations = Vec::new();
    for planned in &pipeline.completion_plan.requirements {
        let closure = diagnostic_closure
            .requirements
            .iter()
            .find(|closure| closure.requirement_id == planned.requirement_id)
            .ok_or(DgclStandaloneCandidateError::PipelineBindingMismatch)?;
        requirement_realizations.push(DgclRequirementOutputRealization {
            requirement_id: planned.requirement_id.clone(),
            closure_status: closure.status,
            open_gap_ids: closure
                .gap_ids
                .iter()
                .filter(|gap_id| {
                    diagnostic_closure.gaps.iter().any(|gap| {
                        &gap.gap_id == *gap_id
                            && !matches!(
                                gap.state,
                                ImplementationGapState::VerifiedReceipt
                                    | ImplementationGapState::NotApplicable
                            )
                    })
                })
                .cloned()
                .collect(),
            output_realization: DgclOutputRealizationState::NotObserved,
        });
    }
    let candidate_state = if pipeline.completion_plan.requirements.is_empty()
        && diagnostic_closure.status == DgclImplementationStatus::NoAction
    {
        DgclStandaloneCandidateState::NoAction
    } else {
        DgclStandaloneCandidateState::ExactCandidateCaptured
    };
    Ok(DgclStandaloneCandidate {
        schema_version: DGCL_STANDALONE_CANDIDATE_SCHEMA,
        source_revision,
        program_ir_digest: pipeline.program_ir.program_digest.clone(),
        completion_plan_digest: pipeline.completion_plan.plan_digest.clone(),
        implementation_closure_status: diagnostic_closure.status,
        // Public reports are descriptive and caller-constructible. Only the
        // reverified constructor above may positively set this field.
        implementation_complete_candidate: false,
        candidate_state,
        candidate_digest: stable_sha256(&candidate_text),
        candidate_byte_len: candidate_bytes.len(),
        expected_schema_version: expected_schema_version.to_string(),
        observed_schema_version: observed_schema_version.to_string(),
        schema_version_matches: true,
        candidate_text,
        requirement_realizations,
        host_observation: DgclHostObservationStages {
            precommit: HostObservationState::Pending,
            post_send: HostObservationState::Pending,
            sink_delivery: HostObservationState::Pending,
            durable_replay: HostObservationState::Pending,
        },
        host_send_authorized: false,
        output_commit_allowed: false,
        authority_created: false,
        claim_boundary: "exact UTF-8 candidate bytes and top-level schema_version were captured; requirement-to-output semantic realization is not observed, implementation closure is a separate gate, and no host precommit/postsend/sink/durable-replay observation or send authority is created",
    })
}
