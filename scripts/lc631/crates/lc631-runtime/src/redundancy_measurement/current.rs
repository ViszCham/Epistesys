//! 日本語: historical dataと、同じ借用ownerのcurrent eligibilityを分ける。
//! English: Separate historical data from eligibility of the actual current borrowed owner.
use super::*;
use lc631_substrate::resource::ResourceReservationView;

impl<'pairs, 'candidate, 'plan, 'input, 'view>
    OwnedRedundancy<'pairs, 'candidate, 'plan, 'input, 'view>
{
    pub(super) fn check(&self, view: &ResourceReservationView<'_>) -> Result<(), Error> {
        self.pair_plan.check(view)?;
        self.retained.check(view)?;
        if self.observations.len() != self.pair_plan.requests.len() {
            return Err(Error::Conflict);
        }
        Ok(())
    }
    pub fn state_current(
        &self,
        view: &ResourceReservationView<'_>,
    ) -> Result<&RedundancyState, Error> {
        self.check(view)?;
        Ok(&self.state)
    }
    pub fn counts_current(
        &self,
        view: &ResourceReservationView<'_>,
    ) -> Result<&NativeCounts, Error> {
        self.check(view)?;
        Ok(&self.counts)
    }
    pub fn report_current(
        &self,
        view: &ResourceReservationView<'_>,
    ) -> Result<&ComponentReport, Error> {
        self.check(view)?;
        Ok(&self.report)
    }
    pub fn report_for_input_current(
        &self,
        expected: &BoundCandidateUnits<'_, '_, '_>,
        view: &ResourceReservationView<'_>,
    ) -> Result<&ComponentReport, Error> {
        expected.check(view)?;
        self.check(view)?;
        if !std::ptr::eq(expected, self.pair_plan.candidate) {
            return Err(Error::BindingMismatch);
        }
        Ok(&self.report)
    }
    pub fn axis_current(
        &self,
        expected: &BoundCandidateUnits<'_, '_, '_>,
        view: &ResourceReservationView<'_>,
    ) -> Result<crate::joint_selection::ReportedComponentAxis<'_>, Error> {
        let report = self.report_for_input_current(expected, view)?;
        crate::joint_selection::project_component_reported(report).map_err(|_| Error::Conflict)
    }
    /// Detached copies are historical caller data, never accepted as owners.
    pub fn historical_report(&self) -> &ComponentReport {
        &self.report
    }
}

pub fn compare_current(
    expected_left: &BoundCandidateUnits<'_, '_, '_>,
    left: &OwnedRedundancy<'_, '_, '_, '_, '_>,
    expected_right: &BoundCandidateUnits<'_, '_, '_>,
    right: &OwnedRedundancy<'_, '_, '_, '_, '_>,
    view: &ResourceReservationView<'_>,
) -> Result<Comparison, Error> {
    left.report_for_input_current(expected_left, view)?;
    right.report_for_input_current(expected_right, view)?;
    if expected_left.root.comparison_digest != expected_right.root.comparison_digest {
        return Err(Error::ScopeMismatch);
    }
    let comparison = match (&left.state, &right.state) {
        (RedundancyState::Measured(a), RedundancyState::Measured(b)) => {
            use std::cmp::Ordering;
            match a.lower().cmp(b.lower()) {
                Ordering::Less => Comparison::Lower,
                Ordering::Equal => Comparison::Equal,
                Ordering::Greater => Comparison::Higher,
            }
        }
        _ => Comparison::NoRecommendation,
    };
    left.report_for_input_current(expected_left, view)?;
    right.report_for_input_current(expected_right, view)?;
    Ok(comparison)
}
