//! Actual P1 guard -> actual K3/archive -> private retained assessment.
use super::*;
impl<'a, 'v> Root<'a, 'v> {
    pub fn measure_current<'s>(
        &'s self,
        e: ExpectedPair<'s, 'a, 'v>,
        view: &'v ResourceReservationView<'_>,
    ) -> Result<Owned<'s, 'a, 'v>, Error> {
        self.check(view)?;
        self.expected(e)?;
        let attempt = self.attempts.get();
        if attempt >= self.registration.limits.max_assessments {
            return Err(Error::AssessmentBudget);
        }
        self.attempts
            .set(attempt.checked_add(1).ok_or(Error::AssessmentBudget)?);
        budget::debit(&self.framework, 4096)?;
        self.payload_guard(e, view)?;
        if self.registration.limits.output_bytes < 65536 {
            return Err(Error::OutputBudget);
        }
        let retained = budget::claim(view, budget::ASSESSMENT, Some(65536))?;
        let scratch = budget::claim(view, budget::SCRATCH, None)?;
        let check = || self.framework.native.check(view);
        let mut work = Work::new(
            &self.framework.native.meter,
            self.framework.native.state.declaration.limits.max_call_work,
            &check,
        );
        let l = budget::native_limits(&self.framework, self.registration.limits);
        let source = signature::set(&self.signature, e.source.expected.kernel)?;
        let target = signature::set(&self.signature, e.target.expected.kernel)?;
        let (distance, union) =
            n::features::distance(&self.signature.universe, &source, &target, l, &mut work)
                .map_err(native)?;
        let construction = construction::set(
            self,
            e.target.expected.subject_query,
            self.framework.native.state.declaration.atoms,
            &mut work,
        )?;
        let mut archive = archive::measure(self, &construction, view, &mut work)?;
        let knownness = std::mem::replace(
            &mut archive.state,
            ComponentState::Unknown(MeasurementGap::MissingObservation),
        );
        let states = [ComponentState::Measured(distance), knownness];
        let reports = identity::reports(self, e, states, &archive, &union, &mut work)?;
        scratch.check().map_err(Error::Resource)?;
        self.payload_guard(e, view)?;
        Ok(Owned {
            root: self,
            expected: e,
            reports,
            union,
            archive,
            checks: Cell::new(0),
            retained,
        })
    }
}
