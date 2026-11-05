//! Child of execution: the actual producer owns private artifacts and captures.
#![forbid(unsafe_code)]
use super::*;
use crate::contextual_repair::{
    attempt::*,
    probe_clock::{sample, shared},
};
use lc631_analysis::{run_bounded_tool_attempt, NativeProcessEffect};

pub(in crate::contextual_repair) fn produce<'res>(
    plan: &CodingProbePlan,
    input: &ProbeAttemptInput<'_>,
    provider: Option<&mut dyn CurrentProbeAuthority>,
    a: &mut ProbeAttempt<'res>,
) -> Result<(), ProbeError> {
    let resources = a.resources;
    resources.check()?;
    let provider = provider.ok_or(ProbeError::Unauthorized)?;
    if input.timeout_ms == 0
        || input.timeout_ms > 30000
        || input.registration.program.len() > 4096
        || input.registration.target_ref.len() > 1024
        || !Path::new(input.registration.program).is_absolute()
        || !input.registration.target_path.is_absolute()
        || input.origin.frame == Digest32::from_bytes([0; 32])
        || digest(input.source.as_bytes()) != resources.source_revision()
        || stable_sha256(input.source) != input.registration.snapshot.source_revision
    {
        return Err(ProbeError::BindingMismatch);
    }
    a.funding = Some(super::super::measurement::claim(resources, 32768)?);
    parse_digest(input.origin.host_fingerprint)?;
    a.identity = Some(ExpectedProbeIdentity {
        principal: input.origin.principal.clone(),
        host: input.origin.host_fingerprint.to_owned(),
        caller: input.origin.caller_origin,
        not_before: input.origin.not_before_epoch,
        clock: input.clock,
    });
    let retained = super::super::measurement::claim(resources, 32768)?;
    let clock = shared(input.clock, input.origin.not_before_epoch)?;
    a.clock = Some(clock.clone());
    a.plan = Some(CodingProbePlan::checked_add(plan.goal(), plan.policy())?);
    a.registered = Some(input.registration.payload_digest(plan));
    let now = sample(input.clock, &clock, input.lease.expires_at_epoch)?;
    registration_current(plan, input, now)?;
    let target = input
        .registration
        .snapshot
        .targets
        .iter()
        .find(|t| t.target_ref == input.registration.target_ref)
        .ok_or(ProbeError::BindingMismatch)?;
    let target_digest = parse_digest(&target.target_digest)?;
    let artifact = ProbeArtifactWitness {
        target_path: owned_path(input.registration.target_path)?,
        program: owned_path(Path::new(input.registration.program))?,
        binary: parse_digest(input.registration.executable_digest)?,
    };
    artifact.check(target_digest, resources)?;
    let mut stamp = None;
    let purpose = a.purpose;
    let revocation_floor = &a.revocation_floor;
    let authority_floor = &a.authority_floor;
    let mut admit = || {
        // This callback is transported to the actual supervisor boundary AFTER
        // nested executable provenance and PreparedIoBuffers validation.
        resources.check()?;
        artifact.check(target_digest, resources)?;
        let epoch = sample(input.clock, &clock, input.lease.expires_at_epoch)?;
        registration_current(
            plan,
            input,
            epoch
                .checked_add(input.timeout_ms.div_ceil(1000))
                .ok_or(ProbeError::Stale)?,
        )?;
        let (permit, expires) = sample_authority(
            plan,
            input,
            purpose,
            ProbeAuthorityPhase::BeforeSpawn,
            provider,
            &clock,
            revocation_floor,
            authority_floor,
            resources,
        )?;
        let epoch = sample(
            input.clock,
            &clock,
            expires.min(input.lease.expires_at_epoch),
        )?;
        stamp = Some((permit, expires, epoch));
        Ok(())
    };
    let process = run_bounded_tool_attempt(
        BoundedToolRequest {
            program: input.registration.program,
            args: &[],
            stdin: Some(PROBE_INPUT),
            timeout_ms: input.timeout_ms,
            stdout_limit: 256,
            stderr_limit: 1024,
        },
        resources,
        &mut admit,
    );
    a.effect = match process.effect() {
        NativeProcessEffect::NotAttempted => NativeStageEffect::NotAttempted,
        NativeProcessEffect::MayHaveStarted => NativeStageEffect::MayHaveExecuted,
        NativeProcessEffect::Completed => NativeStageEffect::ExecutedObserved,
    };
    // Move the actual lower attempt first. Every later ? retains its complete
    // capture, partial child/pipes/cleanup and resource claims in this owner.
    a.process_owner = Some(process);
    let owner = a.process_owner.as_ref().ok_or(ProbeError::ProcessFailed)?;
    if let Some(error) = owner.admission_error() {
        return Err(error.clone());
    }
    if let Some(error) = owner.resource_error() {
        return Err(ProbeError::Resource(error.clone()));
    }
    let capture = owner.process().ok_or(ProbeError::ProcessFailed)?;
    let report = capture.reported();
    if report.state != ToolRunState::Observed
        || report.exit_code != Some(0)
        || report.timed_out
        || report.stdout_truncated
        || report.stderr_truncated
        || !report.process_tree_kill_verified
        || report.executable_sha256.as_deref() != Some(input.registration.executable_digest)
        || report.args_digest != stable_sha256("")
        || !capture.stderr().is_empty()
    {
        return Err(ProbeError::ProcessFailed);
    }
    let (outputs, correct) = super::super::measurement::parse_output(capture.stdout())?;
    let (permit, expires, epoch) = stamp.ok_or(ProbeError::Unauthorized)?;
    let expires = expires.min(input.lease.expires_at_epoch).min(
        input
            .tool_receipt
            .claims()
            .expires_at_epoch()
            .ok_or(ProbeError::UnregisteredTool)?,
    );
    let mut h = Sha256::new();
    h.update(b"Epistesys/TLCX/CodingProbe/ActualObservation/v1\0");
    h.update(plan.digest().as_bytes());
    h.update(input.registration.payload_digest(plan).as_bytes());
    h.update(permit.as_bytes());
    h.update(report.stdout_digest.as_bytes());
    h.update(report.cwd_digest.as_bytes());
    h.update(report.environment_digest.as_bytes());
    a.observed = Some(OwnedCodingProbeObservation {
        plan: plan.digest(),
        source: resources.source_revision(),
        snapshot: parse_digest(&input.registration.snapshot.snapshot_digest)?,
        build_input: parse_digest(&input.registration.snapshot.build_input_digest)?,
        target: target_digest,
        observation: Digest32::from_bytes(h.finalize().into()),
        outputs,
        correct,
        checked_epoch: epoch,
        expires_epoch: expires,
        clock: input.clock,
        observed_epoch: clock.clone(),
        artifact,
        build_authority: None,
        retained,
    });
    sample_authority(
        plan,
        input,
        a.purpose,
        ProbeAuthorityPhase::AfterChild,
        provider,
        &clock,
        &a.revocation_floor,
        &a.authority_floor,
        resources,
    )?;
    registration_current(plan, input, sample(input.clock, &clock, expires)?)?;
    let observed = a.observed.as_mut().ok_or(ProbeError::ProcessFailed)?;
    sample(input.clock, &clock, expires)?;
    observed.check_current_artifact(plan, resources)
}
fn registration_current(
    plan: &CodingProbePlan,
    input: &ProbeAttemptInput<'_>,
    now: u64,
) -> Result<(), ProbeError> {
    let target = input
        .registration
        .snapshot
        .targets
        .iter()
        .find(|t| t.target_ref == input.registration.target_ref)
        .ok_or(ProbeError::BindingMismatch)?;
    if validate_dgcl_target_lease(
        input.lease,
        input.registration.snapshot,
        input.registration.target_ref,
        &target.target_digest,
        now,
    )
    .state
        != DgclLifecycleState::Current
    {
        return Err(ProbeError::Stale);
    }
    if input.tool_receipt.claims().expires_at_epoch().is_none() {
        return Err(ProbeError::UnregisteredTool);
    }
    let policy = ReceiptPolicy::exact(
        ReceiptClass::ToolExecution,
        SubjectRevision::checked(&input.registration.snapshot.source_revision)
            .map_err(|_| ProbeError::UnregisteredTool)?,
        ReceiptScope::checked(input.registration.receipt_scope(plan))
            .map_err(|_| ProbeError::UnregisteredTool)?,
        input.registration.payload_digest(plan),
        now,
    );
    input
        .tool_verifier
        .verify(
            input.tool_receipt.clone(),
            &policy,
            &mut ReplayGuard::default(),
        )
        .map_err(|_| ProbeError::UnregisteredTool)?;
    Ok(())
}
