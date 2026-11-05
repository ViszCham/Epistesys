//! Capture before generation; actual immutable bridge is the sole candidate owner.
use super::*;
pub(super) struct Captured<'a> {
    pub source: &'a SourceStore,
    pub source_query: &'a SmtStatusQuery,
    pub premises: &'a [GivenPremise<'a>],
    pub goal: GoalPolicy<'a>,
    pub bridge_limits: BridgeLimits,
    pub target_index: usize,
    pub mutable: Vec<bool>,
    pub context: ContextRevision,
    pub scope: ContextScope,
    pub declaration: Declaration<'a>,
    pub facts: Universe,
    pub constructions: Universe,
    pub baseline_facts: FeatureSet,
    pub archive: archive::Archive,
    pub digest: Digest32,
    pub producer: Digest32,
}
/// Private owner, no scalar/report constructor; content digest is historical only.
/// ```compile_fail
/// use lc631_runtime::context_novelty_measurement::RootPlan;
/// let _ = RootPlan::from_scalar(0);
/// ```
pub struct RootPlan<'a, 'v> {
    pub(super) bridge: CreativeCandidateBridge<'a, 'v>,
    pub(super) state: Captured<'a>,
    pub(super) meter: Meter,
    pub(super) retained: ResourceClaim<'v>,
}
impl<'a, 'v> RootPlan<'a, 'v> {
    pub fn capture_current(
        input: RootInput<'a>,
        declaration: Declaration<'a>,
        view: &'v ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        resources::limits(declaration.limits)?;
        resources::root_capacity(declaration.limits)?;
        view.check().map_err(Error::Resource)?;
        if view.source_revision() != input.source.id().digest() {
            return Err(Error::SourceMismatch);
        }
        let meter = Meter::new(declaration.limits.max_total_work);
        // One fixed metadata-admission slot, with no AST visit/allocation. An
        // already-known impossible payload is rejected before work-intensive
        // canonicalization, validation assignments or feature Vec reservation.
        meter.debit(1)?;
        known_fact_payload(&declaration)?;
        let retained = resources::claim(view, 4 * MIB, None)?;
        let scratch = resources::claim(view, 8 * MIB, None)?;
        let check = || resources::current(&retained, view, input.source.id().digest());
        let mut work = Work::new(&meter, declaration.limits.max_total_work, &check);
        let (mutable, target_index) = validate::input(&input, &declaration, &mut work)?;
        let facts = features::facts(&declaration, &mut work)?;
        let constructions = features::constructions(&declaration, &mut work)?;
        let baseline_facts =
            features::source_facts(&input, &mutable, &facts, &declaration, &mut work)?;
        // Root source construction admission cannot silently drop outside-U ASTs.
        features::construction_set(input.source_query, &constructions, &declaration, &mut work)?;
        let archive = archive::capture(&input, &declaration, &constructions, &mut work)?;
        let producer = identity::producer(&mut work)?;
        let digest = identity::root(
            &input,
            &declaration,
            &facts,
            &constructions,
            producer,
            &mut work,
        )?;
        work.charge(200_000)?; // bounded existing admission/IR/preparation envelope
        let context = input.original.state().revision();
        let scope = input.original.state().scope();
        scratch.check().map_err(Error::Resource)?;
        retained.check().map_err(Error::Resource)?;
        let bridge = CreativeCandidateBridge::with_resources(
            input.source,
            input.original,
            input.target,
            input.premises,
            input.goal,
            input.bridge_limits,
            view,
        )
        .map_err(Error::Creative)?;
        let state = Captured {
            source: input.source,
            source_query: input.source_query,
            premises: input.premises,
            goal: input.goal,
            bridge_limits: input.bridge_limits,
            target_index,
            mutable,
            context,
            scope,
            declaration,
            facts,
            constructions,
            baseline_facts,
            archive,
            digest,
            producer,
        };
        resources::current(&retained, view, state.source.id().digest())?;
        Ok(Self {
            bridge,
            state,
            meter,
            retained,
        })
    }
    pub(super) fn check(&self, view: &ResourceReservationView<'_>) -> Result<(), Error> {
        self.meter.debit(1)?;
        resources::current(&self.retained, view, self.state.source.id().digest())
    }
    pub fn generate_current(
        &mut self,
        operator: Operator,
        view: &'v ResourceReservationView<'_>,
    ) -> Result<ValidatedFormalCandidate<'a, 'v>, Error> {
        self.check(view)?;
        // Existing generator has its own fixed admission and validation caps.
        // Prepay its bounded work envelope on THIS root without changing them.
        self.meter.debit(
            u64::from(self.state.bridge_limits.max_validation_checks)
                .checked_add(100_000)
                .ok_or(Error::WorkBudget)?,
        )?;
        self.bridge
            .generate(operator, view)
            .map_err(Error::Creative)
    }
    pub(super) fn project<'s>(
        &self,
        candidate: &'s ValidatedFormalCandidate<'a, 'v>,
        view: &ResourceReservationView<'_>,
    ) -> Result<&'s SmtStatusQuery, Error> {
        candidate
            .project_current(&self.bridge, view)
            .map_err(|e| match e {
                CreativeError::BindingMismatch => Error::ForeignOwner,
                CreativeError::Resource(e) => Error::Resource(e),
                other => Error::Creative(other),
            })
    }
    pub fn expected_inputs_current<'s>(
        &'s self,
        candidate: &'s ValidatedFormalCandidate<'a, 'v>,
        view: &ResourceReservationView<'_>,
    ) -> Result<ExpectedInputs<'s, 'a, 'v>, Error> {
        self.check(view)?;
        self.meter.debit(CURRENT_WORK)?;
        let candidate_ast = self.project(candidate, view)?;
        let s = &self.state;
        let d = &s.declaration;
        Ok(ExpectedInputs {
            root: self,
            source: s.source,
            source_query: s.source_query,
            candidate,
            candidate_ast,
            atoms: d.atoms,
            facts: d.facts,
            constructions: d.constructions,
            archive: d.archive,
        })
    }
    pub(super) fn expected(
        &self,
        e: ExpectedInputs<'_, 'a, 'v>,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), Error> {
        let s = &self.state;
        let d = &s.declaration;
        let archive = match (e.archive, d.archive) {
            (ArchiveInput::Read(a), ArchiveInput::Read(b)) => std::ptr::eq(a, b),
            (ArchiveInput::Unread, ArchiveInput::Unread)
            | (ArchiveInput::Unavailable, ArchiveInput::Unavailable) => true,
            _ => false,
        };
        if !std::ptr::eq(self, e.root)
            || !std::ptr::eq(s.source, e.source)
            || !std::ptr::eq(s.source_query, e.source_query)
            || !std::ptr::eq(d.atoms, e.atoms)
            || !std::ptr::eq(d.facts, e.facts)
            || !std::ptr::eq(d.constructions, e.constructions)
            || !archive
        {
            return Err(Error::ExpectedInputMismatch);
        }
        if !std::ptr::eq(self.project(e.candidate, view)?, e.candidate_ast)
            || e.candidate.prepared().parent() != s.context
        {
            return Err(Error::ExpectedInputMismatch);
        }
        Ok(())
    }
    pub fn work_remaining_current(&self, view: &ResourceReservationView<'_>) -> Result<u64, Error> {
        self.check(view)?;
        Ok(self.meter.remaining())
    }
    pub fn historical_digest(&self) -> Digest32 {
        self.state.digest
    }
}

/// Exact required truth-vector bytes are known from finite declared dimensions,
/// not estimated from a candidate or obtained by running the evaluator first.
fn known_fact_payload(d: &Declaration<'_>) -> Result<(), Error> {
    let l = d.limits;
    if d.atoms.len() > usize::from(l.max_atoms) || d.atoms.len() > 8 {
        return Err(Error::InvalidMap);
    }
    if d.facts.len() > l.max_features || d.facts.len() > 256 {
        return Err(Error::PayloadBudget);
    }
    let shift = u32::try_from(d.atoms.len()).map_err(|_| Error::InvalidMap)?;
    let rows = 1usize.checked_shl(shift).ok_or(Error::PayloadBudget)?;
    let total = d
        .facts
        .len()
        .checked_mul(rows)
        .ok_or(Error::PayloadBudget)?;
    if (!d.facts.is_empty() && (rows > 4096 || rows > l.max_payload_bytes))
        || total > l.max_payload_bytes
    {
        return Err(Error::PayloadBudget);
    }
    Ok(())
}
