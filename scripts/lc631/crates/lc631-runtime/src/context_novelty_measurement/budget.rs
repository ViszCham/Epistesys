//! One root-local, nonrefundable work ledger. No hidden process-global quota.
use super::*;
use std::cell::Cell;
pub(super) struct Meter {
    remaining: Cell<u64>,
}
impl Meter {
    pub fn new(total: u64) -> Self {
        Self {
            remaining: Cell::new(total),
        }
    }
    pub fn debit(&self, n: u64) -> Result<(), Error> {
        let after = self
            .remaining
            .get()
            .checked_sub(n)
            .ok_or(Error::WorkBudget)?;
        self.remaining.set(after);
        Ok(())
    }
    pub fn remaining(&self) -> u64 {
        self.remaining.get()
    }
}
pub(super) struct Work<'a> {
    meter: &'a Meter,
    check: &'a dyn Fn() -> Result<(), Error>,
    remaining: u64,
    used: u64,
    pub assignments: u64,
}
impl<'a> Work<'a> {
    pub fn new(meter: &'a Meter, limit: u64, check: &'a dyn Fn() -> Result<(), Error>) -> Self {
        Self {
            meter,
            check,
            remaining: limit,
            used: 0,
            assignments: 0,
        }
    }
    pub fn charge(&mut self, n: u64) -> Result<(), Error> {
        let cost = n.checked_add(1).ok_or(Error::WorkBudget)?;
        let next = self.remaining.checked_sub(cost).ok_or(Error::WorkBudget)?;
        self.meter.debit(cost)?;
        self.remaining = next;
        self.used = self.used.checked_add(cost).ok_or(Error::WorkBudget)?;
        (self.check)()
    }
    pub fn assignment(&mut self) -> Result<(), Error> {
        self.charge(1)?;
        self.assignments = self.assignments.checked_add(1).ok_or(Error::WorkBudget)?;
        Ok(())
    }
    pub fn used(&self) -> u64 {
        self.used
    }
}
