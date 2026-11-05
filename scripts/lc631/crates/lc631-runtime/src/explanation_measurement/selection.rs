use super::*;
use crate::joint_selection::*;
#[derive(Clone, Copy)]
pub struct ExplainRow<'a, 'r, 'i, 'c, 'v> {
    pub owner: &'a OwnedExplainGain<'a, 'r, 'i, 'c, 'v>,
    pub candidate: &'a ExplainCandidate<'r, 'i, 'c, 'v>,
    pub expected: Expected<'i, 'c>,
    pub row: usize,
    pub axis: usize,
}
pub struct OwnedExplainSelection<'a, 'r, 'i, 'c, 'v> {
    report: JointSelectionReport<'a, 'a>,
    policy: &'a SelectionPolicy<'a>,
    owners: &'a [ExplainRow<'a, 'r, 'i, 'c, 'v>],
    retained: ResourceClaim<'v>,
}
pub fn select_explain_current<'a, 'r, 'i, 'c, 'v>(
    policy: &'a SelectionPolicy<'a>,
    rows: &'a [JointCandidate<'a>],
    owners: &'a [ExplainRow<'a, 'r, 'i, 'c, 'v>],
    coverage: SearchCoverage,
    v: &'v ResourceReservationView<'_>,
) -> Result<OwnedExplainSelection<'a, 'r, 'i, 'c, 'v>, Error> {
    v.check().map_err(Error::Resource)?;
    let retained = kernel::claim(v, 256 * 1024, true)?;
    validate(policy, rows, owners, v)?;
    let report = select_joint_reported(policy, rows, coverage).map_err(Error::Selection)?;
    validate(policy, report.candidates, owners, v)?;
    kernel::check_claim(&retained, v)?;
    Ok(OwnedExplainSelection {
        report,
        policy,
        owners,
        retained,
    })
}
impl OwnedExplainSelection<'_, '_, '_, '_, '_> {
    pub fn report_current(
        &self,
        v: &ResourceReservationView<'_>,
    ) -> Result<&JointSelectionReport<'_, '_>, Error> {
        kernel::check_claim(&self.retained, v)?;
        validate(self.policy, self.report.candidates, self.owners, v)?;
        kernel::check_claim(&self.retained, v)?;
        Ok(&self.report)
    }
}
fn validate(
    policy: &SelectionPolicy<'_>,
    rows: &[JointCandidate<'_>],
    owners: &[ExplainRow<'_, '_, '_, '_, '_>],
    v: &ResourceReservationView<'_>,
) -> Result<(), Error> {
    if rows.is_empty()
        || rows.len() > 64
        || policy.axes.len() > 32
        || owners.len() != rows.len()
        || policy
            .axes
            .iter()
            .filter(|a| **a == AxisKind::ExplainGain)
            .count()
            != 1
    {
        return Err(Error::InvalidRows);
    }
    let fixed_root = owners.first().ok_or(Error::InvalidRows)?.candidate.root;
    for (i, o) in owners.iter().enumerate() {
        if owners[..i].iter().any(|p| p.row == o.row) {
            return Err(Error::InvalidRows);
        }
        let row = rows.get(o.row).ok_or(Error::InvalidRows)?;
        let axis = o
            .owner
            .axis_current(o.candidate, o.expected, policy.pins, v)?;
        if !std::ptr::eq(o.candidate.root, fixed_root) {
            return Err(Error::BindingMismatch);
        }
        let c = o.candidate.input.binding;
        if row.id != c.id
            || row.context != c.context
            || row.pins != policy.pins
            || row.measurement_binding != o.owner.measurement_binding()
            || row.axes.len() != policy.axes.len()
            || policy.axes.get(o.axis) != Some(&AxisKind::ExplainGain)
        {
            return Err(Error::BindingMismatch);
        }
        let same = match (row.axes.get(o.axis), axis.value) {
            (Some(AxisValue::Exact(a)), AxisValue::Exact(b))
            | (Some(AxisValue::Bounded(a)), AxisValue::Bounded(b)) => std::ptr::eq(*a, b),
            _ => false,
        };
        if !same {
            return Err(Error::BindingMismatch);
        }
    }
    Ok(())
}
