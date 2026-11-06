//! 同じ実 lease に、登録試行・endpoint・role・実読取りの上限を保持する。
//! Profile-local resource and work accounting shared by every role and reader.
use super::{
    registry::RegistryHistory, ClockFault, ClockGap, Error, Protocol, Role, TrustRequirement,
};
use lc631_substrate::{
    resource::{
        ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
        ResourceReservationView,
    },
    Digest32,
};
use std::sync::{Arc, Mutex};

const MAX_METADATA: usize = 4096;
const MAX_RETAINED_RAM: u64 = 64 * 1024;
const PROFILE_RAM: u64 = 8 * 1024;
const ENDPOINT_RAM: u64 = 4 * 1024;
const ROLE_RAM: u64 = 16 * 1024;
const SAMPLE_RAM: u64 = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    pub max_registration_attempts: u32,
    pub max_endpoints: u32,
    pub max_roles: u32,
    pub max_samples: u32,
    pub max_metadata_bytes: usize,
    pub max_added_ram_bytes: u64,
}
impl Limits {
    fn validate(self) -> Result<(), Error> {
        if !(1..=8).contains(&self.max_registration_attempts)
            || !(1..=4).contains(&self.max_endpoints)
            || !(1..=2).contains(&self.max_roles)
            || !(1..=1024).contains(&self.max_samples)
            || !(1..=MAX_METADATA).contains(&self.max_metadata_bytes)
            || self.max_added_ram_bytes == 0
            || self.max_added_ram_bytes > MAX_RETAINED_RAM
        {
            return Err(Error::InputLimit);
        }
        if self.max_added_ram_bytes < PROFILE_RAM {
            return Err(Error::Clock(ClockFault::BudgetExhausted));
        }
        Ok(())
    }
}

pub struct ProfileInput<'a> {
    pub domain: Digest32,
    pub protocol: Protocol,
    pub trusted_registry_fingerprint: &'a str,
    pub requirement: TrustRequirement,
    pub limits: Limits,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClockWork {
    pub registration_attempts: u32,
    pub endpoint_reads: u32,
}

#[derive(Default)]
struct Usage {
    attempts: u32,
    reads: u32,
    live_endpoints: u32,
    roles: u8,
}

/// Opaque process-local profile. Its shared reservation stays alive until the
/// last checked reader drops, including a reader in an existing native gain.
pub struct Profile<'v> {
    pub(super) state: Arc<ProfileState<'v>>,
}
impl<'v> Profile<'v> {
    pub fn given_local(
        input: ProfileInput<'_>,
        view: &'v ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        view.check()
            .map_err(|e| Error::Clock(ClockFault::Resource(e)))?;
        input.limits.validate()?;
        let fingerprint = input.trusted_registry_fingerprint;
        if fingerprint.len() > input.limits.max_metadata_bytes {
            return Err(Error::InputLimit);
        }
        if input.domain == Digest32::from_bytes([0; 32]) {
            return Err(Error::Clock(ClockFault::NeedsEvidence(
                ClockGap::UnknownDomain,
            )));
        }
        if input.protocol != Protocol::EpochSecondsV1 {
            return Err(Error::Clock(ClockFault::NeedsEvidence(
                ClockGap::UnknownProtocol,
            )));
        }
        if input.requirement != TrustRequirement::GivenLocalRegistered {
            return Err(Error::Clock(ClockFault::NeedsEvidence(
                ClockGap::HostAnchorUnavailable,
            )));
        }
        if !super::registry::canonical_digest(fingerprint) {
            return Err(Error::Clock(ClockFault::NeedsEvidence(
                ClockGap::UntrustedIssuer,
            )));
        }
        // One retained claim covers the profile, live candidates/registrations
        // and one binding per role. Transient sample backing is separate.
        let retained = ram(view, input.limits.max_added_ram_bytes).map_err(Error::Clock)?;
        let history = RegistryHistory::new(
            input.limits.max_registration_attempts,
            input.limits.max_endpoints,
        )?;
        let state = ProfileState {
            domain: input.domain,
            fingerprint: fingerprint.to_owned(),
            limits: input.limits,
            registry: Mutex::new(history),
            usage: Mutex::new(Usage::default()),
            retained,
        };
        state.check(view).map_err(Error::Clock)?;
        Ok(Self {
            state: Arc::new(state),
        })
    }

    pub fn work_current(&self, view: &ResourceReservationView<'_>) -> Result<ClockWork, Error> {
        self.check(view).map_err(Error::Clock)?;
        let usage = self
            .state
            .usage
            .lock()
            .map_err(|_| Error::Clock(poisoned()))?;
        let work = ClockWork {
            registration_attempts: usage.attempts,
            endpoint_reads: usage.reads,
        };
        drop(usage);
        self.check(view).map_err(Error::Clock)?;
        Ok(work)
    }

    pub(crate) fn domain(&self) -> Digest32 {
        self.state.domain
    }
    pub(crate) fn protocol(&self) -> Protocol {
        Protocol::EpochSecondsV1
    }
    pub(crate) fn metadata_limit(&self) -> usize {
        self.state.limits.max_metadata_bytes
    }
    pub(crate) fn check(&self, view: &ResourceReservationView<'_>) -> Result<(), ClockFault> {
        self.state.check(view)
    }
    pub(crate) fn reserve_role(
        &self,
        role: Role,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), ClockFault> {
        self.check(view)?;
        let mut usage = self.state.usage.lock().map_err(|_| poisoned())?;
        let bit = match role {
            Role::Baseline => 1,
            Role::Candidate => 2,
        };
        if usage.roles & bit != 0 {
            // Minting a second session for this role would reset its epoch ledger.
            return Err(ClockFault::BindingMismatch);
        }
        let roles = usage.roles | bit;
        if roles.count_ones() > self.state.limits.max_roles
            || !self.state.fits(usage.live_endpoints, roles)
        {
            return Err(ClockFault::BudgetExhausted);
        }
        usage.roles = roles;
        Ok(())
    }
}

pub(super) struct ProfileState<'v> {
    pub(super) domain: Digest32,
    pub(super) fingerprint: String,
    pub(super) limits: Limits,
    pub(super) registry: Mutex<RegistryHistory>,
    usage: Mutex<Usage>,
    retained: ResourceClaim<'v>,
}
impl ProfileState<'_> {
    pub(super) fn check(&self, view: &ResourceReservationView<'_>) -> Result<(), ClockFault> {
        view.check().map_err(ClockFault::Resource)?;
        self.retained.check().map_err(ClockFault::Resource)?;
        if !self.retained.same_reservation(view) {
            return Err(ClockFault::Resource(ResourceError::ForeignOwner));
        }
        Ok(())
    }
    pub(super) fn begin_attempt(&self) -> Result<(), ClockFault> {
        let mut usage = self.usage.lock().map_err(|_| poisoned())?;
        debit(&mut usage.attempts, self.limits.max_registration_attempts)
    }
    pub(super) fn debit_read(&self) -> Result<(), ClockFault> {
        let mut usage = self.usage.lock().map_err(|_| poisoned())?;
        debit(&mut usage.reads, self.limits.max_samples)
    }
    fn fits(&self, endpoints: u32, roles: u8) -> bool {
        // Constructors cap endpoints at four and role bits at two.
        PROFILE_RAM + ENDPOINT_RAM * u64::from(endpoints) + ROLE_RAM * u64::from(roles.count_ones())
            <= self.limits.max_added_ram_bytes
    }
}

pub(super) struct EndpointSlot<'v> {
    profile: Arc<ProfileState<'v>>,
}
impl<'v> EndpointSlot<'v> {
    pub(super) fn claim(profile: &Arc<ProfileState<'v>>) -> Result<Self, ClockFault> {
        let mut usage = profile.usage.lock().map_err(|_| poisoned())?;
        let next = usage
            .live_endpoints
            .checked_add(1)
            .ok_or(ClockFault::BudgetExhausted)?;
        if next > profile.limits.max_endpoints || !profile.fits(next, usage.roles) {
            return Err(ClockFault::BudgetExhausted);
        }
        usage.live_endpoints = next;
        Ok(Self {
            profile: Arc::clone(profile),
        })
    }
}
impl Drop for EndpointSlot<'_> {
    fn drop(&mut self) {
        if let Ok(mut usage) = self.profile.usage.lock() {
            // Exactly one slot exists per admitted occurrence; sampled work and
            // role bindings are never refunded by dropping an actor or reader.
            if let Some(live) = usage.live_endpoints.checked_sub(1) {
                usage.live_endpoints = live;
            }
        }
    }
}

fn debit(used: &mut u32, limit: u32) -> Result<(), ClockFault> {
    if *used >= limit {
        return Err(ClockFault::BudgetExhausted);
    }
    *used += 1; // strictly below a validated limit of at most 1024
    Ok(())
}
fn poisoned() -> ClockFault {
    ClockFault::Resource(ResourceError::Poisoned)
}
fn ram<'v>(
    view: &'v ResourceReservationView<'_>,
    amount: u64,
) -> Result<ResourceClaim<'v>, ClockFault> {
    let bytes = ResourceAmount::checked(amount).map_err(ClockFault::Resource)?;
    view.claim_resources(&[ResourceRequest::new(ResourceKind::Ram, bytes)])
        .map_err(ClockFault::Resource)
}
pub(super) fn sample_scratch<'v>(
    view: &'v ResourceReservationView<'_>,
) -> Result<ResourceClaim<'v>, ClockFault> {
    ram(view, SAMPLE_RAM)
}

pub(super) fn verification_scratch<'v>(
    view: &'v ResourceReservationView<'_>,
) -> Result<ResourceClaim<'v>, ClockFault> {
    ram(view, 16 * 1024)
}
