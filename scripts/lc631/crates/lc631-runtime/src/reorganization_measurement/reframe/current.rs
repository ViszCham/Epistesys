//! HRTB views and private exact-row constructor; historical reports are not owners.
use super::{
    CauseRecord, Error, OwnedReframe, PaidFrameMap, PerspectiveOutcome, ProbeId, ReframePlan,
};
use crate::joint_selection::{
    project_component_reported, AxisKind, AxisValue, ReportedComponentAxis,
};
use lc631_analysis::creativity_measurement::ComponentReport;
use lc631_core::CandidateId;
use lc631_substrate::resource::ResourceReservationView;
pub struct CurrentReframe<'cur, 'o, 'p, 'i, 's, 'v> {
    owner: &'cur OwnedReframe<'o, 'p, 'i, 's, 'v>,
}
pub struct ReframeRow<'a, 'o, 'p, 'i, 's, 'v> {
    pub(super) owner: &'a OwnedReframe<'o, 'p, 'i, 's, 'v>,
    axis: &'a ReportedComponentAxis<'a>,
}
impl<'o, 'p, 'i, 's, 'v> OwnedReframe<'o, 'p, 'i, 's, 'v> {
    pub fn with_current<T>(
        &self,
        r: &ResourceReservationView<'_>,
        callback: impl for<'cur> FnOnce(CurrentReframe<'cur, 'o, 'p, 'i, 's, 'v>) -> T,
    ) -> Result<T, Error> {
        self.check(r)?;
        let value = callback(CurrentReframe { owner: self });
        self.check(r)?;
        Ok(value)
    }
}
impl<'cur, 'o, 'p, 'i, 's, 'v> CurrentReframe<'cur, 'o, 'p, 'i, 's, 'v> {
    pub fn report(&self) -> &ComponentReport {
        &self.owner.report
    }
    pub fn baseline_results(&self) -> &[PerspectiveOutcome] {
        &self.owner.base_outcomes
    }
    pub fn candidate_results(&self) -> &[PerspectiveOutcome] {
        &self.owner.candidate_outcomes
    }
    pub fn semantic_frame_changed(&self) -> bool {
        self.owner.changed
    }
    /// Structural owner correspondence only; not independent semantic evidence.
    pub fn same_oracle_and_budget(&self) -> bool {
        std::ptr::eq(self.owner.plan, self.owner.baseline.data.plan)
            && std::ptr::eq(self.owner.plan, self.owner.candidate.data.plan)
    }
    pub fn root_reachability_before(&self) -> bool {
        self.owner.plan.root_before
    }
    pub fn root_reachability_after(&self) -> bool {
        self.owner.root_after
    }
    pub fn has_gaps(&self) -> bool {
        self.owner.gaps
    }
    pub fn causes(&self) -> &[CauseRecord] {
        &self.owner.causes
    }
    pub fn used_base_path(&self, id: ProbeId) -> Option<&[u16]> {
        self.owner.used_path(id)
    }
    pub fn recommendation(&self) -> Option<CandidateId> {
        None
    }
    pub fn bind_axis<'a>(
        &'a self,
        plan: &ReframePlan<'i, 's, 'v>,
        baseline: &PaidFrameMap<'p, 'i, 's, 'v>,
        candidate: &PaidFrameMap<'p, 'i, 's, 'v>,
        axis: &'a ReportedComponentAxis<'a>,
    ) -> Result<ReframeRow<'a, 'o, 'p, 'i, 's, 'v>, Error> {
        if !std::ptr::eq(plan, self.owner.plan)
            || !std::ptr::eq(baseline, self.owner.baseline)
            || !std::ptr::eq(candidate, self.owner.candidate)
        {
            return Err(Error::ForeignOwner);
        }
        verify(self.owner, axis)?;
        Ok(ReframeRow {
            owner: self.owner,
            axis,
        })
    }
}
impl ReframeRow<'_, '_, '_, '_, '_, '_> {
    pub(super) fn check(&self, r: &ResourceReservationView<'_>) -> Result<(), Error> {
        self.owner.check(r)?;
        verify(self.owner, self.axis)
    }
    pub(super) fn value(&self) -> &AxisValue<'_> {
        &self.axis.value
    }
}
fn verify(
    owner: &OwnedReframe<'_, '_, '_, '_, '_>,
    axis: &ReportedComponentAxis<'_>,
) -> Result<(), Error> {
    if !std::ptr::eq(axis.report, &owner.report) {
        return Err(Error::ForeignReport);
    }
    let expected = project_component_reported(&owner.report)?;
    if axis.kind != AxisKind::Reframe {
        return Err(Error::ForeignInterval);
    }
    let same = match (&axis.value, &expected.value) {
        (AxisValue::Exact(a), AxisValue::Exact(b))
        | (AxisValue::Bounded(a), AxisValue::Bounded(b)) => std::ptr::eq(*a, *b),
        (AxisValue::Unavailable(a), AxisValue::Unavailable(b)) => a == b,
        _ => false,
    };
    if !same {
        return Err(Error::ForeignInterval);
    }
    Ok(())
}
