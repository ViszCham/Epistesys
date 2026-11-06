//! 日本語: 実producerとimmutable入力を生存させ、現在lease下でlossを選択へ渡す。
//! ReportedGate・goal・map-backはGiven申告であり、意味真理や権限を発行しない。
//! English: Keep actual producers/immutable inputs live; consume loss under current leases.
//! Reported gates/goals/map-back are Given declarations, not truth or authority.
#![forbid(unsafe_code)]

use crate::causal_spatial_observation::{CausalSpatialObservations, ObservationError};
use crate::finite_results::{FiniteResultError, FiniteResultStore};
use crate::joint_selection::{AxisValue, MissingAxis};
use crate::spatial_preservation::{
    evaluate_spatial_preservation, SpatialModelInput, SpatialPreservationError,
    SpatialPreservationPolicy, SpatialPreservationReport, SpatialPreservationState,
};
use crate::temporal_preservation::{
    evaluate_temporal_preservation, Gap, TemporalModelInput, TemporalPreservationError,
    TemporalPreservationPolicy, TemporalPreservationReport, TemporalPreservationState,
};
use lc631_analysis::creativity_measurement::{InputBasis, RationalInterval};
use lc631_core::context::{ContextRevision, ContextScope};
use lc631_solvers::temporal::EvaluationBudget;
use lc631_substrate::resource::{
    ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
    ResourceReservationView,
};
use lc631_substrate::Digest32;

mod selection;
pub use selection::{select_current, CurrentSelection, RowOwners};

pub const INPUT_BASIS: InputBasis = InputBasis::Given;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    Resource(ResourceError),
    Temporal(TemporalPreservationError),
    Spatial(SpatialPreservationError),
    FiniteStore(FiniteResultError),
    SpatialStore(ObservationError),
    ForeignReservation,
    BindingMismatch,
    InvalidRows,
    Selection(crate::joint_selection::SelectionError),
}

/// No detached report constructor, Clone, Deref, Deserialize or mutable view.
/// Both private stores and their exact caller-owned query/trace remain live.
/// Content identity is not an authenticated receipt or independent evidence.
///
/// ```compile_fail
/// use lc631_runtime::spatiotemporal_selection::TemporalOwner;
/// use lc631_runtime::temporal_preservation::TemporalPreservationReport;
/// fn inject(report: TemporalPreservationReport) { TemporalOwner::from_report(report); }
/// ```
pub struct TemporalOwner<'input, 'view> {
    report: TemporalPreservationReport,
    policy: &'input TemporalPreservationPolicy,
    source: TemporalModelInput<'input>,
    target: TemporalModelInput<'input>,
    source_store: FiniteResultStore<'input, 'view>,
    target_store: FiniteResultStore<'input, 'view>,
    source_claim: ResourceClaim<'view>,
    target_claim: ResourceClaim<'view>,
}

impl<'input, 'view> TemporalOwner<'input, 'view> {
    /// Construct by fresh real evaluation only, charging bounded stores and
    /// retained arithmetic. A failed second producer publishes no owner.
    pub fn produce(
        policy: &'input TemporalPreservationPolicy,
        source: TemporalModelInput<'input>,
        target: TemporalModelInput<'input>,
        source_resources: &'view ResourceReservationView<'_>,
        target_resources: &'view ResourceReservationView<'_>,
        budget: EvaluationBudget,
    ) -> Result<Self, Error> {
        if source.query.context().source().digest() != source_resources.source_revision()
            || target.query.context().source().digest() != target_resources.source_revision()
        {
            return Err(Error::BindingMismatch);
        }
        let source_claim = claim(source_resources, 32 * 1024)?;
        let target_claim = claim(target_resources, 32 * 1024)?;
        let mut source_store =
            FiniteResultStore::with_resources(1, source_resources).map_err(Error::FiniteStore)?;
        let mut target_store =
            FiniteResultStore::with_resources(1, target_resources).map_err(Error::FiniteStore)?;
        let report = evaluate_temporal_preservation(
            policy,
            source,
            target,
            &mut source_store,
            &mut target_store,
            source_resources,
            target_resources,
            budget,
        )
        .map_err(Error::Temporal)?;
        source_claim.check().map_err(Error::Resource)?;
        target_claim.check().map_err(Error::Resource)?;
        Ok(Self {
            report,
            policy,
            source,
            target,
            source_store,
            target_store,
            source_claim,
            target_claim,
        })
    }

    pub fn historical_report(&self) -> &TemporalPreservationReport {
        &self.report
    }
    /// Source inspection only; original policy/report and actual models stay borrowed.
    pub(crate) fn inspect_product_current<R>(
        &self,
        view: &ResourceReservationView<'_>,
        consume: impl for<'p> FnOnce(
            TemporalModelInput<'p>,
            TemporalModelInput<'p>,
            &'p TemporalPreservationPolicy,
            &'p TemporalPreservationReport,
        ) -> R,
    ) -> Result<R, Error> {
        self.check_current(view)?;
        let result = consume(self.source, self.target, self.policy, &self.report);
        self.check_current(view)?;
        Ok(result)
    }

    pub fn axis_current(
        &self,
        resources: &ResourceReservationView<'_>,
    ) -> Result<AxisValue<'_>, Error> {
        self.check_current(resources)?;
        Ok(match self.report.state() {
            TemporalPreservationState::GivenFiniteModel { loss, .. } => numeric(loss),
            TemporalPreservationState::NoScalar { reason, .. } => unavailable(*reason),
            TemporalPreservationState::Incomparable(_) => {
                AxisValue::Unavailable(MissingAxis::Incomparable)
            }
        })
    }

    fn check_current(&self, resources: &ResourceReservationView<'_>) -> Result<(), Error> {
        check_claims(&self.source_claim, &self.target_claim, resources)?;
        if self.report.policy_digest() != self.policy.digest()
            || self.report.binding_digest() != self.policy.binding_digest()
            || self.report.denominator() != self.policy.denominator()
            || self.source.scope != self.target.scope
            || self.policy.scope() != self.source.scope
            || self.target.query.context().source().digest() != resources.source_revision()
        {
            return Err(Error::BindingMismatch);
        }
        // These stores have no escaping mutable reference. Fresh construction
        // and ownership preserve actual payloads; historical lookup alone is
        // deliberately not used as a current-lease certificate.
        if self.source_store.len() != self.target_store.len()
            || (self.source_store.is_empty() && self.report.source_observation().is_some())
            || (self.target_store.is_empty() && self.report.target_observation().is_some())
        {
            return Err(Error::BindingMismatch);
        }
        Ok(())
    }
    fn context(&self) -> ContextRevision {
        self.target.query.context()
    }
    fn scope(&self) -> ContextScope {
        self.target.scope
    }
    fn binding(&self) -> Digest32 {
        self.report.binding_digest()
    }
    // Shared policy deliberately excludes candidate model pins. The actual
    // immutable root query/trace must independently match across candidates;
    // a caller's equal SelectionPins::baseline cannot establish that fact.
    fn source_baseline(&self) -> Digest32 {
        use sha2::{Digest, Sha256};
        let mut hash = Sha256::new();
        hash.update(b"epistesys.current-temporal.actual-source-baseline.v1\0");
        for value in [
            self.source.query.context().source().digest(),
            self.source.query.context().interpretation_digest(),
            self.source.query.digest(),
            self.source.trace.digest(),
            self.policy.digest(),
        ] {
            hash.update(value.as_bytes());
        }
        Digest32::from_bytes(hash.finalize().into())
    }
}

/// Owns the real private observation stores and immutable rectangle inputs.
/// It cannot be created by borrowing or cloning a historical spatial report.
///
/// ```compile_fail
/// use lc631_runtime::spatiotemporal_selection::SpatialOwner;
/// use lc631_runtime::spatial_preservation::SpatialPreservationReport;
/// fn inject(report: SpatialPreservationReport) { SpatialOwner::from_report(report); }
/// ```
pub struct SpatialOwner<'policy, 'view> {
    report: SpatialPreservationReport,
    policy: &'policy SpatialPreservationPolicy,
    source: SpatialModelInput,
    target: SpatialModelInput,
    source_store: CausalSpatialObservations<'view>,
    target_store: CausalSpatialObservations<'view>,
    source_claim: ResourceClaim<'view>,
    target_claim: ResourceClaim<'view>,
}

impl<'policy, 'view> SpatialOwner<'policy, 'view> {
    pub fn produce(
        policy: &'policy SpatialPreservationPolicy,
        source: SpatialModelInput,
        target: SpatialModelInput,
        source_resources: &'view ResourceReservationView<'_>,
        target_resources: &'view ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        if source.context.source().digest() != source_resources.source_revision()
            || target.context.source().digest() != target_resources.source_revision()
        {
            return Err(Error::BindingMismatch);
        }
        let source_claim = claim(source_resources, 32 * 1024)?;
        let target_claim = claim(target_resources, 32 * 1024)?;
        let mut source_store = CausalSpatialObservations::with_resources(4, source_resources)
            .map_err(Error::SpatialStore)?;
        let mut target_store = CausalSpatialObservations::with_resources(4, target_resources)
            .map_err(Error::SpatialStore)?;
        let report = evaluate_spatial_preservation(
            policy,
            source,
            target,
            &mut source_store,
            &mut target_store,
            source_resources,
            target_resources,
        )
        .map_err(Error::Spatial)?;
        source_claim.check().map_err(Error::Resource)?;
        target_claim.check().map_err(Error::Resource)?;
        Ok(Self {
            report,
            policy,
            source,
            target,
            source_store,
            target_store,
            source_claim,
            target_claim,
        })
    }

    pub fn historical_report(&self) -> &SpatialPreservationReport {
        &self.report
    }
    /// Source inspection only; no decoder source lookup or detached report ingress.
    pub(crate) fn inspect_product_current<R>(
        &self,
        view: &ResourceReservationView<'_>,
        consume: impl for<'p> FnOnce(
            SpatialModelInput,
            SpatialModelInput,
            &'p SpatialPreservationPolicy,
            &'p SpatialPreservationReport,
        ) -> R,
    ) -> Result<R, Error> {
        self.check_current(view)?;
        let result = consume(self.source, self.target, self.policy, &self.report);
        self.check_current(view)?;
        Ok(result)
    }

    pub fn axis_current(
        &self,
        resources: &ResourceReservationView<'_>,
    ) -> Result<AxisValue<'_>, Error> {
        self.check_current(resources)?;
        Ok(match self.report.state() {
            SpatialPreservationState::GivenFiniteModel { loss, .. } => numeric(loss),
            SpatialPreservationState::NoScalar { reason, .. } => unavailable(*reason),
            SpatialPreservationState::Incomparable(_) => {
                AxisValue::Unavailable(MissingAxis::Incomparable)
            }
        })
    }

    fn check_current(&self, resources: &ResourceReservationView<'_>) -> Result<(), Error> {
        check_claims(&self.source_claim, &self.target_claim, resources)?;
        if self.report.policy_digest() != self.policy.digest()
            || self.report.binding_digest() != self.policy.binding_digest()
            || self.report.denominator() != self.policy.denominator()
            || self.source.scope != self.target.scope
            || self.target.context.source().digest() != resources.source_revision()
            || self.source_store.len() != self.report.source_observations().len()
            || self.target_store.len() != self.report.target_observations().len()
        {
            return Err(Error::BindingMismatch);
        }
        Ok(())
    }
    fn context(&self) -> ContextRevision {
        self.target.context
    }
    fn scope(&self) -> ContextScope {
        self.target.scope
    }
    fn binding(&self) -> Digest32 {
        self.report.binding_digest()
    }
    fn source_baseline(&self) -> Digest32 {
        use sha2::{Digest, Sha256};
        let mut hash = Sha256::new();
        hash.update(b"epistesys.current-spatial.actual-source-baseline.v1\0");
        for value in [
            self.source.context.source().digest(),
            self.source.context.interpretation_digest(),
            self.policy.digest(),
        ] {
            hash.update(value.as_bytes());
        }
        hash.update(self.source.frame.0);
        hash.update([match self.source.unit {
            crate::spatial_preservation::SpatialUnit::Pixel => 0,
            crate::spatial_preservation::SpatialUnit::Millimeter => 1,
        }]);
        for rectangle in [self.source.left, self.source.right] {
            match rectangle {
                None => hash.update([0]),
                Some(rectangle) => {
                    hash.update([1]);
                    hash.update(rectangle.frame().0);
                    for coordinate in rectangle.min().into_iter().chain(rectangle.max()) {
                        hash.update(coordinate.to_le_bytes());
                    }
                }
            }
        }
        Digest32::from_bytes(hash.finalize().into())
    }
}

fn claim<'v>(
    resources: &'v ResourceReservationView<'_>,
    bytes: u64,
) -> Result<ResourceClaim<'v>, Error> {
    resources.check().map_err(Error::Resource)?;
    resources
        .claim_resources(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(bytes).map_err(Error::Resource)?,
        )])
        .map_err(Error::Resource)
}
fn check_claims(
    source: &ResourceClaim<'_>,
    target: &ResourceClaim<'_>,
    resources: &ResourceReservationView<'_>,
) -> Result<(), Error> {
    resources.check().map_err(Error::Resource)?;
    source.check().map_err(Error::Resource)?;
    target.check().map_err(Error::Resource)?;
    if !target.same_reservation(resources) {
        return Err(Error::ForeignReservation);
    }
    Ok(())
}
fn numeric(value: &RationalInterval) -> AxisValue<'_> {
    if value.is_exact() {
        AxisValue::Exact(value)
    } else {
        AxisValue::Bounded(value)
    }
}
fn unavailable(gap: Gap) -> AxisValue<'static> {
    AxisValue::Unavailable(match gap {
        Gap::MissingCalibration => MissingAxis::Uncalibrated,
        Gap::UnsupportedQuantity => MissingAxis::Unsupported,
        Gap::MissingEvidence | Gap::MissingObservation => MissingAxis::Unknown,
    })
}

fn same_axis(actual: &AxisValue<'_>, expected: &AxisValue<'_>) -> bool {
    match (actual, expected) {
        (AxisValue::Exact(a), AxisValue::Exact(b))
        | (AxisValue::Bounded(a), AxisValue::Bounded(b)) => std::ptr::eq(*a, *b),
        (AxisValue::Unavailable(a), AxisValue::Unavailable(b)) => a == b,
        _ => false,
    }
}
