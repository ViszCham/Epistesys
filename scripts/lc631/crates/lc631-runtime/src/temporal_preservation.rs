//! 日本語: 実有限評価を既存時間比較/K1へ接続する。source truthや証跡を発行しない。
//! English: Actual finite evaluations feed existing temporal comparison/K1, not source truth or evidence issuance.
use crate::facet_ledger::FiniteObservationBinding;
use crate::finite_results::{FiniteResultError, FiniteResultStore};
use lc631_analysis::creativity_measurement::{
    InputBasis, MeasurementError, RationalInterval, SemanticValidation,
};
pub use lc631_analysis::spatiotemporal_loss::{Gap, IncomparableReason, TemporalProbe};
use lc631_core::context::ContextScope;
use lc631_solvers::temporal::{EvaluationBudget, FiniteTrace, TemporalQuery};
use lc631_substrate::resource::{
    ResourceAmount, ResourceError, ResourceKind, ResourceRequest, ResourceReservationView,
};
use lc631_substrate::Digest32;

mod comparison;
mod policy;
pub use policy::{
    DeclaredTimeTranslation, ProtectedTemporalProbe, ProtectedTemporalRelation,
    TemporalPreservationPolicy,
};

/// Immutable caller-owned exact model inputs; predicate correspondence is declared by the policy.
#[derive(Clone, Copy)]
pub struct TemporalModelInput<'a> {
    pub query: &'a TemporalQuery,
    pub trace: &'a FiniteTrace,
    pub scope: ContextScope,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TemporalPreservationError {
    InvalidProbePlan,
    AllocationFailed,
    Store(FiniteResultError),
    Resource(ResourceError),
    Measurement(MeasurementError),
    TemporalInput(lc631_analysis::spatiotemporal_loss::InputError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TemporalPreservationState {
    /// Arithmetic on Given finite models and declared correspondence only.
    GivenFiniteModel {
        loss: RationalInterval,
        unresolved_probes: usize,
        uncertainty: TemporalUncertainty,
    },
    /// Undefined/unimplemented map or quantity is not a numerical unknown observation.
    NoScalar {
        reason: Gap,
        uncertainty: TemporalUncertainty,
    },
    Incomparable(IncomparableReason),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TemporalUncertainty {
    pub missing_observation: usize,
    pub missing_calibration: usize,
    pub missing_evidence: usize,
    pub unsupported_quantity: usize,
}

/// Only the fresh-admission adapter constructs this bounded arithmetic report.
/// Store observations are lineage, never independent or authenticated evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemporalPreservationReport {
    state: TemporalPreservationState,
    policy: Digest32,
    binding: Digest32,
    denominator: u64,
    source: Option<FiniteObservationBinding>,
    target: Option<FiniteObservationBinding>,
}

impl TemporalPreservationReport {
    pub fn state(&self) -> &TemporalPreservationState {
        &self.state
    }
    pub fn loss(&self) -> Option<&RationalInterval> {
        match &self.state {
            TemporalPreservationState::GivenFiniteModel { loss, .. } => Some(loss),
            TemporalPreservationState::NoScalar { .. } => None,
            TemporalPreservationState::Incomparable(_) => None,
        }
    }
    pub const fn policy_digest(&self) -> Digest32 {
        self.policy
    }
    pub const fn binding_digest(&self) -> Digest32 {
        self.binding
    }
    pub const fn denominator(&self) -> u64 {
        self.denominator
    }
    pub const fn source_observation(&self) -> Option<FiniteObservationBinding> {
        self.source
    }
    pub const fn target_observation(&self) -> Option<FiniteObservationBinding> {
        self.target
    }
    pub const fn validation_gap(&self) -> Gap {
        Gap::MissingEvidence
    }
    pub const fn input_basis(&self) -> InputBasis {
        InputBasis::Given
    }
    pub const fn semantic_validation(&self) -> SemanticValidation {
        SemanticValidation::Unassessed
    }
    pub const fn definition_revision(&self) -> &'static str {
        lc631_analysis::spatiotemporal_loss::DEFINITION_REVISION
    }
}

/// Execute both exact models using each store's fresh matching reservation.
/// A failure in the second admission may retain the first observation, never
/// a completed pair. Historical lookup alone is not an admission/permission.
/// Temporary aggregation workspace is charged; bounded returned arithmetic is
/// caller-owned. No whole-process RSS or allocator-overhead guarantee is made.
#[allow(clippy::too_many_arguments)]
pub fn evaluate_temporal_preservation<'input>(
    policy: &TemporalPreservationPolicy,
    source: TemporalModelInput<'input>,
    target: TemporalModelInput<'input>,
    source_store: &mut FiniteResultStore<'input, '_>,
    target_store: &mut FiniteResultStore<'input, '_>,
    source_resources: &ResourceReservationView<'_>,
    target_resources: &ResourceReservationView<'_>,
    budget: EvaluationBudget,
) -> Result<TemporalPreservationReport, TemporalPreservationError> {
    source_resources
        .check()
        .map_err(TemporalPreservationError::Resource)?;
    target_resources
        .check()
        .map_err(TemporalPreservationError::Resource)?;
    if !policy.matches(source, target) {
        return Ok(report(
            policy,
            TemporalPreservationState::Incomparable(IncomparableReason::BindingMismatch),
            None,
            None,
        ));
    }
    let amount = ResourceAmount::checked((policy.probes().len() as u64) * 4096)
        .map_err(TemporalPreservationError::Resource)?;
    let workspace = source_resources
        .claim_resources(&[ResourceRequest::new(ResourceKind::Ram, amount)])
        .map_err(TemporalPreservationError::Resource)?;
    let source_id = source_store
        .evaluate_temporal(
            source.query,
            source.trace,
            source.query.context(),
            source.scope,
            budget,
            source_resources,
        )
        .map_err(TemporalPreservationError::Store)?;
    let target_id = target_store
        .evaluate_temporal(
            target.query,
            target.trace,
            target.query.context(),
            target.scope,
            budget,
            target_resources,
        )
        .map_err(TemporalPreservationError::Store)?;
    let a = source_store
        .lookup_temporal(
            source_id,
            source.query,
            source.trace,
            source.query.context(),
            source.scope,
        )
        .map_err(TemporalPreservationError::Store)?;
    let b = target_store
        .lookup_temporal(
            target_id,
            target.query,
            target.trace,
            target.query.context(),
            target.scope,
        )
        .map_err(TemporalPreservationError::Store)?;
    let state = comparison::compare(policy, source.trace, target.trace, a.truth(), b.truth())?;
    let ab = source_store
        .temporal_binding(
            source_id,
            source.query,
            source.trace,
            source.query.context(),
            source.scope,
        )
        .map_err(TemporalPreservationError::Store)?;
    let bb = target_store
        .temporal_binding(
            target_id,
            target.query,
            target.trace,
            target.query.context(),
            target.scope,
        )
        .map_err(TemporalPreservationError::Store)?;
    workspace
        .check()
        .map_err(TemporalPreservationError::Resource)?;
    target_resources
        .check()
        .map_err(TemporalPreservationError::Resource)?;
    Ok(report(policy, state, Some(ab), Some(bb)))
}

fn report(
    policy: &TemporalPreservationPolicy,
    state: TemporalPreservationState,
    source: Option<FiniteObservationBinding>,
    target: Option<FiniteObservationBinding>,
) -> TemporalPreservationReport {
    TemporalPreservationReport {
        state,
        policy: policy.digest(),
        binding: policy.binding_digest(),
        denominator: policy.denominator(),
        source,
        target,
    }
}
