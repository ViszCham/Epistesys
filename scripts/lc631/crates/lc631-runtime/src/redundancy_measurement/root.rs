//! 日本語: 変更不可のroot/model/意味対応と、同じownerの共有予算。
//! English: Immutable root/model/meaning correspondence and one owner's shared budget.
use super::*;
use lc631_substrate::resource::ResourceReservationView;

impl<'input, 'view> RootRedundancyPlan<'input, 'view> {
    pub fn capture_closed(
        input: &'input RootInput<'input>,
        limits: Limits,
        view: &'view ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        valid_limits(limits)?;
        view.check().map_err(Error::Resource)?;
        let symbols = input.model.theory.boolean_symbols();
        if symbols.len() > usize::from(limits.max_atoms) {
            return Err(Error::AtomLimit);
        }
        if input.slots.len() > usize::from(limits.max_units) {
            return Err(Error::UnitLimit);
        }
        source(input.model.source, input.model.context, view)?;
        source(input.baseline.source, input.baseline.context, view)?;
        for (i, slot) in input.slots.iter().enumerate() {
            if input.slots[..i].iter().any(|s| s.slot == slot.slot) {
                return Err(Error::DuplicateSlot);
            }
        }
        let bytes = resources::backing::<(BoolSymbolId, BoolSymbolId)>(
            symbols.len(),
            std::mem::size_of::<Self>() + 512,
        )?;
        if bytes > limits.max_retained_ram {
            return Err(Error::Resource(ResourceError::Exhausted(
                lc631_substrate::resource::ResourceKind::Ram,
            )));
        }
        let retained = resources::claim(view, bytes, 0)?;
        let budget = Counters::new(bytes);
        budget.debit(
            admission_envelope(input.slots.len(), limits, true)?,
            limits.max_total_work,
        )?;
        let scratch = scratch(limits, view)?;
        let native_symbols = validate_map(input)?;
        let (theory_stats, native_model) = syntax::theory(input.model, limits)?;
        let meanings = meanings_digest(input);
        let model_digest = syntax::hash(
            b"epistesys.redundancy.given-model-owner.v1\0",
            &[native_model.as_bytes(), meanings.as_bytes()],
        );
        let base = syntax::artifact(input.baseline, input.slots, model_digest, limits, symbols)?;
        let comparison_digest = comparison(input, base.digest, model_digest, limits);
        scratch.check().map_err(Error::Resource)?;
        let result = Self {
            input,
            limits,
            native_symbols,
            consistency_true: BoolExpr::Constant(true),
            theory_stats,
            baseline_gap: base.gap,
            baseline_digest: base.digest,
            model_digest,
            comparison_digest,
            budget,
            retained,
        };
        result.check(view)?;
        Ok(result)
    }
    pub(super) fn check(&self, view: &ResourceReservationView<'_>) -> Result<(), Error> {
        resources::check_claim(&self.retained, view)?;
        self.budget.check()?;
        source(self.input.model.source, self.input.model.context, view)?;
        source(
            self.input.baseline.source,
            self.input.baseline.context,
            view,
        )
    }
    pub(super) fn debit(&self, amount: u64) -> Result<(), Error> {
        self.budget.debit(amount, self.limits.max_total_work)
    }
}

// Nonrefundable modeled scan/hash/map/Delta allowance, reserved BEFORE scanning.
// Constants do not acquire per-assignment variable-lookup charges here.
pub(super) fn admission_envelope(units: usize, limits: Limits, theory: bool) -> Result<u64, Error> {
    let nodes = syntax::mul(units as u64, u64::from(limits.max_unit_nodes))?;
    let nodes = syntax::add(
        nodes,
        if theory {
            u64::from(limits.max_theory_nodes)
        } else {
            0
        },
    )?;
    syntax::add(4096, syntax::mul(nodes, u64::from(limits.max_atoms) + 48)?)
}

pub(super) fn source(
    store: &SourceStore,
    context: ContextRevision,
    view: &ResourceReservationView<'_>,
) -> Result<(), Error> {
    // SourceStore already retains the byte digest. Never rehash arbitrary text
    // during bounded admission/current checks or promote it into authenticity.
    if store.id().digest() != context.source().digest()
        || store.id().digest() != view.source_revision()
    {
        return Err(Error::SourceMismatch);
    }
    Ok(())
}
pub(super) fn scratch<'view>(
    limits: Limits,
    view: &'view ResourceReservationView<'_>,
) -> Result<ResourceClaim<'view>, Error> {
    if limits.max_scratch_ram < PROFILE_LIMITS.max_scratch_ram {
        return Err(Error::Resource(ResourceError::Exhausted(
            lc631_substrate::resource::ResourceKind::Ram,
        )));
    }
    resources::claim(view, PROFILE_LIMITS.max_scratch_ram, 0)
}
fn valid_limits(v: Limits) -> Result<(), Error> {
    let p = PROFILE_LIMITS;
    if v.max_atoms > p.max_atoms
        || v.max_units > p.max_units
        || v.max_unit_nodes > p.max_unit_nodes
        || v.max_theory_nodes > p.max_theory_nodes
        || v.max_depth == 0
        || v.max_depth > p.max_depth
        || v.max_call_work == 0
        || v.max_call_work > p.max_call_work
        || v.max_total_work == 0
        || v.max_total_work > p.max_total_work
        || v.max_retained_ram == 0
        || v.max_retained_ram > p.max_retained_ram
        || v.max_scratch_ram == 0
        || v.max_scratch_ram > p.max_scratch_ram
        || v.max_output_bytes == 0
        || v.max_output_bytes > p.max_output_bytes
    {
        Err(Error::InvalidLimits)
    } else {
        Ok(())
    }
}
fn validate_map(input: &RootInput<'_>) -> Result<Vec<(BoolSymbolId, BoolSymbolId)>, Error> {
    let symbols = input.model.theory.boolean_symbols();
    let map = input.model.atom_map;
    if input.atom_meanings.len() != symbols.len() || map.len() != symbols.len() {
        return Err(Error::InvalidAtomMap);
    }
    for (i, meaning) in input.atom_meanings.iter().enumerate() {
        if input.atom_meanings[..i]
            .iter()
            .any(|m| m.atom == meaning.atom)
            || meaning.grounding.start() == meaning.grounding.end()
            || input.model.source.slice(meaning.grounding).is_err()
        {
            return Err(Error::InvalidAtomMap);
        }
    }
    for (i, entry) in map.iter().enumerate() {
        if map[..i]
            .iter()
            .any(|m| m.local == entry.local || m.meaning == entry.meaning)
            || !symbols.contains(&entry.local)
            || !input.atom_meanings.iter().any(|m| m.atom == entry.meaning)
        {
            return Err(Error::InvalidAtomMap);
        }
    }
    let mut native = resources::vector(symbols.len())?;
    // One fixed global meaning order is shared by EVERY predicate/pair. The
    // numeric IDs only address that Given correspondence, never establish it.
    for meaning in input.atom_meanings {
        let entry = map
            .iter()
            .find(|m| m.meaning == meaning.atom)
            .ok_or(Error::InvalidAtomMap)?;
        native.push((entry.local, entry.local));
    }
    Ok(native)
}
fn meanings_digest(input: &RootInput<'_>) -> Digest32 {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    hash.update(b"epistesys.redundancy.given-global-meanings.v1\0");
    for m in input.atom_meanings {
        hash.update([m.atom.0]);
        hash.update(input.model.source.id().digest().as_bytes());
        hash.update((m.grounding.start() as u64).to_le_bytes());
        hash.update((m.grounding.end() as u64).to_le_bytes());
    }
    for m in input.model.atom_map {
        hash.update(m.local.get().to_le_bytes());
        hash.update([m.meaning.0]);
    }
    Digest32::from_bytes(hash.finalize().into())
}
fn comparison(
    input: &RootInput<'_>,
    baseline: Digest32,
    model: Digest32,
    limits: Limits,
) -> Digest32 {
    // Explicit canonical fields: padding/Debug formatting cannot hide policy changes.
    use sha2::{Digest, Sha256};
    let mut policy = Sha256::new();
    policy.update(b"epistesys.redundancy.actual-limits.v1\0");
    policy.update([limits.max_atoms]);
    for n in [
        limits.max_units,
        limits.max_unit_nodes,
        limits.max_theory_nodes,
        limits.max_depth,
    ] {
        policy.update(n.to_le_bytes());
    }
    for n in [
        limits.max_call_work,
        limits.max_total_work,
        limits.max_retained_ram,
        limits.max_scratch_ram,
        limits.max_output_bytes,
    ] {
        policy.update(n.to_le_bytes());
    }
    let policy = Digest32::from_bytes(policy.finalize().into());
    syntax::hash(
        b"epistesys.redundancy.actual-comparison-scope.v1\0",
        &[
            input.scope.comparison().as_bytes(),
            input.scope.goal().as_bytes(),
            input.scope.probes().as_bytes(),
            input.scope.semantics().as_bytes(),
            baseline.as_bytes(),
            model.as_bytes(),
            policy.as_bytes(),
            PROFILE.as_bytes(),
        ],
    )
}
