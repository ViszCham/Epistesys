//! Both clock modes use this one actual native execution and capture path.
//! A late error retains its typed cause and any process output already owned.
#![forbid(unsafe_code)]
use super::*;

/// Keeps the actual completed process and any parsed observation through
/// later clock, lease or physical refusals. Entering the lower producer alone
/// gives no proof of spawn. Its legacy Err does not expose a partial handle.
pub(crate) struct NativeRun<'v> {
    pub(crate) capture: Option<ResourceToolResult<'v>>,
    pub(crate) parsed_outputs: Option<[Option<u32>; 4]>,
    pub(crate) observed: Option<OwnedCodingProbeObservation<'v>>,
    pub(crate) entered_lower: bool,
    pub(crate) diagnostic: Option<ProbeError>,
}

impl<'v> NativeRun<'v> {
    pub(super) fn into_legacy_result(self) -> Result<OwnedCodingProbeObservation<'v>, ProbeError> {
        match self.diagnostic {
            Some(error) => Err(error),
            None => self.observed.ok_or(ProbeError::ProcessFailed),
        }
    }
}

pub(crate) fn observe_registered_run<'v>(
    plan: &CodingProbePlan,
    input: ProbeInputs<'_>,
    resources: &'v ResourceReservationView<'_>,
    reader: crate::goal_clock_domain::EndpointReader<'v>,
    history: std::sync::Arc<ObservedEpoch>,
) -> NativeRun<'v> {
    observe_native_run(plan, input, resources, NativeBirth::Registered(reader, history))
}

pub(super) fn observe_native_run<'v>(
    plan: &CodingProbePlan,
    input: ProbeInputs<'_>,
    resources: &'v ResourceReservationView<'_>,
    birth: NativeBirth<'v>,
) -> NativeRun<'v> {
    let mut run = NativeRun {
        capture: None,
        parsed_outputs: None,
        observed: None,
        entered_lower: false,
        diagnostic: None,
    };
    run.diagnostic = observe_native_into(plan, input, resources, birth, &mut run).err();
    run
}

fn observe_native_into<'v>(
    plan: &CodingProbePlan,
    input: ProbeInputs<'_>,
    resources: &'v ResourceReservationView<'_>,
    birth: NativeBirth<'v>,
    run: &mut NativeRun<'v>,
) -> Result<(), ProbeError> {
    resources.check()?;
    if matches!(&birth, NativeBirth::Registered(..)) {
        goal_current::bounds(input)?;
        // Check the supplied Test authority and lease before reading the
        // registered actor. Context epochs are never recorded as samples.
        check_current_at(plan, input, input.authority.now_epoch)?;
    }
    if input.timeout_ms == 0
        || input.timeout_ms > 30_000
        || input.registration.program.len() > 4096
        || input.registration.target_ref.len() > 1024
        || !Path::new(input.registration.program).is_absolute()
        || !input.registration.target_path.is_absolute()
        || digest(input.source.as_bytes()) != resources.source_revision()
        || stable_sha256(input.source) != input.registration.snapshot.source_revision
    {
        return Err(ProbeError::BindingMismatch);
    }
    let retained = super::super::measurement::claim(resources, 32 * 1024)?;
    let scratch = super::super::measurement::claim(resources, 32 * 1024)?;
    let artifact = ProbeArtifactWitness {
        target_path: owned_path(input.registration.target_path)?,
        program: owned_path(Path::new(input.registration.program))?,
        binary: parse_digest(input.registration.executable_digest)?,
    };
    let expires = input
        .lease
        .expires_at_epoch
        .min(input.permit.expires_at_epoch())
        .min(
            input
                .tool_receipt
                .claims()
                .expires_at_epoch()
                .ok_or(ProbeError::UnregisteredTool)?,
        );
    // Legacy creates a new cursor at the first read. Registered birth must
    // reuse the role cursor first initialized by an actual before-capture.
    let (clock, observed_epoch, before) = birth.start(expires, resources)?;
    check_current_at(plan, input, before)?;
    let target = input
        .registration
        .snapshot
        .targets
        .iter()
        .find(|t| t.target_ref == input.registration.target_ref)
        .ok_or(ProbeError::BindingMismatch)?;
    let (target_digest, target_handle) =
        fingerprint(input.registration.target_path, 1024 * 1024, resources)?;
    if target_digest != target.target_digest {
        return Err(ProbeError::BindingMismatch);
    }
    let (binary_digest, binary_handle) = fingerprint(
        Path::new(input.registration.program),
        64 * 1024 * 1024,
        resources,
    )?;
    if binary_digest != input.registration.executable_digest {
        return Err(ProbeError::BindingMismatch);
    }
    let spawn_epoch = clock.sample(&observed_epoch, expires, resources)?;
    if spawn_epoch < before {
        return Err(ProbeError::ClockRollback);
    }
    check_current_at(plan, input, spawn_epoch)?;
    // Validate the full bounded invocation horizon before entering the process
    // owner, so preparation cannot cross expiry before its spawn. Immediate
    // host revocation still depends on the caller's current revocation view.
    let horizon = spawn_epoch
        .checked_add(input.timeout_ms.div_ceil(1000))
        .ok_or(ProbeError::Stale)?;
    check_current_at(plan, input, horizon)?;
    run.entered_lower = true;
    run.capture = Some(run_bounded_tool_with_resources(
        BoundedToolRequest {
            program: input.registration.program,
            args: &[],
            stdin: Some(PROBE_INPUT),
            timeout_ms: input.timeout_ms,
            stdout_limit: 256,
            stderr_limit: 1024,
        },
        resources,
    )?);
    let capture = run.capture.as_ref().ok_or(ProbeError::ProcessFailed)?;
    let report = capture.reported();
    let valid_process = report.state == ToolRunState::Observed
        && report.exit_code == Some(0)
        && !report.timed_out
        && !report.stdout_truncated
        && !report.stderr_truncated
        && report.process_tree_kill_verified
        && report.executable_sha256.as_deref() == Some(input.registration.executable_digest)
        && report.args_digest == stable_sha256("")
        && capture.stderr().is_empty();
    let parsed = if valid_process {
        super::super::measurement::parse_output(capture.stdout())
    } else {
        Err(ProbeError::ProcessFailed)
    };
    if let Ok((outputs, _)) = &parsed {
        // A later clock, authority or physical failure does not erase these
        // historical output bytes or make them a checked current observation.
        run.parsed_outputs = Some(*outputs);
    }
    let now = clock.sample(&observed_epoch, expires, resources)?;
    if now < spawn_epoch {
        return Err(ProbeError::ClockRollback);
    }
    check_current_at(plan, input, now)?;
    if !valid_process {
        return Err(ProbeError::ProcessFailed);
    }
    target_handle.validate_current().map_err(io)?;
    binary_handle.validate_current().map_err(io)?;
    if fingerprint(input.registration.target_path, 1024 * 1024, resources)?.0 != target_digest
        || fingerprint(
            Path::new(input.registration.program),
            64 * 1024 * 1024,
            resources,
        )?
        .0 != binary_digest
    {
        return Err(ProbeError::BindingMismatch);
    }
    let (outputs, correct) = parsed?;
    let mut h = Sha256::new();
    h.update(b"Epistesys/TLCX/CodingProbe/ActualObservation/v1\0");
    h.update(plan.digest().as_bytes());
    h.update(input.registration.payload_digest(plan).as_bytes());
    h.update(input.permit.digest().as_bytes());
    h.update(report.stdout_digest.as_bytes());
    h.update(report.cwd_digest.as_bytes());
    h.update(report.environment_digest.as_bytes());
    run.observed = Some(OwnedCodingProbeObservation {
        plan: plan.digest(),
        source: resources.source_revision(),
        snapshot: parse_digest(&input.registration.snapshot.snapshot_digest)?,
        build_input: parse_digest(&input.registration.snapshot.build_input_digest)?,
        target: parse_digest(&target_digest)?,
        observation: Digest32::from_bytes(h.finalize().into()),
        outputs,
        correct,
        checked_epoch: now,
        expires_epoch: expires,
        clock,
        observed_epoch,
        artifact,
        build_authority: None,
        retained,
    });
    scratch.check()?;
    run.observed.as_ref().ok_or(ProbeError::ProcessFailed)?.check_current(plan, resources)
}
