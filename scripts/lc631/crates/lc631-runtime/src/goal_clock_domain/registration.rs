//! 登録は Given の local epoch 座標に限る。Test 権限や host 時刻を発行しない。
//! Bind actual clock actors to one configured local epoch domain and resource owner.
//! Registration supplies neither a snapshot permission nor a host clock attestation.
#![forbid(unsafe_code)]

mod credential;
mod profile;
mod registry;

use super::{ClockFault, ClockGap, Error, ProviderUnavailable, SnapshotGrantSource};
use credential::{Credential, Identity};
use lc631_receipt_kernel::{ReceiptVerifier, UntrustedReceipt};
use lc631_substrate::{resource::ResourceReservationView, Digest32};
use profile::{EndpointSlot, ProfileState};
use std::sync::Arc;

pub use profile::{ClockWork, Limits, Profile, ProfileInput};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    Baseline,
    Candidate,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Purpose {
    Execution,
    Capture,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Protocol {
    EpochSecondsV1,
    Unknown(Digest32),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrustRequirement {
    GivenLocalRegistered,
    HostAnchored,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EndpointBinding {
    pub role: Role,
    pub purpose: Purpose,
    pub root: Digest32,
    pub source: Digest32,
    pub program: Digest32,
    pub completion_plan: Digest32,
    pub probe_plan: Digest32,
    pub endpoint_revision: Digest32,
}

pub trait ClockEpochSource: Send + Sync {
    fn read_epoch(&self) -> Result<u64, ProviderUnavailable>;
}
pub struct RegistryView<'a> {
    pub verifier: &'a ReceiptVerifier,
    pub now_epoch: u64,
    pub revision: u64,
    pub revocation_revision: u64,
    pub revoked_receipt_digests: &'a [String],
}
pub trait RegistryAuthority: Send + Sync {
    fn with_current(
        &self,
        use_view: &mut dyn FnMut(RegistryView<'_>) -> Result<(), Error>,
    ) -> Result<(), Error>;
}

/// Untrusted actor plus a newly funded, private occurrence identity. There is
/// no constructor from a report, function identity, timestamp or receipt alone.
pub struct EndpointCandidate<'p, 'v> {
    profile: &'p Profile<'v>,
    binding: EndpointBinding,
    actor: Arc<dyn ClockEpochSource>,
    grant: Option<Arc<dyn SnapshotGrantSource>>,
    identity: Identity,
    slot: EndpointSlot<'v>,
}
impl<'p, 'v> EndpointCandidate<'p, 'v> {
    pub fn execution(
        profile: &'p Profile<'v>,
        binding: EndpointBinding,
        actor: Arc<dyn ClockEpochSource>,
        view: &ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        Self::new(profile, binding, Purpose::Execution, actor, None, view)
    }

    pub fn capture(
        profile: &'p Profile<'v>,
        binding: EndpointBinding,
        actor: Arc<dyn ClockEpochSource>,
        grant: Arc<dyn SnapshotGrantSource>,
        view: &ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        Self::new(profile, binding, Purpose::Capture, actor, Some(grant), view)
    }

    fn new(
        profile: &'p Profile<'v>,
        binding: EndpointBinding,
        purpose: Purpose,
        actor: Arc<dyn ClockEpochSource>,
        grant: Option<Arc<dyn SnapshotGrantSource>>,
        view: &ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        profile.check(view).map_err(Error::Clock)?;
        if binding.purpose != purpose
            || binding.source != view.source_revision()
            || [
                binding.root,
                binding.source,
                binding.program,
                binding.completion_plan,
                binding.probe_plan,
                binding.endpoint_revision,
            ]
            .contains(&Digest32::from_bytes([0; 32]))
        {
            return Err(Error::Clock(ClockFault::BindingMismatch));
        }
        let slot = EndpointSlot::claim(&profile.state).map_err(Error::Clock)?;
        let identity = Identity::new(profile, binding, view)?;
        profile.check(view).map_err(Error::Clock)?;
        Ok(Self {
            profile,
            binding,
            actor,
            grant,
            identity,
            slot,
        })
    }

    pub fn registration_subject(&self) -> String {
        self.identity.subject().to_owned()
    }
    pub fn registration_scope(&self) -> String {
        self.identity.scope().to_owned()
    }
    pub fn registration_payload(&self) -> String {
        self.identity.payload().to_owned()
    }
}

/// Register the exact candidate with the configured ToolExecution verifier.
/// The private reader may subsequently be borrowed by native owners and gains.
pub fn register<'p, 'v>(
    profile: &'p Profile<'v>,
    candidate: EndpointCandidate<'p, 'v>,
    receipt: UntrustedReceipt,
    registry: Arc<dyn RegistryAuthority>,
    view: &ResourceReservationView<'_>,
) -> Result<RegisteredEndpoint<'p, 'v>, Error> {
    profile.check(view).map_err(Error::Clock)?;
    profile.state.begin_attempt().map_err(Error::Clock)?;
    if !std::ptr::eq(profile, candidate.profile) {
        return Err(Error::Clock(ClockFault::BindingMismatch));
    }
    let _scratch = profile::verification_scratch(view).map_err(Error::Clock)?;
    let credential = Credential::admit(
        &profile.state,
        &candidate.identity,
        receipt,
        registry.as_ref(),
    )?;
    profile.check(view).map_err(Error::Clock)?;
    let core = Arc::new(RegisteredActor {
        profile: Arc::clone(&profile.state),
        binding: candidate.binding,
        actor: candidate.actor,
        registry,
        credential,
        _slot: candidate.slot,
    });
    Ok(RegisteredEndpoint {
        profile,
        reader: EndpointReader(core),
        grant: candidate.grant,
    })
}

pub struct RegisteredEndpoint<'p, 'v> {
    profile: &'p Profile<'v>,
    reader: EndpointReader<'v>,
    // A native execution reader cannot retain the capture-side Test provider.
    grant: Option<Arc<dyn SnapshotGrantSource>>,
}
impl<'p, 'v> RegisteredEndpoint<'p, 'v> {
    pub fn check_registration_current(
        &self,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), Error> {
        self.reader.check_current(view).map_err(Error::Clock)
    }
    pub(super) fn profile(&self) -> &'p Profile<'v> {
        self.profile
    }
    pub(super) fn binding(&self) -> &EndpointBinding {
        self.reader.binding()
    }
    pub(super) fn snapshot_grant(&self) -> Option<&dyn SnapshotGrantSource> {
        self.grant.as_deref()
    }
    pub(super) fn reader(&self) -> EndpointReader<'v> {
        self.reader.clone()
    }
}

struct RegisteredActor<'v> {
    profile: Arc<ProfileState<'v>>,
    binding: EndpointBinding,
    actor: Arc<dyn ClockEpochSource>,
    registry: Arc<dyn RegistryAuthority>,
    credential: Credential,
    // Held by all private sampler/gain handles to this occurrence.
    _slot: EndpointSlot<'v>,
}

#[derive(Clone)]
pub(crate) struct EndpointReader<'v>(Arc<RegisteredActor<'v>>);
impl EndpointReader<'_> {
    pub(crate) fn binding(&self) -> &EndpointBinding {
        &self.0.binding
    }
    pub(crate) fn same_profile(&self, other: &EndpointReader<'_>) -> bool {
        Arc::ptr_eq(&self.0.profile, &other.0.profile)
    }
    pub(crate) fn check_profile_current(
        &self,
        profile: &Profile<'_>,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), ClockFault> {
        profile.check(view)?;
        if self.0.profile.domain != profile.domain() {
            return Err(ClockFault::NeedsEvidence(ClockGap::IncomparableDomains));
        }
        if !Arc::ptr_eq(&self.0.profile, &profile.state) {
            return Err(ClockFault::BindingMismatch);
        }
        self.check_current(view)
    }
    fn check_current(&self, view: &ResourceReservationView<'_>) -> Result<(), ClockFault> {
        self.0.profile.check(view)?;
        let _scratch = profile::verification_scratch(view)?;
        self.check_funded(view)
    }
    fn check_funded(&self, view: &ResourceReservationView<'_>) -> Result<(), ClockFault> {
        self.0.profile.check(view)?;
        self.0
            .credential
            .check(&self.0.profile, self.0.registry.as_ref())?;
        self.0.profile.check(view)
    }

    /// The observer receives every successful raw read before later checks can
    /// refuse publication. A failed provider call still consumes one read.
    pub(crate) fn sample(
        &self,
        view: &ResourceReservationView<'_>,
        observe: impl FnOnce(u64) -> Result<(), ClockFault>,
    ) -> Result<u64, ClockFault> {
        self.0.profile.check(view)?;
        let _scratch = profile::sample_scratch(view)?;
        self.check_funded(view)?;
        self.0.profile.debit_read()?;
        let observed = match self.0.actor.read_epoch() {
            Ok(epoch) => observe(epoch).map(|()| epoch),
            Err(_) => Err(ClockFault::NeedsEvidence(ClockGap::ProviderUnavailable)),
        };
        let current = self.check_funded(view);
        let epoch = observed?;
        current?;
        Ok(epoch)
    }
}
