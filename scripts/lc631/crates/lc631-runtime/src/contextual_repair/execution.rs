//! 実process所有者とfresh Test権限を使う。検証tokenは権限ではない。
//! Real owned processes with fresh Test authority; validation tokens grant none.
#![forbid(unsafe_code)]
use super::measurement::*;
use lc631_analysis::{
    run_bounded_tool_with_resources, validate_dgcl_target_lease, BoundedToolRequest,
    DgclEvidenceLease, DgclExecutionSnapshot, DgclLifecycleState, ToolRunState,
};
use lc631_core::{
    stable_sha256, Action, ExecutionPermit, ExecutionPermitContext, PermitValidation,
};
use lc631_process_supervisor::filesystem::{DirectoryAnchor, ReadOnlyAnchoredFile};
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptPolicy, ReceiptScope, ReceiptVerifier, ReplayGuard, SubjectRevision,
    UntrustedReceipt,
};
use lc631_substrate::{resource::ResourceReservationView, Digest32};
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    path::{Path, PathBuf},
};
mod driver;
pub use driver::{CodingBuildProfile, RemeasuringRepairDriver};
mod goal_current;
pub(crate) use goal_current::{GoalEpochError, GoalInvocationGate};

/// The caller allocates and explicitly supplies a separate current Test
/// authority provider before native effects. No grant is derived from an Edit
/// permit, validation receipt, registration assertion or observation token.
pub fn observe_current_with_build_authority<'v>(
    plan: &CodingProbePlan,
    input: ProbeInvocation<'_>,
    resources: &'v ResourceReservationView<'_>,
    authority: Box<dyn lc631_analysis::declared_build_capture::CurrentSnapshotAuthority + 'v>,
) -> Result<OwnedCodingProbeObservation<'v>, ProbeError> {
    let mut observed = observe_current(plan, input, resources)?;
    observed.build_authority = Some(authority);
    Ok(observed)
}

/// Actual path/digest witness created only by observe_current. Rechecking opens
/// new bounded handles, never a lifetime-long read lock on a repairable target.
pub(super) struct ProbeArtifactWitness {
    target_path: PathBuf,
    program: PathBuf,
    binary: Digest32,
}
impl ProbeArtifactWitness {
    pub(super) fn check(
        &self,
        target: Digest32,
        resources: &ResourceReservationView<'_>,
    ) -> Result<(), ProbeError> {
        let scratch = super::measurement::claim(resources, 32 * 1024)?;
        let actual =
            fingerprint(&self.target_path, 1024 * 1024, resources).map_err(
                |error| match error {
                    ProbeError::BindingMismatch => ProbeError::PhysicalTargetChanged,
                    error => error,
                },
            )?;
        if parse_digest(&actual.0)? != target {
            return Err(ProbeError::PhysicalTargetChanged);
        }
        let binary =
            fingerprint(&self.program, 64 * 1024 * 1024, resources).map_err(
                |error| match error {
                    ProbeError::BindingMismatch => ProbeError::NativeArtifactChanged,
                    error => error,
                },
            )?;
        if parse_digest(&binary.0)? != self.binary {
            return Err(ProbeError::NativeArtifactChanged);
        }
        scratch.check()?;
        Ok(())
    }
    pub(super) fn binds_repair_target(
        &self,
        request: &lc631_analysis::DgclRepairRequest,
    ) -> Result<(), ProbeError> {
        let relative = Path::new(&request.target_ref);
        if !self.target_path.ends_with(relative) {
            return Err(ProbeError::BindingMismatch);
        }
        let root = self
            .target_path
            .ancestors()
            .nth(relative.components().count())
            .ok_or(ProbeError::BindingMismatch)?;
        let root_digest = lc631_analysis::dgcl_repository_root_digest(root)
            .map_err(|_| ProbeError::BindingMismatch)?;
        if request.repository_root_digest.as_deref() != Some(root_digest.as_str()) {
            return Err(ProbeError::BindingMismatch);
        }
        Ok(())
    }
}

fn owned_path(path: &Path) -> Result<PathBuf, ProbeError> {
    let text = path.to_str().ok_or(ProbeError::BindingMismatch)?;
    if text.len() > 4096 {
        return Err(ProbeError::BindingMismatch);
    }
    let mut owned = PathBuf::new();
    owned
        .try_reserve_exact(text.len())
        .map_err(|_| ProbeError::InvalidPlan)?;
    owned.push(path);
    if owned.capacity() > 8192 {
        return Err(ProbeError::InvalidPlan);
    }
    Ok(owned)
}

/// Trusted registration binds the native binary to the declared target/build.
/// It is an issuer assertion, not a proof of a hermetic compiler/source mapping.
pub struct ProbeToolRegistration<'a> {
    pub program: &'a str,
    pub executable_digest: &'a str,
    pub target_path: &'a Path,
    pub target_ref: &'a str,
    pub snapshot: &'a DgclExecutionSnapshot,
}
impl ProbeToolRegistration<'_> {
    pub fn receipt_scope(&self, plan: &CodingProbePlan) -> String {
        format!("tlcx/coding-probe/tool/v1/{}", hex(plan.digest()))
    }
    pub fn execution_scope(&self, plan: &CodingProbePlan) -> String {
        format!("tlcx/coding-probe/test/v1/{}", self.payload_digest(plan))
    }
    pub fn payload_digest(&self, plan: &CodingProbePlan) -> String {
        stable_sha256(&format!(
            "Epistesys/TLCX/CodingProbe/RegisteredArtifact/v1\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}",
            hex(plan.digest()),
            hex(plan.goal()),
            hex(plan.policy()),
            self.program,
            self.executable_digest,
            self.target_path.to_string_lossy(),
            self.target_ref,
            self.snapshot.snapshot_digest
        ))
    }
}
pub struct ProbeInvocation<'a> {
    pub source: &'a str,
    pub registration: ProbeToolRegistration<'a>,
    pub lease: &'a DgclEvidenceLease,
    pub tool_receipt: &'a UntrustedReceipt,
    pub tool_verifier: &'a ReceiptVerifier,
    pub permit: &'a ExecutionPermit,
    pub authority: &'a ExecutionPermitContext<'a>,
    /// Trusted caller clock; public report timestamps never advance it.
    pub clock: fn() -> u64,
    pub timeout_ms: u64,
}

/// Both resume and live repair call this exact producer. No ToolRunResult or
/// report-injection overload exists. Reobserve a retained old artifact for the
/// baseline after process exit; never deserialize an observed baseline owner.
pub fn observe_current<'v>(
    plan: &CodingProbePlan,
    input: ProbeInvocation<'_>,
    resources: &'v ResourceReservationView<'_>,
) -> Result<OwnedCodingProbeObservation<'v>, ProbeError> {
    resources.check()?;
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
    let retained = super::measurement::claim(resources, 32 * 1024)?;
    let scratch = super::measurement::claim(resources, 32 * 1024)?;
    let artifact = ProbeArtifactWitness {
        target_path: owned_path(input.registration.target_path)?,
        program: owned_path(Path::new(input.registration.program))?,
        binary: parse_digest(input.registration.executable_digest)?,
    };
    let before = (input.clock)();
    check_current(plan, &input, before)?;
    // Allocate the small shared cursor under the retained claim BEFORE native
    // effects. Later gain construction only clones these owner-local Arcs.
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
    let observed_epoch = ObservedEpoch::shared(before);
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
    let spawn_epoch = (input.clock)();
    observed_epoch.observe(spawn_epoch, expires)?;
    if spawn_epoch < before {
        return Err(ProbeError::ClockRollback);
    }
    check_current(plan, &input, spawn_epoch)?;
    // Validate the full bounded invocation horizon before entering the process
    // owner, so preparation cannot cross expiry before its spawn. Immediate
    // host revocation still depends on the caller's current revocation view.
    let horizon = spawn_epoch
        .checked_add(input.timeout_ms.div_ceil(1000))
        .ok_or(ProbeError::Stale)?;
    check_current(plan, &input, horizon)?;
    let capture = run_bounded_tool_with_resources(
        BoundedToolRequest {
            program: input.registration.program,
            args: &[],
            stdin: Some(PROBE_INPUT),
            timeout_ms: input.timeout_ms,
            stdout_limit: 256,
            stderr_limit: 1024,
        },
        resources,
    )?;
    let now = (input.clock)();
    observed_epoch.observe(now, expires)?;
    if now < spawn_epoch {
        return Err(ProbeError::ClockRollback);
    }
    check_current(plan, &input, now)?;
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
    let (outputs, correct) = super::measurement::parse_output(capture.stdout())?;
    let mut h = Sha256::new();
    h.update(b"Epistesys/TLCX/CodingProbe/ActualObservation/v1\0");
    h.update(plan.digest().as_bytes());
    h.update(input.registration.payload_digest(plan).as_bytes());
    h.update(input.permit.digest().as_bytes());
    h.update(report.stdout_digest.as_bytes());
    h.update(report.cwd_digest.as_bytes());
    h.update(report.environment_digest.as_bytes());
    let result = OwnedCodingProbeObservation {
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
        clock: input.clock,
        observed_epoch,
        artifact,
        build_authority: None,
        retained,
    };
    scratch.check()?;
    result.check_current(plan, resources)?;
    Ok(result)
}
fn check_current(
    plan: &CodingProbePlan,
    input: &ProbeInvocation<'_>,
    now: u64,
) -> Result<(), ProbeError> {
    if now < input.authority.now_epoch {
        return Err(ProbeError::ClockRollback);
    }
    let scope = input.registration.execution_scope(plan);
    let authority = ExecutionPermitContext {
        principal: input.authority.principal,
        action: Action::Test,
        scope: &scope,
        source_revision: &input.registration.snapshot.source_revision,
        authority_revision: input.authority.authority_revision,
        now_epoch: now,
        revocation_revision: input.authority.revocation_revision,
        revoked_permit_digests: input.authority.revoked_permit_digests,
        caller_origin: input.authority.caller_origin,
        trusted_host_fingerprint: input.authority.trusted_host_fingerprint,
    };
    if input.authority.action != Action::Test
        || input.authority.scope != scope
        || input.authority.source_revision != authority.source_revision
        || input.permit.action() != Action::Test
        || input.permit.validate(&authority) != PermitValidation::ValidGrant
    {
        return Err(ProbeError::Unauthorized);
    }
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
fn fingerprint(
    path: &Path,
    cap: usize,
    resources: &ResourceReservationView<'_>,
) -> Result<(String, ReadOnlyAnchoredFile), ProbeError> {
    let anchor = DirectoryAnchor::open_existing(path.parent().ok_or(ProbeError::BindingMismatch)?)
        .map_err(io)?;
    let mut file = anchor
        .open_read_only(Path::new(
            path.file_name().ok_or(ProbeError::BindingMismatch)?,
        ))
        .map_err(io)?;
    let mut buffer = [0_u8; 8192];
    let mut total = 0_usize;
    let mut h = Sha256::new();
    loop {
        resources.check()?;
        let n = file.read(&mut buffer).map_err(io)?;
        if n == 0 {
            break;
        }
        total = total.checked_add(n).ok_or(ProbeError::BindingMismatch)?;
        if total > cap {
            return Err(ProbeError::BindingMismatch);
        }
        h.update(&buffer[..n]);
    }
    file.validate_current().map_err(io)?;
    Ok((hex(Digest32::from_bytes(h.finalize().into())), file))
}
fn io(e: std::io::Error) -> ProbeError {
    ProbeError::Io(e.kind())
}
pub(super) fn hex(value: Digest32) -> String {
    let mut s = String::from("sha256:");
    for b in value.as_bytes() {
        s.push_str(&format!("{b:02x}"));
    }
    s
}
pub(super) fn parse_digest(value: &str) -> Result<Digest32, ProbeError> {
    let value = value
        .strip_prefix("sha256:")
        .ok_or(ProbeError::BindingMismatch)?;
    if value.len() != 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(ProbeError::BindingMismatch);
    }
    let mut out = [0; 32];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&value[i * 2..i * 2 + 2], 16)
            .map_err(|_| ProbeError::BindingMismatch)?;
    }
    Ok(Digest32::from_bytes(out))
}
