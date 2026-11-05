//! 日本語: Given閉有限MODELの実最短路B_mem producer。reportは証明・権限でない。
//! English: Actual owned shortest-path B_mem over a Given finite MODEL, not truth or Authority.
#![forbid(unsafe_code)]
mod path;
mod policy;
mod registry;
mod selection;
use crate::joint_selection::{
    project_component_reported, AxisKind, AxisValue, JointCandidate, MissingAxis,
    ReportedComponentAxis, SelectionError, SelectionPins, SelectionPolicy,
};
use lc631_analysis::creativity_measurement::{
    ComponentMetric, ComponentReport, ComponentState, IndependenceStatus, InputBasis,
    MeasurementBinding, MeasurementGap, SemanticValidation,
};
use lc631_core::measurement_numeric::{self as numeric, NumericError, Rational, RationalInterval};
use lc631_core::{context::ContextRevision, CandidateId};
use lc631_substrate::{
    resource::{
        ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
        ResourceReservationView,
    },
    Digest32,
};
pub use path::PathWitness;
pub use registry::{CostUnit, Pair, PairDisposition, RegistryPlan, State, StateId, WorkBudget};
pub use selection::{select_current, MemoryRow, OwnedMemorySelection};
use sha2::{Digest, Sha256};
pub const SCRATCH_RAM: u64 = 64 * 1024 * 1024;
pub const RETAINED_RAM: u64 = 32 * 1024 * 1024;
pub const RETAINED_OUTPUT: u64 = 2 * 1024 * 1024;
pub const PRODUCER: &[u8] = b"finite-formal-v1.4/CR-BMEM/owned-two-dijkstra-given-model.v1";
/// All declaration inputs remain Given. Candidate numeric IDs are mapping slots only.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Binding {
    pub selection: SelectionPins,
    pub candidate: CandidateId,
    pub candidate_digest: Digest32,
    pub context: ContextRevision,
    pub target: Digest32,
    pub transform: Digest32,
    pub definition: Digest32,
    pub probe_set: Digest32,
    pub oracle: Digest32,
    pub dependency_group: Digest32,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    Resource(ResourceError),
    Numeric(NumericError),
    Selection(SelectionError),
    ModelLimit,
    InvalidPlan,
    InvalidKappa,
    NegativeCost,
    ReversedCost,
    UnitMismatch,
    MissingBasis,
    DuplicateState,
    DuplicateContent,
    UnknownState,
    SelfTransition,
    DuplicatePair,
    AllocationFailed,
    WorkBudget,
    Witness,
    DefinitionMismatch,
    BindingMismatch,
    ForeignReservation,
    InvalidRows,
}
impl From<NumericError> for Error {
    fn from(e: NumericError) -> Self {
        Self::Numeric(e)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PathState {
    Connected,
    Disconnected,
    Unresolved,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComputationBasis {
    RuntimeComputedGivenModel,
}
pub fn definition_revision() -> Digest32 {
    hash(b"OperationalMeasurement/finite-formal-v1.4/CR-BMEM")
}
fn hash(input: &[u8]) -> Digest32 {
    Digest32::from_bytes(Sha256::digest(input).into())
}
fn integer(value: i64) -> Rational {
    Rational::from_integer(value.into())
}
fn reserved<T>(count: usize) -> Result<Vec<T>, Error> {
    let mut value = Vec::new();
    value
        .try_reserve_exact(count)
        .map_err(|_| Error::AllocationFailed)?;
    Ok(value)
}
pub(super) struct Work<'a, 'lease> {
    used: u64,
    limit: u64,
    resources: &'a ResourceReservationView<'lease>,
}
impl Work<'_, '_> {
    fn step(&mut self, count: u64) -> Result<(), Error> {
        self.resources.check().map_err(Error::Resource)?;
        self.used = self.used.checked_add(count).ok_or(Error::WorkBudget)?;
        if self.used > self.limit {
            return Err(Error::WorkBudget);
        }
        Ok(())
    }
}
pub(super) fn request(kind: ResourceKind, bytes: u64) -> Result<ResourceRequest, Error> {
    Ok(ResourceRequest::new(
        kind,
        ResourceAmount::checked(bytes).map_err(Error::Resource)?,
    ))
}
pub(super) fn claim<'v>(
    view: &'v ResourceReservationView<'_>,
    ram: u64,
    output: u64,
) -> Result<ResourceClaim<'v>, Error> {
    if output == 0 {
        view.claim_resources(&[request(ResourceKind::Ram, ram)?])
            .map_err(Error::Resource)
    } else {
        view.claim_resources(&[
            request(ResourceKind::Ram, ram)?,
            request(ResourceKind::Output, output)?,
        ])
        .map_err(Error::Resource)
    }
}
fn check_source(
    plan: &RegistryPlan<'_>,
    resources: &ResourceReservationView<'_>,
) -> Result<(), Error> {
    resources.check().map_err(Error::Resource)?;
    if resources.source_revision() != plan.binding.selection.source.digest() {
        return Err(Error::Resource(ResourceError::SourceMismatch));
    }
    if plan.binding.context.source() != plan.binding.selection.source
        || plan.binding.probe_set != plan.binding.selection.scope.probes()
    {
        return Err(Error::BindingMismatch);
    }
    Ok(())
}
/// Canonical report is an export view, not a checked certificate. Private owner
/// construction/claims/current projection cannot be replaced by a public report.
pub struct OwnedBridgeCost<'a, 'v> {
    report: ComponentReport,
    model: registry::FrozenModel,
    cost: Option<RationalInterval>,
    lower: Option<path::Endpoint>,
    upper: Option<path::Endpoint>,
    state: PathState,
    plan: &'a RegistryPlan<'a>,
    row_binding: Digest32,
    common_policy: Digest32,
    retained: ResourceClaim<'v>, // Backing above drops before the retained claim.
}
impl<'a> OwnedBridgeCost<'a, '_> {
    pub fn historical_report(&self) -> &ComponentReport {
        &self.report
    }
    pub fn historical_model_digest(&self) -> Digest32 {
        self.model.digest
    }
    pub fn historical_evidence_refs(&self) -> impl Iterator<Item = Digest32> + '_ {
        self.model.edges.iter().map(|edge| edge.evidence)
    }
    pub fn historical_cost(&self) -> Option<&RationalInterval> {
        self.cost.as_ref()
    }
    pub fn historical_path_state(&self) -> PathState {
        self.state
    }
    pub fn computation_basis(&self) -> ComputationBasis {
        ComputationBasis::RuntimeComputedGivenModel
    }
    pub fn lower_path(&self) -> Option<&PathWitness> {
        self.lower.as_ref().and_then(|e| e.witness.as_ref())
    }
    pub fn upper_path(&self) -> Option<&PathWitness> {
        self.upper.as_ref().and_then(|e| e.witness.as_ref())
    }
    pub fn measurement_binding(&self) -> Digest32 {
        self.row_binding
    }
    fn check(
        &self,
        plan: &RegistryPlan<'_>,
        resources: &ResourceReservationView<'_>,
    ) -> Result<(), Error> {
        check_source(plan, resources)?;
        self.retained.check().map_err(Error::Resource)?;
        if !self.retained.same_reservation(resources) {
            return Err(Error::ForeignReservation);
        }
        if !std::ptr::eq(plan, self.plan) || plan.binding != self.plan.binding {
            return Err(Error::BindingMismatch);
        }
        // Private model/result/report stay immutable after production.
        if self.report.binding.result != self.row_binding
            || self.report.evidence_payload != self.row_binding
        {
            return Err(Error::BindingMismatch);
        }
        Ok(())
    }
    pub fn report_current(
        &self,
        plan: &RegistryPlan<'_>,
        resources: &ResourceReservationView<'_>,
    ) -> Result<&ComponentReport, Error> {
        self.check(plan, resources)?;
        Ok(&self.report)
    }
    pub fn axis_current(
        &self,
        plan: &RegistryPlan<'_>,
        pins: SelectionPins,
        resources: &ResourceReservationView<'_>,
    ) -> Result<ReportedComponentAxis<'_>, Error> {
        self.check(plan, resources)?;
        if pins != self.plan.binding.selection {
            return Err(Error::BindingMismatch);
        }
        project_component_reported(&self.report).map_err(Error::Selection)
    }
    pub fn check_selection_row_current(
        &self,
        plan: &RegistryPlan<'_>,
        policy: &SelectionPolicy<'_>,
        row: &JointCandidate<'_>,
        index: usize,
        resources: &ResourceReservationView<'_>,
    ) -> Result<(), Error> {
        let current = self.axis_current(plan, policy.pins, resources)?;
        if row.pins != policy.pins
            || row.id != self.plan.binding.candidate
            || row.context != self.plan.binding.context
            || row.measurement_binding != self.row_binding
            || row.axes.len() != policy.axes.len()
            || policy.axes.get(index) != Some(&AxisKind::MigrationCost)
        {
            return Err(Error::BindingMismatch);
        }
        let same = match (row.axes.get(index), current.value) {
            (Some(AxisValue::Exact(attached)), AxisValue::Exact(actual))
            | (Some(AxisValue::Bounded(attached)), AxisValue::Bounded(actual)) => {
                std::ptr::eq(*attached, actual)
            }
            (
                Some(AxisValue::Unavailable(MissingAxis::Unknown)),
                AxisValue::Unavailable(MissingAxis::Unknown),
            ) => matches!(
                self.report.state,
                ComponentState::Unknown(MeasurementGap::MissingObservation)
            ),
            _ => false,
        };
        if !same {
            return Err(Error::BindingMismatch);
        }
        Ok(())
    }
}
/// Freeze validated copies only after real scratch + retained/output admission.
/// Two 32MiB owners + producer2's64MiB scratch fit the declared128MiB RAM profile.
pub fn execute_owned<'a, 'v>(
    plan: &'a RegistryPlan<'a>,
    resources: &'v ResourceReservationView<'_>,
) -> Result<OwnedBridgeCost<'a, 'v>, Error> {
    check_source(plan, resources)?;
    let scratch = claim(resources, SCRATCH_RAM, 0)?;
    let retained = claim(resources, RETAINED_RAM, RETAINED_OUTPUT)?;
    let mut work = Work {
        used: 0,
        limit: plan.budget.max_steps,
        resources,
    };
    let model = registry::freeze(plan, &mut work)?;
    let common_policy = policy::derive(plan, &model)?;
    let (lower, upper, cost, state, component) = if !model.closed {
        (
            None,
            None,
            None,
            PathState::Unresolved,
            ComponentState::Unknown(MeasurementGap::MissingObservation),
        )
    } else {
        let lower = path::compute(&model, false, &mut work)?;
        let upper = path::compute(&model, true, &mut work)?;
        match (&lower.distances[model.goal], &upper.distances[model.goal]) {
            (Some(lo), Some(hi)) => {
                let cost = RationalInterval::new(lo.clone(), hi.clone())?;
                let low = numeric::checked_divide(lo, &numeric::checked_add(lo, &model.kappa)?)?;
                let high = numeric::checked_divide(hi, &numeric::checked_add(hi, &model.kappa)?)?;
                let bounds = RationalInterval::new(low, high)?;
                (
                    Some(lower),
                    Some(upper),
                    Some(cost),
                    PathState::Connected,
                    ComponentState::Measured(bounds),
                )
            }
            (None, None) => (
                Some(lower),
                Some(upper),
                None,
                PathState::Disconnected,
                ComponentState::Measured(RationalInterval::exact_ratio(1, 1)?),
            ),
            _ => return Err(Error::Witness),
        }
    };
    let row_binding = result_digest(
        plan,
        &model,
        state,
        &component,
        lower.as_ref(),
        upper.as_ref(),
        common_policy,
    )?;
    let b = plan.binding;
    let report = ComponentReport {
        metric: ComponentMetric::BridgeCost,
        definition: b.definition,
        binding: MeasurementBinding {
            scope: b.selection.scope.comparison(),
            candidate: b.candidate_digest,
            source: b.selection.source.digest(),
            target: b.target,
            context: b.context.interpretation_digest(),
            baseline: b.selection.baseline,
            producer: hash(PRODUCER),
            transform: b.transform,
            result: row_binding,
        },
        probe_set: b.probe_set,
        oracle: b.oracle,
        dependency_group: b.dependency_group,
        evidence_payload: row_binding,
        basis: InputBasis::Given,
        validation: SemanticValidation::Unassessed,
        independence: IndependenceStatus::SharedDependency,
        state: component,
    };
    scratch.check().map_err(Error::Resource)?;
    retained.check().map_err(Error::Resource)?;
    Ok(OwnedBridgeCost {
        report,
        model,
        cost,
        lower,
        upper,
        state,
        plan,
        row_binding,
        common_policy,
        retained,
    })
}
fn result_digest(
    plan: &RegistryPlan<'_>,
    model: &registry::FrozenModel,
    state: PathState,
    component: &ComponentState,
    lower: Option<&path::Endpoint>,
    upper: Option<&path::Endpoint>,
    common_policy: Digest32,
) -> Result<Digest32, Error> {
    let b = plan.binding;
    let p = b.selection;
    let mut h = Sha256::new();
    h.update(PRODUCER);
    h.update(common_policy.as_bytes());
    for d in [
        p.source.digest(),
        p.scope.comparison(),
        p.scope.goal(),
        p.scope.probes(),
        p.scope.semantics(),
        p.baseline,
        p.hard_invariants,
        p.policy,
        p.evidence_revision,
        b.candidate_digest,
        b.context.interpretation_digest(),
        b.target,
        b.transform,
        b.definition,
        b.probe_set,
        b.oracle,
        b.dependency_group,
        model.digest,
    ] {
        registry::hash_field(&mut h, d.as_bytes());
    }
    match p.goal_profile {
        Some(d) => {
            h.update([1]);
            h.update(d.as_bytes());
        }
        None => h.update([0]),
    }
    h.update(plan.budget.max_steps.to_le_bytes());
    h.update([match state {
        PathState::Connected => 0,
        PathState::Disconnected => 1,
        PathState::Unresolved => 2,
    }]);
    match component {
        ComponentState::Measured(bounds) => {
            registry::hash_rational(&mut h, bounds.lower())?;
            registry::hash_rational(&mut h, bounds.upper())?;
        }
        ComponentState::Unknown(MeasurementGap::MissingObservation) => h.update(b"unknown"),
        _ => return Err(Error::Witness),
    }
    for endpoint in [lower, upper] {
        match endpoint {
            None => h.update([0]),
            Some(endpoint) => {
                h.update([1]);
                for distance in &endpoint.distances {
                    match distance {
                        None => h.update([0]),
                        Some(value) => {
                            h.update([1]);
                            registry::hash_rational(&mut h, value)?;
                        }
                    }
                }
                match &endpoint.witness {
                    None => h.update([0]),
                    Some(witness) => {
                        h.update([1]);
                        registry::hash_field(&mut h, witness.canonical_payload());
                        registry::hash_rational(&mut h, witness.cost())?;
                    }
                }
            }
        }
    }
    Ok(Digest32::from_bytes(h.finalize().into()))
}
