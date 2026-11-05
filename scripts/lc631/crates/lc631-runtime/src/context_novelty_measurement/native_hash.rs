//! Exact typed native syntax hashing, NOT semantic distance coordinates.
use super::*;
use lc631_solvers::typed_query::IntExpr;
use sha2::{Digest, Sha256};
pub(super) struct Hash<'w, 'm> {
    pub hash: Sha256,
    pub work: &'w mut Work<'m>,
    pub limits: Limits,
    nodes: usize,
}
impl Hash<'_, '_> {
    pub fn bytes(&mut self, b: &[u8]) -> Result<(), Error> {
        self.work.charge(b.len() as u64 + 1)?;
        self.hash.update((b.len() as u64).to_le_bytes());
        self.hash.update(b);
        Ok(())
    }
    pub fn n(&mut self, n: u64) -> Result<(), Error> {
        self.bytes(&n.to_le_bytes())
    }
    fn node(&mut self, depth: usize) -> Result<(), Error> {
        self.work.charge(1)?;
        self.nodes = self.nodes.checked_add(1).ok_or(Error::PayloadBudget)?;
        if self.nodes > self.limits.max_ast_nodes || depth > self.limits.max_depth {
            return Err(Error::PayloadBudget);
        }
        Ok(())
    }
    fn int(&mut self, e: &IntExpr, depth: usize) -> Result<(), Error> {
        self.node(depth)?;
        match e {
            IntExpr::Constant(v) => {
                self.n(10)?;
                self.bytes(&v.to_le_bytes())?;
            }
            IntExpr::Variable(v) => {
                self.n(11)?;
                self.n(u64::from(v.get()))?;
            }
            IntExpr::Add(c) => {
                if c.len() > self.limits.max_ast_nodes {
                    return Err(Error::PayloadBudget);
                }
                self.n(12)?;
                self.n(c.len() as u64)?;
                for e in c {
                    self.int(e, depth + 1)?;
                }
            }
            IntExpr::Subtract(a, b) => {
                self.n(13)?;
                self.int(a, depth + 1)?;
                self.int(b, depth + 1)?;
            }
            IntExpr::Negate(c) => {
                self.n(14)?;
                self.int(c, depth + 1)?;
            }
        }
        Ok(())
    }
    fn boolean(&mut self, e: &BoolExpr, depth: usize) -> Result<(), Error> {
        self.node(depth)?;
        match e {
            BoolExpr::Constant(v) => {
                self.n(0)?;
                self.n(u64::from(*v))?;
            }
            BoolExpr::Variable(v) => {
                self.n(1)?;
                self.n(u64::from(v.get()))?;
            }
            BoolExpr::Not(c) => {
                self.n(2)?;
                self.boolean(c, depth + 1)?;
            }
            BoolExpr::And(c) | BoolExpr::Or(c) => {
                if c.len() > self.limits.max_ast_nodes {
                    return Err(Error::PayloadBudget);
                }
                self.n(if matches!(e, BoolExpr::And(_)) { 3 } else { 4 })?;
                self.n(c.len() as u64)?;
                for e in c {
                    self.boolean(e, depth + 1)?;
                }
            }
            BoolExpr::Equal(a, b) | BoolExpr::LessEqual(a, b) | BoolExpr::LessThan(a, b) => {
                self.n(match e {
                    BoolExpr::Equal(_, _) => 5,
                    BoolExpr::LessEqual(_, _) => 6,
                    _ => 7,
                })?;
                self.int(a, depth + 1)?;
                self.int(b, depth + 1)?;
            }
        }
        Ok(())
    }
    pub fn query(&mut self, q: &SmtStatusQuery) -> Result<(), Error> {
        if q.boolean_symbols().len() > 8
            || q.integer_symbols().len() > 8
            || q.assertions().len() > self.limits.max_assertions
        {
            return Err(Error::PayloadBudget);
        }
        self.n(q.boolean_symbols().len() as u64)?;
        for id in q.boolean_symbols() {
            self.n(u64::from(id.get()))?;
        }
        self.n(q.integer_symbols().len() as u64)?;
        for id in q.integer_symbols() {
            self.n(u64::from(id.get()))?;
        }
        self.n(q.assertions().len() as u64)?;
        self.nodes = 0;
        for e in q.assertions() {
            self.boolean(e, 1)?;
        }
        Ok(())
    }
    pub fn rational(&mut self, r: &Rational) -> Result<(), Error> {
        let bits = r
            .numer()
            .bits()
            .checked_add(r.denom().bits())
            .ok_or(Error::PayloadBudget)?;
        self.work.charge(bits / 8 + 64)?;
        self.bytes(&r.numer().to_signed_bytes_le())?;
        self.bytes(&r.denom().to_signed_bytes_le())
    }
    pub fn finish(self) -> Digest32 {
        Digest32::from_bytes(self.hash.finalize().into())
    }
}
pub(super) fn new<'w, 'm>(work: &'w mut Work<'m>, limits: Limits) -> Hash<'w, 'm> {
    Hash {
        hash: Sha256::new(),
        work,
        limits,
        nodes: 0,
    }
}
