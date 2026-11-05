//! 日本語: CR-PRESERVEのprivate生産元を実joint selectorに接続する。
//! English: Connect private CR-PRESERVE producers to the actual joint selector.
use super::*;
use crate::joint_selection::{
    select_joint_reported, AxisKind, AxisValue, JointCandidate, JointSelectionReport,
    SearchCoverage, SelectionPolicy,
};

pub struct PreservationRow<'a, 'input, 'view> {
    pub row: usize,
    pub owner: &'a OwnedPreservation<'input, 'view>,
}
/// Given goals/gates/map-back/total-loss remain caller declarations. No F2
/// score or missing creative components are manufactured by this consumer.
pub struct CurrentSelection<'a, 'input, 'view> {
    report: JointSelectionReport<'a, 'a>,
    policy: &'a SelectionPolicy<'a>,
    owners: &'a [PreservationRow<'a, 'input, 'view>],
    retained: ResourceClaim<'view>,
}
impl CurrentSelection<'_, '_, '_> {
    pub fn historical_report(&self) -> &JointSelectionReport<'_, '_> {
        &self.report
    }
    pub fn report_current(
        &self,
        resources: &ResourceReservationView<'_>,
    ) -> Result<&JointSelectionReport<'_, '_>, Error> {
        self.retained.check().map_err(Error::Resource)?;
        if !self.retained.same_reservation(resources) {
            return Err(Error::ForeignReservation);
        }
        validate(self.policy, self.report.candidates, self.owners, resources)?;
        self.retained.check().map_err(Error::Resource)?;
        Ok(&self.report)
    }
}
pub fn select_current<'a, 'input, 'view>(
    policy: &'a SelectionPolicy<'a>,
    rows: &'a [JointCandidate<'a>],
    owners: &'a [PreservationRow<'a, 'input, 'view>],
    coverage: SearchCoverage,
    resources: &'view ResourceReservationView<'_>,
) -> Result<CurrentSelection<'a, 'input, 'view>, Error> {
    resources.check().map_err(Error::Resource)?;
    if rows.is_empty() || rows.len() > 64 || policy.axes.len() > 32 {
        return Err(Error::InvalidPlan);
    }
    let retained = claim(resources, 256 * 1024)?;
    validate(policy, rows, owners, resources)?;
    let report = select_joint_reported(policy, rows, coverage).map_err(Error::Selection)?;
    validate(policy, report.candidates, owners, resources)?;
    retained.check().map_err(Error::Resource)?;
    Ok(CurrentSelection {
        report,
        policy,
        owners,
        retained,
    })
}
fn validate(
    policy: &SelectionPolicy<'_>,
    rows: &[JointCandidate<'_>],
    owners: &[PreservationRow<'_, '_, '_>],
    resources: &ResourceReservationView<'_>,
) -> Result<(), Error> {
    resources.check().map_err(Error::Resource)?;
    let mut axes = policy
        .axes
        .iter()
        .enumerate()
        .filter(|(_, axis)| **axis == AxisKind::Preserve);
    let index = axes.next().ok_or(Error::InvalidPlan)?.0;
    if axes.next().is_some() || rows.len() != owners.len() {
        return Err(Error::InvalidPlan);
    }
    let plan = owners.first().ok_or(Error::InvalidPlan)?.owner.plan;
    for (i, binding) in owners.iter().enumerate() {
        let owner = binding.owner;
        let row = rows.get(binding.row).ok_or(Error::InvalidPlan)?;
        if owners[..i].iter().any(|p| p.row == binding.row)
            || owner.plan.digest != plan.digest
            || !std::ptr::eq(owner.plan.source, plan.source)
            || row.id != owner.binding.id
            || row.context != owner.binding.context
            || row.pins != owner.binding.pins
            || row.pins != policy.pins
            || row.measurement_binding != owner.measurement_binding()
        {
            return Err(Error::BindingMismatch);
        }
        let actual = owner.axis_current(resources)?;
        if !same_axis(
            row.axes.get(index).ok_or(Error::InvalidPlan)?,
            &actual.value,
        ) {
            return Err(Error::BindingMismatch);
        }
    }
    Ok(())
}
fn same_axis(a: &AxisValue<'_>, b: &AxisValue<'_>) -> bool {
    match (a, b) {
        (AxisValue::Exact(a), AxisValue::Exact(b))
        | (AxisValue::Bounded(a), AxisValue::Bounded(b)) => std::ptr::eq(*a, *b),
        (AxisValue::Unavailable(a), AxisValue::Unavailable(b)) => a == b,
        _ => false,
    }
}
