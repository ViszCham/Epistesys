//! 日本語: 実loss軸の所有を各行・各利用時に再確認する。
//! English: Recheck real loss-axis ownership per row and at each consumption.
use super::*;
use crate::joint_selection::{
    select_joint_reported, AxisKind, JointCandidate, JointSelectionReport, SearchCoverage,
    SelectionPolicy,
};
use sha2::{Digest, Sha256};

pub struct RowOwners<'a, 'input, 'view> {
    pub row: usize,
    pub temporal: Option<&'a TemporalOwner<'input, 'view>>,
    pub spatial: Option<&'a SpatialOwner<'input, 'view>>,
}
impl RowOwners<'_, '_, '_> {
    /// Candidate-specific actual transform bindings, not a shared policy hash
    /// or an authenticated evidence signature. No original score is generated.
    pub fn binding_digest(&self) -> Digest32 {
        let mut hash = Sha256::new();
        hash.update(b"epistesys.current-spatiotemporal-row.v1\0");
        for binding in [
            self.temporal.map(TemporalOwner::binding),
            self.spatial.map(SpatialOwner::binding),
        ] {
            match binding {
                None => hash.update([0]),
                Some(value) => {
                    hash.update([1]);
                    hash.update(value.as_bytes());
                }
            }
        }
        Digest32::from_bytes(hash.finalize().into())
    }
}

/// The result borrows exact live producer owners, rows, policy and leases.
/// Given reported goal/gates/map-back are not independently verified here.
/// Cancellation may race after checks; this is not an atomic commit fence.
pub struct CurrentSelection<'a, 'input, 'view> {
    report: JointSelectionReport<'a, 'a>,
    policy: &'a SelectionPolicy<'a>,
    owners: &'a [RowOwners<'a, 'input, 'view>],
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
    owners: &'a [RowOwners<'a, 'input, 'view>],
    coverage: SearchCoverage,
    resources: &'view ResourceReservationView<'_>,
) -> Result<CurrentSelection<'a, 'input, 'view>, Error> {
    resources.check().map_err(Error::Resource)?;
    if rows.is_empty() || rows.len() > 64 || policy.axes.len() > 32 {
        return Err(Error::InvalidRows);
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
    owners: &[RowOwners<'_, '_, '_>],
    resources: &ResourceReservationView<'_>,
) -> Result<(), Error> {
    resources.check().map_err(Error::Resource)?;
    let temporal = position(policy.axes, AxisKind::TemporalLoss)?;
    let spatial = position(policy.axes, AxisKind::SpatialLoss)?;
    if (temporal.is_none() && spatial.is_none())
        || owners.len() != rows.len()
        || policy.pins.source.digest() != resources.source_revision()
    {
        return Err(Error::InvalidRows);
    }
    let first = owners.first().ok_or(Error::InvalidRows)?;
    let temporal_policy = first.temporal.map(|owner| owner.policy.digest());
    let spatial_policy = first.spatial.map(|owner| owner.policy.digest());
    let temporal_source = first.temporal.map(TemporalOwner::source_baseline);
    let spatial_source = first.spatial.map(SpatialOwner::source_baseline);
    for (i, owner) in owners.iter().enumerate() {
        let row = rows.get(owner.row).ok_or(Error::InvalidRows)?;
        if owners[..i].iter().any(|prior| prior.row == owner.row)
            || row.measurement_binding != owner.binding_digest()
            || row.pins != policy.pins
            || owner.temporal.map(|value| value.policy.digest()) != temporal_policy
            || owner.spatial.map(|value| value.policy.digest()) != spatial_policy
            || owner.temporal.map(TemporalOwner::source_baseline) != temporal_source
            || owner.spatial.map(SpatialOwner::source_baseline) != spatial_source
        {
            return Err(Error::BindingMismatch);
        }
        match (temporal, owner.temporal) {
            (Some(index), Some(actual)) => {
                let expected = actual.axis_current(resources)?;
                check_row(row, actual.context(), actual.scope(), index, &expected)?;
            }
            (None, None) => {}
            _ => return Err(Error::InvalidRows),
        }
        match (spatial, owner.spatial) {
            (Some(index), Some(actual)) => {
                let expected = actual.axis_current(resources)?;
                check_row(row, actual.context(), actual.scope(), index, &expected)?;
            }
            (None, None) => {}
            _ => return Err(Error::InvalidRows),
        }
    }
    Ok(())
}
fn position(axes: &[AxisKind], kind: AxisKind) -> Result<Option<usize>, Error> {
    let mut matches = axes.iter().enumerate().filter(|(_, axis)| **axis == kind);
    let first = matches.next().map(|(index, _)| index);
    if matches.next().is_some() {
        return Err(Error::InvalidRows);
    }
    Ok(first)
}
fn check_row(
    row: &JointCandidate<'_>,
    context: ContextRevision,
    scope: ContextScope,
    index: usize,
    expected: &AxisValue<'_>,
) -> Result<(), Error> {
    if row.context != context
        || row.pins.scope != scope
        || !same_axis(row.axes.get(index).ok_or(Error::InvalidRows)?, expected)
    {
        return Err(Error::BindingMismatch);
    }
    Ok(())
}
