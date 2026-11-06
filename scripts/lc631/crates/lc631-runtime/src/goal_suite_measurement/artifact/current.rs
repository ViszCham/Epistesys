//! Cheap bounded admission precedes hashing, providers and physical artifact I/O.
use super::*;
use crate::contextual_repair::execution::{GoalEpochError, GoalInvocationGate};
use crate::goal_clock_domain as clock;
use lc631_analysis::declared_build_capture::{declared_capture_digest, SnapshotAuthorityView};
use lc631_core::ExecutionPermitContext;
use std::path::{Component, Path};

fn text(value: &str, cap: usize) -> Result<(), Error> {
    if value.len() > cap {
        return Err(Error::InputLimit);
    }
    Ok(())
}
pub(super) fn authority(context: &ExecutionPermitContext<'_>) -> Result<(), Error> {
    text(context.principal.as_str(), 256)?;
    text(context.scope, 4096)?;
    text(context.source_revision, 71)?;
    text(context.trusted_host_fingerprint, 256)?;
    if context.revoked_permit_digests.len() > 256 {
        return Err(Error::InputLimit);
    }
    for digest in context.revoked_permit_digests {
        text(digest, 71)?;
        declared_capture_digest(digest).map_err(|_| Error::BindingMismatch)?;
    }
    Ok(())
}
pub(super) fn invocation(input: &ProbeInvocation<'_>) -> Result<(), Error> {
    fields(&clock::ProbeInputs::without_clock(input))
}
fn fields(input: &clock::ProbeInputs<'_>) -> Result<(), Error> {
    // Length inspection is constant-time and precedes source hashing/H2.
    text(input.source, SOURCE_CAP)?;
    let r = &input.registration;
    text(r.program, 4096)?;
    text(r.target_ref, 1024)?;
    text(r.executable_digest, 71)?;
    if r.target_path.as_os_str().len() > 4096 {
        return Err(Error::InputLimit);
    }
    text(r.target_path.to_str().ok_or(Error::InputLimit)?, 4096)?;
    if input.timeout_ms == 0 || input.timeout_ms > 30_000 {
        return Err(Error::InvalidLimits);
    }
    text(input.permit.scope(), 4096)?;
    text(input.permit.digest(), 71)?;
    input
        .tool_receipt
        .check_size_bounds()
        .map_err(|_| Error::InputLimit)?;
    authority(input.authority)?;
    let s = r.snapshot;
    text(&s.schema_version, 128)?;
    text(&s.observed_boundary, 1024)?;
    for field in [
        &s.source_revision,
        &s.build_input_digest,
        &s.profile_digest,
        &s.validator_digest,
        &s.authority_revision_digest,
        &s.targets_digest,
        &s.snapshot_digest,
    ] {
        text(field, 71)?;
    }
    // Narrow artifact profile, not the entire4096-input Cargo inventory profile.
    if s.targets.len() > 64 {
        return Err(Error::InputLimit);
    }
    let mut bytes = 0usize;
    for target in &s.targets {
        text(&target.target_ref, 1024)?;
        text(&target.target_digest, 71)?;
        bytes = bytes
            .checked_add(target.target_ref.len())
            .and_then(|v| v.checked_add(target.target_digest.len()))
            .ok_or(Error::InputLimit)?;
        if bytes > 8192 {
            return Err(Error::InputLimit);
        }
    }
    let l = input.lease;
    text(&l.schema_version, 128)?;
    for field in [
        &l.snapshot_digest,
        &l.source_revision,
        &l.build_input_digest,
        &l.profile_digest,
        &l.validator_digest,
        &l.authority_revision_digest,
        &l.targets_digest,
        &l.lease_digest,
    ] {
        text(field, 71)?;
    }
    Ok(())
}
struct BoundedAuthority<'a>(&'a mut dyn CurrentSnapshotAuthority);
impl CurrentSnapshotAuthority for BoundedAuthority<'_> {
    fn current(&mut self) -> Result<SnapshotAuthorityView<'_>, SnapshotCaptureError> {
        let view = self.0.current()?;
        authority(&view.context).map_err(|_| SnapshotCaptureError::InventoryLimit)?;
        if view.expected_principal.as_str().len() > 256
            || view.permit.scope().len() > 4096
            || view.permit.digest().len() > 71
        {
            return Err(SnapshotCaptureError::InventoryLimit);
        }
        Ok(view)
    }
}
fn parsed(text: &str) -> Result<Digest32, Error> {
    declared_capture_digest(text).map_err(Error::BuildCapture)
}
fn binds(
    origin: &OwnedDeclaredBuildCapture<'_>,
    owner: &OwnedCodingProbeObservation<'_>,
    input: &clock::ProbeInputs<'_>,
    plan: &CodingProbePlan,
) -> Result<(), Error> {
    let p = origin.content_pins();
    let s = input.registration.snapshot;
    if owner.plan_digest() != plan.digest()
        || owner.snapshot() != origin.snapshot_digest()
        || owner.task_source() != p.source
        || owner.build_input_digest() != p.build
        || parsed(&s.source_revision)? != p.source
        || parsed(&s.build_input_digest)? != p.build
        || parsed(&s.profile_digest)? != p.profile
        || parsed(&s.validator_digest)? != p.validator
        || parsed(&s.targets_digest)? != p.targets
        || parsed(&s.snapshot_digest)? != owner.snapshot()
    {
        return Err(Error::RoleOrOriginMismatch);
    }
    Ok(())
}
fn epoch_error(error: GoalEpochError) -> Error {
    match error {
        GoalEpochError::ObservedExpiry => Error::Expired,
        GoalEpochError::Probe(error) => probe_error(error),
    }
}
fn cheap_current(
    root: &ArtifactPairRoot<'_, '_>,
    gate: &GoalInvocationGate<'_, '_, '_>,
    role: usize,
) -> Result<u64, Error> {
    let now = gate.sample_current_epoch().map_err(epoch_error)?.epoch();
    if now < root.epochs.get()[role] {
        return Err(Error::ClockRollback);
    }
    Ok(now)
}
fn physical_root(input: &clock::ProbeInputs<'_>, expected: Digest32) -> Result<(), Error> {
    let relative = Path::new(input.registration.target_ref);
    let mut count = 0usize;
    for part in relative.components() {
        if !matches!(part, Component::Normal(_)) || count == 32 {
            return Err(Error::BindingMismatch);
        }
        count += 1;
    }
    if count == 0 || !input.registration.target_path.ends_with(relative) {
        return Err(Error::BindingMismatch);
    }
    let mut root = input.registration.target_path;
    for _ in 0..count {
        root = root.parent().ok_or(Error::BindingMismatch)?;
    }
    let actual =
        lc631_analysis::dgcl_repository_root_digest(root).map_err(|_| Error::BindingMismatch)?;
    if parsed(&actual)? != expected {
        return Err(Error::RoleOrOriginMismatch);
    }
    Ok(())
}
fn port(
    origin: &OwnedDeclaredBuildCapture<'_>,
    port: &CodingBuildPort<'_, '_>,
    view: &ResourceReservationView<'_>,
) -> Result<(), Error> {
    let actual = port.binding();
    if actual.root() != origin.root()
        || actual.source() != origin.content_pins().source
        || actual.program() != origin.program()
        || actual.plan() != origin.plan()
    {
        return Err(Error::BindingMismatch);
    }
    // H4 central-only checked forwarding seam, intentionally no success fallback.
    // It admits the private source/pipeline before the legacy verifier serializes it.
    port.check_goal_profile_inputs(view)
        .map_err(Error::SnapshotGate)
}
fn capture(
    origin: &OwnedDeclaredBuildCapture<'_>,
    port: &mut CodingBuildPort<'_, '_>,
    authority: &mut dyn CurrentSnapshotAuthority,
    view: &ResourceReservationView<'_>,
) -> Result<(), Error> {
    let attempt = port.capture_current(&mut BoundedAuthority(authority), view);
    let actual = attempt.current().map_err(Error::BuildCapture)?;
    match_capture(origin, actual, view)
}
fn match_capture(
    origin: &OwnedDeclaredBuildCapture<'_>,
    actual: &OwnedDeclaredBuildCapture<'_>,
    view: &ResourceReservationView<'_>,
) -> Result<(), Error> {
    actual.check_current(view).map_err(Error::BuildCapture)?;
    if actual.root() != origin.root()
        || actual.program() != origin.program()
        || actual.plan() != origin.plan()
    {
        return Err(Error::BindingMismatch);
    }
    actual
        .matches_expected(origin.content_pins())
        .map_err(|e| match e {
            SnapshotCaptureError::ContentMismatch => Error::BuildDrift,
            other => Error::BuildCapture(other),
        })?;
    view.check()?;
    Ok(())
}
pub(super) fn check(
    root: &ArtifactPairRoot<'_, '_>,
    pair: CodingPair<'_, '_>,
    gate: &mut ArtifactCurrent<'_, '_, '_, '_, '_>,
    view: &ResourceReservationView<'_>,
) -> Result<(), Error> {
    root.check(view)?;
    if std::ptr::eq(pair.baseline, pair.candidate) {
        return Err(Error::RoleOrOriginMismatch);
    }
    invocation(&gate.baseline_invocation)?;
    invocation(&gate.candidate_invocation)?;
    binds(
        root.baseline_before,
        pair.baseline,
        &clock::ProbeInputs::without_clock(&gate.baseline_invocation),
        root.plan,
    )?;
    binds(
        root.candidate_expected,
        pair.candidate,
        &clock::ProbeInputs::without_clock(&gate.candidate_invocation),
        root.plan,
    )?;
    let baseline =
        GoalInvocationGate::admit(root.plan, pair.baseline, &gate.baseline_invocation, view)
            .map_err(probe_error)?;
    let candidate =
        GoalInvocationGate::admit(root.plan, pair.candidate, &gate.candidate_invocation, view)
            .map_err(probe_error)?;
    let before = [
        cheap_current(root, &baseline, 0)?,
        cheap_current(root, &candidate, 1)?,
    ];
    port(root.baseline_before, gate.baseline_build, view)?;
    port(root.candidate_expected, gate.candidate_build, view)?;
    // H2 retains the private permission checker. Cheap rejection happens before
    // target/native hashing in that shared checked helper; no caller Pass flag.
    baseline.check_artifact_current().map_err(epoch_error)?;
    candidate.check_artifact_current().map_err(epoch_error)?;
    physical_root(
        &clock::ProbeInputs::without_clock(&gate.baseline_invocation),
        root.baseline_before.root(),
    )?;
    physical_root(
        &clock::ProbeInputs::without_clock(&gate.candidate_invocation),
        root.candidate_expected.root(),
    )?;
    capture(
        root.baseline_before,
        gate.baseline_build,
        gate.baseline_authority,
        view,
    )?;
    capture(
        root.candidate_expected,
        gate.candidate_build,
        gate.candidate_authority,
        view,
    )?;
    root.check(view)?;
    let after = [
        cheap_current(root, &baseline, 0)?,
        cheap_current(root, &candidate, 1)?,
    ];
    if after.iter().zip(before).any(|(end, start)| *end < start) {
        return Err(Error::ClockRollback);
    }
    // Rejected foreign/binding/authority inputs never advance the trusted cursor.
    root.check(view)?;
    root.epochs.set(after);
    Ok(())
}

/// The registered path uses the same bounded metadata, physical-target and Goal
/// checks as the Given path. Its execution and capture actors share one floor per
/// role. Only the outer operation may commit the accepted cursors after its last
/// successful check; rejected samples stay in the separate observation ledgers.
pub(super) fn check_registered<'o, 'p, 'v>(
    root: &clock::RegisteredArtifactRoot<'o, 'p, 'v>,
    pair: clock::RegisteredPair<'o, 'p, 'v>,
    ports: &mut clock::CurrentPorts<'_, '_, '_, '_, '_>,
    view: &'v ResourceReservationView<'_>,
) -> Result<[u64; 2], clock::Error> {
    root.artifact.check(view)?;
    root.check_occurrences(pair, view)?;
    let owners = pair.actual();
    if std::ptr::eq(owners.baseline, owners.candidate) {
        return Err(clock::ClockFault::BindingMismatch.into());
    }
    let base = ports.baseline_probe;
    let next = ports.candidate_probe;
    fields(&base)?;
    fields(&next)?;
    binds(root.before.actual(), owners.baseline, &base, root.plan())?;
    binds(root.expected.actual(), owners.candidate, &next, root.plan())?;
    port(root.before.actual(), ports.baseline_build, view)?;
    port(root.expected.actual(), ports.candidate_build, view)?;

    let before = [
        pair.baseline
            .check_invocation_current(root.plan(), base, view)?,
        pair.candidate
            .check_invocation_current(root.plan(), next, view)?,
    ];
    if before
        .iter()
        .zip(root.artifact.epochs.get())
        .any(|(now, floor)| *now < floor)
    {
        return Err(clock::ClockFault::ClockRollback.into());
    }
    pair.baseline
        .check_artifact_current(root.plan(), base, view)?;
    pair.candidate
        .check_artifact_current(root.plan(), next, view)?;
    physical_root(&base, root.before.actual().root())?;
    physical_root(&next, root.expected.actual().root())?;
    capture_registered(root.baseline, root.before, ports.baseline_build, view)?;
    capture_registered(root.candidate, root.expected, ports.candidate_build, view)?;
    root.artifact.check(view)?;
    let after = [
        pair.baseline
            .check_invocation_current(root.plan(), base, view)?,
        pair.candidate
            .check_invocation_current(root.plan(), next, view)?,
    ];
    if after.iter().zip(before).any(|(end, start)| *end < start) {
        return Err(clock::ClockFault::ClockRollback.into());
    }
    root.check_occurrences(pair, view)?;
    root.artifact.check(view)?;
    Ok(after)
}

fn capture_registered<'o, 'p, 'v>(
    session: &clock::RoleSession<'o, 'p, 'v>,
    origin: &clock::RegisteredCapture<'o, 'p, 'v>,
    port: &mut CodingBuildPort<'_, '_>,
    view: &'v ResourceReservationView<'_>,
) -> Result<(), clock::Error> {
    let attempt = session.capture_current(port, view);
    let captured = attempt.current()?;
    if !std::ptr::eq(captured.session(), session) {
        return Err(clock::ClockFault::BindingMismatch.into());
    }
    match_capture(origin.actual(), captured.actual(), view)?;
    Ok(())
}
