//! finite-formal-v1.4 K6. Description length is a proxy, not semantic truth.
//! Callers must reserve resources before invoking this bounded allocating codec.
mod reader;
mod writer;
use crate::measurement_numeric::BigInt;
pub const GRAMMAR_REVISION: &str = "finite-formal-v1.4/k6";
pub const MAX_BYTES: usize = 16 * 1024 * 1024;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BinderKind {
    Forall,
    Exists,
    Lambda,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Term {
    Bool(bool),
    Integer(BigInt),
    Symbol(String),
    Apply(String, Vec<Term>),
    Binder(BinderKind, String, Box<Term>),
    BoundVar(usize),
}
#[derive(Clone, Copy, Debug)]
pub struct TermLimits {
    pub max_bytes: usize,
    pub max_nodes: usize,
    pub max_depth: usize,
}
impl Default for TermLimits {
    fn default() -> Self {
        Self {
            max_bytes: MAX_BYTES,
            max_nodes: 65_536,
            max_depth: 256,
        }
    }
}
impl TermLimits {
    fn validate(self) -> Result<Self, TermError> {
        if self.max_bytes == 0
            || self.max_bytes > MAX_BYTES
            || self.max_nodes == 0
            || self.max_nodes > MAX_BYTES
            || self.max_depth == 0
            || self.max_depth > 256
        {
            return Err(TermError::Budget);
        }
        Ok(self)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TermError {
    InvalidInput,
    Unsupported,
    Budget,
    Allocation,
}
pub fn decode(bytes: &[u8], limits: TermLimits) -> Result<Term, TermError> {
    let limits = limits.validate()?;
    // The first pass has no heap allocations. All lengths, scope and caps are
    // checked before constructing strings, integers or child vectors.
    reader::validate(bytes, limits)?;
    reader::materialize(bytes, limits)
}
pub fn encoded_len(term: &Term, limits: TermLimits) -> Result<usize, TermError> {
    writer::length(term, limits.validate()?)
}
pub fn encode(term: &Term, limits: TermLimits) -> Result<Vec<u8>, TermError> {
    let len = encoded_len(term, limits)?;
    let mut out = Vec::new();
    out.try_reserve_exact(len)
        .map_err(|_| TermError::Allocation)?;
    writer::write(term, &mut out);
    Ok(out)
}
/// Exact cost of Apply(Symbol("bundle-v1"), [semantic, decoder, dictionary]).
/// Required code/dictionary terms cannot be silently treated as absent.
pub fn bundle_bits(
    semantic: &Term,
    decoder: &Term,
    dictionary: &Term,
    limits: TermLimits,
) -> Result<u64, TermError> {
    let limits = limits.validate()?;
    let parts = [semantic, decoder, dictionary];
    let mut bytes = 13_usize; // 04 03 09 "bundle-v1" 03
    for part in parts {
        bytes = bytes
            .checked_add(encoded_len(part, limits)?)
            .ok_or(TermError::Budget)?;
    }
    if bytes > limits.max_bytes {
        return Err(TermError::Budget);
    }
    // Validate aggregate node/depth budgets too, without cloning any term.
    writer::bundle_limits(parts, limits)?;
    Ok((bytes as u64) * 8)
}
