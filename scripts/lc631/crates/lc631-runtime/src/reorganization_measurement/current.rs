//! Private current views and exact producer/report/interval correspondence.
use super::{CheckRecord, Error, OwnedRebind, PreparedRebindProgram, RebindPlan, RebindQueries};
use crate::joint_selection::{
    project_component_reported, AxisKind, AxisValue, ReportedComponentAxis,
};
use lc631_analysis::creativity_measurement::ComponentReport;
use lc631_substrate::resource::ResourceReservationView;

pub struct CurrentRebind<'cur, 'q, 'g, 'p, 'i, 'v> {
    owner: &'cur OwnedRebind<'q, 'g, 'p, 'i, 'v>,
}
pub struct RebindRow<'row, 'q, 'g, 'p, 'i, 'v> {
    pub(super) owner: &'row OwnedRebind<'q, 'g, 'p, 'i, 'v>,
    axis: &'row ReportedComponentAxis<'row>,
}
impl<'q, 'g, 'p, 'i, 'v> OwnedRebind<'q, 'g, 'p, 'i, 'v> {
    pub fn with_current<T>(
        &self,
        resources: &ResourceReservationView<'_>,
        callback: impl for<'cur> FnOnce(CurrentRebind<'cur, 'q, 'g, 'p, 'i, 'v>) -> T,
    ) -> Result<T, Error> {
        self.check(resources)?;
        let value = callback(CurrentRebind { owner: self });
        self.check(resources)?;
        Ok(value)
    }
}
impl<'cur, 'q, 'g, 'p, 'i, 'v> CurrentRebind<'cur, 'q, 'g, 'p, 'i, 'v> {
    pub fn report(&self) -> &ComponentReport {
        &self.owner.report
    }
    pub fn checks(&self) -> &[CheckRecord<'q>] {
        &self.owner.checks
    }
    pub fn has_gaps(&self) -> bool {
        self.owner.gaps
    }
    pub fn bind_axis<'row>(
        &'row self,
        plan: &RebindPlan<'i, 'v>,
        program: &PreparedRebindProgram<'p, 'i, 'v>,
        queries: &RebindQueries<'g, 'p, 'i, 'v>,
        axis: &'row ReportedComponentAxis<'row>,
    ) -> Result<RebindRow<'row, 'q, 'g, 'p, 'i, 'v>, Error> {
        if !std::ptr::eq(plan, self.owner.queries.program.plan) {
            return Err(Error::ForeignPlan);
        }
        if !std::ptr::eq(program, self.owner.queries.program) {
            return Err(Error::ForeignProgram);
        }
        if !std::ptr::eq(queries, self.owner.queries) {
            return Err(Error::ForeignQueries);
        }
        verify_axis(self.owner, axis)?;
        Ok(RebindRow {
            owner: self.owner,
            axis,
        })
    }
}
impl RebindRow<'_, '_, '_, '_, '_, '_> {
    pub(super) fn check(&self, resources: &ResourceReservationView<'_>) -> Result<(), Error> {
        self.owner.check(resources)?;
        verify_axis(self.owner, self.axis)
    }
    pub(super) fn value(&self) -> &AxisValue<'_> {
        &self.axis.value
    }
}
fn verify_axis(
    owner: &OwnedRebind<'_, '_, '_, '_, '_>,
    axis: &ReportedComponentAxis<'_>,
) -> Result<(), Error> {
    if axis.kind != AxisKind::Rebind {
        return Err(Error::MetricMismatch);
    }
    if !std::ptr::eq(axis.report, &owner.report) {
        return Err(Error::ForeignReport);
    }
    let actual = project_component_reported(&owner.report)?;
    let same = match (&axis.value, &actual.value) {
        (AxisValue::Exact(a), AxisValue::Exact(b))
        | (AxisValue::Bounded(a), AxisValue::Bounded(b))
        | (AxisValue::Estimated(a), AxisValue::Estimated(b)) => std::ptr::eq(*a, *b),
        (AxisValue::Unavailable(a), AxisValue::Unavailable(b)) => a == b,
        _ => false,
    };
    if !same {
        return Err(Error::ForeignInterval);
    }
    Ok(())
}
