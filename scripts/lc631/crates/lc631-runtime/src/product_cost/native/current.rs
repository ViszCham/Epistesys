//! Private exact expected-owner/current joins, including post-callback cancellation.
use super::*;
impl Verified<'_> {
    fn check_current(&self, view: &ResourceReservationView<'_>) -> Result<(), CostError> {
        view.check()?;
        self.retained.check()?;
        self.rows_claim.check()?;
        self.lower.check_current(view)?;
        if !self.retained.same_reservation(view) || !self.rows_claim.same_reservation(view) {
            return Err(CostError::ForeignOwner);
        }
        Ok(())
    }
}
impl OwnedSemanticProductBaseline<'_, '_> {
    pub fn historical_report(&self) -> &CostObservation {
        &self.verified.report
    }
    pub(super) fn check_expected(
        &self,
        input: &NativeProductInput<'_, '_>,
    ) -> Result<(), CostError> {
        if !std::ptr::eq(self.plan, input.plan)
            || !std::ptr::eq(self.reported, input.baseline)
            || !self.ports.matches(input)
        {
            return Err(CostError::ForeignOwner);
        }
        Ok(())
    }
    pub(super) fn check_current(
        &self,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), CostError> {
        self.profile.check_current(view)?;
        self.verified.check_current(view)?;
        let mut scratch = execution::reserve(view, self.profile.limits)?;
        scratch.admit_native_host(8 * 1024 * 1024)?;
        let actual = ports::baseline(self.plan, self.reported, self.ports, view)?;
        if actual.catalog != self.catalog || actual.root != self.root {
            return Err(CostError::StaleBaseline);
        }
        let _observed_operational_work =
            validation::check(&actual.fields, self.reported.term(), view)?;
        // Immutable borrowed backing was fully checked at production. Rechecking
        // current native visitors cannot silently replace an owner or snapshot.
        drop(actual);
        drop(scratch);
        self.verified.check_current(view)
    }
}
impl OwnedSemanticProductCost<'_, '_> {
    pub fn historical_report(&self) -> &CostObservation {
        &self.verified.report
    }
    pub(super) fn check_current(
        &self,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), CostError> {
        self.baseline.check_expected(self.input)?;
        self.baseline.check_current(view)?;
        self.verified.check_current(view)?;
        let mut scratch = execution::reserve(view, self.baseline.profile.limits)?;
        scratch.admit_native_host(8 * 1024 * 1024)?;
        let actual = ports::candidate(self.input, self.baseline.catalog, view)?;
        if actual.root != self.baseline.root {
            return Err(CostError::CorrespondenceMismatch);
        }
        let _observed_operational_work =
            validation::check(&actual.fields, self.input.target.term(), view)?;
        drop(actual);
        drop(scratch);
        self.verified.check_current(view)
    }
    pub fn with_current<R>(
        &self,
        expected_baseline: &OwnedSemanticProductBaseline<'_, '_>,
        input: &NativeProductInput<'_, '_>,
        view: &ResourceReservationView<'_>,
        consume: impl for<'o> FnOnce(CurrentCostObservation<'o>) -> R,
    ) -> Result<R, CostError> {
        if !std::ptr::eq(self.baseline, expected_baseline) || !same_input(self.input, input) {
            return Err(CostError::ForeignOwner);
        }
        self.check_current(view)?;
        let o = self.input.owners;
        let result = consume(CurrentCostObservation {
            cost: &self.verified.report,
            ledger: &self.verified.ledger,
            rows: &self.verified.rows,
            baseline_description: o
                .baseline_description
                .report_current(view)
                .map_err(errors::description)?,
            target_description: o
                .target_description
                .report_current(view)
                .map_err(errors::description)?,
            baseline_risk: o
                .baseline_risk
                .report_current(o.baseline_responses, view)
                .map_err(errors::risk)?,
            target_risk: o
                .target_risk
                .report_current(o.target_responses, view)
                .map_err(errors::risk)?,
            translation: o
                .tl_reduction
                .current(o.tl_root, o.tl_candidate, view)
                .map_err(errors::tl)?,
        });
        self.check_current(view)?;
        Ok(result)
    }
}
fn same_input(a: &NativeProductInput<'_, '_>, b: &NativeProductInput<'_, '_>) -> bool {
    let x = a.owners;
    let y = b.owners;
    std::ptr::eq(a.plan, b.plan)
        && std::ptr::eq(a.baseline, b.baseline)
        && std::ptr::eq(a.target, b.target)
        && std::ptr::eq(x.baseline_description, y.baseline_description)
        && std::ptr::eq(x.target_description, y.target_description)
        && std::ptr::eq(x.baseline_risk, y.baseline_risk)
        && std::ptr::eq(x.baseline_responses, y.baseline_responses)
        && std::ptr::eq(x.target_risk, y.target_risk)
        && std::ptr::eq(x.target_responses, y.target_responses)
        && std::ptr::eq(x.tl_root, y.tl_root)
        && std::ptr::eq(x.tl_candidate, y.tl_candidate)
        && std::ptr::eq(x.tl_reduction, y.tl_reduction)
}
