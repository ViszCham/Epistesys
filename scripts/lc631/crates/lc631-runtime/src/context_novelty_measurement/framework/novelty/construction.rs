//! Reuse the existing whole-assertion canonical encoder/K3,guard common grammar.
use super::*;
pub(super) fn positive(
    e: &BoolExpr,
    w: &mut Work<'_>,
    depth: usize,
    nodes: &mut usize,
) -> Result<bool, Error> {
    w.charge(1).map_err(native)?;
    *nodes = nodes.checked_add(1).ok_or(Error::PayloadBudget)?;
    if depth > 32 || *nodes > 128 {
        return Err(Error::PayloadBudget);
    }
    match e {
        BoolExpr::Constant(_) | BoolExpr::Variable(_) => Ok(true),
        BoolExpr::Not(c) => {
            positive(c, w, depth + 1, nodes)?;
            Ok(false)
        }
        BoolExpr::And(cs) | BoolExpr::Or(cs) => {
            if cs.is_empty() || cs.len() > 128 {
                return Err(Error::UnsupportedConstruction);
            }
            let mut good = true;
            for c in cs {
                good &= positive(c, w, depth + 1, nodes)?;
            }
            Ok(good)
        }
        BoolExpr::Equal(_, _) | BoolExpr::LessEqual(_, _) | BoolExpr::LessThan(_, _) => Ok(false),
    }
}
pub(super) fn universe(
    root: &f::FrameworkRoot<'_, '_>,
    r: &Registration<'_>,
    w: &mut Work<'_>,
) -> Result<n::Universe, Error> {
    for f in r.constructions {
        if !positive(f.formula, w, 1, &mut 0)? {
            return Err(Error::UnsupportedConstruction);
        }
        budget::weight(f.weight, r.limits, w)?;
    }
    let d = declaration(root, r, root.native.state.declaration.atoms);
    let u = n::features::constructions(&d, w).map_err(native)?;
    if u.features
        .iter()
        .any(|f| f.payload.len() > r.limits.max_feature_bytes)
    {
        return Err(Error::PayloadBudget);
    }
    n::features::construction_set(root.native.state.source_query, &u, &d, w).map_err(native)?;
    Ok(u)
}
pub(super) fn declaration<'a>(
    root: &f::FrameworkRoot<'a, '_>,
    r: &Registration<'a>,
    atoms: &'a [n::AtomMap],
) -> n::Declaration<'a> {
    n::Declaration {
        atoms,
        facts: &[],
        constructions: r.constructions,
        archive: n::ArchiveInput::Unread,
        corpus_revision: r.corpus_revision,
        oracle_revision: r.oracle_revision,
        policy_revision: r.policy_revision,
        limits: budget::native_limits(root, r.limits),
    }
}
pub(super) fn set(
    root: &Root<'_, '_>,
    query: &SmtStatusQuery,
    atoms: &[n::AtomMap],
    w: &mut Work<'_>,
) -> Result<n::FeatureSet, Error> {
    let mut nodes = 0;
    for e in query.assertions() {
        if !positive(e, w, 1, &mut nodes)? {
            return Err(Error::UnsupportedConstruction);
        }
    }
    let d = declaration(&root.framework, root.registration, atoms);
    n::features::construction_set(query, &root.constructions, &d, w).map_err(native)
}
