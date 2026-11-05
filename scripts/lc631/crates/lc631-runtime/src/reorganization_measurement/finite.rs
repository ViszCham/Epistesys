//! Bounded shape, internal substitution and canonical content hashing.
//! This does not evaluate truth: the existing finite formal evaluator does that.
use super::{vector, BindingInput, Error, RootUseInput, UseExpr};
use lc631_core::context::ContextRevision;
use lc631_solvers::typed_query::{BoolExpr, BoolSymbolId, SmtStatusQuery};
use lc631_substrate::{source::SourceStore, Digest32};
use sha2::{Digest, Sha256};

pub(super) fn scan(
    e: &BoolExpr,
    symbols: &[BoolSymbolId],
    nodes: u64,
    depth: u16,
) -> Result<u64, Error> {
    fn go(
        e: &BoolExpr,
        symbols: &[BoolSymbolId],
        left: &mut u64,
        depth: u16,
    ) -> Result<u64, Error> {
        if *left == 0 || depth == 0 {
            return Err(Error::ShapeLimit);
        }
        *left -= 1;
        let mut count = 1u64;
        match e {
            BoolExpr::Constant(_) => {}
            BoolExpr::Variable(s) if symbols.contains(s) => {}
            BoolExpr::Variable(_) => return Err(Error::UndeclaredSymbol),
            BoolExpr::Not(c) => count += go(c, symbols, left, depth - 1)?,
            BoolExpr::And(children) | BoolExpr::Or(children) => {
                if children.is_empty() {
                    return Err(Error::InvalidModel);
                }
                for c in children {
                    count += go(c, symbols, left, depth - 1)?;
                }
            }
            BoolExpr::Equal(_, _) | BoolExpr::LessEqual(_, _) | BoolExpr::LessThan(_, _) => {
                return Err(Error::UnsupportedInteger);
            }
        }
        Ok(count)
    }
    let mut remaining = nodes;
    go(e, symbols, &mut remaining, depth)
}
pub(super) fn depth(e: &BoolExpr) -> u16 {
    match e {
        BoolExpr::Not(c) => 1 + depth(c),
        BoolExpr::And(cs) | BoolExpr::Or(cs) => 1 + cs.iter().map(depth).max().unwrap_or(0),
        BoolExpr::Constant(_)
        | BoolExpr::Variable(_)
        | BoolExpr::Equal(_, _)
        | BoolExpr::LessEqual(_, _)
        | BoolExpr::LessThan(_, _) => 1,
    }
}
pub(super) fn expanded_depth(e: &BoolExpr, hole: BoolSymbolId, replacement: u16) -> u16 {
    match e {
        BoolExpr::Variable(s) if *s == hole => replacement,
        BoolExpr::Not(c) => 1 + expanded_depth(c, hole, replacement),
        BoolExpr::And(cs) | BoolExpr::Or(cs) => {
            1 + cs
                .iter()
                .map(|c| expanded_depth(c, hole, replacement))
                .max()
                .unwrap_or(0)
        }
        BoolExpr::Constant(_)
        | BoolExpr::Variable(_)
        | BoolExpr::Equal(_, _)
        | BoolExpr::LessEqual(_, _)
        | BoolExpr::LessThan(_, _) => 1,
    }
}
pub(super) fn expanded_nodes(
    e: &BoolExpr,
    hole: BoolSymbolId,
    replacement_nodes: u64,
) -> Result<u64, Error> {
    match e {
        BoolExpr::Variable(s) if *s == hole => Ok(replacement_nodes),
        BoolExpr::Constant(_) | BoolExpr::Variable(_) => Ok(1),
        BoolExpr::Not(c) => expanded_nodes(c, hole, replacement_nodes)?
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow),
        BoolExpr::And(cs) | BoolExpr::Or(cs) => cs.iter().try_fold(1u64, |n, c| {
            n.checked_add(expanded_nodes(c, hole, replacement_nodes)?)
                .ok_or(Error::ArithmeticOverflow)
        }),
        BoolExpr::Equal(_, _) | BoolExpr::LessEqual(_, _) | BoolExpr::LessThan(_, _) => {
            Err(Error::UnsupportedInteger)
        }
    }
}
pub(super) fn substitute(
    e: &BoolExpr,
    hole: Option<(BoolSymbolId, &BoolExpr)>,
) -> Result<BoolExpr, Error> {
    Ok(match e {
        BoolExpr::Constant(v) => BoolExpr::Constant(*v),
        BoolExpr::Variable(s) => match hole {
            Some((h, r)) if *s == h => substitute(r, None)?,
            _ => BoolExpr::Variable(*s),
        },
        BoolExpr::Not(c) => BoolExpr::Not(Box::new(substitute(c, hole)?)),
        BoolExpr::And(cs) | BoolExpr::Or(cs) => {
            let mut out = vector(cs.len())?;
            for c in cs {
                out.push(substitute(c, hole)?);
            }
            if matches!(e, BoolExpr::And(_)) {
                BoolExpr::And(out)
            } else {
                BoolExpr::Or(out)
            }
        }
        BoolExpr::Equal(_, _) | BoolExpr::LessEqual(_, _) | BoolExpr::LessThan(_, _) => {
            return Err(Error::UnsupportedInteger);
        }
    })
}
pub(super) fn expression(h: &mut Sha256, e: &BoolExpr) {
    match e {
        BoolExpr::Constant(v) => h.update([0, *v as u8]),
        BoolExpr::Variable(s) => {
            h.update([1]);
            h.update(s.get().to_le_bytes());
        }
        BoolExpr::Not(c) => {
            h.update([2]);
            expression(h, c);
        }
        BoolExpr::And(cs) | BoolExpr::Or(cs) => {
            h.update([if matches!(e, BoolExpr::And(_)) { 3 } else { 4 }]);
            h.update((cs.len() as u64).to_le_bytes());
            for c in cs {
                expression(h, c);
            }
        }
        // Unreachable for an admitted model, but keep a distinct invalid tag.
        BoolExpr::Equal(_, _) | BoolExpr::LessEqual(_, _) | BoolExpr::LessThan(_, _) => {
            h.update([255])
        }
    }
}
pub(super) fn finish(h: Sha256) -> Digest32 {
    Digest32::from_bytes(h.finalize().into())
}
pub(super) fn tag(bytes: &[u8]) -> Digest32 {
    let mut h = Sha256::new();
    h.update(bytes);
    finish(h)
}
pub(super) fn model_digest(
    source: &SourceStore,
    context: ContextRevision,
    theory: &SmtStatusQuery,
    bindings: &[BindingInput<'_>],
    uses: &[RootUseInput<'_>],
) -> Digest32 {
    let mut h = Sha256::new();
    h.update(b"epistesys.rebind.given-model.v1\0");
    h.update(source.id().digest().as_bytes());
    h.update(context.interpretation_digest().as_bytes());
    h.update((theory.boolean_symbols().len() as u64).to_le_bytes());
    for s in theory.boolean_symbols() {
        h.update(s.get().to_le_bytes());
    }
    h.update((theory.assertions().len() as u64).to_le_bytes());
    for a in theory.assertions() {
        expression(&mut h, a);
    }
    h.update((bindings.len() as u64).to_le_bytes());
    // Bounded canonical order by typed unit identity, never as a metric value.
    let mut order: [usize; 16] = [0; 16];
    for (i, o) in order.iter_mut().take(bindings.len()).enumerate() {
        *o = i;
    }
    order[..bindings.len()].sort_unstable_by_key(|i| bindings[*i].unit);
    for i in &order[..bindings.len()] {
        let b = &bindings[*i];
        h.update(b.unit.0.to_le_bytes());
        h.update(b.role.0.to_le_bytes());
        h.update(b.parameter.get().to_le_bytes());
        expression(&mut h, b.body);
        if let Some(u) = uses.iter().find(|u| u.unit == b.unit) {
            match u.body {
                UseExpr::MappedUnit(n) => {
                    h.update([0]);
                    h.update(n.0.to_le_bytes());
                }
                UseExpr::RawInput(n) => {
                    h.update([1]);
                    h.update(n.get().to_le_bytes());
                }
            }
        }
    }
    finish(h)
}
