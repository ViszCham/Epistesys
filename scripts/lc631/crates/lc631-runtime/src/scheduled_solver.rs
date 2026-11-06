//! Actual world selection -> source-bound runtime broker -> native SMT model.
//! No injected solver reports, operation permits, proof or GPU/CPU-time quota.
#![forbid(unsafe_code)]

mod plan;
pub use plan::{NativeDemand, NativeWork, NativeWorkSpec};
mod capability;
pub use capability::{CapabilityDispatch, CapabilityFreshness, CapabilityNativeWork};

use crate::resource::{ResourceBroker, RunResources};
use lc631_solvers::{
    protocol::{QueryResultKind, SolverProtocol},
    registry::{BackendId, LogicProfile, SolverRegistry, SolverRegistryError},
    runner::{ProcessCleanupState, SolverExecutionReport},
    smt::native_model::{execute_native_model, NativeCheckedModel, NativeModelError},
    supervisor_adapter::{OwnedPayloadError, OwnedPayloadStore, OwnedPayloadStoreInitError},
};
use lc631_substrate::resource::{
    ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
    ResourceReservationView,
};
use lc631_world::exploration_schedule::{
    priority::ScheduleError, ExplorationScheduler, ScheduleBinding, ScheduleStep, SchedulingTicket,
    StopReason, MAX_GROUPS,
};
use std::path::Path;

/// Conservative bounded backing for records, keys and transient proposal views.
/// Logical-byte reservation, excluding allocator headers and process RSS.
pub const SCHEDULER_RAM_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug)]
pub enum DispatchError {
    Resource(ResourceError),
    Schedule(ScheduleError),
    Store(OwnedPayloadStoreInitError),
    GpuUnsupported,
    InvalidDemand,
    InvalidKey,
    ContextMismatch,
    SourceMismatch,
    ConflictingWork,
    GroupLimit,
    ArithmeticOverflow,
    AllocationFailed,
    RelativeWorkingDirectory,
    InternalSelection,
    Capability(crate::capability_measurement::Error),
    CapabilityBinding,
    CapabilityProfileRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnavailableReason {
    Registry(SolverRegistryError),
    ModelProtocol,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReservationSettlement {
    Released,
    Retained(ResourceError),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TicketSettlement {
    Settled,
    Retained,
    Failed(ScheduleError),
}

pub type NativeResult<'query, 'view> =
    Result<NativeCheckedModel<'query, 'view>, NativeModelError<'view>>;

/// The callback consumed the actual native result before lease release.
/// This summary is not durable replay; full payload retention is caller policy.
pub struct DispatchReceipt<T> {
    ticket: SchedulingTicket,
    value: T,
    report: Option<SolverExecutionReport>,
    reservation: ReservationSettlement,
    scheduler: TicketSettlement,
}
impl<T> DispatchReceipt<T> {
    pub fn selected_key(&self) -> &[u8] {
        self.ticket.canonical_key()
    }
    pub fn value(&self) -> &T {
        &self.value
    }
    pub const fn report(&self) -> Option<SolverExecutionReport> {
        self.report
    }
    pub fn reservation_settlement(&self) -> &ReservationSettlement {
        &self.reservation
    }
    pub fn ticket_settlement(&self) -> &TicketSettlement {
        &self.scheduler
    }
}
// Keep the actual post-I/O report and ticket inline. Boxing this variant would
// add a fallible allocation after native execution and risk losing its sole report.
#[allow(clippy::large_enum_variant)]
pub enum DispatchStep<T> {
    Stopped(StopReason),
    Unavailable(UnavailableReason),
    /// Native API returned; report may still say NotStarted/failed/unknown.
    Observed(DispatchReceipt<T>),
}

/// Owns the actual scheduler and retains its backing claim. Tickets are private.
/// Unknown cleanup keeps scheduler demand live, so repeated keys cannot dispatch.
pub struct NativeScheduler<'view, 'lease> {
    scheduler: ExplorationScheduler,
    broker: &'view ResourceBroker,
    metadata: &'view ResourceReservationView<'lease>,
    _claim: ResourceClaim<'view>,
    capability_basis: Option<&'view dyn capability::CurrentProfile>,
}
impl<'view, 'lease> NativeScheduler<'view, 'lease> {
    pub fn with_resources(
        broker: &'view ResourceBroker,
        metadata: &'view ResourceReservationView<'lease>,
        binding: ScheduleBinding,
        slots: u64,
        max_attempts: u32,
        deadline: u64,
    ) -> Result<Self, DispatchError> {
        broker
            .validate_lease(
                metadata,
                metadata.run_id(),
                metadata.source_revision(),
                ResourceKind::Ram,
            )
            .map_err(DispatchError::Resource)?;
        let amount =
            ResourceAmount::checked(SCHEDULER_RAM_BYTES).map_err(DispatchError::Resource)?;
        let claim = metadata
            .claim_resources(&[ResourceRequest::new(ResourceKind::Ram, amount)])
            .map_err(DispatchError::Resource)?;
        let scheduler =
            ExplorationScheduler::new(binding, slots, max_attempts, [1, 0, 1], deadline)
                .map_err(DispatchError::Schedule)?;
        Ok(Self {
            scheduler,
            broker,
            metadata,
            _claim: claim,
            capability_basis: None,
        })
    }

    pub fn slots_consumed(&self) -> u64 {
        self.scheduler.fairness().slots_consumed
    }

    /// Callback input can ONLY come from execute_native_model. It cannot return
    /// a borrow of the local payload/lease; panic/forgotten claims retain quotas.
    #[allow(clippy::too_many_arguments)]
    pub fn dispatch_next<'q, T, F>(
        &mut self,
        run: &RunResources,
        works: &[NativeWork<'q>],
        frontier: [u8; 32],
        now: u64,
        cancelled: bool,
        registry: &SolverRegistry,
        working_directory: &Path,
        consume: F,
    ) -> Result<DispatchStep<T>, DispatchError>
    where
        F: for<'call> FnOnce(NativeResult<'q, 'call>) -> T,
    {
        if self.capability_basis.is_some() {
            return Err(DispatchError::CapabilityProfileRequired);
        }
        self.dispatch_inner(
            run,
            works,
            frontier,
            now,
            cancelled,
            registry,
            working_directory,
            None,
            consume,
        )
    }

    fn validate_dispatch_run(&self, run: &RunResources) -> Result<(), DispatchError> {
        self.broker
            .validate_lease(
                self.metadata,
                self.metadata.run_id(),
                self.metadata.source_revision(),
                ResourceKind::Ram,
            )
            .map_err(DispatchError::Resource)?;
        self.broker
            .validate_run(run, self.metadata.run_id(), self.metadata.source_revision())
            .map_err(DispatchError::Resource)
    }

    #[allow(clippy::too_many_arguments)]
    fn dispatch_inner<'q, T, F>(
        &mut self,
        run: &RunResources,
        works: &[NativeWork<'q>],
        frontier: [u8; 32],
        now: u64,
        cancelled: bool,
        registry: &SolverRegistry,
        working_directory: &Path,
        current: Option<&dyn capability::CurrentBatch>,
        consume: F,
    ) -> Result<DispatchStep<T>, DispatchError>
    where
        F: for<'call> FnOnce(NativeResult<'q, 'call>) -> T,
    {
        self.validate_dispatch_run(run)?;
        if !working_directory.is_absolute() {
            return Err(DispatchError::RelativeWorkingDirectory);
        }
        if works.len() > MAX_GROUPS {
            return Err(DispatchError::GroupLimit);
        }
        validate_work(works, self.metadata)?;
        check_current(current, self.metadata)?;
        if cancelled || works.is_empty() {
            return match self
                .scheduler
                .propose(&[], frontier, now, cancelled)
                .map_err(DispatchError::Schedule)?
            {
                ScheduleStep::Stopped(reason) => Ok(DispatchStep::Stopped(reason)),
                ScheduleStep::Proposed(_) => Err(DispatchError::InternalSelection),
            };
        }
        match registry.resolve_for_query(
            BackendId::Z3,
            LogicProfile::SmtBooleanQfLia,
            QueryResultKind::ModelPayload,
        ) {
            Ok(entry) if entry.protocol() == SolverProtocol::SmtLibPayloadWireV2 => {}
            Ok(_) => return Ok(DispatchStep::Unavailable(UnavailableReason::ModelProtocol)),
            Err(error) => {
                return Ok(DispatchStep::Unavailable(UnavailableReason::Registry(
                    error,
                )))
            }
        }
        let mut proposals = Vec::new();
        proposals
            .try_reserve_exact(works.len())
            .map_err(|_| DispatchError::AllocationFailed)?;
        proposals.extend(works.iter().map(NativeWork::proposal));
        check_current(current, self.metadata)?;
        let ticket = match self
            .scheduler
            .propose(&proposals, frontier, now, cancelled)
            .map_err(DispatchError::Schedule)?
        {
            ScheduleStep::Stopped(reason) => return Ok(DispatchStep::Stopped(reason)),
            ScheduleStep::Proposed(ticket) => ticket,
        };
        let work = works
            .iter()
            .find(|w| w.canonical_key() == ticket.canonical_key())
            .ok_or(DispatchError::InternalSelection)?;
        let demand = work.spec.demand;
        check_current(current, self.metadata)?;
        let amount = |n| ResourceAmount::checked(n).map_err(DispatchError::Resource);
        // Ordinary reserve enforces the existing run source; never rebind it.
        // Broker rejection conservatively leaves this ticket blocked/consumed.
        let mut reservation = run
            .reserve(&[
                ResourceRequest::new(ResourceKind::Ram, amount(demand.ram)?),
                ResourceRequest::new(ResourceKind::Output, amount(demand.output)?),
                ResourceRequest::new(ResourceKind::Child, amount(1)?),
                ResourceRequest::new(ResourceKind::Cleanup, amount(demand.cleanup)?),
            ])
            .map_err(DispatchError::Resource)?;
        let (value, report) = {
            let view = reservation.lease();
            self.broker
                .validate_lease(
                    &view,
                    self.metadata.run_id(),
                    self.metadata.source_revision(),
                    ResourceKind::Child,
                )
                .map_err(DispatchError::Resource)?;
            let mut store =
                OwnedPayloadStore::with_resources(1, &view).map_err(DispatchError::Store)?;
            check_current(current, self.metadata)?;
            let native = execute_native_model(
                registry,
                work.spec.query,
                work.spec.context.revision(),
                &work.spec.budget,
                &view,
                working_directory,
                &mut store,
                work.spec.parser,
            );
            let report = actual_report(&native);
            // The actual checked model OR full typed rejection is consumed while
            // its resource-bound bytes still exist. No caller reports are input.
            let value = consume(native);
            drop(store);
            (value, report)
        };
        let confirmed = report
            .is_some_and(|r| r.cleanup_state() == ProcessCleanupState::RunnerReportedVerified);
        let released = if confirmed {
            reservation.release_confirmed()
        } else {
            reservation.lease().record_cleanup_unknown();
            Err(ResourceError::CleanupUnverified)
        };
        let (reservation, scheduler) = match released {
            Ok(()) => (
                ReservationSettlement::Released,
                match self.scheduler.finish(&ticket, true) {
                    Ok(()) => TicketSettlement::Settled,
                    Err(e) => TicketSettlement::Failed(e),
                },
            ),
            Err(e) => (
                ReservationSettlement::Retained(e),
                TicketSettlement::Retained,
            ),
        };
        Ok(DispatchStep::Observed(DispatchReceipt {
            ticket,
            value,
            report,
            reservation,
            scheduler,
        }))
    }
}

fn check_current(
    current: Option<&dyn capability::CurrentBatch>,
    metadata: &ResourceReservationView<'_>,
) -> Result<(), DispatchError> {
    match current {
        Some(current) => current.check(metadata),
        None => Ok(()),
    }
}

fn validate_work(
    works: &[NativeWork<'_>],
    metadata: &ResourceReservationView<'_>,
) -> Result<(), DispatchError> {
    for (index, work) in works.iter().enumerate() {
        if work.spec.context.revision().source().digest() != metadata.source_revision() {
            return Err(DispatchError::SourceMismatch);
        }
        for other in &works[..index] {
            if work.canonical_key() == other.canonical_key() && !work.same_payload(other) {
                return Err(DispatchError::ConflictingWork);
            }
        }
    }
    Ok(())
}

/// Preserve only actual upstream reports; None remains unobserved, not success.
fn actual_report(result: &NativeResult<'_, '_>) -> Option<SolverExecutionReport> {
    match result {
        Ok(model) => Some(model.observation().report()),
        Err(NativeModelError::Rejected { observation, .. }) => Some(observation.report()),
        Err(NativeModelError::Execution(OwnedPayloadError::Report(report)))
        | Err(NativeModelError::Execution(OwnedPayloadError::StoreAfterExecution {
            report, ..
        })) => Some(*report),
        Err(NativeModelError::SourceMismatch)
        | Err(NativeModelError::Resource(_))
        | Err(NativeModelError::Query(_))
        | Err(NativeModelError::Execution(OwnedPayloadError::Adapter(_)))
        | Err(NativeModelError::Execution(OwnedPayloadError::Registry(_)))
        | Err(NativeModelError::Execution(OwnedPayloadError::UnsupportedMapping))
        | Err(NativeModelError::Execution(OwnedPayloadError::Resource(_)))
        | Err(NativeModelError::Execution(OwnedPayloadError::AllocationFailed))
        | Err(NativeModelError::Execution(OwnedPayloadError::StoreBeforeExecution(_))) => None,
    }
}
