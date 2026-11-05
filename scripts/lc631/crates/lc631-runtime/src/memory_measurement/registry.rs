//! 日本語: Given有限MODELの完全pair台帳を所有化。外部世界の閉鎖や真理は認証しない。
//! English: Owned Given finite-model pair catalog, never environmental closure or truth.
use super::*;
use sha2::{Digest, Sha256};
pub const MAX_STATES: usize = 16;
pub const MAX_PAIRS: usize = 240;
pub const MAX_PAYLOAD: usize = 65_536;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CostUnit(pub Digest32);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StateId(pub u16);
#[derive(Clone, Copy, Debug)]
pub struct State<'a> {
    pub id: StateId,
    pub canonical_payload: &'a [u8],
}
#[derive(Clone, Copy, Debug)]
pub enum PairDisposition<'a> {
    Allowed {
        unit: CostUnit,
        lower: &'a Rational,
        upper: &'a Rational,
        canonical_payload: &'a [u8],
        evidence: Digest32,
        evidence_payload: &'a [u8],
    },
    Excluded {
        basis: Digest32,
    },
    Unknown,
}
#[derive(Clone, Copy, Debug)]
pub struct Pair<'a> {
    pub from: StateId,
    pub to: StateId,
    pub disposition: PairDisposition<'a>,
}
#[derive(Clone, Copy, Debug)]
pub struct WorkBudget {
    pub max_steps: u64,
}
/// Borrowed immutable declaration only. Numeric validation/ownership occurs under claims.
pub struct RegistryPlan<'a> {
    pub(super) binding: Binding,
    pub(super) states: &'a [State<'a>],
    pub(super) pairs: &'a [Pair<'a>],
    pub(super) from: StateId,
    pub(super) to: StateId,
    pub(super) unit: CostUnit,
    pub(super) kappa: &'a Rational,
    pub(super) budget: WorkBudget,
}
impl<'a> RegistryPlan<'a> {
    #[allow(clippy::too_many_arguments)] // Every independent declared binding remains explicit.
    pub fn declared(
        binding: Binding,
        states: &'a [State<'a>],
        pairs: &'a [Pair<'a>],
        from: StateId,
        to: StateId,
        unit: CostUnit,
        kappa: &'a Rational,
        budget: WorkBudget,
    ) -> Self {
        Self {
            binding,
            states,
            pairs,
            from,
            to,
            unit,
            kappa,
            budget,
        }
    }
    pub fn binding(&self) -> Binding {
        self.binding
    }
}
pub(super) struct OwnedState {
    pub id: StateId,
    pub payload: Vec<u8>,
}
pub(super) struct Edge {
    pub from: usize,
    pub to: usize,
    pub lower: Rational,
    pub upper: Rational,
    pub key: Vec<u8>,
    pub evidence: Digest32,
}
enum Slot {
    Missing,
    Unknown,
    Excluded(Digest32),
    Allowed(usize),
    Diagonal,
}
pub(super) struct FrozenModel {
    pub states: Vec<OwnedState>,
    pub edges: Vec<Edge>,
    pub start: usize,
    pub goal: usize,
    pub kappa: Rational,
    pub closed: bool,
    pub digest: Digest32,
}
fn lookup(states: &[OwnedState], id: StateId) -> Result<usize, Error> {
    states
        .iter()
        .position(|s| s.id == id)
        .ok_or(Error::UnknownState)
}
pub(super) fn freeze(
    plan: &RegistryPlan<'_>,
    work: &mut Work<'_, '_>,
) -> Result<FrozenModel, Error> {
    if plan.states.is_empty() || plan.states.len() > MAX_STATES || plan.pairs.len() > MAX_PAIRS {
        return Err(Error::ModelLimit);
    }
    if plan.budget.max_steps == 0 || plan.budget.max_steps > 1_000_000 {
        return Err(Error::InvalidPlan);
    }
    if plan.binding.definition != definition_revision() {
        return Err(Error::DefinitionMismatch);
    }
    // check new_raw denominator/sign/GCD/bit limits BEFORE comparison, arithmetic or cloning.
    numeric::check(plan.kappa)?;
    if !numeric::checked_compare(plan.kappa, &integer(0))?.is_gt() {
        return Err(Error::InvalidKappa);
    }
    let mut payload_bytes = 0usize;
    for (i, state) in plan.states.iter().enumerate() {
        work.step(1)?;
        if state.canonical_payload.is_empty() || state.canonical_payload.len() > 4096 {
            return Err(Error::ModelLimit);
        }
        payload_bytes = payload_bytes
            .checked_add(state.canonical_payload.len())
            .ok_or(Error::ModelLimit)?;
        if plan.states[..i].iter().any(|s| s.id == state.id) {
            return Err(Error::DuplicateState);
        }
        if plan.states[..i]
            .iter()
            .any(|s| s.canonical_payload == state.canonical_payload)
        {
            return Err(Error::DuplicateContent);
        }
    }
    for pair in plan.pairs {
        work.step(1)?;
        match pair.disposition {
            PairDisposition::Allowed {
                unit,
                lower,
                upper,
                canonical_payload,
                evidence,
                evidence_payload,
            } => {
                numeric::check(lower)?;
                numeric::check(upper)?;
                if unit != plan.unit {
                    return Err(Error::UnitMismatch);
                }
                if numeric::checked_compare(lower, &integer(0))?.is_lt() {
                    return Err(Error::NegativeCost);
                }
                if numeric::checked_compare(lower, upper)?.is_gt() {
                    return Err(Error::ReversedCost);
                }
                if canonical_payload.is_empty() || canonical_payload.len() > 4096 {
                    return Err(Error::ModelLimit);
                }
                if evidence == Digest32::from_bytes([0; 32]) {
                    return Err(Error::MissingBasis);
                }
                if evidence_payload.is_empty() || evidence_payload.len() > 4096 {
                    return Err(Error::MissingBasis);
                }
                payload_bytes = payload_bytes
                    .checked_add(canonical_payload.len())
                    .ok_or(Error::ModelLimit)?;
                payload_bytes = payload_bytes
                    .checked_add(evidence_payload.len())
                    .ok_or(Error::ModelLimit)?;
            }
            PairDisposition::Excluded { basis } => {
                if basis == Digest32::from_bytes([0; 32]) {
                    return Err(Error::MissingBasis);
                }
            }
            PairDisposition::Unknown => {}
        }
    }
    if payload_bytes > MAX_PAYLOAD {
        return Err(Error::ModelLimit);
    }
    let mut states = reserved(plan.states.len())?;
    for state in plan.states {
        states.push(OwnedState {
            id: state.id,
            payload: copy_bytes(state.canonical_payload)?,
        });
    }
    states.sort_unstable_by(|a, b| a.payload.cmp(&b.payload));
    let start = lookup(&states, plan.from)?;
    let goal = lookup(&states, plan.to)?;
    let n = states.len();
    let mut slots = reserved(n * n)?;
    slots.resize_with(n * n, || Slot::Missing);
    for i in 0..n {
        slots[i * n + i] = Slot::Diagonal;
    }
    let mut edges = reserved(plan.pairs.len())?;
    for pair in plan.pairs {
        work.step(1)?;
        let from = lookup(&states, pair.from)?;
        let to = lookup(&states, pair.to)?;
        if from == to {
            return Err(Error::SelfTransition);
        }
        let slot = &mut slots[from * n + to];
        if !matches!(slot, Slot::Missing) {
            return Err(Error::DuplicatePair);
        }
        *slot = match pair.disposition {
            PairDisposition::Unknown => Slot::Unknown,
            PairDisposition::Excluded { basis } => Slot::Excluded(basis),
            PairDisposition::Allowed {
                lower,
                upper,
                unit,
                canonical_payload,
                evidence,
                evidence_payload,
            } => {
                let mut key = Vec::new();
                field(&mut key, &states[from].payload)?;
                field(&mut key, &states[to].payload)?;
                field(&mut key, unit.0.as_bytes())?;
                rational_bytes(&mut key, lower)?;
                rational_bytes(&mut key, upper)?;
                field(&mut key, canonical_payload)?;
                field(&mut key, evidence_payload)?;
                let index = edges.len();
                edges.push(Edge {
                    from,
                    to,
                    lower: lower.clone(),
                    upper: upper.clone(),
                    key,
                    evidence,
                });
                Slot::Allowed(index)
            }
        };
    }
    let closed = slots
        .iter()
        .all(|s| matches!(s, Slot::Excluded(_) | Slot::Allowed(_) | Slot::Diagonal));
    let mut hash = Sha256::new();
    hash.update(b"lc631.bmem.given-pair-catalog.v1\0");
    hash_field(&mut hash, plan.unit.0.as_bytes());
    hash_rational(&mut hash, plan.kappa)?;
    hash_field(&mut hash, &states[start].payload);
    hash_field(&mut hash, &states[goal].payload);
    hash.update((n as u64).to_le_bytes());
    for state in &states {
        hash_field(&mut hash, &state.payload);
    }
    for (i, slot) in slots.iter().enumerate() {
        work.step(1)?;
        hash.update((i as u64).to_le_bytes());
        match slot {
            Slot::Missing => hash.update([0]),
            Slot::Unknown => hash.update([1]),
            Slot::Diagonal => hash.update([2]),
            Slot::Excluded(basis) => {
                hash.update([3]);
                hash_field(&mut hash, basis.as_bytes());
            }
            Slot::Allowed(index) => {
                hash.update([4]);
                hash_field(&mut hash, &edges[*index].key);
            }
        }
    }
    Ok(FrozenModel {
        states,
        edges,
        start,
        goal,
        kappa: plan.kappa.clone(),
        closed,
        digest: Digest32::from_bytes(hash.finalize().into()),
    })
}
pub(super) fn hash_field(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}
pub(super) fn hash_rational(hash: &mut Sha256, value: &Rational) -> Result<(), Error> {
    numeric::check(value)?;
    hash_field(hash, &value.numer().to_signed_bytes_le());
    hash_field(hash, &value.denom().to_signed_bytes_le());
    Ok(())
}
pub(super) fn field(out: &mut Vec<u8>, bytes: &[u8]) -> Result<(), Error> {
    let end = out
        .len()
        .checked_add(8)
        .and_then(|n| n.checked_add(bytes.len()))
        .ok_or(Error::ModelLimit)?;
    if end > 2 * 1024 * 1024 {
        return Err(Error::ModelLimit);
    }
    out.try_reserve(8 + bytes.len())
        .map_err(|_| Error::AllocationFailed)?;
    out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    out.extend_from_slice(bytes);
    Ok(())
}
fn rational_bytes(out: &mut Vec<u8>, value: &Rational) -> Result<(), Error> {
    numeric::check(value)?;
    field(out, &value.numer().to_signed_bytes_le())?;
    field(out, &value.denom().to_signed_bytes_le())
}
pub(super) fn copy_bytes(input: &[u8]) -> Result<Vec<u8>, Error> {
    let mut out = reserved(input.len())?;
    out.extend_from_slice(input);
    Ok(out)
}
