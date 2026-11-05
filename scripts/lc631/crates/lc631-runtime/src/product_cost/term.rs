//! Existing K6 codec, wrapped with this profile's bounds; no new wire tags.
use super::{CostError, CostLimits, Term};
use lc631_substrate::{
    resource::ResourceError,
    semantic_term::{self, TermError, TermLimits},
};
pub(super) fn app(name: &str, children: Vec<Term>) -> Term {
    Term::Apply(name.to_owned(), children)
}
pub(super) fn sym(name: &str) -> Term {
    Term::Symbol(name.to_owned())
}
pub(super) fn int(n: i128) -> Term {
    Term::Integer(n.into())
}
pub(super) fn bounds(l: CostLimits) -> TermLimits {
    TermLimits {
        max_bytes: l.bundle_bytes as usize,
        max_nodes: l.total_nodes as usize,
        max_depth: l.depth as usize,
    }
}
pub(super) fn error(e: TermError) -> CostError {
    match e {
        TermError::Budget => CostError::ResourceExhausted,
        TermError::Allocation => CostError::Resource(ResourceError::AllocationFailed),
        _ => CostError::InvalidCanonicalEncoding,
    }
}
pub(super) fn encode(t: &Term, l: CostLimits) -> Result<Vec<u8>, CostError> {
    semantic_term::encode(t, bounds(l)).map_err(error)
}
pub(super) fn decode(b: &[u8], l: CostLimits) -> Result<Term, CostError> {
    semantic_term::decode(b, bounds(l)).map_err(error)
}
pub(super) fn length(t: &Term, l: CostLimits) -> Result<u64, CostError> {
    Ok(semantic_term::encoded_len(t, bounds(l)).map_err(error)? as u64)
}
pub(super) fn unsigned(t: &Term) -> Result<u64, CostError> {
    match t {
        Term::Integer(n) => u64::try_from(n).map_err(|_| CostError::InvalidProgram),
        _ => Err(CostError::InvalidProgram),
    }
}
pub(super) fn symbol(t: &Term) -> Result<&str, CostError> {
    match t {
        Term::Symbol(s) => Ok(s),
        _ => Err(CostError::InvalidProgram),
    }
}
pub(super) fn children<'a>(t: &'a Term, name: &str) -> Result<&'a [Term], CostError> {
    match t {
        Term::Apply(n, c) if n == name => Ok(c),
        _ => Err(CostError::InvalidProgram),
    }
}
pub(super) fn nodes(t: &Term) -> Result<u64, CostError> {
    let n = match t {
        Term::Apply(_, c) => c.iter().try_fold(2u64, |a, x| {
            a.checked_add(nodes(x)?).ok_or(CostError::ResourceExhausted)
        })?,
        Term::Binder(_, _, b) => 2u64
            .checked_add(nodes(b)?)
            .ok_or(CostError::ResourceExhausted)?,
        _ => 1,
    };
    Ok(n)
}
pub(super) fn work(t: &Term, l: CostLimits) -> Result<u64, CostError> {
    length(t, l)?
        .checked_add(nodes(t)?)
        .ok_or(CostError::ResourceExhausted)
}
pub(super) fn copy(b: &[u8]) -> Result<Vec<u8>, CostError> {
    let mut v = Vec::new();
    v.try_reserve_exact(b.len())
        .map_err(|_| CostError::Resource(ResourceError::AllocationFailed))?;
    v.extend_from_slice(b);
    Ok(v)
}
