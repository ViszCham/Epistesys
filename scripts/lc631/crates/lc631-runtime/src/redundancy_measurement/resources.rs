//! 日本語: 同じroot ownerの単調workとlive backingを別計数する。全run quotaではない。
//! English: Separate monotonic work from live backing for one root owner, not a run-global quota.
use super::{Error, Limits};
use lc631_substrate::resource::{
    ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
    ResourceReservationView,
};
use std::cell::Cell;

pub(super) struct Counters {
    work: Cell<u64>,
    ram: Cell<u64>,
    output: Cell<u64>,
    poisoned: Cell<bool>,
}
impl Counters {
    pub(super) fn new(initial_ram: u64) -> Self {
        Self {
            work: Cell::new(0),
            ram: Cell::new(initial_ram),
            output: Cell::new(0),
            poisoned: Cell::new(false),
        }
    }
    /// Commit one checked debit. Failed debit changes nothing; successful debit
    /// is never refunded on drop, failed hashing/dispatch, cancellation or retry.
    pub(super) fn debit(&self, charge: u64, limit: u64) -> Result<(), Error> {
        if self.poisoned.get() {
            return Err(Error::Conflict);
        }
        let next = self
            .work
            .get()
            .checked_add(charge)
            .ok_or(Error::TotalWorkBudget)?;
        if next > limit {
            return Err(Error::TotalWorkBudget);
        }
        self.work.set(next);
        Ok(())
    }
    pub(super) fn spent(&self) -> u64 {
        self.work.get()
    }
    pub(super) fn check(&self) -> Result<(), Error> {
        if self.poisoned.get() {
            Err(Error::Conflict)
        } else {
            Ok(())
        }
    }
}

/// Backing is declared before this claim in every owner, so backing drops first.
/// This releases LIVE bytes only. Logical-work reservations are not refundable.
pub(super) struct ChainClaim<'owner, 'view> {
    counters: &'owner Counters,
    ram: u64,
    output: u64,
    physical: ResourceClaim<'view>,
}
impl<'owner, 'view> ChainClaim<'owner, 'view> {
    pub(super) fn acquire(
        counters: &'owner Counters,
        limits: Limits,
        ram: u64,
        output: u64,
        view: &'view ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        counters.check()?;
        let next_ram = counters
            .ram
            .get()
            .checked_add(ram)
            .ok_or(Error::ArithmeticOverflow)?;
        let next_output = counters
            .output
            .get()
            .checked_add(output)
            .ok_or(Error::ArithmeticOverflow)?;
        if next_ram > limits.max_retained_ram {
            return Err(Error::Resource(ResourceError::Exhausted(ResourceKind::Ram)));
        }
        if next_output > limits.max_output_bytes {
            return Err(Error::Resource(ResourceError::Exhausted(
                ResourceKind::Output,
            )));
        }
        let physical = claim(view, ram, output)?;
        counters.ram.set(next_ram);
        counters.output.set(next_output);
        Ok(Self {
            counters,
            ram,
            output,
            physical,
        })
    }
    pub(super) fn check(&self, view: &ResourceReservationView<'_>) -> Result<(), Error> {
        check_claim(&self.physical, view)?;
        self.counters.check()
    }
}
impl Drop for ChainClaim<'_, '_> {
    fn drop(&mut self) {
        match (
            self.counters.ram.get().checked_sub(self.ram),
            self.counters.output.get().checked_sub(self.output),
        ) {
            (Some(ram), Some(output)) => {
                self.counters.ram.set(ram);
                self.counters.output.set(output);
            }
            _ => self.counters.poisoned.set(true),
        }
    }
}

pub(super) fn check_claim(
    held: &ResourceClaim<'_>,
    view: &ResourceReservationView<'_>,
) -> Result<(), Error> {
    view.check().map_err(Error::Resource)?;
    held.check().map_err(Error::Resource)?;
    if !held.same_reservation(view) {
        return Err(Error::ForeignReservation);
    }
    Ok(())
}
pub(super) fn claim<'view>(
    view: &'view ResourceReservationView<'_>,
    ram: u64,
    output: u64,
) -> Result<ResourceClaim<'view>, Error> {
    let request = |kind, bytes| {
        ResourceAmount::checked(bytes)
            .map(|n| ResourceRequest::new(kind, n))
            .map_err(Error::Resource)
    };
    let ram = request(ResourceKind::Ram, ram)?;
    if output == 0 {
        view.claim_resources(&[ram]).map_err(Error::Resource)
    } else {
        view.claim_resources(&[ram, request(ResourceKind::Output, output)?])
            .map_err(Error::Resource)
    }
}
pub(super) fn vector<T>(count: usize) -> Result<Vec<T>, Error> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Error::AllocationFailed)?;
    if values.capacity() > count {
        return Err(Error::AllocationFailed);
    }
    Ok(values)
}
pub(super) fn backing<T>(count: usize, fixed: usize) -> Result<u64, Error> {
    count
        .checked_mul(std::mem::size_of::<T>())
        .and_then(|n| n.checked_add(fixed))
        .and_then(|n| u64::try_from(n).ok())
        .ok_or(Error::ArithmeticOverflow)
}
