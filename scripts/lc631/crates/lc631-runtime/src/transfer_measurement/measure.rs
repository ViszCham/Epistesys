use super::*;
use crate::joint_selection::{project_component_reported, ReportedComponentAxis, SelectionPins};
pub struct OwnedTransferGain<'a, 'r, 'i, 'v> {
    pub(super) candidate: &'a TransferCandidate<'r, 'i, 'v>,
    outcomes: Vec<types::Outcome>,
    report: ComponentReport,
    retained: ResourceClaim<'v>,
}
pub fn execute_transfer<'a, 'r, 'i, 'v>(
    candidate: &'a TransferCandidate<'r, 'i, 'v>,
    v: &'v ResourceReservationView<'_>,
) -> Result<OwnedTransferGain<'a, 'r, 'i, 'v>, Error> {
    candidate.check(v)?;
    let scratch = k::claim(v, e::SCRATCH_RAM, false).map_err(mapped)?;
    let retained = k::claim(v, e::GAIN_RAM, true).map_err(mapped)?;
    let root = candidate.root;
    root.work.charge(root.input.cases.len()).map_err(mapped)?;
    let mut outcomes = k::vector(root.input.cases.len()).map_err(mapped)?;
    for case in root.input.cases {
        candidate.check(v)?;
        outcomes.push(program::run(
            *case,
            root.input.binding,
            candidate.input.program,
            root.input.limits.max_case_work,
        )?);
    }
    let mut pairs = k::vector(outcomes.len()).map_err(mapped)?;
    for ((case, baseline), actual) in root.input.cases.iter().zip(&root.baseline).zip(&outcomes) {
        pairs.push(k::identity::Pair {
            key: case.key,
            weight: case.weight,
            baseline: &baseline.node,
            candidate: &actual.node,
        });
    }
    let report = k::make_report(
        ComponentMetric::Transfer,
        root.input.binding,
        candidate.input.binding,
        candidate.digest,
        &pairs,
    )
    .map_err(mapped)?;
    candidate.check(v)?;
    k::check_claim(&retained, v).map_err(mapped)?;
    scratch.check().map_err(Error::Resource)?;
    Ok(OwnedTransferGain {
        candidate,
        outcomes,
        report,
        retained,
    })
}
impl OwnedTransferGain<'_, '_, '_, '_> {
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
        candidate: &TransferCandidate<'_, '_, '_>,
        expected: Expected<'_>,
        v: &ResourceReservationView<'_>,
    ) -> Result<&ComponentReport, Error> {
        self.candidate.check(v)?;
        k::check_claim(&self.retained, v).map_err(mapped)?;
        if !std::ptr::eq(self.candidate, candidate) {
            return Err(Error::CandidateMismatch);
        }
        if !self.candidate.expected(expected) {
            return Err(Error::ExpectedInputMismatch);
        }
        k::check_claim(&self.retained, v).map_err(mapped)?;
        Ok(&self.report)
    }
    pub fn axis_current(
        &self,
        candidate: &TransferCandidate<'_, '_, '_>,
        expected: Expected<'_>,
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
    pub fn candidate_certificate_current(
        &self,
        candidate: &TransferCandidate<'_, '_, '_>,
        expected: Expected<'_>,
        index: usize,
        v: &ResourceReservationView<'_>,
    ) -> Result<Option<&[e::OwnedStep]>, Error> {
        self.report_current(candidate, expected, v)?;
        let value = self
            .outcomes
            .get(index)
            .and_then(|o| o.certificate.as_deref());
        self.report_current(candidate, expected, v)?;
        Ok(value)
    }
}
