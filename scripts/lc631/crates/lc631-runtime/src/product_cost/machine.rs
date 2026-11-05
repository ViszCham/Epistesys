//! Generic bounded B_boot evaluator; domain operations exist only as paid R.
mod eval;
mod validate;
mod value;
use super::{CostError, ExecutionStats};
pub(super) use eval::run;
pub(super) use eval::Engine;
use lc631_substrate::resource::{ResourceClaim, ResourceReservationView};
pub(super) use validate::{validate_package, validate_runtime};
pub(super) trait Resources {
    fn check(&self) -> Result<(), CostError>;
    fn claim(&self, bytes: u64) -> Result<ResourceClaim<'_>, CostError>;
}
pub(super) trait Work {
    fn debit_work(&mut self, n: u64) -> Result<(), CostError>;
}
impl Resources for ResourceReservationView<'_> {
    fn check(&self) -> Result<(), CostError> {
        Ok(ResourceReservationView::check(self)?)
    }
    fn claim(&self, bytes: u64) -> Result<ResourceClaim<'_>, CostError> {
        super::claim(self, bytes)
    }
}
pub(super) struct Meter<'a> {
    pub resources: &'a dyn Resources,
    pub cap: u64,
    pub used: u64,
}
impl<'a> Meter<'a> {
    pub fn new(resources: &'a dyn Resources, cap: u64) -> Self {
        Self {
            resources,
            cap,
            used: 0,
        }
    }
    pub fn debit(&mut self, n: u64) -> Result<(), CostError> {
        self.resources.check()?;
        let next = self.used.checked_add(n).ok_or(CostError::InvalidProgram)?;
        if next > self.cap {
            return Err(CostError::FuelExhausted);
        }
        self.used = next;
        Ok(())
    }
    pub fn chunks(&mut self, n: u64) -> Result<(), CostError> {
        let mut left = n;
        while left > 0 {
            let c = left.min(1024);
            self.debit(c)?;
            left -= c;
        }
        self.resources.check()
    }
}
impl Work for Meter<'_> {
    fn debit_work(&mut self, n: u64) -> Result<(), CostError> {
        self.chunks(n)
    }
}
#[derive(Clone, Copy)]
pub(super) enum Phase {
    Adapter,
    Projection,
    Inverse,
}
impl Phase {
    pub fn step(self, s: &mut ExecutionStats) -> Result<(), CostError> {
        let p = match self {
            Self::Adapter => &mut s.adapter_steps,
            Self::Projection => &mut s.projection_steps,
            Self::Inverse => &mut s.inverse_steps,
        };
        *p = p.checked_add(1).ok_or(CostError::InvalidProgram)?;
        Ok(())
    }
}
