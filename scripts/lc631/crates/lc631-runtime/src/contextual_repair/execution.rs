//! 実process所有者とfresh Test権限を使う。検証tokenは権限ではない。
//! Real owned processes with fresh Test authority; validation tokens grant none.
#![forbid(unsafe_code)]
use super::measurement::*;
use lc631_analysis::{
    run_bounded_tool_with_resources, validate_dgcl_target_lease, BoundedToolRequest,
    DgclEvidenceLease, DgclExecutionSnapshot, DgclLifecycleState, ResourceToolResult, ToolRunState,
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
pub(super) mod attempt_producer;
mod clock;
pub use clock::ProbeInputs;
pub(crate) use clock::ObservationClock;
use clock::NativeBirth;
mod clock_worker;
use clock_worker::observe_native_run;
pub(crate) use clock_worker::{observe_registered_run, NativeRun};
mod driver;
pub use driver::{CodingBuildProfile, RemeasuringRepairDriver};
mod goal_current;
pub(crate) use goal_current::{GoalEpochError, GoalInvocationGate};

pub(crate) fn preflight_registered_inputs(
    plan: &CodingProbePlan,
    input: ProbeInputs<'_>,
    resources: &ResourceReservationView<'_>,
) -> Result<(), ProbeError> {
    resources.check()?;
    goal_current::bounds(input)?;
    let scratch = super::measurement::claim(resources, 64 * 1024)?;
    if digest(input.source.as_bytes()) != resources.source_revision() {
        return Err(ProbeError::BindingMismatch);
    }
    check_current_at(plan, input, input.authority.now_epoch)?;
    scratch.check()?;
    Ok(())
}

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
    let metadata = ProbeInputs::without_clock(&input);
    observe_native_run(plan, metadata, resources, NativeBirth::Legacy(input.clock))
        .into_legacy_result()
}


fn check_current_at(
    plan: &CodingProbePlan,
    input: ProbeInputs<'_>,
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
