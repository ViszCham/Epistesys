use super::*;
use kernel::{identity, WorkBook};
pub struct ExplainRoot<'i, 'v> {
    pub(super) input: ExplainRootInput<'i>,
    pub(super) baseline: Vec<types::Outcome>,
    pub(super) weight: Rational,
    pub(super) digest: Digest32,
    pub(super) work: WorkBook,
    retained: ResourceClaim<'v>,
}
pub struct ExplainCandidate<'r, 'i, 'c, 'v> {
    pub(super) root: &'r ExplainRoot<'i, 'v>,
    pub(super) input: ExplainCandidateInput<'c>,
    pub(super) digest: Digest32,
    retained: ResourceClaim<'v>,
}
pub fn capture_explain<'i, 'v>(
    input: ExplainRootInput<'i>,
    v: &'v ResourceReservationView<'_>,
) -> Result<ExplainRoot<'i, 'v>, Error> {
    kernel::source(input.binding, v)?;
    let scratch = kernel::claim(v, SCRATCH_RAM, false)?;
    let retained = kernel::claim(v, ROOT_RAM, true)?;
    if input.binding.definition != definition_revision() {
        return Err(Error::DefinitionMismatch);
    }
    if input.binding.role != DatasetRole::ExplanationValidation {
        return Err(Error::UnsupportedRole);
    }
    if input.cases.is_empty() || input.cases.len() > 8 {
        return Err(Error::InvalidInput(InvalidInput::EmptySuite));
    }
    let work = WorkBook::new(input.limits)?;
    for (i, c) in input.cases.iter().enumerate() {
        kernel::admit_case(c.key, c.theory, c.goal, c.weight)?;
        kernel::admit_submission(c.theory, c.baseline)?;
        if input.cases[..i].iter().any(|p| p.key == c.key) {
            return Err(Error::InvalidInput(InvalidInput::DuplicateCase));
        }
    }
    let weight = kernel::weight_sum(input.cases.iter().map(|c| c.weight))?;
    let mut h = Sha256::new();
    h.update(b"CR-EXPLAIN.fixed-root.v1");
    kernel::hash_binding(&mut h, input.binding, input.limits);
    for c in input.cases {
        kernel::field(&mut h, c.key);
        identity::hash_theory(&mut h, c.theory);
        kernel::hash_formula(&mut h, c.goal);
        identity::hash_rational(&mut h, c.weight);
        kernel::hash_submission(&mut h, c.baseline);
    }
    let digest = Digest32::from_bytes(h.finalize().into());
    work.charge(input.cases.len())?;
    let mut baseline = kernel::vector(input.cases.len())?;
    for c in input.cases {
        retained.check().map_err(Error::Resource)?;
        baseline.push(kernel::evaluate(
            c.theory,
            c.goal,
            c.baseline,
            input.binding,
            input.limits.max_case_work,
        )?);
    }
    kernel::check_claim(&retained, v)?;
    scratch.check().map_err(Error::Resource)?;
    Ok(ExplainRoot {
        input,
        baseline,
        weight,
        digest,
        work,
        retained,
    })
}
pub fn admit_explain<'r, 'i, 'c, 'v>(
    root: &'r ExplainRoot<'i, 'v>,
    input: ExplainCandidateInput<'c>,
    v: &'v ResourceReservationView<'_>,
) -> Result<ExplainCandidate<'r, 'i, 'c, 'v>, Error> {
    root.check(v)?;
    let scratch = kernel::claim(v, SCRATCH_RAM, false)?;
    let retained = kernel::claim(v, CANDIDATE_RAM, true)?;
    if input.binding.context != root.input.binding.context {
        return Err(Error::BindingMismatch);
    }
    if input.responses.len() > 8 {
        return Err(Error::InvalidInput(InvalidInput::SizeLimit));
    }
    for response in input.responses {
        if response.key.is_empty() || response.key.len() > 4096 || response.prose.len() > 4096 {
            return Err(Error::InvalidInput(InvalidInput::InvalidProse));
        }
    }
    for (i, r) in input.responses.iter().enumerate() {
        if input.responses[..i].iter().any(|p| p.key == r.key) {
            return Err(Error::InvalidInput(InvalidInput::DuplicateResponse));
        }
        let case = root
            .input
            .cases
            .iter()
            .find(|c| c.key == r.key)
            .ok_or(Error::InvalidInput(InvalidInput::UnknownCase))?;
        kernel::admit_submission(case.theory, r.submission)?;
    }
    let mut h = Sha256::new();
    h.update(b"CR-EXPLAIN.actual-candidate.v1");
    h.update(root.digest.as_bytes());
    kernel::hash_candidate(&mut h, input.binding);
    for case in root.input.cases {
        kernel::field(&mut h, case.key);
        match input.responses.iter().find(|r| r.key == case.key) {
            Some(r) => {
                kernel::hash_submission(&mut h, r.submission);
                kernel::field(&mut h, r.prose);
            }
            None => kernel::hash_submission(&mut h, Submission::Missing),
        }
    }
    let digest = Digest32::from_bytes(h.finalize().into());
    root.check(v)?;
    kernel::check_claim(&retained, v)?;
    scratch.check().map_err(Error::Resource)?;
    Ok(ExplainCandidate {
        root,
        input,
        digest,
        retained,
    })
}
impl ExplainRoot<'_, '_> {
    pub fn baseline_state(&self, i: usize) -> Option<CaseState> {
        self.baseline.get(i).map(|o| o.state)
    }
    pub fn baseline_score(&self, i: usize) -> Option<&RationalInterval> {
        self.baseline.get(i).map(|o| &o.node.value)
    }
    pub fn baseline_node(&self, i: usize) -> Option<&ScoreNode> {
        self.baseline.get(i).map(|o| &o.node)
    }
    pub fn work_charged_current(&self, v: &ResourceReservationView<'_>) -> Result<u64, Error> {
        self.check(v)?;
        Ok(self.work.charged())
    }
    pub(super) fn check(&self, v: &ResourceReservationView<'_>) -> Result<(), Error> {
        kernel::check_claim(&self.retained, v)?;
        kernel::source(self.input.binding, v)
    }
    pub(super) fn expected(&self, expected: ExplainRootInput<'_>) -> bool {
        self.input.binding == expected.binding
            && self.input.limits == expected.limits
            && std::ptr::eq(self.input.cases, expected.cases)
    }
}
impl ExplainCandidate<'_, '_, '_, '_> {
    pub(super) fn check(&self, v: &ResourceReservationView<'_>) -> Result<(), Error> {
        self.root.check(v)?;
        kernel::check_claim(&self.retained, v)
    }
    pub(super) fn expected(&self, expected: Expected<'_, '_>) -> bool {
        self.root.expected(expected.root)
            && self.input.binding == expected.candidate.binding
            && std::ptr::eq(self.input.responses, expected.candidate.responses)
    }
}
