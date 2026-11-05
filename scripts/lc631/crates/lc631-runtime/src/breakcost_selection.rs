//! 日本語: 実BreakCost ownerをcurrent検査して既存reported selectorへ接続。
//! English: Current actual BreakCost ownership wrapping the existing reported selector.
//! Other goal/TL/gates remain Given/reported, not authenticated by compatibility.
#![forbid(unsafe_code)]
use crate::joint_selection::*;
use lc631_analysis::{
    breakcost_measurement::{self as measurement, ExpectedInputs, LocalHardStatus, OwnedBreakCost},
    creativity_measurement::ComponentState,
};
use lc631_substrate::resource::{
    ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
    ResourceReservationView,
};

pub struct BreakCostRow<'a, 'v> {
    pub owner: &'a OwnedBreakCost<'a, 'v>,
    pub expected: ExpectedInputs<'a, 'v>,
    pub row: usize,
    pub axis: usize,
}
#[derive(Debug)]
pub enum Error {
    Measurement(measurement::Error),
    Resource(ResourceError),
    ForeignReservation,
    BindingMismatch,
    InvalidRows,
    HardGateMismatch,
    IncompleteObservation,
    Selection(SelectionError),
}
/// Owned result retains its current producer occurrences, policy and borrowed rows.
/// ```compile_fail
/// use lc631_runtime::breakcost_selection::OwnedBreakCostSelection;
/// fn inject() {OwnedBreakCostSelection::from_report(());}
/// ```
/// ```compile_fail
/// use lc631_runtime::breakcost_selection::OwnedBreakCostSelection;
/// let _: OwnedBreakCostSelection<'_, '_> = serde_json::from_str("{}").unwrap();
/// ```
pub struct OwnedBreakCostSelection<'a, 'v> {
    report: JointSelectionReport<'a, 'a>,
    policy: &'a SelectionPolicy<'a>,
    bindings: &'a [BreakCostRow<'a, 'v>],
    retained: ResourceClaim<'v>,
}
impl OwnedBreakCostSelection<'_, '_> {
    pub fn historical_report(&self) -> &JointSelectionReport<'_, '_> {
        &self.report
    }
    pub fn report_current(
        &self,
        view: &ResourceReservationView<'_>,
    ) -> Result<&JointSelectionReport<'_, '_>, Error> {
        current(&self.retained, view)?;
        validate(self.policy, self.report.candidates, self.bindings, view)?;
        current(&self.retained, view)?;
        Ok(&self.report)
    }
}
fn pins(expected: ExpectedInputs<'_, '_>, actual: SelectionPins) -> Result<(), Error> {
    let b = expected.root.binding();
    let required = SelectionPins {
        source: b.source,
        scope: b.scope,
        baseline: b.baseline,
        hard_invariants: b.hard_invariants,
        policy: b.policy,
        goal_profile: b.goal_profile,
        evidence_revision: b.evidence_revision,
    };
    if required != actual {
        return Err(Error::BindingMismatch);
    }
    Ok(())
}
pub fn project_current<'a, 'v>(
    owner: &'a OwnedBreakCost<'a, 'v>,
    expected: ExpectedInputs<'a, 'v>,
    p: SelectionPins,
    view: &ResourceReservationView<'_>,
) -> Result<ReportedComponentAxis<'a>, Error> {
    let report = owner
        .report_current(expected, view)
        .map_err(Error::Measurement)?;
    pins(expected, p)?;
    let axis = project_component_reported(report).map_err(Error::Selection)?;
    owner
        .report_current(expected, view)
        .map_err(Error::Measurement)?;
    Ok(axis)
}
pub fn select_current<'a, 'v>(
    policy: &'a SelectionPolicy<'a>,
    rows: &'a [JointCandidate<'a>],
    bindings: &'a [BreakCostRow<'a, 'v>],
    coverage: SearchCoverage,
    view: &'v ResourceReservationView<'_>,
) -> Result<OwnedBreakCostSelection<'a, 'v>, Error> {
    let retained = claim(view)?;
    validate(policy, rows, bindings, view)?;
    let report = select_joint_reported(policy, rows, coverage).map_err(Error::Selection)?;
    validate(policy, rows, bindings, view)?;
    current(&retained, view)?;
    Ok(OwnedBreakCostSelection {
        report,
        policy,
        bindings,
        retained,
    })
}
fn validate(
    policy: &SelectionPolicy<'_>,
    rows: &[JointCandidate<'_>],
    bindings: &[BreakCostRow<'_, '_>],
    view: &ResourceReservationView<'_>,
) -> Result<(), Error> {
    view.check().map_err(Error::Resource)?;
    let axes = policy
        .axes
        .iter()
        .filter(|a| **a == AxisKind::BreakCost)
        .count();
    if rows.is_empty()
        || rows.len() > 8
        || axes == 0
        || bindings.len() != rows.len().checked_mul(axes).ok_or(Error::InvalidRows)?
    {
        return Err(Error::InvalidRows);
    }
    let first = bindings.first().ok_or(Error::InvalidRows)?;
    for (index, b) in bindings.iter().enumerate() {
        if bindings[..index]
            .iter()
            .any(|p| p.row == b.row && p.axis == b.axis)
            || policy.axes.get(b.axis) != Some(&AxisKind::BreakCost)
        {
            return Err(Error::InvalidRows);
        }
        if !std::ptr::eq(first.expected.root, b.expected.root) {
            return Err(Error::BindingMismatch);
        }
        let row = rows.get(b.row).ok_or(Error::InvalidRows)?;
        check_row(policy, row, b, view)?;
    }
    view.check().map_err(Error::Resource)
}
fn check_row(
    policy: &SelectionPolicy<'_>,
    row: &JointCandidate<'_>,
    b: &BreakCostRow<'_, '_>,
    view: &ResourceReservationView<'_>,
) -> Result<(), Error> {
    let projected = project_current(b.owner, b.expected, policy.pins, view)?;
    pins(b.expected, row.pins)?;
    let candidate = b.expected.candidate.binding();
    if row.id != candidate.id
        || row.context != candidate.context
        || row.measurement_binding != b.owner.measurement_binding()
    {
        return Err(Error::BindingMismatch);
    }
    let actual = row.axes.get(b.axis).ok_or(Error::InvalidRows)?;
    if !same_axis(actual, &projected.value) {
        return Err(Error::BindingMismatch);
    }
    match b.owner.historical_hard_status() {
        LocalHardStatus::Failed if row.hard_gate != ReportedGate::Fail => {
            return Err(Error::HardGateMismatch)
        }
        LocalHardStatus::Pending if row.hard_gate == ReportedGate::Pass => {
            return Err(Error::HardGateMismatch)
        }
        LocalHardStatus::NotRejected | LocalHardStatus::Failed | LocalHardStatus::Pending => {}
    }
    if matches!(
        projected.report.state,
        ComponentState::ResourceExhausted { .. }
    ) {
        return Err(Error::IncompleteObservation);
    }
    b.owner
        .report_current(b.expected, view)
        .map_err(Error::Measurement)?;
    Ok(())
}
fn same_axis(a: &AxisValue<'_>, b: &AxisValue<'_>) -> bool {
    match (a, b) {
        (AxisValue::Exact(a), AxisValue::Exact(b))
        | (AxisValue::Bounded(a), AxisValue::Bounded(b))
        | (AxisValue::Estimated(a), AxisValue::Estimated(b)) => std::ptr::eq(*a, *b),
        (AxisValue::Unavailable(a), AxisValue::Unavailable(b)) => a == b,
        _ => false,
    }
}
fn claim<'v>(view: &'v ResourceReservationView<'_>) -> Result<ResourceClaim<'v>, Error> {
    let request = |kind, n| {
        ResourceAmount::checked(n)
            .map(|a| ResourceRequest::new(kind, a))
            .map_err(Error::Resource)
    };
    view.claim_resources(&[
        request(ResourceKind::Ram, 262144)?,
        request(ResourceKind::Output, 65536)?,
    ])
    .map_err(Error::Resource)
}
fn current(claim: &ResourceClaim<'_>, view: &ResourceReservationView<'_>) -> Result<(), Error> {
    claim.check().map_err(Error::Resource)?;
    if !claim.same_reservation(view) {
        return Err(Error::ForeignReservation);
    }
    view.check().map_err(Error::Resource)
}
