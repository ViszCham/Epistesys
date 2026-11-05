//! Current-owner capability proposals consumed by the existing local scheduler.
//! This is not a process dispatch, permission, or global interception boundary.
use super::{check_claim, claim, Error, OwnedPriority, Plan, Registry, HELD, SCRATCH};
use lc631_substrate::resource::{ResourceClaim, ResourceReservationView};
use lc631_world::exploration_schedule::{
    ExplorationScheduler, ScheduleBinding, ScheduleStep, SchedulingProposal,
};

pub struct CapabilityWork<'a, 'plan, 'reg, 'view> {
    pub priority: &'a OwnedPriority<'a, 'plan, 'reg, 'view>,
    pub plan: &'plan Plan<'reg, 'view>,
    pub registry: &'reg Registry<'view>,
    pub canonical_key: &'a [u8],
    pub estimated_query_slots: u64,
    pub demand: [u64; 3],
}

/// Owns one preclaimed actual scheduler and binds its entire future series to
/// one immutable plan. Registration availability may vary per candidate.
pub struct CapabilityScheduler<'basis, 'reg, 'view> {
    scheduler: ExplorationScheduler,
    basis: &'basis Plan<'reg, 'view>,
    registry: &'reg Registry<'view>,
    held: ResourceClaim<'view>,
}
impl<'basis, 'reg, 'view> CapabilityScheduler<'basis, 'reg, 'view> {
    pub fn new(
        binding: ScheduleBinding,
        slots: u64,
        max_attempts: u32,
        deadline: u64,
        basis: &'basis Plan<'reg, 'view>,
        registry: &'reg Registry<'view>,
        view: &'view ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        basis.check(registry, view)?;
        if binding.comparison_scope != registry.scope() {
            return Err(Error::Binding);
        }
        // The existing bounded scheduler reserves at most MAX_GROUPS records
        // and MAX_KEY_BYTES per stored key. This is logical RAM, not RSS.
        let held = claim(view, crate::scheduled_solver::SCHEDULER_RAM_BYTES, false)?;
        let scheduler =
            ExplorationScheduler::new(binding, slots, max_attempts, [1, 0, 1], deadline)
                .map_err(Error::Schedule)?;
        Ok(Self {
            scheduler,
            basis,
            registry,
            held,
        })
    }
    pub fn slots_consumed(&self) -> u64 {
        self.scheduler.fairness().slots_consumed
    }
    /// Reject profile drift before invoking the mutating scheduler, even on a
    /// later call with an unchanged caller-supplied scope label.
    pub fn propose<'works, 'a, 'plan, 'other>(
        &mut self,
        works: &'works [CapabilityWork<'a, 'plan, 'other, 'view>],
        frontier: [u8; 32],
        now: u64,
        view: &'view ResourceReservationView<'_>,
    ) -> Result<OwnedSchedule<'works, 'a, 'plan, 'other, 'view>, Error> {
        self.basis.check(self.registry, view)?;
        check_claim(&self.held, view)?;
        for work in works {
            if !same_profile(self.basis, self.registry, work.plan, work.registry) {
                return Err(Error::Binding);
            }
        }
        schedule_current(&mut self.scheduler, works, frontier, now, view)
    }
    /// `cleanup_reported` is local scheduler accounting, not OS observation.
    pub fn finish_current(
        &mut self,
        observed: &OwnedSchedule<'_, '_, '_, '_, '_>,
        view: &ResourceReservationView<'_>,
        cleanup_reported: bool,
    ) -> Result<(), Error> {
        self.basis.check(self.registry, view)?;
        check_claim(&self.held, view)?;
        match observed.current(view)? {
            ScheduleStep::Proposed(ticket) => self
                .scheduler
                .finish(ticket, cleanup_reported)
                .map_err(Error::Schedule),
            ScheduleStep::Stopped(_) => Err(Error::InvalidPlan),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchedulingFreshness {
    CurrentAtReturn,
    ObservedStale,
}

/// The actual step is always retained even when post-proposal checks fail.
/// Historical access never renews the associated lease or a ticket.
pub struct OwnedSchedule<'works, 'a, 'plan, 'reg, 'view> {
    step: ScheduleStep,
    works: &'works [CapabilityWork<'a, 'plan, 'reg, 'view>],
    scope: [u8; 32],
    freshness: SchedulingFreshness,
    held: ResourceClaim<'view>,
}
impl OwnedSchedule<'_, '_, '_, '_, '_> {
    pub fn freshness_at_return(&self) -> SchedulingFreshness {
        self.freshness
    }
    pub fn historical(&self) -> &ScheduleStep {
        &self.step
    }
    pub fn current(&self, view: &ResourceReservationView<'_>) -> Result<&ScheduleStep, Error> {
        if self.freshness != SchedulingFreshness::CurrentAtReturn {
            return Err(Error::Binding);
        }
        check_claim(&self.held, view)?;
        for work in self.works {
            validate(work, self.scope, view)?;
        }
        Ok(&self.step)
    }
}

/// Prevalidate all owners, scope and claims, then call the actual scheduler once.
/// A cancellation/stale check after that mutation returns ObservedStale WITH the
/// actual step, never an error that loses an already-accounted ticket. There is
/// no allocation introduced after the scheduler returns. Callers must handle
/// this as local scheduling accounting, not as an execution permit.
pub fn schedule_current<'works, 'a, 'plan, 'reg, 'view>(
    scheduler: &mut ExplorationScheduler,
    works: &'works [CapabilityWork<'a, 'plan, 'reg, 'view>],
    frontier: [u8; 32],
    now: u64,
    view: &'view ResourceReservationView<'_>,
) -> Result<OwnedSchedule<'works, 'a, 'plan, 'reg, 'view>, Error> {
    view.check().map_err(Error::Resource)?;
    if works.is_empty() || works.len() > 2 {
        return Err(Error::InvalidPlan);
    }
    let held = claim(view, HELD, true)?;
    let scratch = claim(view, SCRATCH, false)?;
    let binding = scheduler.fairness().binding;
    let mut proposals = Vec::new();
    proposals
        .try_reserve_exact(works.len())
        .map_err(|_| Error::Allocation)?;
    let baseline = &works[0];
    for work in works {
        if !same_profile(baseline.plan, baseline.registry, work.plan, work.registry) {
            return Err(Error::Binding);
        }
        validate(work, binding.comparison_scope, view)?;
        proposals.push(SchedulingProposal {
            binding,
            canonical_key: work.canonical_key,
            priority: work.priority.current(work.plan, work.registry, view)?,
            estimated_query_slots: work.estimated_query_slots,
            demand: work.demand,
        });
    }
    scratch.check().map_err(Error::Resource)?;
    check_claim(&held, view)?;
    // propose may mutate scheduling state; preserve its returned step on every
    // later invalidation instead of pretending the proposal did not happen.
    let step = scheduler
        .propose(&proposals, frontier, now, false)
        .map_err(Error::Schedule)?;
    let freshness = if check_claim(&held, view).is_ok()
        && works
            .iter()
            .all(|work| validate(work, binding.comparison_scope, view).is_ok())
    {
        SchedulingFreshness::CurrentAtReturn
    } else {
        SchedulingFreshness::ObservedStale
    };
    Ok(OwnedSchedule {
        step,
        works,
        scope: binding.comparison_scope,
        freshness,
        held,
    })
}
fn same_profile(a: &Plan<'_, '_>, ar: &Registry<'_>, b: &Plan<'_, '_>, br: &Registry<'_>) -> bool {
    ar.scope == br.scope
        && ar.context == br.context
        && ar.backend_source_digest == br.backend_source_digest
        && ar.executable_digest == br.executable_digest
        && a.max_steps == b.max_steps
        && a.probes.len() == b.probes.len()
        && a.probes
            .iter()
            .zip(&b.probes)
            .all(|(x, y)| x.capability == y.capability && x.weight == y.weight)
}
fn validate(
    work: &CapabilityWork<'_, '_, '_, '_>,
    scope: [u8; 32],
    view: &ResourceReservationView<'_>,
) -> Result<(), Error> {
    if work.registry.scope() != scope {
        return Err(Error::Binding);
    }
    work.priority.current(work.plan, work.registry, view)?;
    Ok(())
}
