//! Actual native candidate -> existing finite kernels -> retained private owner.
use super::*;
impl<'a, 'v> RootPlan<'a, 'v> {
    pub fn measure_current<'s>(
        &'s self,
        expected: ExpectedInputs<'s, 'a, 'v>,
        view: &'v ResourceReservationView<'_>,
    ) -> Result<OwnedAssessment<'s, 'a, 'v>, Error> {
        self.check(view)?;
        let l = self.state.declaration.limits;
        let check = || resources::current(&self.retained, view, self.state.source.id().digest());
        let mut work = Work::new(&self.meter, l.max_call_work, &check);
        work.charge(CURRENT_WORK)?;
        self.expected(expected, view)?;
        if l.output_bytes < 65536 {
            return Err(Error::Resource(
                lc631_substrate::resource::ResourceError::Exhausted(
                    lc631_substrate::resource::ResourceKind::Output,
                ),
            ));
        }
        let retained = resources::claim(view, MIB, Some(65536))?;
        let scratch = resources::claim(view, 8 * MIB, None)?;
        // Hard and complete same-signature checks precede ANY distance.
        validate::candidate(self, expected.candidate_ast, &mut work)?;
        let fact_set = features::candidate_facts(self, expected.candidate_ast, &mut work)?;
        let construction_set = features::construction_set(
            expected.candidate_ast,
            &self.state.constructions,
            &self.state.declaration,
            &mut work,
        )?;
        let (dctx, union_weight) = features::distance(
            &self.state.facts,
            &self.state.baseline_facts,
            &fact_set,
            l,
            &mut work,
        )?;
        let dpar = point(0, 1)?;
        let (knownness, status, comparisons) =
            archive::measure(self, &construction_set, &mut work)?;
        let counts = Counts {
            archive_comparisons: comparisons,
            assignment_evaluations: work.assignments,
            work_debited: 0,
        };
        let states = [
            ComponentState::Measured(dctx),
            ComponentState::Measured(dpar),
            knownness,
        ];
        let (reports, counts) = identity::reports(
            self,
            expected.candidate,
            view,
            states,
            counts,
            status,
            &mut work,
        )?;
        scratch.check().map_err(Error::Resource)?;
        resources::current(&retained, view, self.state.source.id().digest())?;
        self.expected(expected, view)?;
        Ok(OwnedAssessment {
            root: self,
            candidate: expected.candidate,
            ast: expected.candidate_ast,
            reports,
            union_weight,
            counts,
            status,
            retained,
        })
    }
}
