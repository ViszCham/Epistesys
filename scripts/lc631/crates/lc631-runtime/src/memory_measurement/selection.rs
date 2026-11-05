//! Current private memory owners wrap actual reported joint selection and
//! recheck its consumption. Other goals/gates/maps remain caller-reported.
use super::*;
use crate::joint_selection::{select_joint_reported, JointSelectionReport, SearchCoverage};
pub const SELECTION_RAM: u64 = 256 * 1024;
pub const SELECTION_OUTPUT: u64 = 64 * 1024;
pub struct MemoryRow<'a, 'v> {
    pub owner: &'a OwnedBridgeCost<'a, 'v>,
    pub plan: &'a RegistryPlan<'a>,
    pub row: usize,
    pub axis: usize,
}
/// No public constructor/fields/Deref/Deserialize can replace the actual result.
pub struct OwnedMemorySelection<'a, 'v> {
    report: JointSelectionReport<'a, 'a>,
    policy: &'a SelectionPolicy<'a>,
    bindings: &'a [MemoryRow<'a, 'v>],
    retained: ResourceClaim<'v>,
}
impl OwnedMemorySelection<'_, '_> {
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
        validate(
            self.policy,
            self.report.candidates,
            self.bindings,
            resources,
        )?;
        self.retained.check().map_err(Error::Resource)?;
        Ok(&self.report)
    }
}
/// Every migration-cost row needs exactly one private current owner. This
/// validates before AND after the actual selector, then at result consumption.
/// Cancellation may race after checks; this is not an atomic commit fence.
pub fn select_current<'a, 'v>(
    policy: &'a SelectionPolicy<'a>,
    rows: &'a [JointCandidate<'a>],
    bindings: &'a [MemoryRow<'a, 'v>],
    coverage: SearchCoverage,
    resources: &'v ResourceReservationView<'_>,
) -> Result<OwnedMemorySelection<'a, 'v>, Error> {
    resources.check().map_err(Error::Resource)?;
    if rows.is_empty() || rows.len() > 64 || policy.axes.len() > 32 {
        return Err(Error::InvalidRows);
    }
    let retained = claim(resources, SELECTION_RAM, SELECTION_OUTPUT)?;
    validate(policy, rows, bindings, resources)?;
    let report = select_joint_reported(policy, rows, coverage).map_err(Error::Selection)?;
    validate(policy, report.candidates, bindings, resources)?;
    retained.check().map_err(Error::Resource)?;
    Ok(OwnedMemorySelection {
        report,
        policy,
        bindings,
        retained,
    })
}
fn validate(
    policy: &SelectionPolicy<'_>,
    rows: &[JointCandidate<'_>],
    bindings: &[MemoryRow<'_, '_>],
    resources: &ResourceReservationView<'_>,
) -> Result<(), Error> {
    resources.check().map_err(Error::Resource)?;
    if bindings.len() != rows.len()
        || policy
            .axes
            .iter()
            .filter(|axis| **axis == AxisKind::MigrationCost)
            .count()
            != 1
    {
        return Err(Error::InvalidRows);
    }
    let mut common_policy = None;
    for (i, binding) in bindings.iter().enumerate() {
        let row = rows.get(binding.row).ok_or(Error::InvalidRows)?;
        if bindings[..i].iter().any(|prior| prior.row == binding.row) {
            return Err(Error::InvalidRows);
        }
        binding.owner.check_selection_row_current(
            binding.plan,
            policy,
            row,
            binding.axis,
            resources,
        )?;
        // Compare only identities minted from validated inputs of CURRENT owners.
        match common_policy {
            Some(expected) if expected != binding.owner.common_policy => {
                return Err(Error::BindingMismatch)
            }
            None => common_policy = Some(binding.owner.common_policy),
            _ => {}
        }
    }
    Ok(())
}
