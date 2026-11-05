//! Explicit finite Boolean witness-versus-theory profile; no implicit goal/TL.
use super::{decision::DecisionParts, types::*};
use crate::{facet_ledger::FacetLedger, formal_measurement as m};
use lc631_analysis::creativity_measurement::RationalInterval;
use lc631_core::context::{ContextRevision, ContextScope, SourceRevision};
use lc631_solvers::typed_query::SmtStatusQuery;
use lc631_substrate::{
    resource::{ResourceClaim, ResourceError, ResourceReservationView},
    Digest32,
};
use sha2::{Digest, Sha256};

mod produce;
pub const FINITE_BOOLEAN_MAX_ATOMS: usize = 8;
pub const FINITE_BOOLEAN_RECEIPT_RAM: u64 = 256 * 1024;
pub const FINITE_BOOLEAN_RECEIPT_OUTPUT: u64 = 256 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeclaredEvaluation {
    source: SourceRevision,
    scope: ContextScope,
    goal: GoalPolicy,
}
impl DeclaredEvaluation {
    /// Declarative registry lookup, BEFORE OriginalContext and formal parsing.
    /// This neither evaluates the input nor claims a supported parsed fragment.
    pub fn finite_boolean(
        source: SourceRevision,
        base_scope: ContextScope,
        goal: GoalPolicy,
    ) -> Result<Self, AdmissionError> {
        if let GoalPolicy::SatisfyingModel { revision }
        | GoalPolicy::DeclaredSourceObjective { revision } = goal
        {
            if revision != base_scope.goal() {
                return Err(AdmissionError::GoalRevision);
            }
        }
        let goal_tag = match goal {
            GoalPolicy::NoGoal => 0,
            GoalPolicy::SatisfyingModel { .. } => 1,
            GoalPolicy::DeclaredSourceObjective { .. } => 2,
        };
        let policy = binding(b"canonical.finite-boolean-v1.policy", &[
            b"theory-to-native-valuation;all-source-atoms-both-polarities;uniform-positive-1;both-directions;atoms1..8;source-nodes64;depth32;work1000000;per-probe32768;no-total-TL;TheoryToWitnessModelSpace-v1",
            source.digest().as_bytes(), base_scope.semantics().as_bytes(), base_scope.goal().as_bytes(), &[goal_tag],
        ]);
        let probes = binding(
            b"canonical.finite-boolean-v1.probes",
            &[
                source.digest().as_bytes(),
                policy.as_bytes(),
                base_scope.probes().as_bytes(),
            ],
        );
        Ok(Self {
            source,
            scope: ContextScope::declared(
                base_scope.comparison(),
                base_scope.goal(),
                probes,
                policy,
            ),
            goal,
        })
    }
    pub const fn scope(self) -> ContextScope {
        self.scope
    }
    pub const fn source(self) -> SourceRevision {
        self.source
    }
    pub const fn profile_name(self) -> &'static str {
        "finite-boolean-v1"
    }
    pub(super) fn admit(self, request: CanonicalRequest<'_, '_>) -> Result<(), AdmissionError> {
        if self.source != request.source_revision() || self.scope != request.context.scope() {
            return Err(AdmissionError::ContextMismatch);
        }
        if self.goal != request.goal {
            return Err(AdmissionError::GoalConflict);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BooleanEvaluationState {
    Measured,
    UnsupportedInteger,
    EmptyAtomSet,
    AtomLimit,
    UnsupportedFormal,
    SourceComplexityLimit,
    MissingNativeModel,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EvaluationError {
    Resource(ResourceError),
    AllocationFailed,
    SourceBinding,
    Query(lc631_solvers::typed_query::QueryEmitError),
    Formal(crate::formal_boolean::Failure),
    Bridge(m::BridgeError),
    Produced(m::produced::ProducedError),
    Owned(m::owned::OwnedError),
    Facets(crate::facet_measurement_bridge::IngressError),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BooleanProbeReceipt {
    pub binding: Digest32,
    pub positive: bool,
    pub source_symbol: lc631_solvers::typed_query::BoolSymbolId,
    pub target_symbol: lc631_solvers::typed_query::BoolSymbolId,
    pub weight_numerator: u64,
    pub weight_denominator: u64,
    pub soundness: m::ProbeState,
    pub completeness: m::ProbeState,
    pub assignments: Option<u32>,
    pub ast_visits: Option<u64>,
    pub source_theory: Option<Digest32>,
    pub target_theory: Option<Digest32>,
}

/// Constructed only by the scoped actual producer. Source/query/context backing
/// remains borrowed through NativeEvidence.request; target/probes/ledger and all
/// numeric backing here are owned. This is ephemeral, never durable replay.
pub struct OwnedBooleanMeasurement<'view> {
    pub(super) descriptor: DeclaredEvaluation,
    pub(super) source_context: ContextRevision,
    pub(super) model_observation: Option<Digest32>,
    pub(super) target: Option<SmtStatusQuery>,
    pub(super) target_bytes: Vec<u8>,
    pub(super) target_revision: Option<SourceRevision>,
    pub(super) probes: Vec<BooleanProbeReceipt>,
    pub(super) soundness: Option<RationalInterval>,
    pub(super) completeness: Option<RationalInterval>,
    pub(super) semantic: Option<RationalInterval>,
    pub(super) ledger: Option<FacetLedger>,
    pub(super) state: BooleanEvaluationState,
    pub(super) claim: ResourceClaim<'view>,
}
impl OwnedBooleanMeasurement<'_> {
    pub fn state(&self) -> BooleanEvaluationState {
        self.state
    }
    pub fn profile(&self) -> DeclaredEvaluation {
        self.descriptor
    }
    pub fn probes(&self) -> &[BooleanProbeReceipt] {
        &self.probes
    }
    pub fn soundness(&self) -> Option<&RationalInterval> {
        self.soundness.as_ref()
    }
    pub fn completeness(&self) -> Option<&RationalInterval> {
        self.completeness.as_ref()
    }
    pub fn semantic(&self) -> Option<&RationalInterval> {
        self.semantic.as_ref()
    }
    pub fn target_bytes(&self) -> &[u8] {
        &self.target_bytes
    }
    pub fn target_query(&self) -> Option<&SmtStatusQuery> {
        self.target.as_ref()
    }
    pub fn target_revision(&self) -> Option<SourceRevision> {
        self.target_revision
    }
    pub fn facets(&self) -> Option<&FacetLedger> {
        self.ledger.as_ref()
    }
    pub fn source_context(&self) -> ContextRevision {
        self.source_context
    }
    pub fn model_observation_binding(&self) -> Option<Digest32> {
        self.model_observation
    }
    pub(super) fn check_current(
        &self,
        evidence: &NativeEvidence<'_, '_>,
        current: &ResourceReservationView<'_>,
    ) -> Result<(), AdmissionError> {
        self.claim.check().map_err(AdmissionError::Resource)?;
        if !self.claim.same_reservation(current) {
            return Err(AdmissionError::Resource(ResourceError::ForeignOwner));
        }
        self.descriptor.admit(evidence.request)?;
        if self.source_context != evidence.request.context.revision() {
            return Err(AdmissionError::ContextMismatch);
        }
        if let Some(native) = evidence.native_model() {
            if self.model_observation != Some(model_binding(native.observation().stdout())) {
                return Err(AdmissionError::ContextMismatch);
            }
        } else if self.state != BooleanEvaluationState::MissingNativeModel {
            return Err(AdmissionError::ContextMismatch);
        }
        if self.state == BooleanEvaluationState::Measured {
            let native = evidence
                .native_model()
                .ok_or(AdmissionError::ContextMismatch)?;
            let target = self
                .target
                .as_ref()
                .ok_or(AdmissionError::ContextMismatch)?;
            if target.boolean_symbols() != native.model().query().boolean_symbols()
                || target.assertions().len() != target.boolean_symbols().len()
                || self.probes.len() != target.boolean_symbols().len() * 2
            {
                return Err(AdmissionError::ContextMismatch);
            }
            for (&id, assertion) in target.boolean_symbols().iter().zip(target.assertions()) {
                let matches = match (native.model().boolean(id), assertion) {
                    (Some(true), lc631_solvers::typed_query::BoolExpr::Variable(actual)) => {
                        *actual == id
                    }
                    (Some(false), lc631_solvers::typed_query::BoolExpr::Not(child)) => {
                        matches!(child.as_ref(),lc631_solvers::typed_query::BoolExpr::Variable(actual) if *actual == id)
                    }
                    _ => false,
                };
                if !matches {
                    return Err(AdmissionError::ContextMismatch);
                }
            }
            let text = std::str::from_utf8(&self.target_bytes)
                .map_err(|_| AdmissionError::ContextMismatch)?;
            if self.target_revision != Some(SourceRevision::of(text)) {
                return Err(AdmissionError::ContextMismatch);
            }
        }
        Ok(())
    }
}
pub(super) fn model_binding(bytes: &[u8]) -> Digest32 {
    binding(b"canonical.finite-boolean.actual-native.v1", &[bytes])
}
pub(super) fn binding(domain: &[u8], parts: &[&[u8]]) -> Digest32 {
    let mut hash = Sha256::new();
    hash.update(domain);
    for part in parts {
        hash.update((part.len() as u64).to_le_bytes());
        hash.update(part);
    }
    Digest32::from_bytes(hash.finalize().into())
}
pub(super) fn build<'view>(
    evidence: &NativeEvidence<'_, 'view>,
    current: &'view ResourceReservationView<'_>,
    cap: usize,
    objective: Option<&super::optimization::OwnedObjectiveStep<'_>>,
) -> Result<
    (
        Result<DecisionParts, PostprocessError>,
        OwnedBooleanMeasurement<'view>,
    ),
    PostprocessError,
> {
    produce::build(evidence, current, cap, objective)
}
