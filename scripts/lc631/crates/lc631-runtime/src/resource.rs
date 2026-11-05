//! One runtime process-root owner for work reservations across all its runs.
//! Limits are operational payload quotas, not OS-global or total-RSS guarantees.
use lc631_substrate::resource::{
    checked_requests, ResourceLease, ResourceLeaseIssuer, RESOURCE_KINDS,
};
pub use lc631_substrate::resource::{
    Cancellation, Deadline, ReportedResourceLease, ResourceAmount, ResourceClaim, ResourceError,
    ResourceKind, ResourceRequest, ResourceReservationView,
};
pub use lc631_substrate::Digest32;
use std::sync::{Arc, Mutex};

const MAX_RUNS: usize = 1024;
const MAX_RESERVATIONS: usize = 4096;
const MAX_METADATA_BACKING: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug)]
pub struct ResourceLimits([u64; RESOURCE_KINDS]);
impl ResourceLimits {
    pub fn uniform(amount: u64) -> Result<Self, ResourceError> {
        if amount == 0 {
            return Err(ResourceError::InvalidConfig);
        }
        Ok(Self([amount; RESOURCE_KINDS]))
    }
    pub fn checked(amounts: [u64; RESOURCE_KINDS]) -> Result<Self, ResourceError> {
        if amounts.iter().all(|amount| *amount == 0) {
            return Err(ResourceError::InvalidConfig);
        }
        Ok(Self(amounts))
    }
    pub fn amount(self, kind: ResourceKind) -> u64 {
        self.0[kind.index()]
    }
}

struct RunRecord {
    id: u64,
    source: Digest32,
    limits: ResourceLimits,
    used: [u64; RESOURCE_KINDS],
    deadline: Deadline,
    cancellation: Cancellation,
}
#[derive(Clone, Copy, Eq, PartialEq)]
enum RecordStatus {
    Active,
    Retained,
}
struct ReservationRecord {
    id: u64,
    run_id: u64,
    parent_source: Digest32,
    payload_source: Digest32,
    amounts: [u64; RESOURCE_KINDS],
    status: RecordStatus,
}
struct Ledger {
    used: [u64; RESOURCE_KINDS],
    runs: Vec<RunRecord>,
    reservations: Vec<ReservationRecord>,
    next_id: u64,
    highest_run_id: u64,
}
struct ProcessRoot {
    limits: ResourceLimits,
    issuer: ResourceLeaseIssuer,
    ledger: Mutex<Ledger>,
    metadata_backing: usize,
}

/// Clone shares the SAME root and counters. A new() is a separately configured
/// accounting root, not another view of OS-enforced process-global limits.
#[derive(Clone)]
pub struct ResourceBroker {
    root: Arc<ProcessRoot>,
}

impl ResourceBroker {
    pub fn new(limits: ResourceLimits) -> Result<Self, ResourceError> {
        let mut runs = Vec::new();
        let mut reservations = Vec::new();
        let requested = std::mem::size_of::<RunRecord>()
            .checked_mul(MAX_RUNS)
            .and_then(|runs| {
                std::mem::size_of::<ReservationRecord>()
                    .checked_mul(MAX_RESERVATIONS)
                    .and_then(|records| runs.checked_add(records))
            })
            .ok_or(ResourceError::ArithmeticOverflow)?;
        if requested > MAX_METADATA_BACKING {
            return Err(ResourceError::AllocationFailed);
        }
        runs.try_reserve_exact(MAX_RUNS)
            .map_err(|_| ResourceError::AllocationFailed)?;
        reservations
            .try_reserve_exact(MAX_RESERVATIONS)
            .map_err(|_| ResourceError::AllocationFailed)?;
        let backing = runs
            .capacity()
            .checked_mul(std::mem::size_of::<RunRecord>())
            .and_then(|runs| {
                reservations
                    .capacity()
                    .checked_mul(std::mem::size_of::<ReservationRecord>())
                    .and_then(|records| runs.checked_add(records))
            })
            .filter(|bytes| *bytes <= MAX_METADATA_BACKING)
            .ok_or(ResourceError::AllocationFailed)?;
        Ok(Self {
            root: Arc::new(ProcessRoot {
                limits,
                issuer: ResourceLeaseIssuer::new()?,
                metadata_backing: backing,
                ledger: Mutex::new(Ledger {
                    used: [0; RESOURCE_KINDS],
                    runs,
                    reservations,
                    next_id: 1,
                    highest_run_id: 0,
                }),
            }),
        })
    }

    /// Separately bounded ledger backing; excludes allocator headers, Arc/Mutex
    /// runtime overhead, stack and payloads. Never report it as measured RSS.
    pub fn metadata_backing_bytes(&self) -> usize {
        self.root.metadata_backing
    }

    pub fn begin_run(
        &self,
        run_id: u64,
        source: Digest32,
        limits: ResourceLimits,
        deadline: Deadline,
        cancellation: Cancellation,
    ) -> Result<RunResources, ResourceError> {
        if run_id == 0
            || limits
                .0
                .iter()
                .zip(self.root.limits.0)
                .any(|(run, process)| *run > process)
        {
            return Err(ResourceError::InvalidConfig);
        }
        cancellation.check()?;
        deadline.check()?;
        let mut ledger = self
            .root
            .ledger
            .lock()
            .map_err(|_| ResourceError::Poisoned)?;
        if run_id <= ledger.highest_run_id {
            return Err(ResourceError::RunIdReused);
        }
        if ledger.runs.len() == MAX_RUNS {
            return Err(ResourceError::AllocationFailed);
        }
        ledger.runs.push(RunRecord {
            id: run_id,
            source,
            limits,
            used: [0; RESOURCE_KINDS],
            deadline,
            cancellation,
        });
        ledger.highest_run_id = run_id;
        Ok(RunResources {
            root: self.root.clone(),
            run_id,
            closed: false,
        })
    }

    pub fn used(&self, kind: ResourceKind) -> Result<u64, ResourceError> {
        Ok(self
            .root
            .ledger
            .lock()
            .map_err(|_| ResourceError::Poisoned)?
            .used[kind.index()])
    }

    pub fn validate_lease(
        &self,
        view: &ResourceReservationView<'_>,
        run_id: u64,
        source: Digest32,
        kind: ResourceKind,
    ) -> Result<(), ResourceError> {
        if view.owner_id() != self.root.issuer.owner_id() {
            return Err(ResourceError::ForeignOwner);
        }
        if view.run_id() != run_id {
            return Err(ResourceError::UnknownRun);
        }
        if view.source_revision() != source {
            return Err(ResourceError::SourceMismatch);
        }
        let ledger = self
            .root
            .ledger
            .lock()
            .map_err(|_| ResourceError::Poisoned)?;
        let run = ledger
            .runs
            .iter()
            .find(|run| run.id == run_id)
            .ok_or(ResourceError::UnknownRun)?;
        run.cancellation.check()?;
        run.deadline.check()?;
        let record = ledger
            .reservations
            .iter()
            .find(|record| record.id == view.reservation_id())
            .ok_or(ResourceError::StaleLease)?;
        if record.run_id != run_id || record.status != RecordStatus::Active {
            return Err(ResourceError::StaleLease);
        }
        if record.parent_source != run.source || record.payload_source != source {
            return Err(ResourceError::SourceMismatch);
        }
        view.amount(kind)?;
        Ok(())
    }
}

/// A run borrows no separate quota; it refers to its process-root's ledger.
pub struct RunResources {
    root: Arc<ProcessRoot>,
    run_id: u64,
    closed: bool,
}

impl RunResources {
    pub fn close_confirmed(&mut self) -> Result<(), ResourceError> {
        if self.closed {
            return Err(ResourceError::AlreadyReleased);
        }
        let mut ledger = self
            .root
            .ledger
            .lock()
            .map_err(|_| ResourceError::Poisoned)?;
        let index = ledger
            .runs
            .iter()
            .position(|run| run.id == self.run_id)
            .ok_or(ResourceError::UnknownRun)?;
        if ledger.runs[index].used.iter().any(|amount| *amount != 0)
            || ledger
                .reservations
                .iter()
                .any(|record| record.run_id == self.run_id)
        {
            return Err(ResourceError::CleanupUnverified);
        }
        ledger.runs.swap_remove(index);
        // The high-water mark remains: retired IDs never become reusable.
        self.closed = true;
        Ok(())
    }

    pub fn reserve(&self, requests: &[ResourceRequest]) -> Result<Reservation, ResourceError> {
        self.reserve_bound(None, requests)
    }

    /// Bind an exact payload within the SAME run's quota/deadline/cancellation.
    /// This does not replace the parent source, create a new run, or grant action authority.
    pub fn reserve_for_source(
        &self,
        source: Digest32,
        requests: &[ResourceRequest],
    ) -> Result<Reservation, ResourceError> {
        self.reserve_bound(Some(source), requests)
    }

    fn reserve_bound(
        &self,
        source: Option<Digest32>,
        requests: &[ResourceRequest],
    ) -> Result<Reservation, ResourceError> {
        if self.closed {
            return Err(ResourceError::UnknownRun);
        }
        let amounts = checked_requests(requests)?;
        let mut ledger = self
            .root
            .ledger
            .lock()
            .map_err(|_| ResourceError::Poisoned)?;
        let index = ledger
            .runs
            .iter()
            .position(|run| run.id == self.run_id)
            .ok_or(ResourceError::UnknownRun)?;
        let run = &ledger.runs[index];
        let parent_source = run.source;
        let payload_source = source.unwrap_or(parent_source);
        run.cancellation.check()?;
        run.deadline.check()?;
        if ledger.reservations.len() == MAX_RESERVATIONS {
            return Err(ResourceError::AllocationFailed);
        }
        let process_after = add_checked(ledger.used, amounts, self.root.limits)?;
        let run_after = add_checked(run.used, amounts, run.limits)?;
        let id = ledger.next_id;
        let next_id = id.checked_add(1).ok_or(ResourceError::ArithmeticOverflow)?;
        let report = ReportedResourceLease::checked(
            run.id,
            payload_source,
            requests,
            run.deadline,
            run.cancellation.clone(),
        )?;
        // The mutex is the transaction boundary: no reader sees provisional
        // charges or a live lease until all components are ready to publish.
        let lease = self.root.issuer.issue_admitted(id, report)?;
        ledger.used = process_after;
        ledger.runs[index].used = run_after;
        ledger.next_id = next_id;
        ledger.reservations.push(ReservationRecord {
            id,
            run_id: self.run_id,
            parent_source,
            payload_source,
            amounts,
            status: RecordStatus::Active,
        });
        Ok(Reservation {
            root: self.root.clone(),
            run_id: self.run_id,
            id,
            lease,
            released: false,
        })
    }
}

fn add_checked(
    before: [u64; RESOURCE_KINDS],
    amounts: [u64; RESOURCE_KINDS],
    limits: ResourceLimits,
) -> Result<[u64; RESOURCE_KINDS], ResourceError> {
    let mut after = before;
    for kind in ResourceKind::ALL {
        let index = kind.index();
        after[index] = after[index]
            .checked_add(amounts[index])
            .ok_or(ResourceError::ArithmeticOverflow)?;
        if after[index] > limits.0[index] {
            return Err(ResourceError::Exhausted(kind));
        }
    }
    Ok(after)
}

fn subtract_checked(
    before: [u64; RESOURCE_KINDS],
    amounts: [u64; RESOURCE_KINDS],
) -> Result<[u64; RESOURCE_KINDS], ResourceError> {
    let mut after = before;
    for (value, amount) in after.iter_mut().zip(amounts) {
        *value = value
            .checked_sub(amount)
            .ok_or(ResourceError::ArithmeticOverflow)?;
    }
    Ok(after)
}

pub struct Reservation {
    root: Arc<ProcessRoot>,
    run_id: u64,
    id: u64,
    lease: ResourceLease,
    released: bool,
}
impl Reservation {
    pub fn lease(&self) -> ResourceReservationView<'_> {
        self.lease.view()
    }
    pub fn release_confirmed(&mut self) -> Result<(), ResourceError> {
        if self.released {
            return Err(ResourceError::AlreadyReleased);
        }
        let mut ledger = self
            .root
            .ledger
            .lock()
            .map_err(|_| ResourceError::Poisoned)?;
        let record_index = ledger
            .reservations
            .iter()
            .position(|record| record.id == self.id)
            .ok_or(ResourceError::StaleLease)?;
        let record = &ledger.reservations[record_index];
        if record.status != RecordStatus::Active || record.run_id != self.run_id {
            return Err(ResourceError::StaleLease);
        }
        let run_index = ledger
            .runs
            .iter()
            .position(|run| run.id == self.run_id)
            .ok_or(ResourceError::UnknownRun)?;
        let process_after = subtract_checked(ledger.used, record.amounts)?;
        let run_after = subtract_checked(ledger.runs[run_index].used, record.amounts)?;
        // A live borrow blocks caller-side release; the shared state additionally
        // rejects live buffers and unknown cleanup before any counter refund.
        self.lease.close_confirmed()?;
        ledger.used = process_after;
        ledger.runs[run_index].used = run_after;
        // Released accounting slots are reusable, but next_id never rewinds.
        // Retained/unknown entries are not eligible for this removal path.
        ledger.reservations.swap_remove(record_index);
        self.released = true;
        Ok(())
    }
}
impl Drop for Reservation {
    fn drop(&mut self) {
        if self.released {
            return;
        }
        self.lease.view().record_cleanup_unknown();
        if let Ok(mut ledger) = self.root.ledger.lock() {
            if let Some(record) = ledger
                .reservations
                .iter_mut()
                .find(|record| record.id == self.id)
            {
                record.status = RecordStatus::Retained;
            }
        }
        // No refund, filesystem cleanup, child kill, or evidence deletion here.
    }
}

#[cfg(test)]
mod tlcx05_authoritative_run_tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn fixture() -> (ResourceBroker, Reservation) {
        let owner = ResourceBroker::new(ResourceLimits::uniform(8).unwrap()).unwrap();
        let run = owner
            .begin_run(
                1,
                Digest32::from_bytes([1; 32]),
                ResourceLimits::uniform(8).unwrap(),
                Deadline::after(Duration::from_secs(30)).unwrap(),
                Cancellation::default(),
            )
            .unwrap();
        let reservation = run
            .reserve(&[ResourceRequest::new(
                ResourceKind::Output,
                ResourceAmount::checked(2).unwrap(),
            )])
            .unwrap();
        (owner, reservation)
    }

    #[test]
    fn tlcx05_validation_uses_authoritative_run_source_not_only_view_metadata() {
        let (owner, reservation) = fixture();
        owner.root.ledger.lock().unwrap().runs[0].source = Digest32::from_bytes([2; 32]);
        assert_eq!(
            owner
                .validate_lease(
                    &reservation.lease(),
                    1,
                    Digest32::from_bytes([1; 32]),
                    ResourceKind::Output
                )
                .unwrap_err(),
            ResourceError::SourceMismatch
        );
    }

    #[test]
    fn tlcx05_validation_uses_authoritative_run_deadline_not_only_view_metadata() {
        let (owner, reservation) = fixture();
        owner.root.ledger.lock().unwrap().runs[0].deadline =
            Deadline::at(Instant::now() - Duration::from_millis(1));
        assert_eq!(
            owner
                .validate_lease(
                    &reservation.lease(),
                    1,
                    Digest32::from_bytes([1; 32]),
                    ResourceKind::Output
                )
                .unwrap_err(),
            ResourceError::DeadlineExceeded
        );
    }

    #[test]
    fn tlcx05_validation_uses_authoritative_run_cancel_not_only_view_metadata() {
        let (owner, reservation) = fixture();
        let cancelled = Cancellation::default();
        cancelled.cancel();
        owner.root.ledger.lock().unwrap().runs[0].cancellation = cancelled;
        assert_eq!(
            owner
                .validate_lease(
                    &reservation.lease(),
                    1,
                    Digest32::from_bytes([1; 32]),
                    ResourceKind::Output
                )
                .unwrap_err(),
            ResourceError::Cancelled
        );
    }
}
