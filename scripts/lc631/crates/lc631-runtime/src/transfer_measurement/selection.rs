use super::*;
use crate::joint_selection::*;
#[derive(Clone, Copy)]
pub struct TransferRow<'a, 'r, 'i, 'v> {
    pub owner: &'a OwnedTransferGain<'a, 'r, 'i, 'v>,
    pub candidate: &'a TransferCandidate<'r, 'i, 'v>,
    pub expected: Expected<'i>,
    pub row: usize,
    pub axis: usize,
}
pub struct OwnedTransferSelection<'a, 'r, 'i, 'v> {
    report: JointSelectionReport<'a, 'a>,
    policy: &'a SelectionPolicy<'a>,
    owners: &'a [TransferRow<'a, 'r, 'i, 'v>],
    retained: ResourceClaim<'v>,
}
pub fn select_transfer_current<'a, 'r, 'i, 'v>(
    policy: &'a SelectionPolicy<'a>,
    rows: &'a [JointCandidate<'a>],
    owners: &'a [TransferRow<'a, 'r, 'i, 'v>],
    coverage: SearchCoverage,
    v: &'v ResourceReservationView<'_>,
) -> Result<OwnedTransferSelection<'a, 'r, 'i, 'v>, Error> {
    v.check().map_err(Error::Resource)?;
    let retained = k::claim(v, 256 * 1024, true).map_err(mapped)?;
    validate(policy, rows, owners, v)?;
    let report = select_joint_reported(policy, rows, coverage).map_err(Error::Selection)?;
    validate(policy, report.candidates, owners, v)?;
    k::check_claim(&retained, v).map_err(mapped)?;
    Ok(OwnedTransferSelection {
        report,
        policy,
        owners,
        retained,
    })
}
impl OwnedTransferSelection<'_, '_, '_, '_> {
    pub fn report_current(
        &self,
        v: &ResourceReservationView<'_>,
    ) -> Result<&JointSelectionReport<'_, '_>, Error> {
        k::check_claim(&self.retained, v).map_err(mapped)?;
        validate(self.policy, self.report.candidates, self.owners, v)?;
        k::check_claim(&self.retained, v).map_err(mapped)?;
        Ok(&self.report)
    }
}
fn validate(
    policy: &SelectionPolicy<'_>,
    rows: &[JointCandidate<'_>],
    owners: &[TransferRow<'_, '_, '_, '_>],
    v: &ResourceReservationView<'_>,
) -> Result<(), Error> {
    if rows.is_empty()
        || rows.len() > 64
        || policy.axes.len() > 32
        || owners.len() != rows.len()
        || policy
            .axes
            .iter()
            .filter(|a| **a == AxisKind::TransferGain)
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
            || policy.axes.get(o.axis) != Some(&AxisKind::TransferGain)
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
