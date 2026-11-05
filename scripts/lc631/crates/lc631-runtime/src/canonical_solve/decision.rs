use super::types::*;
use crate::{
    facet_ledger::FacetLedger,
    joint_selection::{RecommendationState, SelectionOptimality},
    recommendation_output::RenderedDecision,
    resource::ResourceBroker,
};
use lc631_analysis::creativity_measurement::RationalInterval;
use lc631_core::{context::SourceRevision, CandidateId};
use lc631_solvers::smt::native_model::{NativeCheckedModel, NativeModelError};
use lc631_substrate::{
    resource::{ResourceClaim, ResourceKind, ResourceReservationView},
    Digest32,
};
use sha2::{Digest, Sha256};

pub(super) mod construct;
mod objective_scope;
pub(in crate::canonical_solve) mod report_identity;
mod text;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectionSummary {
    pub candidate_count: usize,
    pub frontier_count: usize,
    pub recommendation: Option<CandidateId>,
    pub recommendation_state: RecommendationState,
    pub optimality: SelectionOptimality,
}
pub enum CanonicalRendered {
    Conditional(RenderedDecision),
    Diagnostic {
        source: SourceRevision,
        text: String,
        binding: Digest32,
    },
}
impl CanonicalRendered {
    pub fn text(&self) -> &str {
        match self {
            Self::Conditional(r) => r.text(),
            Self::Diagnostic { text, .. } => text,
        }
    }
    pub fn binding(&self) -> Digest32 {
        match self {
            Self::Conditional(r) => r.binding(),
            Self::Diagnostic { binding, .. } => *binding,
        }
    }
    pub fn matches_exact(&self, source: &str, bytes: &[u8]) -> bool {
        match self {
            Self::Conditional(r) => r.matches_exact(source, bytes),
            Self::Diagnostic {
                source: expected,
                text,
                binding,
            } => {
                SourceRevision::of(source) == *expected
                    && bytes == text.as_bytes()
                    && diagnostic_binding(*expected, bytes) == *binding
            }
        }
    }
}
pub(super) fn diagnostic_binding(source: SourceRevision, bytes: &[u8]) -> Digest32 {
    let mut hash = Sha256::new();
    hash.update(b"epistesys.canonical-solve.diagnostic.v1\0");
    hash.update(source.digest().as_bytes());
    hash.update(bytes);
    Digest32::from_bytes(hash.finalize().into())
}

pub(super) struct DecisionParts {
    pub ledger: FacetLedger,
    pub rendered: CanonicalRendered,
    pub selection: Option<SelectionSummary>,
    pub goal: Option<RationalInterval>,
    pub status: CanonicalStatus,
    pub pending: NoRecommendationReason,
}

/// Actual model/stdout -> store -> ledger/text drop before the outer RAM/output
/// claim. Query/context backing remains borrowed from the caller-owned parser.
pub struct OwnedCanonicalDecision<'p, 'view> {
    evidence: NativeEvidence<'p, 'view>,
    ledger: FacetLedger,
    rendered: CanonicalRendered,
    selection: Option<SelectionSummary>,
    goal: Option<RationalInterval>,
    status: CanonicalStatus,
    pending: NoRecommendationReason,
    claim: ResourceClaim<'view>,
}
impl<'p, 'view> OwnedCanonicalDecision<'p, 'view> {
    pub(super) fn replay_request(&self) -> CanonicalRequest<'p, 'view> {
        self.evidence.request
    }
    pub(super) fn assemble(
        evidence: NativeEvidence<'p, 'view>,
        parts: DecisionParts,
        claim: ResourceClaim<'view>,
    ) -> Self {
        Self {
            evidence,
            ledger: parts.ledger,
            rendered: parts.rendered,
            selection: parts.selection,
            goal: parts.goal,
            status: parts.status,
            pending: parts.pending,
            claim,
        }
    }
    /// Historical exact text, not renewed eligibility or delivery.
    pub fn historical_text(&self) -> &str {
        self.rendered.text()
    }
    pub fn matches_exact(&self, source: &str, bytes: &[u8]) -> bool {
        self.rendered.matches_exact(source, bytes)
    }
    pub fn native_model(&self) -> Option<&NativeCheckedModel<'p, 'view>> {
        match self.evidence.attempts.first() {
            Some(NativeAttempt::Model(m)) => Some(m),
            _ => None,
        }
    }
    pub fn native_error(&self) -> Option<&NativeModelError<'view>> {
        match self.evidence.attempts.first() {
            Some(NativeAttempt::Rejected(e)) => Some(e),
            _ => None,
        }
    }
    pub fn backend_unavailable(&self) -> Option<&BackendUnavailable> {
        match self.evidence.attempts.first() {
            Some(NativeAttempt::Unavailable(e)) => Some(e),
            _ => None,
        }
    }
    pub fn native_kind(&self) -> NativeEvidenceKind {
        self.evidence.kind()
    }
    pub fn stored_results(&self) -> usize {
        self.evidence.stored_results()
    }
    pub fn goal_observation(&self) -> Option<&RationalInterval> {
        self.goal.as_ref()
    }
    pub fn selection_summary(&self) -> Option<&SelectionSummary> {
        self.selection.as_ref()
    }
    /// The original native_model accessor remains the initial observation;
    /// this receipt identifies the model actually selected by the explicit step.
    pub fn objective_step(&self) -> Option<&super::optimization::OwnedObjectiveStep<'view>> {
        self.evidence.objective.first()
    }
    pub fn boolean_measurement(
        &self,
    ) -> Option<&super::evaluation::OwnedBooleanMeasurement<'view>> {
        self.evidence.measurement.as_ref()
    }
    pub fn facets(&self) -> &FacetLedger {
        &self.ledger
    }
    pub const fn status(&self) -> CanonicalStatus {
        self.status
    }
    pub const fn no_recommendation_reason(&self) -> NoRecommendationReason {
        self.pending
    }
    pub fn project_current(
        &self,
        broker: &ResourceBroker,
        current: &ResourceReservationView<'_>,
    ) -> Result<&CanonicalRendered, AdmissionError> {
        let request = self.evidence.request;
        broker
            .validate_lease(
                current,
                current.run_id(),
                request.source_revision().digest(),
                ResourceKind::Ram,
            )
            .map_err(AdmissionError::Resource)?;
        self.claim.check().map_err(AdmissionError::Resource)?;
        if !self.claim.same_reservation(current) {
            return Err(AdmissionError::Resource(
                lc631_substrate::resource::ResourceError::ForeignOwner,
            ));
        }
        if let FormalInput::Problem(problem) = request.input {
            problem
                .check_current(current)
                .map_err(AdmissionError::Resource)?;
            if problem.context() != request.context.revision()
                || problem.scope() != request.context.scope()
            {
                return Err(AdmissionError::ContextMismatch);
            }
        }
        if let FormalInput::Unsupported(report) = request.input {
            report
                .check_current(current)
                .map_err(AdmissionError::Resource)?;
        }
        if let Some(measured) = self.evidence.measurement.as_ref() {
            measured.check_current(&self.evidence, current)?;
        }
        if let Some(objective) = self.evidence.objective.first() {
            objective.check_current(&self.evidence, current)?;
        }
        Ok(&self.rendered)
    }
}
