//! Goal-only bounded admission and shared owner-clock floor. No Grant or raw
//! epoch setter: every recorded epoch is sampled here or by the actual owner.
#![forbid(unsafe_code)]
use super::*;
use std::marker::PhantomData;

const SCRATCH_RAM: u64 = 64 * 1024;
const SOURCE_CAP: usize = 256 * 1024;

/// Distinguish directly observed expiry from legacy Stale causes (lease/tool/
/// lifecycle). Consumers must not classify every Probe(Stale) as Expired.
pub(crate) enum GoalEpochError {
    ObservedExpiry,
    Probe(ProbeError),
}
impl From<ProbeError> for GoalEpochError {
    fn from(error: ProbeError) -> Self {
        Self::Probe(error)
    }
}

/// Borrowed from the checked gate; not clonable, serialized or independently
/// constructible. The number is a current comparison view, never authority.
pub(crate) struct BoundGoalEpoch<'gate> {
    epoch: u64,
    _gate: PhantomData<&'gate ()>,
}
impl BoundGoalEpoch<'_> {
    pub(crate) fn epoch(&self) -> u64 {
        self.epoch
    }
}

/// No owned payload/claim escapes admission. Immutable borrows keep exact
/// metadata fixed; resource/permission checks are repeated at every sample.
pub(crate) struct GoalInvocationGate<'a, 'obs, 'lease> {
    plan: &'a CodingProbePlan,
    owner: &'a OwnedCodingProbeObservation<'obs>,
    input: ProbeInputs<'a>,
    direct: DirectClock,
    resources: &'a ResourceReservationView<'lease>,
}

#[derive(Clone, Copy)]
enum DirectClock {
    Legacy(fn() -> u64),
    Registered,
}
impl<'a, 'obs, 'lease> GoalInvocationGate<'a, 'obs, 'lease> {
    pub(crate) fn admit(
        plan: &'a CodingProbePlan,
        owner: &'a OwnedCodingProbeObservation<'obs>,
        input: &'a ProbeInvocation<'a>,
        resources: &'a ResourceReservationView<'lease>,
    ) -> Result<Self, ProbeError> {
        if !owner.clock.is_legacy() {
            return Err(ProbeError::ClockDomain(crate::goal_clock_domain::ClockFault::BindingMismatch));
        }
        Self::admit_with(plan, owner, ProbeInputs::without_clock(input), DirectClock::Legacy(input.clock), resources)
    }

    pub(crate) fn admit_registered(
        plan: &'a CodingProbePlan,
        owner: &'a OwnedCodingProbeObservation<'obs>,
        input: ProbeInputs<'a>,
        resources: &'a ResourceReservationView<'lease>,
    ) -> Result<Self, ProbeError> {
        if owner.clock.is_legacy() {
            return Err(ProbeError::ClockDomain(crate::goal_clock_domain::ClockFault::NeedsEvidence(
                crate::goal_clock_domain::ClockGap::LegacyClockProvenanceUnassessed,
            )));
        }
        Self::admit_with(plan, owner, input, DirectClock::Registered, resources)
    }

    fn admit_with(
        plan: &'a CodingProbePlan,
        owner: &'a OwnedCodingProbeObservation<'obs>,
        input: ProbeInputs<'a>,
        direct: DirectClock,
        resources: &'a ResourceReservationView<'lease>,
    ) -> Result<Self, ProbeError> {
        resources.check()?;
        bounds(input)?;
        let gate = Self { plan, owner, input, direct, resources };
        gate.binding_current()?;
        let scratch = super::super::measurement::claim(resources, SCRATCH_RAM)?;
        let registration = input.registration;
        let target = registration
            .snapshot
            .targets
            .iter()
            .find(|target| target.target_ref == registration.target_ref)
            .ok_or(ProbeError::BindingMismatch)?;
        if owner.snapshot != parse_digest(&registration.snapshot.snapshot_digest)?
            || owner.source != parse_digest(&registration.snapshot.source_revision)?
            || owner.build_input != parse_digest(&registration.snapshot.build_input_digest)?
            || owner.target != parse_digest(&target.target_digest)?
            || owner.artifact.target_path.as_path() != registration.target_path
            || owner.artifact.program.as_path() != Path::new(registration.program)
            || owner.artifact.binary != parse_digest(registration.executable_digest)?
            || digest(input.source.as_bytes()) != owner.source
        {
            return Err(ProbeError::BindingMismatch);
        }
        // Do not call owner.check_current here: it would read the hidden clock
        // before the consumer's first direct sample has entered the same Arc.
        check_current_at(plan, input, input.authority.now_epoch)?;
        gate.binding_current()?;
        scratch.check()?;
        Ok(gate)
    }

    fn binding_current(&self) -> Result<(), ProbeError> {
        self.resources.check()?;
        self.owner.retained.check()?;
        if !self.owner.retained.same_reservation(self.resources)
            || self.owner.source != self.resources.source_revision()
            || self.owner.plan != self.plan.digest()
        {
            return Err(ProbeError::BindingMismatch);
        }
        Ok(())
    }

    /// The sole direct sampling site. Record even an expiry/rollback failure;
    /// do not put provided context epochs or a future horizon into this ledger.
    fn sample(&self) -> Result<u64, GoalEpochError> {
        let now = match self.direct {
            DirectClock::Legacy(clock) => {
                let now = clock();
                match self.owner.observed_epoch.observe(now, self.owner.expires_epoch) {
                    Ok(()) => now,
                    Err(ProbeError::Stale) if now >= self.owner.expires_epoch => {
                        return Err(GoalEpochError::ObservedExpiry);
                    }
                    Err(error) => return Err(error.into()),
                }
            }
            DirectClock::Registered => self.owner.clock.sample(
                &self.owner.observed_epoch, self.owner.expires_epoch, self.resources,
            )?,
        };
        if now < self.owner.checked_epoch || now < self.input.authority.now_epoch {
            return Err(ProbeError::ClockRollback.into());
        }
        check_current_at(self.plan, self.input, now)?;
        self.binding_current()?;
        Ok(now)
    }

    /// At most two direct input samples and one actual hidden-owner sample.
    /// The second direct sample prevents returning200 after hidden250. No retry
    /// loop, public scalar admission, or hidden sample after the returned epoch.
    pub(crate) fn sample_current_epoch(&self) -> Result<BoundGoalEpoch<'_>, GoalEpochError> {
        self.binding_current()?;
        let scratch = super::super::measurement::claim(self.resources, SCRATCH_RAM)?;
        check_current_at(self.plan, self.input, self.input.authority.now_epoch)?;
        let before = self.sample()?;
        self.owner.check_current(self.plan, self.resources)?;
        let after = self.sample()?;
        if after < before {
            return Err(ProbeError::ClockRollback.into());
        }
        self.binding_current()?;
        scratch.check().map_err(ProbeError::Resource)?;
        Ok(BoundGoalEpoch {
            epoch: after,
            _gate: PhantomData,
        })
    }

    /// Real target/native checks twice, each with the original owner's own
    /// pre/post current checks and funded bounded hashes. Preserve typed sample
    /// expiry here too; mapping it to generic Stale would lose its exact cause.
    pub(crate) fn check_artifact_current(&self) -> Result<(), GoalEpochError> {
        self.sample_current_epoch()?;
        self.owner
            .check_current_artifact(self.plan, self.resources)?;
        self.sample_current_epoch()?;
        self.owner
            .check_current_artifact(self.plan, self.resources)?;
        self.sample_current_epoch()?;
        self.owner.check_current(self.plan, self.resources)?;
        self.binding_current()?;
        Ok(())
    }
}

fn text(value: &str, cap: usize) -> Result<(), ProbeError> {
    if value.len() > cap {
        return Err(ProbeError::InvalidPlan);
    }
    Ok(())
}
pub(super) fn bounds(input: ProbeInputs<'_>) -> Result<(), ProbeError> {
    // Length/count guards precede any hash, clone, allocation or physical I/O.
    text(input.source, SOURCE_CAP)?;
    let r = &input.registration;
    text(r.program, 4096)?;
    text(r.target_ref, 1024)?;
    text(r.executable_digest, 71)?;
    if r.target_path.as_os_str().len() > 4096 {
        return Err(ProbeError::InvalidPlan);
    }
    text(r.target_path.to_str().ok_or(ProbeError::InvalidPlan)?, 4096)?;
    if !Path::new(r.program).is_absolute()
        || !r.target_path.is_absolute()
        || input.timeout_ms == 0
        || input.timeout_ms > 30_000
    {
        return Err(ProbeError::InvalidPlan);
    }
    text(input.permit.scope(), 4096)?;
    text(input.permit.digest(), 71)?;
    input
        .tool_receipt
        .check_size_bounds()
        .map_err(|_| ProbeError::InvalidPlan)?;
    let a = input.authority;
    text(a.principal.as_str(), 256)?;
    text(a.scope, 4096)?;
    text(a.source_revision, 71)?;
    text(a.trusted_host_fingerprint, 256)?;
    if a.revoked_permit_digests.len() > 256 {
        return Err(ProbeError::InvalidPlan);
    }
    for value in a.revoked_permit_digests {
        text(value, 71)?;
        parse_digest(value)?;
    }
    let s = r.snapshot;
    text(&s.schema_version, 128)?;
    text(&s.observed_boundary, 1024)?;
    for value in [
        &s.source_revision,
        &s.build_input_digest,
        &s.profile_digest,
        &s.validator_digest,
        &s.authority_revision_digest,
        &s.targets_digest,
        &s.snapshot_digest,
    ] {
        text(value, 71)?;
    }
    if s.targets.len() > 64 {
        return Err(ProbeError::InvalidPlan);
    }
    let mut total = 0usize;
    for target in &s.targets {
        text(&target.target_ref, 1024)?;
        text(&target.target_digest, 71)?;
        total = total
            .checked_add(target.target_ref.len())
            .and_then(|n| n.checked_add(target.target_digest.len()))
            .ok_or(ProbeError::InvalidPlan)?;
        if total > 8192 {
            return Err(ProbeError::InvalidPlan);
        }
    }
    let lease = input.lease;
    text(&lease.schema_version, 128)?;
    for value in [
        &lease.snapshot_digest,
        &lease.source_revision,
        &lease.build_input_digest,
        &lease.profile_digest,
        &lease.validator_digest,
        &lease.authority_revision_digest,
        &lease.targets_digest,
        &lease.lease_digest,
    ] {
        text(value, 71)?;
    }
    Ok(())
}
