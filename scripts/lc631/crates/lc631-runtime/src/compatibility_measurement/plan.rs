//! Immutable Given declaration, one monotone logical-work series and a root lease.
use super::*;
use std::cell::Cell;

/// Given declarations are not observations or execution permits.
/// Fields drop before their root RAM claim; no Output is claimed here.
pub struct Plan<'i, 'v> {
    pub(super) input: PlanInput<'i>,
    pub(super) limits: Limits,
    pub(super) order: [usize; 4],
    pub(super) source_use: BoolSymbolId,
    pub(super) target_use: BoolSymbolId,
    pub(super) digest: Digest32,
    work: Cell<u64>,
    retained: ResourceClaim<'v>,
}
impl<'i, 'v> Plan<'i, 'v> {
    pub fn given(
        input: PlanInput<'i>,
        limits: Limits,
        view: &'v ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        view.check().map_err(Error::Resource)?;
        if limits.max_work == 0
            || limits.max_work > 1_000_000
            || limits.max_frame_bytes == 0
            || limits.max_frame_bytes > 4096
            || limits.max_program_steps == 0
            || limits.max_program_steps > 64
        {
            return Err(Error::InvalidLimits);
        }
        check_sources(input, view)?;
        let retained = claim(view, 128 * 1024, 0)?;
        let (order, source_use, target_use) = admission::check(input)?;
        let digest = identity::plan(input, limits, order)?;
        retained.check().map_err(Error::Resource)?;
        Ok(Self {
            input,
            limits,
            order,
            source_use,
            target_use,
            digest,
            work: Cell::new(0),
            retained,
        })
    }
    pub fn permutation<'p>(
        &'p self,
        forward: &[PortMap],
        inverse: &[PortMap],
        view: &'v ResourceReservationView<'_>,
    ) -> Result<Program<'p, 'i, 'v>, Error> {
        Program::checked(self, forward, inverse, view)
    }
    pub fn work_used_current(&self, view: &ResourceReservationView<'_>) -> Result<u64, Error> {
        self.check(view)?;
        Ok(self.work.get())
    }
    pub(super) fn check(&self, view: &ResourceReservationView<'_>) -> Result<(), Error> {
        view.check().map_err(Error::Resource)?;
        self.retained.check().map_err(Error::Resource)?;
        if !self.retained.same_reservation(view) {
            return Err(Error::ForeignReservation);
        }
        check_sources(self.input, view)
    }
    pub(super) fn preflight(
        &self,
        slots: u64,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), Error> {
        self.check(view)?;
        if slots > self.limits.max_work - self.work.get() {
            return Err(Error::WorkBudget);
        }
        Ok(())
    }
    pub(super) fn debit(
        &self,
        slots: u64,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), Error> {
        self.preflight(slots, view)?;
        // preflight proves this sum <= the fixed cap <=1,000,000; never reset/refund.
        self.work.set(self.work.get() + slots);
        Ok(())
    }
    pub(super) fn case_frame(&self, case: usize) -> Result<Frame, Error> {
        let input = self.input.cases.get(case).ok_or(Error::InvalidInput)?.input;
        let symbols = self.input.source_theory.boolean_symbols();
        let mut frame = [Assignment {
            symbol: symbols[0],
            value: false,
        }; 3];
        for (index, symbol) in symbols.iter().enumerate() {
            frame[index] = *input
                .iter()
                .find(|a| a.symbol == *symbol)
                .ok_or(Error::InvalidInput)?;
        }
        Ok(frame)
    }
}
fn check_sources(input: PlanInput<'_>, view: &ResourceReservationView<'_>) -> Result<(), Error> {
    if view.source_revision() != input.source.id().digest()
        || input.source_context.source().digest() != input.source.id().digest()
        || input.target_context.source().digest() != input.target.id().digest()
    {
        return Err(Error::BindingMismatch);
    }
    // Target is separately bound Given data, not a second runtime observation/permit.
    if input.source.text().len() > 4096 || input.target.text().len() > 4096 {
        return Err(Error::Unsupported(Unsupported::BridgeProfile));
    }
    Ok(())
}
