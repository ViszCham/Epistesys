use super::*;
pub struct TransferRoot<'i, 'v> {
    pub(super) input: TransferRootInput<'i>,
    pub(super) baseline: Vec<types::Outcome>,
    pub(super) weight: Rational,
    pub(super) digest: Digest32,
    pub(super) work: k::WorkBook,
    retained: ResourceClaim<'v>,
}
pub struct TransferCandidate<'r, 'i, 'v> {
    pub(super) root: &'r TransferRoot<'i, 'v>,
    pub(super) input: TransferCandidateInput,
    pub(super) digest: Digest32,
    retained: ResourceClaim<'v>,
}
pub fn capture_transfer<'i, 'v>(
    input: TransferRootInput<'i>,
    v: &'v ResourceReservationView<'_>,
) -> Result<TransferRoot<'i, 'v>, Error> {
    k::source(input.binding, v).map_err(mapped)?;
    let scratch = k::claim(v, e::SCRATCH_RAM, false).map_err(mapped)?;
    let retained = k::claim(v, e::ROOT_RAM, true).map_err(mapped)?;
    if input.binding.definition != definition_revision() {
        return Err(Error::DefinitionMismatch);
    }
    if input.binding.role != e::DatasetRole::TransferValidation {
        return Err(Error::UnsupportedRole);
    }
    if input.normalizer_revision != split_normalizer_revision() {
        return Err(Error::NormalizerMismatch);
    }
    if input.protocol
        != (Protocol {
            demonstrations: 0,
            adaptive_updates: 0,
        })
    {
        return Err(Error::UnsupportedProtocol);
    }
    if input.cases.is_empty()
        || input.cases.len() > 8
        || input.registry.is_empty()
        || input.registry.len() > 40
    {
        return Err(Error::InvalidInput(e::InvalidInput::EmptySuite));
    }
    let work = k::WorkBook::new(input.limits).map_err(mapped)?;
    for entry in input.registry {
        for bytes in [
            entry.key,
            entry.family,
            entry.clone_family,
            entry.source_payload,
        ] {
            if bytes.is_empty() || bytes.len() > 4096 {
                return Err(Error::InvalidInput(e::InvalidInput::SizeLimit));
            }
        }
    }
    for (i, c) in input.cases.iter().enumerate() {
        k::admit_case(c.key, c.theory, c.goal, c.weight).map_err(mapped)?;
        if input.cases[..i].iter().any(|p| p.key == c.key) {
            return Err(Error::InvalidInput(e::InvalidInput::DuplicateCase));
        }
        let entry = input
            .registry
            .iter()
            .find(|e| e.key == c.key)
            .ok_or(Error::InvalidInput(e::InvalidInput::UnknownCase))?;
        if entry.role != e::DatasetRole::TransferValidation {
            return Err(Error::UnsupportedRole);
        }
        if !std::ptr::eq(entry.theory, c.theory) || !std::ptr::eq(entry.goal, c.goal) {
            return Err(Error::BindingMismatch);
        }
    }
    let split = split::validate(input)?;
    let weight = k::weight_sum(input.cases.iter().map(|c| c.weight)).map_err(mapped)?;
    let mut h = Sha256::new();
    h.update(b"CR-TRANSFER.fixed-root.v1");
    k::hash_binding(&mut h, input.binding, input.limits);
    h.update(split.as_bytes());
    h.update([code(input.baseline)]);
    for c in input.cases {
        k::field(&mut h, c.key);
        k::identity::hash_theory(&mut h, c.theory);
        k::hash_formula(&mut h, c.goal);
        k::identity::hash_rational(&mut h, c.weight);
    }
    let digest = Digest32::from_bytes(h.finalize().into());
    work.charge(input.cases.len()).map_err(mapped)?;
    let mut baseline = k::vector(input.cases.len()).map_err(mapped)?;
    for c in input.cases {
        retained.check().map_err(Error::Resource)?;
        baseline.push(program::run(
            *c,
            input.binding,
            input.baseline,
            input.limits.max_case_work,
        )?);
    }
    k::check_claim(&retained, v).map_err(mapped)?;
    scratch.check().map_err(Error::Resource)?;
    Ok(TransferRoot {
        input,
        baseline,
        weight,
        digest,
        work,
        retained,
    })
}
pub fn admit_transfer<'r, 'i, 'v>(
    root: &'r TransferRoot<'i, 'v>,
    input: TransferCandidateInput,
    v: &'v ResourceReservationView<'_>,
) -> Result<TransferCandidate<'r, 'i, 'v>, Error> {
    root.check(v)?;
    let retained = k::claim(v, e::CANDIDATE_RAM, true).map_err(mapped)?;
    if input.protocol != root.input.protocol {
        return Err(Error::ProtocolMismatch);
    }
    if input.binding.context != root.input.binding.context {
        return Err(Error::BindingMismatch);
    }
    let mut h = Sha256::new();
    h.update(b"CR-TRANSFER.closed-program-candidate.v1");
    h.update(root.digest.as_bytes());
    k::hash_candidate(&mut h, input.binding);
    h.update([code(input.program)]);
    let digest = Digest32::from_bytes(h.finalize().into());
    root.check(v)?;
    k::check_claim(&retained, v).map_err(mapped)?;
    Ok(TransferCandidate {
        root,
        input,
        digest,
        retained,
    })
}
pub(super) fn code(program: Program) -> u8 {
    match program {
        Program::ExtractV1 => 0,
        Program::ComposeV1 => 1,
    }
}
impl TransferRoot<'_, '_> {
    pub fn baseline_state(&self, i: usize) -> Option<CaseState> {
        self.baseline.get(i).map(|o| o.state)
    }
    pub fn baseline_node(&self, i: usize) -> Option<&ScoreNode> {
        self.baseline.get(i).map(|o| &o.node)
    }
    pub fn work_charged_current(&self, v: &ResourceReservationView<'_>) -> Result<u64, Error> {
        self.check(v)?;
        Ok(self.work.charged())
    }
    pub(super) fn check(&self, v: &ResourceReservationView<'_>) -> Result<(), Error> {
        k::check_claim(&self.retained, v).map_err(mapped)?;
        k::source(self.input.binding, v).map_err(mapped)
    }
    fn expected(&self, x: TransferRootInput<'_>) -> bool {
        self.input.binding == x.binding
            && self.input.limits == x.limits
            && self.input.baseline == x.baseline
            && self.input.protocol == x.protocol
            && self.input.split_revision == x.split_revision
            && self.input.normalizer_revision == x.normalizer_revision
            && std::ptr::eq(self.input.cases, x.cases)
            && std::ptr::eq(self.input.registry, x.registry)
    }
}
impl TransferCandidate<'_, '_, '_> {
    pub(super) fn check(&self, v: &ResourceReservationView<'_>) -> Result<(), Error> {
        self.root.check(v)?;
        k::check_claim(&self.retained, v).map_err(mapped)
    }
    pub(super) fn expected(&self, x: Expected<'_>) -> bool {
        self.input == x.candidate && self.root.expected(x.root)
    }
}
