//! Admission, canonical identities and bounded Boolean-fragment AST scans.
//! This is an input boundary, not source interpretation or independent proof.
use super::{Budget, Failure, Request};
use lc631_solvers::typed_query::{BoolExpr, BoolSymbolId, SmtStatusQuery};
use lc631_substrate::Digest32;
use sha2::{Digest, Sha256};

pub(super) fn admit(request: &Request<'_>) -> Result<(), Failure> {
    let a = request.source.boolean_symbols();
    let b = request.target.boolean_symbols();
    if a.len() > 16 || b.len() > 16 {
        return Err(Failure::AtomLimit);
    }
    if !request.source.integer_symbols().is_empty() || !request.target.integer_symbols().is_empty()
    {
        return Err(Failure::UnsupportedInteger);
    }
    if request.binding.source_context.source() != request.binding.source
        || request.binding.target_context.source() != request.binding.target
    {
        return Err(Failure::SourceBindingMismatch);
    }
    if request.symbols.len() != a.len() || request.symbols.len() != b.len() {
        return Err(Failure::IncompleteMap);
    }
    for (index, &(left, right)) in request.symbols.iter().enumerate() {
        if request.symbols[..index]
            .iter()
            .any(|&(x, y)| x == left || y == right)
        {
            return Err(Failure::DuplicateMap);
        }
        if !a.contains(&left) || !b.contains(&right) {
            return Err(Failure::IncompleteMap);
        }
    }
    Ok(())
}

/// A bounded canonical identity of the Given map and protected probe pair.
/// This binds exact IR syntax, not semantic equivalence or the original prose.
pub fn probe_map_digest(request: &Request<'_>, limit: u64) -> Result<Digest32, Failure> {
    let mut budget = Budget::new(limit)?;
    admit(request)?;
    hash_probe(request, &mut budget)
}
pub(super) fn hash_probe(request: &Request<'_>, budget: &mut Budget) -> Result<Digest32, Failure> {
    let mut hash = Sha256::new();
    hash.update(b"epistesys.formal-boolean.probe-map.v1\0");
    hash.update(request.binding.probe_set.as_bytes());
    hash.update((request.symbols.len() as u64).to_le_bytes());
    for &(source, target) in request.symbols {
        hash.update(source.get().to_le_bytes());
        hash.update(target.get().to_le_bytes());
    }
    scan(
        request.source_predicate,
        request.source.boolean_symbols(),
        &mut hash,
        budget,
        1,
    )?;
    scan(
        request.target_predicate,
        request.target.boolean_symbols(),
        &mut hash,
        budget,
        1,
    )?;
    Ok(Digest32::from_bytes(hash.finalize().into()))
}
pub(super) fn hash_theory(
    query: &SmtStatusQuery,
    budget: &mut Budget,
) -> Result<Digest32, Failure> {
    let mut hash = Sha256::new();
    hash.update(b"epistesys.formal-boolean.theory.v1\0");
    hash.update((query.boolean_symbols().len() as u64).to_le_bytes());
    for symbol in query.boolean_symbols() {
        hash.update(symbol.get().to_le_bytes());
    }
    hash.update((query.assertions().len() as u64).to_le_bytes());
    budget.admits(query.assertions().len() as u64)?;
    for assertion in query.assertions() {
        scan(assertion, query.boolean_symbols(), &mut hash, budget, 1)?;
    }
    Ok(Digest32::from_bytes(hash.finalize().into()))
}
fn scan(
    expr: &BoolExpr,
    symbols: &[BoolSymbolId],
    hash: &mut Sha256,
    budget: &mut Budget,
    depth: u16,
) -> Result<(), Failure> {
    budget.visit()?;
    if depth > 128 {
        return Err(Failure::DepthLimit);
    }
    match expr {
        BoolExpr::Constant(value) => hash.update([0, u8::from(*value)]),
        BoolExpr::Variable(symbol) => {
            if !symbols.contains(symbol) {
                return Err(Failure::UndeclaredSymbol);
            }
            hash.update([1]);
            hash.update(symbol.get().to_le_bytes());
        }
        BoolExpr::Not(child) => {
            hash.update([2]);
            scan(child, symbols, hash, budget, depth + 1)?;
        }
        BoolExpr::And(children) | BoolExpr::Or(children) => {
            if children.is_empty() {
                return Err(Failure::EmptyConnective);
            }
            budget.admits(children.len() as u64)?;
            hash.update([if matches!(expr, BoolExpr::And(_)) {
                3
            } else {
                4
            }]);
            hash.update((children.len() as u64).to_le_bytes());
            for child in children {
                scan(child, symbols, hash, budget, depth + 1)?;
            }
        }
        BoolExpr::Equal(_, _) | BoolExpr::LessEqual(_, _) | BoolExpr::LessThan(_, _) => {
            return Err(Failure::UnsupportedInteger);
        }
    }
    Ok(())
}
