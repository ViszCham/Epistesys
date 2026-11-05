//! 日本語: 有限Boolean構文のadmissionとprovenance。意味同値は判定しない。
//! English: Bounded Boolean syntax admission/provenance, never semantic equivalence.
use super::*;
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Stats {
    pub nodes: u64,
    pub variables: u64,
    pub bytes: u64,
}
impl Stats {
    pub(super) fn add(self, other: Self) -> Result<Self, Error> {
        Ok(Self {
            nodes: add(self.nodes, other.nodes)?,
            variables: add(self.variables, other.variables)?,
            bytes: add(self.bytes, other.bytes)?,
        })
    }
    pub(super) fn admission(self, atoms: usize) -> Result<u64, Error> {
        add(
            add(self.nodes, self.bytes)?,
            mul(self.variables, atoms as u64)?,
        )
    }
}
pub(super) fn add(a: u64, b: u64) -> Result<u64, Error> {
    a.checked_add(b).ok_or(Error::ArithmeticOverflow)
}
pub(super) fn mul(a: u64, b: u64) -> Result<u64, Error> {
    a.checked_mul(b).ok_or(Error::ArithmeticOverflow)
}
pub(super) fn hash(domain: &[u8], parts: &[&[u8]]) -> Digest32 {
    let mut hash = Sha256::new();
    hash.update(domain);
    for part in parts {
        hash.update((part.len() as u64).to_le_bytes());
        hash.update(part);
    }
    Digest32::from_bytes(hash.finalize().into())
}
pub(super) fn expr(
    value: &BoolExpr,
    symbols: &[BoolSymbolId],
    limits: Limits,
    max_nodes: u16,
) -> Result<(Stats, Digest32), Error> {
    let mut stats = Stats::default();
    let mut hash = Sha256::new();
    hash.update(b"epistesys.redundancy.declared-boolean-expression.v1\0");
    walk(value, symbols, limits, max_nodes, 1, &mut stats, &mut hash)?;
    Ok((stats, Digest32::from_bytes(hash.finalize().into())))
}
fn walk(
    value: &BoolExpr,
    symbols: &[BoolSymbolId],
    limits: Limits,
    max_nodes: u16,
    depth: u16,
    stats: &mut Stats,
    hash: &mut Sha256,
) -> Result<(), Error> {
    if depth > limits.max_depth {
        return Err(Error::DepthLimit);
    }
    stats.nodes = add(stats.nodes, 1)?;
    if stats.nodes > u64::from(max_nodes) {
        return Err(Error::NodeLimit);
    }
    match value {
        BoolExpr::Constant(v) => {
            hash.update([0, u8::from(*v)]);
            stats.bytes = add(stats.bytes, 2)?;
        }
        BoolExpr::Variable(id) => {
            if !symbols.contains(id) {
                return Err(Error::Formal(formal_boolean::Failure::UndeclaredSymbol));
            }
            hash.update([1]);
            hash.update(id.get().to_le_bytes());
            stats.variables = add(stats.variables, 1)?;
            stats.bytes = add(stats.bytes, 5)?;
        }
        BoolExpr::Not(child) => {
            hash.update([2]);
            stats.bytes = add(stats.bytes, 1)?;
            walk(child, symbols, limits, max_nodes, depth + 1, stats, hash)?;
        }
        BoolExpr::And(children) | BoolExpr::Or(children) => {
            if children.is_empty() {
                return Err(Error::Formal(formal_boolean::Failure::EmptyConnective));
            }
            // Reject oversized fanout before traversing/allocating child views.
            if children.len() > usize::from(max_nodes) {
                return Err(Error::NodeLimit);
            }
            hash.update([if matches!(value, BoolExpr::And(_)) {
                3
            } else {
                4
            }]);
            hash.update((children.len() as u64).to_le_bytes());
            stats.bytes = add(stats.bytes, 9)?;
            for child in children {
                walk(child, symbols, limits, max_nodes, depth + 1, stats, hash)?;
            }
        }
        BoolExpr::Equal(_, _) | BoolExpr::LessEqual(_, _) | BoolExpr::LessThan(_, _) => {
            return Err(Error::Formal(formal_boolean::Failure::UnsupportedInteger));
        }
    }
    Ok(())
}

pub(super) fn theory(input: &ModelInput<'_>, limits: Limits) -> Result<(Stats, Digest32), Error> {
    if !input.theory.integer_symbols().is_empty() {
        return Err(Error::Formal(formal_boolean::Failure::UnsupportedInteger));
    }
    let mut stats = Stats::default();
    let mut hash = Sha256::new();
    hash.update(b"epistesys.redundancy.actual-given-theory.v1\0");
    hash.update(input.source.id().digest().as_bytes());
    hash.update(input.context.interpretation_digest().as_bytes());
    hash.update([match input.unconstrained {
        formal_boolean::UnconstrainedPolicy::ExplicitlyGiven => 0,
        formal_boolean::UnconstrainedPolicy::MissingScope => 1,
    }]);
    if input.theory.assertions().len() > usize::from(limits.max_theory_nodes) {
        return Err(Error::NodeLimit);
    }
    for assertion in input.theory.assertions() {
        let (next, digest) = expr(
            assertion,
            input.theory.boolean_symbols(),
            limits,
            limits.max_theory_nodes,
        )?;
        stats = stats.add(next)?;
        if stats.nodes > u64::from(limits.max_theory_nodes) {
            return Err(Error::NodeLimit);
        }
        hash.update(digest.as_bytes());
    }
    for id in input.theory.boolean_symbols() {
        hash.update(id.get().to_le_bytes());
    }
    Ok((stats, Digest32::from_bytes(hash.finalize().into())))
}

pub(super) struct ArtifactScan<'input> {
    pub formulas: [Option<&'input BoolExpr>; 16],
    pub stats: [Stats; 16],
    pub gap: Option<Gap>,
    pub digest: Digest32,
    pub work: u64,
}
pub(super) fn artifact<'input>(
    input: &'input SemanticArtifactInput<'input>,
    slots: &[SlotSpec],
    model: Digest32,
    limits: Limits,
    symbols: &[BoolSymbolId],
) -> Result<ArtifactScan<'input>, Error> {
    if input.units.len() > usize::from(limits.max_units) {
        return Err(Error::UnitLimit);
    }
    for (i, unit) in input.units.iter().enumerate() {
        if !slots.iter().any(|s| s.slot == unit.slot) {
            return Err(Error::ForeignSlot);
        }
        if input.units[..i].iter().any(|old| old.slot == unit.slot) {
            return Err(Error::DuplicateSlot);
        }
    }
    if input.units.len() != slots.len() {
        return Err(Error::MissingSlot);
    }
    let mut result = ArtifactScan {
        formulas: [None; 16],
        stats: [Stats::default(); 16],
        gap: None,
        digest: Digest32::from_bytes([0; 32]),
        work: 128,
    };
    let mut hash = Sha256::new();
    hash.update(b"epistesys.redundancy.whole-given-unit-artifact.v1\0");
    hash.update(input.source.id().digest().as_bytes());
    hash.update(input.context.interpretation_digest().as_bytes());
    hash.update(model.as_bytes());
    hash.update((slots.len() as u64).to_le_bytes());
    for (i, slot) in slots.iter().enumerate() {
        let unit = input
            .units
            .iter()
            .find(|u| u.slot == slot.slot)
            .ok_or(Error::MissingSlot)?;
        result.formulas[i] = unit.formula;
        hash.update(slot.slot.0.to_le_bytes());
        hash.update([role(slot.role), protection(slot.protection)]);
        match unit.formula {
            None => {
                hash.update([0]);
                if result.gap.is_none() {
                    result.gap = Some(Gap::UnextractedUnit(slot.slot));
                }
            }
            Some(value) => {
                let (stats, digest) = expr(value, symbols, limits, limits.max_unit_nodes)?;
                result.stats[i] = stats;
                hash.update([1]);
                hash.update(digest.as_bytes());
                result.work = add(result.work, stats.admission(symbols.len())?)?;
            }
        }
    }
    result.work = add(result.work, mul((slots.len() * slots.len()) as u64, 3)?)?;
    result.digest = Digest32::from_bytes(hash.finalize().into());
    Ok(result)
}
pub(super) fn role(value: SemanticRole) -> u8 {
    match value {
        SemanticRole::Premise => 0,
        SemanticRole::Conclusion => 1,
        SemanticRole::Precondition => 2,
        SemanticRole::ActionWarning => 3,
        SemanticRole::ResultWarning => 4,
    }
}
pub(super) fn protection(value: Protection) -> u8 {
    match value {
        Protection::HardUnchanged => 0,
        Protection::Mutable => 1,
    }
}
