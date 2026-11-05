//! Producer-owned native attempts; no report admission, fixed permit or signer.
#![forbid(unsafe_code)]
use super::{
    execution::{parse_digest, ProbeToolRegistration},
    measurement::*,
    probe_clock::{sample, ClockHistory},
};
use lc631_analysis::{DgclEvidenceLease, ResourceToolAttempt, ResourceToolResult};
use lc631_core::{
    Action, CallerOrigin, ExecutionPermit, ExecutionPermitContext, PermitValidation,
    PrincipalBinding,
};
use lc631_receipt_kernel::{ReceiptVerifier, UntrustedReceipt};
use lc631_substrate::{
    resource::{ResourceClaim, ResourceReservationView},
    Digest32,
};
use sha2::{Digest, Sha256};
use std::cell::Cell;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProbePurpose {
    Baseline,
    Current,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProbeAuthorityPhase {
    BeforeSpawn,
    AfterChild,
    BeforePublication,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStageEffect {
    NotAttempted,
    MayHaveExecuted,
    ExecutedObserved,
}
pub struct ProbeExpectedOrigin<'a> {
    pub principal: &'a PrincipalBinding,
    pub host_fingerprint: &'a str,
    pub caller_origin: CallerOrigin,
    pub frame: Digest32,
    pub not_before_epoch: u64,
    pub revocation_floor: u64,
}
pub struct ProbeAttemptInput<'a> {
    pub source: &'a str,
    pub registration: ProbeToolRegistration<'a>,
    pub lease: &'a DgclEvidenceLease,
    pub tool_receipt: &'a UntrustedReceipt,
    pub tool_verifier: &'a ReceiptVerifier,
    pub clock: fn() -> u64,
    pub timeout_ms: u64,
    pub origin: ProbeExpectedOrigin<'a>,
}
pub struct ProbeAuthorityRequest<'a> {
    pub purpose: ProbePurpose,
    pub phase: ProbeAuthorityPhase,
    pub frame: Digest32,
    pub execution_scope: &'a str,
    pub source_revision: &'a str,
}
pub struct ProbeAuthorityView<'a> {
    pub permit: &'a ExecutionPermit,
    pub context: ExecutionPermitContext<'a>,
    pub frame: Digest32,
}
pub trait CurrentProbeAuthority {
    fn current(
        &mut self,
        request: ProbeAuthorityRequest<'_>,
    ) -> Result<ProbeAuthorityView<'_>, ProbeError>;
}
pub(super) struct ExpectedProbeIdentity {
    pub(super) principal: PrincipalBinding,
    pub(super) host: String,
    pub(super) caller: CallerOrigin,
    pub(super) not_before: u64,
    pub(super) clock: fn() -> u64,
}
pub fn native_attempt_scope(
    plan: &CodingProbePlan,
    registration: &ProbeToolRegistration<'_>,
    purpose: ProbePurpose,
    frame: Digest32,
) -> String {
    let mut h = Sha256::new();
    h.update(b"Epistesys/TLCX/CodingProbe/AttemptScope/v1\0");
    h.update([match purpose {
        ProbePurpose::Baseline => 0,
        ProbePurpose::Current => 1,
    }]);
    h.update(frame.as_bytes());
    h.update(plan.digest().as_bytes());
    // RegisteredArtifact always creates a canonical digest from bounded fields.
    let raw = parse_digest(&registration.payload_digest(plan))
        .expect("payload_digest constructs canonical SHA256 from its fixed domain");
    h.update(raw.as_bytes());
    let mut scope = String::from("tlcx/coding-probe/attempt/test/v1/");
    for byte in h.finalize() {
        use std::fmt::Write;
        let _ = write!(&mut scope, "{byte:02x}");
    }
    scope
}
pub struct ProbeAttempt<'res> {
    pub(super) process_owner: Option<ResourceToolAttempt<'res, ProbeError>>,
    pub(super) observed: Option<OwnedCodingProbeObservation<'res>>,
    pub(super) diagnostic: Option<ProbeError>,
    pub(super) effect: NativeStageEffect,
    pub(super) clock: Option<ClockHistory>,
    pub(super) registered: Option<String>,
    pub(super) frame: Digest32,
    pub(super) purpose: ProbePurpose,
    pub(super) plan: Option<CodingProbePlan>,
    pub(super) resources: &'res ResourceReservationView<'res>,
    pub(super) funding: Option<ResourceClaim<'res>>,
    pub(super) identity: Option<ExpectedProbeIdentity>,
    pub(super) revocation_floor: Cell<u64>,
    pub(super) authority_floor: Cell<u64>,
}
impl<'res> ProbeAttempt<'res> {
    pub fn effect(&self) -> NativeStageEffect {
        self.effect
    }
    pub fn process(&self) -> Option<&ResourceToolResult<'res>> {
        self.process_owner.as_ref().and_then(|a| a.process())
    }
    pub fn observation(&self) -> Option<&OwnedCodingProbeObservation<'res>> {
        self.observed.as_ref()
    }
    pub fn diagnostic(&self) -> Option<&ProbeError> {
        self.diagnostic.as_ref()
    }
    pub fn current(&self) -> Result<&OwnedCodingProbeObservation<'res>, ProbeError> {
        if let Some(error) = &self.diagnostic {
            return Err(error.clone());
        }
        let observed = self.observed.as_ref().ok_or(ProbeError::ProcessFailed)?;
        observed.check_current(
            self.plan.as_ref().ok_or(ProbeError::InvalidPlan)?,
            self.resources,
        )?;
        Ok(observed)
    }
    pub fn recheck_current(
        &mut self,
        plan: &CodingProbePlan,
        input: &ProbeAttemptInput<'_>,
        purpose: ProbePurpose,
        provider: &mut dyn CurrentProbeAuthority,
        resources: &ResourceReservationView<'_>,
    ) -> Result<(), ProbeError> {
        let result = (|| {
            self.current()?;
            let origin = self.identity.as_ref().ok_or(ProbeError::Unauthorized)?;
            if origin.principal != *input.origin.principal
                || origin.host != input.origin.host_fingerprint
                || origin.caller != input.origin.caller_origin
                || input.origin.not_before_epoch < origin.not_before
                || !std::ptr::fn_addr_eq(origin.clock, input.clock)
            {
                return Err(ProbeError::BindingMismatch);
            }
            if purpose != self.purpose
                || input.origin.frame != self.frame
                || self.registered.as_deref()
                    != Some(input.registration.payload_digest(plan).as_str())
            {
                return Err(ProbeError::BindingMismatch);
            }
            let clock = self.clock.as_ref().ok_or(ProbeError::Stale)?;
            self.current()?.check_current_artifact(plan, resources)?;
            sample_authority(
                plan,
                input,
                purpose,
                ProbeAuthorityPhase::BeforePublication,
                provider,
                clock,
                &self.revocation_floor,
                &self.authority_floor,
                resources,
            )?;
            self.current()?.check_current(plan, resources)
        })();
        if let Err(error) = &result {
            self.diagnostic = Some(error.clone());
        }
        result
    }
}
pub fn observe_current_attempt<'res>(
    plan: &CodingProbePlan,
    input: &ProbeAttemptInput<'_>,
    purpose: ProbePurpose,
    provider: Option<&mut dyn CurrentProbeAuthority>,
    resources: &'res ResourceReservationView<'_>,
) -> ProbeAttempt<'res> {
    let mut a = ProbeAttempt {
        process_owner: None,
        observed: None,
        diagnostic: None,
        effect: NativeStageEffect::NotAttempted,
        clock: None,
        registered: None,
        frame: input.origin.frame,
        purpose,
        plan: None,
        resources,
        funding: None,
        identity: None,
        revocation_floor: Cell::new(input.origin.revocation_floor),
        authority_floor: Cell::new(0),
    };
    let result = super::execution::attempt_producer::produce(plan, input, provider, &mut a);
    if let Err(error) = result {
        a.diagnostic = Some(error);
    }
    a
}
pub(super) fn sample_authority(
    plan: &CodingProbePlan,
    input: &ProbeAttemptInput<'_>,
    purpose: ProbePurpose,
    phase: ProbeAuthorityPhase,
    provider: &mut dyn CurrentProbeAuthority,
    clock: &ClockHistory,
    revocation_floor: &Cell<u64>,
    authority_floor: &Cell<u64>,
    resources: &ResourceReservationView<'_>,
) -> Result<(String, u64), ProbeError> {
    resources.check()?;
    sample(input.clock, clock, input.lease.expires_at_epoch)?;
    let scope = native_attempt_scope(plan, &input.registration, purpose, input.origin.frame);
    let view = provider.current(ProbeAuthorityRequest {
        purpose,
        phase,
        frame: input.origin.frame,
        execution_scope: &scope,
        source_revision: &input.registration.snapshot.source_revision,
    })?;
    resources.check()?;
    let now = sample(input.clock, clock, input.lease.expires_at_epoch)?;
    if now < input.origin.not_before_epoch
        || view.context.now_epoch < input.origin.not_before_epoch
        || view.context.now_epoch > now
        || view.frame != input.origin.frame
        || view.context.revocation_revision
            < revocation_floor.get().max(input.origin.revocation_floor)
        || view.context.authority_revision.0 < authority_floor.get()
        || view.context.principal != input.origin.principal
        || view.permit.principal_binding() != input.origin.principal
        || view.context.trusted_host_fingerprint != input.origin.host_fingerprint
        || view.context.caller_origin != input.origin.caller_origin
        || view.context.action != Action::Test
        || view.context.scope != scope
        || view.context.source_revision != input.registration.snapshot.source_revision
    {
        return Err(ProbeError::Unauthorized);
    }
    let context = ExecutionPermitContext {
        principal: view.context.principal,
        action: Action::Test,
        scope: &scope,
        source_revision: view.context.source_revision,
        authority_revision: view.context.authority_revision,
        now_epoch: now,
        revocation_revision: view.context.revocation_revision,
        revoked_permit_digests: view.context.revoked_permit_digests,
        caller_origin: view.context.caller_origin,
        trusted_host_fingerprint: view.context.trusted_host_fingerprint,
    };
    if view.permit.validate(&context) != PermitValidation::ValidGrant {
        return Err(ProbeError::Unauthorized);
    }
    if phase == ProbeAuthorityPhase::BeforeSpawn {
        let horizon = now
            .checked_add(input.timeout_ms.div_ceil(1000))
            .ok_or(ProbeError::Stale)?;
        let tool_expires = input
            .tool_receipt
            .claims()
            .expires_at_epoch()
            .ok_or(ProbeError::UnregisteredTool)?;
        if horizon >= input.lease.expires_at_epoch || horizon >= tool_expires {
            return Err(ProbeError::Stale);
        }
        let horizon_context = ExecutionPermitContext {
            now_epoch: horizon,
            ..context
        };
        if view.permit.validate(&horizon_context) != PermitValidation::ValidGrant {
            return Err(ProbeError::Unauthorized);
        }
    }
    revocation_floor.set(view.context.revocation_revision);
    authority_floor.set(view.context.authority_revision.0);
    Ok((
        view.permit.digest().to_owned(),
        view.permit.expires_at_epoch(),
    ))
}
