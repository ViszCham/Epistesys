//! Operational accounting contracts, not Authority, authentication or OS limits.
use crate::Digest32;
use std::num::NonZeroU64;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const RESOURCE_KINDS: usize = 6;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceKind {
    Ram,
    Disk,
    Output,
    Proof,
    Child,
    Cleanup,
}

impl ResourceKind {
    pub const ALL: [Self; RESOURCE_KINDS] = [
        Self::Ram,
        Self::Disk,
        Self::Output,
        Self::Proof,
        Self::Child,
        Self::Cleanup,
    ];
    pub const fn index(self) -> usize {
        match self {
            Self::Ram => 0,
            Self::Disk => 1,
            Self::Output => 2,
            Self::Proof => 3,
            Self::Child => 4,
            Self::Cleanup => 5,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResourceError {
    InvalidAmount,
    InvalidDeadline,
    InvalidConfig,
    EmptyRequest,
    DuplicateResource(ResourceKind),
    Unreserved(ResourceKind),
    NotByteResource(ResourceKind),
    Exhausted(ResourceKind),
    ArithmeticOverflow,
    Cancelled,
    DeadlineExceeded,
    RunIdReused,
    ForeignOwner,
    SourceMismatch,
    UnknownRun,
    StaleLease,
    CleanupUnverified,
    AlreadyReleased,
    LiveBuffer,
    AllocationFailed,
    Poisoned,
    Io(std::io::ErrorKind),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceAmount(NonZeroU64);
impl ResourceAmount {
    pub fn checked(value: u64) -> Result<Self, ResourceError> {
        NonZeroU64::new(value)
            .map(Self)
            .ok_or(ResourceError::InvalidAmount)
    }
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceRequest {
    kind: ResourceKind,
    amount: ResourceAmount,
}
impl ResourceRequest {
    pub const fn new(kind: ResourceKind, amount: ResourceAmount) -> Self {
        Self { kind, amount }
    }
    pub const fn kind(self) -> ResourceKind {
        self.kind
    }
    pub const fn amount(self) -> ResourceAmount {
        self.amount
    }
}

#[derive(Clone, Default, Debug)]
pub struct Cancellation(Arc<AtomicBool>);
impl Cancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
    pub fn check(&self) -> Result<(), ResourceError> {
        if self.is_cancelled() {
            Err(ResourceError::Cancelled)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Deadline(Instant);
impl Deadline {
    pub fn after(duration: Duration) -> Result<Self, ResourceError> {
        if duration.is_zero() {
            return Err(ResourceError::InvalidDeadline);
        }
        Instant::now()
            .checked_add(duration)
            .map(Self)
            .ok_or(ResourceError::InvalidDeadline)
    }
    pub const fn at(instant: Instant) -> Self {
        Self(instant)
    }
    pub fn check(self) -> Result<(), ResourceError> {
        if Instant::now() >= self.0 {
            Err(ResourceError::DeadlineExceeded)
        } else {
            Ok(())
        }
    }
    pub fn remaining(self) -> Result<Duration, ResourceError> {
        self.0
            .checked_duration_since(Instant::now())
            .filter(|value| !value.is_zero())
            .ok_or(ResourceError::DeadlineExceeded)
    }
}

/// Checked report metadata only. No conversion from this report to a live view.
/// No serde implementation is provided for reports, issuers, leases or views.
#[derive(Clone, Debug)]
pub struct ReportedResourceLease {
    run_id: u64,
    source: Digest32,
    amounts: [u64; RESOURCE_KINDS],
    deadline: Deadline,
    cancellation: Cancellation,
}
impl ReportedResourceLease {
    pub fn checked(
        run_id: u64,
        source: Digest32,
        requests: &[ResourceRequest],
        deadline: Deadline,
        cancellation: Cancellation,
    ) -> Result<Self, ResourceError> {
        if run_id == 0 {
            return Err(ResourceError::InvalidConfig);
        }
        let amounts = checked_requests(requests)?;
        Ok(Self {
            run_id,
            source,
            amounts,
            deadline,
            cancellation,
        })
    }
    pub fn run_id(&self) -> u64 {
        self.run_id
    }
    pub fn source_revision(&self) -> Digest32 {
        self.source
    }
    pub fn amounts(&self) -> &[u64; RESOURCE_KINDS] {
        &self.amounts
    }
}

pub fn checked_requests(
    requests: &[ResourceRequest],
) -> Result<[u64; RESOURCE_KINDS], ResourceError> {
    if requests.is_empty() {
        return Err(ResourceError::EmptyRequest);
    }
    let mut amounts = [0; RESOURCE_KINDS];
    for request in requests {
        let value = &mut amounts[request.kind.index()];
        if *value != 0 {
            return Err(ResourceError::DuplicateResource(request.kind));
        }
        *value = request.amount.get();
    }
    Ok(amounts)
}

static NEXT_OWNER: AtomicU64 = AtomicU64::new(1);

/// Root-identity factory held privately by a runtime process-root broker. It
/// owns no quota. Constructing another issuer creates another accounting root;
/// this is not an OS-global limit or a cryptographic security capability.
pub struct ResourceLeaseIssuer {
    owner_id: NonZeroU64,
}
impl ResourceLeaseIssuer {
    pub fn new() -> Result<Self, ResourceError> {
        let id = NEXT_OWNER
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .map_err(|_| ResourceError::ArithmeticOverflow)?;
        let owner_id = NonZeroU64::new(id).ok_or(ResourceError::ArithmeticOverflow)?;
        Ok(Self { owner_id })
    }
    pub fn owner_id(&self) -> u64 {
        self.owner_id.get()
    }
    /// Call only after the owning broker atomically reserves process/run quota.
    pub fn issue_admitted(
        &self,
        reservation_id: u64,
        report: ReportedResourceLease,
    ) -> Result<ResourceLease, ResourceError> {
        if reservation_id == 0 {
            return Err(ResourceError::InvalidConfig);
        }
        report.cancellation.check()?;
        report.deadline.check()?;
        Ok(ResourceLease {
            owner_id: self.owner_id,
            reservation_id,
            report,
            usage: Mutex::new(LeaseUsage {
                active: true,
                live: [0; RESOURCE_KINDS],
                cleanup_unknown: false,
            }),
        })
    }
}

struct LeaseUsage {
    active: bool,
    live: [u64; RESOURCE_KINDS],
    cleanup_unknown: bool,
}

/// Active per-reservation state; quota accounting remains in the runtime owner.
pub struct ResourceLease {
    owner_id: NonZeroU64,
    reservation_id: u64,
    report: ReportedResourceLease,
    usage: Mutex<LeaseUsage>,
}
impl ResourceLease {
    pub fn view(&self) -> ResourceReservationView<'_> {
        ResourceReservationView { lease: self }
    }
    pub fn owner_id(&self) -> u64 {
        self.owner_id.get()
    }
    pub fn reservation_id(&self) -> u64 {
        self.reservation_id
    }
    /// Deactivate before the broker refunds its counters. Live buffers prevent
    /// closure; unknown cleanup cannot be turned into confirmed reclamation.
    pub fn close_confirmed(&self) -> Result<(), ResourceError> {
        let mut state = self.usage.lock().map_err(|_| ResourceError::Poisoned)?;
        if !state.active {
            return Err(ResourceError::AlreadyReleased);
        }
        if state.cleanup_unknown {
            return Err(ResourceError::CleanupUnverified);
        }
        if state.live.iter().any(|amount| *amount != 0) {
            return Err(ResourceError::LiveBuffer);
        }
        state.active = false;
        Ok(())
    }
}

/// Non-Clone, non-Serialize borrowed operational view. Reports are not accepted
/// by readers. Re-fetching a view shares its reservation's SAME live counters.
pub struct ResourceReservationView<'a> {
    lease: &'a ResourceLease,
}
impl ResourceReservationView<'_> {
    pub fn owner_id(&self) -> u64 {
        self.lease.owner_id()
    }
    pub fn reservation_id(&self) -> u64 {
        self.lease.reservation_id()
    }
    pub fn run_id(&self) -> u64 {
        self.lease.report.run_id
    }
    pub fn source_revision(&self) -> Digest32 {
        self.lease.report.source
    }
    pub fn check(&self) -> Result<(), ResourceError> {
        self.lease.report.cancellation.check()?;
        self.lease.report.deadline.check()?;
        let state = self
            .lease
            .usage
            .lock()
            .map_err(|_| ResourceError::Poisoned)?;
        if !state.active {
            return Err(ResourceError::StaleLease);
        }
        if state.cleanup_unknown {
            return Err(ResourceError::CleanupUnverified);
        }
        Ok(())
    }
    pub fn amount(&self, kind: ResourceKind) -> Result<u64, ResourceError> {
        self.check()?;
        let amount = self.lease.report.amounts[kind.index()];
        if amount == 0 {
            Err(ResourceError::Unreserved(kind))
        } else {
            Ok(amount)
        }
    }
    pub fn record_cleanup_unknown(&self) {
        // Poisoned state is already fail-closed in check/close. Never recover it
        // with a guessed zero charge or a successful reclamation observation.
        if let Ok(mut state) = self.lease.usage.lock() {
            state.cleanup_unknown = true;
        }
    }
    pub fn remaining(&self) -> Result<Duration, ResourceError> {
        self.check()?;
        self.lease.report.deadline.remaining()
    }

    pub fn claim_resources(
        &self,
        requests: &[ResourceRequest],
    ) -> Result<ResourceClaim<'_>, ResourceError> {
        self.claim_charges(checked_requests(requests)?)
    }

    pub fn claim_buffer(
        &self,
        kind: ResourceKind,
    ) -> Result<ResourceBufferClaim<'_>, ResourceError> {
        if matches!(kind, ResourceKind::Child | ResourceKind::Cleanup) {
            return Err(ResourceError::NotByteResource(kind));
        }
        let amount = self.amount(kind)?;
        let mut charge = [0; RESOURCE_KINDS];
        charge[ResourceKind::Ram.index()] = amount;
        charge[kind.index()] = amount;
        let claim = self.claim_charges(charge)?;
        Ok(ResourceBufferClaim {
            claim,
            limit: amount,
        })
    }

    fn claim_charges(
        &self,
        charge: [u64; RESOURCE_KINDS],
    ) -> Result<ResourceClaim<'_>, ResourceError> {
        self.check()?;
        let mut state = self
            .lease
            .usage
            .lock()
            .map_err(|_| ResourceError::Poisoned)?;
        if !state.active {
            return Err(ResourceError::StaleLease);
        }
        if state.cleanup_unknown {
            return Err(ResourceError::CleanupUnverified);
        }
        self.lease.report.cancellation.check()?;
        self.lease.report.deadline.check()?;
        let mut next = state.live;
        for resource in ResourceKind::ALL {
            let index = resource.index();
            if charge[index] == 0 {
                continue;
            }
            if self.lease.report.amounts[index] == 0 {
                return Err(ResourceError::Unreserved(resource));
            }
            next[index] = next[index]
                .checked_add(charge[index])
                .ok_or(ResourceError::ArithmeticOverflow)?;
            if next[index] > self.lease.report.amounts[index] {
                return Err(ResourceError::Exhausted(resource));
            }
        }
        state.live = next;
        Ok(ResourceClaim {
            lease: self.lease,
            charge,
        })
    }
}

/// A scoped live claim is not a broker refund or an execution permission.
pub struct ResourceClaim<'a> {
    lease: &'a ResourceLease,
    charge: [u64; RESOURCE_KINDS],
}
impl ResourceClaim<'_> {
    /// Process-local lease identity only; does not prove broker provenance.
    pub fn same_reservation(&self, view: &ResourceReservationView<'_>) -> bool {
        std::ptr::eq(self.lease, view.lease)
    }

    pub fn check(&self) -> Result<(), ResourceError> {
        self.lease.view().check()
    }
    pub fn record_cleanup_unknown(&self) {
        self.lease.view().record_cleanup_unknown();
    }
}
impl Drop for ResourceClaim<'_> {
    fn drop(&mut self) {
        if let Ok(mut state) = self.lease.usage.lock() {
            let mut next = state.live;
            for (amount, charge) in next.iter_mut().zip(self.charge) {
                let Some(remaining) = amount.checked_sub(charge) else {
                    state.cleanup_unknown = true;
                    return;
                };
                *amount = remaining;
            }
            state.live = next;
        }
    }
}

/// Holds live RAM and purpose charge until the owned buffer has been dropped.
/// Returning this local claim never refunds the owning broker's reservation.
pub struct ResourceBufferClaim<'a> {
    claim: ResourceClaim<'a>,
    limit: u64,
}
impl ResourceBufferClaim<'_> {
    pub fn limit(&self) -> u64 {
        self.limit
    }
    pub fn check(&self) -> Result<(), ResourceError> {
        self.claim.check()
    }
    pub fn record_cleanup_unknown(&self) {
        self.claim.record_cleanup_unknown();
    }
}
