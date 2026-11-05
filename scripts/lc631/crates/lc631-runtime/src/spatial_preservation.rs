//! 日本語: 実閉矩形観測を既存空間比較/K1へ接続する。一般3Dやsource truthではない。
//! English: Actual closed-rectangle observations feed existing spatial comparison/K1, not general 3D/source truth.
use crate::causal_spatial_observation::{CausalSpatialObservations, ObservationError, Request};
use crate::facet_ledger::FiniteObservationBinding;
use lc631_analysis::creativity_measurement::{
    InputBasis, MeasurementError, RationalInterval, SemanticValidation,
};
pub use lc631_analysis::spatiotemporal_loss::{AxisSign, Gap, IncomparableReason, SpatialUnit};
use lc631_core::context::{ContextRevision, ContextScope};
use lc631_solvers::causal_spatial::{
    Evaluation, FrameId, Rectangle, SpatialRelation, UnknownReason,
};
use lc631_substrate::resource::{
    ResourceAmount, ResourceError, ResourceKind, ResourceRequest, ResourceReservationView,
};
use lc631_substrate::Digest32;

mod comparison;
mod policy;
pub use policy::{
    DeclaredSpatialMap, ProtectedSpatialProbe, ProtectedSpatialRelation, SpatialPreservationPolicy,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpatialModelInput {
    pub left: Option<Rectangle>,
    pub right: Option<Rectangle>,
    pub frame: FrameId,
    pub unit: SpatialUnit,
    pub context: ContextRevision,
    pub scope: ContextScope,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SpatialPreservationError {
    InvalidProbePlan,
    AllocationFailed,
    Resource(ResourceError),
    Observation(ObservationError),
    UnexpectedOutcome(UnknownReason),
    MissingSpatialPayload,
    Measurement(MeasurementError),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SpatialUncertainty {
    pub missing_objects: usize,
    pub missing_mapping: usize,
    pub unsupported_quantity: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SpatialPreservationState {
    GivenFiniteModel {
        loss: RationalInterval,
        uncertainty: SpatialUncertainty,
    },
    NoScalar {
        reason: Gap,
        uncertainty: SpatialUncertainty,
    },
    Incomparable(IncomparableReason),
}

/// Private producer construction; content bindings are not authenticated evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpatialPreservationReport {
    state: SpatialPreservationState,
    policy: Digest32,
    binding: Digest32,
    denominator: u64,
    source: Vec<FiniteObservationBinding>,
    target: Vec<FiniteObservationBinding>,
}
impl SpatialPreservationReport {
    pub fn state(&self) -> &SpatialPreservationState {
        &self.state
    }
    pub fn loss(&self) -> Option<&RationalInterval> {
        match &self.state {
            SpatialPreservationState::GivenFiniteModel { loss, .. } => Some(loss),
            SpatialPreservationState::NoScalar { .. }
            | SpatialPreservationState::Incomparable(_) => None,
        }
    }
    pub const fn denominator(&self) -> u64 {
        self.denominator
    }
    pub fn source_observations(&self) -> &[FiniteObservationBinding] {
        &self.source
    }
    pub fn target_observations(&self) -> &[FiniteObservationBinding] {
        &self.target
    }
    pub const fn policy_digest(&self) -> Digest32 {
        self.policy
    }
    pub const fn binding_digest(&self) -> Digest32 {
        self.binding
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
}

struct ObservedModel {
    values: [Option<bool>; 4],
    bindings: Vec<FiniteObservationBinding>,
}

/// Each call performs real tuple evaluations through the private producer.
/// Publication checks are not atomic revocation fences. Partial producer
/// admissions can remain on error; no completed preservation report is issued.
#[allow(clippy::too_many_arguments)]
pub fn evaluate_spatial_preservation(
    policy: &SpatialPreservationPolicy,
    source: SpatialModelInput,
    target: SpatialModelInput,
    source_store: &mut CausalSpatialObservations<'_>,
    target_store: &mut CausalSpatialObservations<'_>,
    source_resources: &ResourceReservationView<'_>,
    target_resources: &ResourceReservationView<'_>,
) -> Result<SpatialPreservationReport, SpatialPreservationError> {
    source_resources
        .check()
        .map_err(SpatialPreservationError::Resource)?;
    target_resources
        .check()
        .map_err(SpatialPreservationError::Resource)?;
    if !policy.matches(source, target) {
        return Ok(report(
            policy,
            SpatialPreservationState::Incomparable(IncomparableReason::BindingMismatch),
            Vec::new(),
            Vec::new(),
        ));
    }
    let amount = ResourceAmount::checked(32 * 1024).map_err(SpatialPreservationError::Resource)?;
    let workspace = source_resources
        .claim_resources(&[ResourceRequest::new(ResourceKind::Ram, amount)])
        .map_err(SpatialPreservationError::Resource)?;
    let swap = policy
        .mapping()
        .is_some_and(|map| map.signs.0 == AxisSign::Negative);
    let a = observe(source, source_store, source_resources, false)?;
    let b = observe(target, target_store, target_resources, swap)?;
    let state = comparison::compare(policy, source, target, a.values, b.values)?;
    workspace
        .check()
        .map_err(SpatialPreservationError::Resource)?;
    target_resources
        .check()
        .map_err(SpatialPreservationError::Resource)?;
    Ok(report(policy, state, a.bindings, b.bindings))
}

fn observe(
    input: SpatialModelInput,
    store: &mut CausalSpatialObservations<'_>,
    resources: &ResourceReservationView<'_>,
    reflect_x: bool,
) -> Result<ObservedModel, SpatialPreservationError> {
    let mut bindings = Vec::new();
    bindings
        .try_reserve_exact(4)
        .map_err(|_| SpatialPreservationError::AllocationFailed)?;
    let mut values = [None; 4];
    for (index, relation) in RELATIONS.into_iter().enumerate() {
        // Reflection in x reverses StrictLeft. This is an explicit mapped
        // predicate with exchanged target objects, never flipped truth bits.
        let (left, right) = if reflect_x && relation == SpatialRelation::StrictlyLeftOf {
            (input.right, input.left)
        } else {
            (input.left, input.right)
        };
        let request = Request::Spatial {
            left,
            right,
            relation,
            frame: input.frame,
            context: input.context,
            scope: input.scope,
        };
        let id = store
            .evaluate(&request, resources)
            .map_err(SpatialPreservationError::Observation)?;
        let observation = store
            .lookup(id, &request)
            .map_err(SpatialPreservationError::Observation)?;
        let actual = observation
            .spatial()
            .ok_or(SpatialPreservationError::MissingSpatialPayload)?;
        values[index] = match actual.outcome() {
            Evaluation::Computed(value) => Some(value),
            Evaluation::Unknown(UnknownReason::MissingSpatialObject) => None,
            Evaluation::Unknown(reason) => {
                return Err(SpatialPreservationError::UnexpectedOutcome(reason))
            }
        };
        bindings.push(observation.ledger_binding());
    }
    Ok(ObservedModel { values, bindings })
}

const RELATIONS: [SpatialRelation; 4] = [
    SpatialRelation::Contains,
    SpatialRelation::Connected,
    SpatialRelation::Touches,
    SpatialRelation::StrictlyLeftOf,
];

fn report(
    policy: &SpatialPreservationPolicy,
    state: SpatialPreservationState,
    source: Vec<FiniteObservationBinding>,
    target: Vec<FiniteObservationBinding>,
) -> SpatialPreservationReport {
    SpatialPreservationReport {
        state,
        policy: policy.digest(),
        binding: policy.binding_digest(),
        denominator: policy.denominator(),
        source,
        target,
    }
}
