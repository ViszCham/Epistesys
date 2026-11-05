//! 日本語: 実1:3 TL縮約ownerを保持する追加current consumer。goal/gatesはGiven。
//! English: Additive current consumer retaining real 1:3 reduction owners.
//! Existing current-axis selection keeps its reported-total contract unchanged.
//! No source-interpretation verification, authority or atomic commit fence.
#![forbid(unsafe_code)]
use super::{OwnedReduction, Report};
use crate::checked_tl_reduction::{Error as ReductionError, NoScalar};
use crate::joint_selection::{
    AxisKind, AxisValue, JointCandidate, JointSelectionReport, MissingAxis, SearchCoverage,
    SelectionPolicy,
};
use crate::spatiotemporal_selection::{
    self as current_axes, CurrentSelection, RowOwners, SpatialOwner, TemporalOwner,
};
use lc631_substrate::resource::{ResourceError, ResourceReservationView};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReducedSelectionError {
    Resource(ResourceError),
    Axes(current_axes::Error),
    Reduction(ReductionError),
    InvalidRows,
    BindingMismatch,
}

/// Exact actual reduction owner for one row; no numeric/status constructor.
/// The common input lifetime is the intersection of model and policy borrows.
pub struct ReductionRow<'a, 'owner, 'root, 'input, 'view> {
    pub row: usize,
    pub owner: &'a OwnedReduction<'owner, 'root, 'input, 'input, 'view>,
}

/// Both the existing axis consumer and the actual reductions stay borrowed.
/// Historical reports never renew current eligibility or authorize an action.
/// ```compile_fail
/// use lc631_runtime::checked_tl_reduction::CurrentReducedSelection;
/// fn forge() { CurrentReducedSelection::from_report("measured"); }
/// ```
pub struct CurrentReducedSelection<'a, 'owner, 'root, 'input, 'view> {
    base: CurrentSelection<'a, 'input, 'view>,
    policy: &'a SelectionPolicy<'a>,
    owners: &'a [RowOwners<'a, 'input, 'view>],
    reductions: &'a [ReductionRow<'a, 'owner, 'root, 'input, 'view>],
}
impl CurrentReducedSelection<'_, '_, '_, '_, '_> {
    pub fn historical_report(&self) -> &JointSelectionReport<'_, '_> {
        self.base.historical_report()
    }
    pub fn report_current(
        &self,
        resources: &ResourceReservationView<'_>,
    ) -> Result<&JointSelectionReport<'_, '_>, ReducedSelectionError> {
        let report = self.base.report_current(resources).map_err(ReducedSelectionError::Axes)?;
        validate(self.policy, report.candidates, self.owners, self.reductions, resources)?;
        // Recheck after reduction traversal. Cancellation can still race after
        // returning; this borrowed observation is not an atomic publication.
        self.base.report_current(resources).map_err(ReducedSelectionError::Axes)
    }
}

/// Fixed temporal/spatial tuple only. Goals, eligibility, map-back, canonical
/// display keys and evidence revision remain caller-reported as in the base API.
/// No extra loss/creativity axis is silently authenticated by this connector.
pub fn select_current_reduced<'a, 'owner, 'root, 'input, 'view>(
    policy: &'a SelectionPolicy<'a>,
    rows: &'a [JointCandidate<'a>],
    owners: &'a [RowOwners<'a, 'input, 'view>],
    reductions: &'a [ReductionRow<'a, 'owner, 'root, 'input, 'view>],
    coverage: SearchCoverage,
    resources: &'view ResourceReservationView<'_>,
) -> Result<CurrentReducedSelection<'a, 'owner, 'root, 'input, 'view>, ReducedSelectionError> {
    validate(policy, rows, owners, reductions, resources)?;
    let base = current_axes::select_current(policy, rows, owners, coverage, resources)
        .map_err(ReducedSelectionError::Axes)?;
    let result = CurrentReducedSelection { base, policy, owners, reductions };
    result.report_current(resources)?;
    Ok(result)
}

fn validate(
    policy: &SelectionPolicy<'_>,
    rows: &[JointCandidate<'_>],
    owners: &[RowOwners<'_, '_, '_>],
    reductions: &[ReductionRow<'_, '_, '_, '_, '_>],
    resources: &ResourceReservationView<'_>,
) -> Result<(), ReducedSelectionError> {
    resources.check().map_err(ReducedSelectionError::Resource)?;
    if rows.is_empty() || rows.len() > 64 || reductions.len() != rows.len()
        || owners.len() != rows.len() || policy.axes.len() != 3
        || policy.axes.iter().filter(|a| matches!(a, AxisKind::Goal(_))).count() != 1
        || policy.axes.iter().filter(|a| **a == AxisKind::TemporalLoss).count() != 1
        || policy.axes.iter().filter(|a| **a == AxisKind::SpatialLoss).count() != 1
    {
        return Err(ReducedSelectionError::InvalidRows);
    }
    let root = reductions.first().ok_or(ReducedSelectionError::InvalidRows)?.owner.root;
    for (index, reduction) in reductions.iter().enumerate() {
        if reductions[..index].iter().any(|prior| prior.row == reduction.row) {
            return Err(ReducedSelectionError::InvalidRows);
        }
        let row = rows.get(reduction.row).ok_or(ReducedSelectionError::InvalidRows)?;
        let axis_owners = owners.iter().find(|owner| owner.row == reduction.row)
            .ok_or(ReducedSelectionError::InvalidRows)?;
        check_reduction(root, row, axis_owners, reduction.owner, resources)?;
    }
    resources.check().map_err(ReducedSelectionError::Resource)
}

fn check_reduction(
    root: &crate::checked_tl_reduction::RootPlan<'_, '_, '_>,
    row: &JointCandidate<'_>,
    axis_owners: &RowOwners<'_, '_, '_>,
    reduction: &OwnedReduction<'_, '_, '_, '_, '_>,
    resources: &ResourceReservationView<'_>,
) -> Result<(), ReducedSelectionError> {
    if !std::ptr::eq(root, reduction.root)
        || row.pins.source != reduction.root.context().source()
        || row.pins.scope != reduction.root.scope()
        || row.pins.baseline != reduction.root.digest()
        || row.pins.policy != reduction.root.digest()
        || row.context != reduction.candidate.context()
        || !same_temporal(axis_owners.temporal, reduction.owners.temporal)
        || !same_spatial(axis_owners.spatial, reduction.owners.spatial)
    {
        return Err(ReducedSelectionError::BindingMismatch);
    }
    let report = reduction.current(reduction.root, reduction.candidate, resources)
        .map_err(ReducedSelectionError::Reduction)?;
    let expected = total_axis(report)?;
    if !same_axis(&row.total_loss, &expected) {
        return Err(ReducedSelectionError::BindingMismatch);
    }
    Ok(())
}
fn total_axis(report: &Report) -> Result<AxisValue<'_>, ReducedSelectionError> {
    if let Some(total) = report.total() {
        return Ok(if total.is_exact() { AxisValue::Exact(total) } else { AxisValue::Bounded(total) });
    }
    // Fixed first-gap order, retaining its declared typed cause. Never reweight
    // away a missing axis or manufacture a total from an observed subset.
    let reason = report.gaps().into_iter().flatten().next()
        .ok_or(ReducedSelectionError::BindingMismatch)?;
    Ok(AxisValue::Unavailable(match reason {
        NoScalar::MissingOwner => MissingAxis::MissingEvaluator,
        NoScalar::Unavailable(reason) => reason,
    }))
}
fn same_axis(actual: &AxisValue<'_>, expected: &AxisValue<'_>) -> bool {
    match (actual, expected) {
        (AxisValue::Exact(a), AxisValue::Exact(b))
        | (AxisValue::Bounded(a), AxisValue::Bounded(b)) => std::ptr::eq(*a, *b),
        (AxisValue::Unavailable(a), AxisValue::Unavailable(b)) => a == b,
        _ => false,
    }
}
fn same_temporal(a: Option<&TemporalOwner<'_, '_>>, b: Option<&TemporalOwner<'_, '_>>) -> bool {
    match (a, b) { (Some(a), Some(b)) => std::ptr::eq(a, b), (None, None) => true, _ => false }
}
fn same_spatial(a: Option<&SpatialOwner<'_, '_>>, b: Option<&SpatialOwner<'_, '_>>) -> bool {
    match (a, b) { (Some(a), Some(b)) => std::ptr::eq(a, b), (None, None) => true, _ => false }
}
