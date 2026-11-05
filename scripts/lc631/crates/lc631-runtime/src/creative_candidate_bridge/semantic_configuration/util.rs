//! Bounded admission, exact content identity and logical work accounting.
use super::*;
use lc631_solvers::typed_query::BoolExpr;
use lc631_substrate::resource::{ResourceAmount, ResourceKind, ResourceRequest};
use sha2::{Digest, Sha256};

pub(super) const RAM: u64 = 8 * 1024 * 1024;
pub(super) const OUTPUT: u64 = 64 * 1024;
pub(super) fn claim<'v>(
    view: &'v ResourceReservationView<'_>,
    ram: u64,
    output: u64,
) -> Result<ResourceClaim<'v>, Error> {
    view.check().map_err(Error::Resource)?;
    let request = |kind, bytes| -> Result<ResourceRequest, Error> {
        Ok(ResourceRequest::new(
            kind,
            ResourceAmount::checked(bytes).map_err(Error::Resource)?,
        ))
    };
    let ram = request(ResourceKind::Ram, ram)?;
    if output == 0 {
        view.claim_resources(&[ram])
    } else {
        view.claim_resources(&[ram, request(ResourceKind::Output, output)?])
    }
    .map_err(Error::Resource)
}
pub(super) fn vector<T>(n: usize) -> Result<Vec<T>, Error> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(n)
        .map_err(|_| Error::AllocationFailed)?;
    if result.capacity() > n {
        return Err(Error::AllocationFailed);
    }
    Ok(result)
}
pub(super) struct Work {
    pub remaining: u64,
    pub queries: usize,
}
impl Work {
    pub fn new(budget: WorkBudget) -> Result<Self, Error> {
        if budget.max_work == 0 || budget.max_work > 1_000_000 {
            return Err(Error::WorkBudget);
        }
        if budget.max_payload_bytes == 0 || budget.max_payload_bytes > 1_048_576 {
            return Err(Error::PayloadBudget);
        }
        Ok(Self {
            remaining: budget.max_work,
            queries: 0,
        })
    }
    pub fn charge(&mut self, n: u64) -> Result<(), Error> {
        self.remaining = self.remaining.checked_sub(n).ok_or(Error::WorkBudget)?;
        Ok(())
    }
    pub fn pair(&mut self, allowance: u64) -> Result<(), Error> {
        if allowance == 0 || allowance > 1_000_000 {
            return Err(Error::WorkBudget);
        }
        self.charge(allowance.checked_mul(2).ok_or(Error::WorkBudget)?)
    }
    pub fn invoked(&mut self) -> Result<(), Error> {
        self.queries = self.queries.checked_add(1).ok_or(Error::WorkBudget)?;
        Ok(())
    }
}
pub(super) fn field(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}
pub(super) fn finish(hash: Sha256) -> Digest32 {
    Digest32::from_bytes(hash.finalize().into())
}
pub(super) fn digest(domain: &[u8], parts: &[&[u8]]) -> Digest32 {
    let mut hash = Sha256::new();
    hash.update(domain);
    for part in parts {
        field(&mut hash, part);
    }
    finish(hash)
}
/// Hashing follows checked native content, not an inferred semantic distance.
pub(super) fn expr(
    hash: &mut Sha256,
    value: &BoolExpr,
    symbols: &[BoolSymbolId],
    nodes: &mut u64,
    depth: u16,
) -> Result<(), Error> {
    *nodes += 1;
    if *nodes > 512 || depth > 32 {
        return Err(Error::Formal(f::Failure::DepthLimit));
    }
    match value {
        BoolExpr::Constant(v) => hash.update([0, u8::from(*v)]),
        BoolExpr::Variable(id) => {
            if !symbols.contains(id) {
                return Err(Error::Formal(f::Failure::UndeclaredSymbol));
            }
            hash.update([1]);
            hash.update(id.get().to_le_bytes());
        }
        BoolExpr::Not(child) => {
            hash.update([2]);
            expr(hash, child, symbols, nodes, depth + 1)?;
        }
        BoolExpr::And(children) | BoolExpr::Or(children) => {
            if children.is_empty() {
                return Err(Error::Formal(f::Failure::EmptyConnective));
            }
            hash.update([if matches!(value, BoolExpr::And(_)) {
                3
            } else {
                4
            }]);
            hash.update((children.len() as u64).to_le_bytes());
            for child in children {
                expr(hash, child, symbols, nodes, depth + 1)?;
            }
        }
        BoolExpr::Equal(..) | BoolExpr::LessEqual(..) | BoolExpr::LessThan(..) => {
            return Err(Error::Formal(f::Failure::UnsupportedInteger))
        }
    }
    Ok(())
}
pub(super) fn theory(query: &SmtStatusQuery) -> Result<(Digest32, u64), Error> {
    if !query.integer_symbols().is_empty() {
        return Err(Error::Formal(f::Failure::UnsupportedInteger));
    }
    if query.boolean_symbols().len() > 8 {
        return Err(Error::Formal(f::Failure::AtomLimit));
    }
    let mut hash = Sha256::new();
    hash.update(b"epistesys.dsem.native-theory.v1\0");
    for id in query.boolean_symbols() {
        hash.update(id.get().to_le_bytes());
    }
    hash.update((query.assertions().len() as u64).to_le_bytes());
    let mut nodes = 0;
    for value in query.assertions() {
        expr(&mut hash, value, query.boolean_symbols(), &mut nodes, 1)?;
    }
    Ok((finish(hash), nodes))
}
pub(super) fn clone_expr(value: &BoolExpr) -> Result<BoolExpr, Error> {
    super::super::admission::clone_expr(value, &mut super::super::admission::Counter::new(512), 1)
        .map_err(Error::Creative)
}
pub(super) fn goal(query: &SmtStatusQuery) -> Result<BoolExpr, Error> {
    if query.assertions().len() == 1 {
        return clone_expr(&query.assertions()[0]);
    }
    let mut values = vector(query.assertions().len())?;
    for value in query.assertions() {
        values.push(clone_expr(value)?);
    }
    Ok(BoolExpr::And(values))
}
