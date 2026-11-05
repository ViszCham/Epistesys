use super::*;
use crate::joint_selection::{project_component_reported, ReportedComponentAxis, SelectionPins};
pub struct OwnedExplainGain<'a, 'r, 'i, 'c, 'v> {
    pub(super) candidate: &'a ExplainCandidate<'r, 'i, 'c, 'v>,
    outcomes: Vec<types::Outcome>,
    report: ComponentReport,
    retained: ResourceClaim<'v>,
}
pub fn measure_explain<'a, 'r, 'i, 'c, 'v>(
    candidate: &'a ExplainCandidate<'r, 'i, 'c, 'v>,
    v: &'v ResourceReservationView<'_>,
) -> Result<OwnedExplainGain<'a, 'r, 'i, 'c, 'v>, Error> {
    candidate.check(v)?;
    let scratch = kernel::claim(v, SCRATCH_RAM, false)?;
    let retained = kernel::claim(v, GAIN_RAM, true)?;
    let root = candidate.root;
    root.work.charge(root.input.cases.len())?;
    let mut outcomes = kernel::vector(root.input.cases.len())?;
    for case in root.input.cases {
        candidate.check(v)?;
        let submission = candidate
            .input
            .responses
            .iter()
            .find(|r| r.key == case.key)
            .map(|r| r.submission)
            .unwrap_or(Submission::Missing);
        outcomes.push(kernel::evaluate(
            case.theory,
            case.goal,
            submission,
            root.input.binding,
            root.input.limits.max_case_work,
        )?);
    }
    let mut pairs = kernel::vector(outcomes.len())?;
    for ((case, base), actual) in root.input.cases.iter().zip(&root.baseline).zip(&outcomes) {
        pairs.push(kernel::identity::Pair {
            key: case.key,
            weight: case.weight,
            baseline: &base.node,
            candidate: &actual.node,
        });
    }
    let report = kernel::make_report(
        ComponentMetric::Explain,
        root.input.binding,
        candidate.input.binding,
        candidate.digest,
        &pairs,
    )?;
    candidate.check(v)?;
    kernel::check_claim(&retained, v)?;
    scratch.check().map_err(Error::Resource)?;
    Ok(OwnedExplainGain {
        candidate,
        outcomes,
        report,
        retained,
    })
}
impl OwnedExplainGain<'_, '_, '_, '_, '_> {
    pub fn candidate_state(&self, i: usize) -> Option<CaseState> {
        self.outcomes.get(i).map(|o| o.state)
    }
    pub fn candidate_node(&self, i: usize) -> Option<&ScoreNode> {
        self.outcomes.get(i).map(|o| &o.node)
    }
    pub fn fixed_weight(&self) -> &Rational {
        &self.candidate.root.weight
    }
    pub fn measurement_binding(&self) -> Digest32 {
        self.report.binding.result
    }
    pub fn historical_report(&self) -> &ComponentReport {
        &self.report
    }
    pub fn report_current(
        &self,
        candidate: &ExplainCandidate<'_, '_, '_, '_>,
        expected: Expected<'_, '_>,
        v: &ResourceReservationView<'_>,
    ) -> Result<&ComponentReport, Error> {
        self.candidate.check(v)?;
        kernel::check_claim(&self.retained, v)?;
        if !std::ptr::eq(self.candidate, candidate) {
            return Err(Error::CandidateMismatch);
        }
        if !self.candidate.expected(expected) {
            return Err(Error::ExpectedInputMismatch);
        }
        kernel::check_claim(&self.retained, v)?;
        Ok(&self.report)
    }
    pub fn axis_current(
        &self,
        candidate: &ExplainCandidate<'_, '_, '_, '_>,
        expected: Expected<'_, '_>,
        pins: SelectionPins,
        v: &ResourceReservationView<'_>,
    ) -> Result<ReportedComponentAxis<'_>, Error> {
        let report = self.report_current(candidate, expected, v)?;
        if pins != self.candidate.root.input.binding.selection {
            return Err(Error::BindingMismatch);
        }
        let axis = project_component_reported(report).map_err(Error::Selection)?;
        self.report_current(candidate, expected, v)?;
        Ok(axis)
    }
}
