use crate::{
    coding_evidence_payload_digest, coding_evidence_scope, issue_dgcl_evidence_lease,
    run_bounded_tool, verify_dgcl_completion_plan, BoundedToolRequest, BoundedToolResult,
    CodingEvidenceClaim, CodingEvidenceKind, DgclExecutionSnapshot, DgclPipelineReport,
    PlannedEvidenceTask, TaskApplicability, ToolRunState,
};
use lc631_core::{
    stable_sha256, Action, ExecutionPermit, ExecutionPermitContext, PermitValidation,
};
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptIssuer, ReceiptPolicy, ReceiptScope, ReceiptVerifier, ReplayGuard,
    SubjectRevision, UntrustedReceipt,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub struct DgclCargoExecutionContext<'a> {
    pub lifecycle: &'a DgclExecutionSnapshot,
    pub permit: &'a ExecutionPermit,
    pub authority: &'a ExecutionPermitContext<'a>,
    pub tool_verifier: &'a ReceiptVerifier,
    pub max_duration: Duration,
}

/// These are planned inputs, not observations. Execution requires an exact
/// host-origin Test permit covering the digest of this whole command binding.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DgclCargoValidationPlan {
    pub schema_version: String,
    pub source_revision: String,
    pub completion_plan_digest: String,
    pub requirement_id: String,
    pub task_id: String,
    pub kind: CodingEvidenceKind,
    pub cargo_path: String,
    pub cargo_digest: String,
    pub manifest_path: String,
    pub manifest_digest: String,
    pub repository_root_digest: String,
    pub acceptance_target: Option<String>,
    pub acceptance_test: Option<String>,
    pub timeout_ms: u64,
    #[serde(default)]
    pub tool_registration: Option<UntrustedReceipt>,
}

pub fn dgcl_repository_root_digest(root: &Path) -> Result<String, DgclValidationProducerError> {
    let absolute =
        std::path::absolute(root).map_err(|_| DgclValidationProducerError::UnsafePath)?;
    #[cfg(windows)]
    if !matches!(absolute.components().next(), Some(Component::Prefix(prefix)) if matches!(prefix.kind(), std::path::Prefix::Disk(_) | std::path::Prefix::VerbatimDisk(_)))
    {
        return Err(DgclValidationProducerError::UnsafePath);
    }
    let text = absolute.to_string_lossy().replace('\\', "/");
    Ok(stable_sha256(text.strip_prefix("//?/").unwrap_or(&text)))
}

pub fn dgcl_cargo_registration_payload(path: &str, digest: &str) -> String {
    stable_sha256(&format!(
        "dgcl-registered-cargo-check-test.v1\0{path}\0{digest}"
    ))
}

pub fn verify_dgcl_cargo_registration(
    plan: &DgclCargoValidationPlan,
    verifier: &ReceiptVerifier,
    now: u64,
) -> Result<(), DgclValidationProducerError> {
    let receipt = plan
        .tool_registration
        .clone()
        .ok_or(DgclValidationProducerError::UnregisteredTool)?;
    if receipt.claims().expires_at_epoch().is_none() {
        return Err(DgclValidationProducerError::UnregisteredTool);
    }
    verifier
        .verify(
            receipt,
            &ReceiptPolicy::exact(
                ReceiptClass::ToolExecution,
                SubjectRevision::checked(&plan.cargo_digest)
                    .map_err(|_| DgclValidationProducerError::UnregisteredTool)?,
                ReceiptScope::checked(format!("dgcl/registered-tool/cargo/{}", plan.cargo_digest))
                    .map_err(|_| DgclValidationProducerError::UnregisteredTool)?,
                dgcl_cargo_registration_payload(&plan.cargo_path, &plan.cargo_digest),
                now,
            ),
            &mut ReplayGuard::default(),
        )
        .map_err(|_| DgclValidationProducerError::UnregisteredTool)?;
    Ok(())
}

pub fn dgcl_cargo_validation_scope(plan: &DgclCargoValidationPlan) -> String {
    format!(
        "dgcl/cargo-validation/{}",
        stable_sha256(&serde_json::to_string(plan).expect("plan serializes"))
    )
}

// Bind leases to the engine's interpretation and verification implementation,
// not only this command producer. Package promotion invalidates prior leases.
pub fn dgcl_validator_revision() -> String {
    stable_sha256(
        &serde_json::to_string(&(
            env!("CARGO_PKG_VERSION"),
            include_str!("validation_producer.rs"),
            include_str!("instruction_grammar.rs"),
            include_str!("dgcl_distillation.rs"),
            include_str!("dgcl_projection.rs"),
            include_str!("completion_plan.rs"),
            include_str!("dgcl_implementation_closure.rs"),
            include_str!("closure.rs"),
            include_str!("evidence_lifecycle.rs"),
        ))
        .expect("engine revision serializes"),
    )
}

/// Non-executing preflight shared by the repair adapter and command observer.
/// A malformed future validation cannot be discovered only after an edit.
pub fn preflight_dgcl_cargo_plan(
    root: &Path,
    pipeline: &DgclPipelineReport,
    plan: &DgclCargoValidationPlan,
) -> Result<(), DgclValidationProducerError> {
    use DgclValidationProducerError as Error;
    if plan.schema_version != "epistesys-dgcl-cargo-validation-plan.v1"
        || !(1..=120_000).contains(&plan.timeout_ms)
        || plan.source_revision != pipeline.source_revision
        || plan.completion_plan_digest != pipeline.completion_plan.plan_digest
        || dgcl_repository_root_digest(root)? != plan.repository_root_digest
        || !pipeline.completion_plan.tasks.iter().any(|task| {
            task.task_id == plan.task_id
                && task.requirement_id == plan.requirement_id
                && task.evidence_kind == plan.kind
                && task.applicability == TaskApplicability::Required
        })
    {
        return Err(Error::InvalidPlan);
    }
    match plan.kind {
        CodingEvidenceKind::AcceptanceTest => {
            if !plan
                .acceptance_target
                .as_deref()
                .is_some_and(safe_test_name)
                || !plan.acceptance_test.as_deref().is_some_and(safe_test_name)
            {
                return Err(Error::InvalidPlan);
            }
        }
        CodingEvidenceKind::StaticValidation
        | CodingEvidenceKind::RuntimeValidation
        | CodingEvidenceKind::ProductionConnection => {
            if plan.acceptance_target.is_some() || plan.acceptance_test.is_some() {
                return Err(Error::InvalidPlan);
            }
        }
        _ => return Err(Error::UnsupportedKind),
    }
    let absolute_root = std::path::absolute(root).map_err(|_| Error::UnsafePath)?;
    for part in absolute_root.ancestors() {
        let metadata = fs::symlink_metadata(part).map_err(|_| Error::UnsafePath)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(Error::UnsafePath);
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                return Err(Error::UnsafePath);
            }
        }
    }
    let canonical_root = absolute_root
        .canonicalize()
        .map_err(|_| Error::UnsafePath)?;
    let manifest = checked_validation_path(&canonical_root, &plan.manifest_path)?;
    let cargo = Path::new(&plan.cargo_path);
    if !cargo.is_absolute() {
        return Err(Error::UnsafePath);
    }
    check_real_file(cargo)?;
    if digest_validation_file(&manifest)? != plan.manifest_digest
        || digest_validation_file(cargo)? != plan.cargo_digest
    {
        return Err(Error::StaleInput);
    }
    Ok(())
}

pub fn dgcl_implementation_binding_payload(
    pipeline: &DgclPipelineReport,
    task_id: &str,
    snapshot: &DgclExecutionSnapshot,
) -> String {
    stable_sha256(&format!(
        "dgcl-implementation-binding.v2\0{}\0{}\0{task_id}\0{}",
        pipeline.source_revision, pipeline.completion_plan.plan_digest, snapshot.snapshot_digest
    ))
}

pub struct DgclBindingIssuerContext<'a> {
    pub author_receipt: UntrustedReceipt,
    pub author_verifier: &'a ReceiptVerifier,
    pub issuer: &'a ReceiptIssuer,
    pub verifier: &'a ReceiptVerifier,
    pub now_epoch: u64,
    pub replay: &'a mut ReplayGuard,
}

/// An independently signed intent-to-build-world mapping is necessary: an
/// identifier match or a compiler success alone cannot establish user intent.
pub fn issue_dgcl_implementation_binding(
    source: &str,
    pipeline: &DgclPipelineReport,
    task_id: &str,
    snapshot: &DgclExecutionSnapshot,
    context: DgclBindingIssuerContext<'_>,
) -> Result<CodingEvidenceClaim, DgclValidationProducerError> {
    use DgclValidationProducerError as Error;
    if !crate::verify_dgcl_pipeline_identity(source, pipeline) {
        return Err(Error::InvalidArtifact);
    }
    verify_dgcl_completion_plan(
        source,
        &pipeline.program_ir,
        &pipeline.translation_projection,
        &pipeline.completion_plan,
    )
    .map_err(|_| Error::InvalidArtifact)?;
    let task = pipeline
        .completion_plan
        .tasks
        .iter()
        .find(|task| {
            task.task_id == task_id
                && task.evidence_kind == CodingEvidenceKind::ImplementationBinding
                && task.applicability == TaskApplicability::Required
        })
        .ok_or(Error::InvalidPlan)?;
    if snapshot.source_revision != pipeline.source_revision
        || !crate::lifecycle_binds_target(snapshot, &task.target_ref, &task.target_digest)
    {
        return Err(Error::InvalidLifecycle);
    }
    if context.issuer.key_fingerprint() != context.verifier.key_fingerprint()
        || context.author_verifier.key_fingerprint() == context.verifier.key_fingerprint()
    {
        return Err(Error::IssuerMismatch);
    }
    let author_expiry = context
        .author_receipt
        .claims()
        .expires_at_epoch()
        .ok_or(Error::Receipt)?;
    let author = context
        .author_verifier
        .verify(
            context.author_receipt,
            &ReceiptPolicy::exact(
                ReceiptClass::Closure,
                SubjectRevision::checked(&pipeline.source_revision).map_err(|_| Error::Receipt)?,
                ReceiptScope::checked(format!("dgcl/implementation-binding/{task_id}"))
                    .map_err(|_| Error::Receipt)?,
                dgcl_implementation_binding_payload(pipeline, task_id, snapshot),
                context.now_epoch,
            ),
            context.replay,
        )
        .map_err(|_| Error::Receipt)?;
    let lease_seconds = author_expiry.saturating_sub(context.now_epoch).min(300);
    let lease = issue_dgcl_evidence_lease(snapshot, context.now_epoch, lease_seconds)
        .map_err(|_| Error::InvalidLifecycle)?;
    let mut claim = CodingEvidenceClaim {
        requirement_id: 1,
        coding_requirement_id: Some(task.requirement_id.clone()),
        coding_task_id: Some(task.task_id.clone()),
        kind: task.evidence_kind,
        source_revision: pipeline.source_revision.clone(),
        target: task.target_ref.clone(),
        target_digest: task.target_digest.clone(),
        producer: Some(task.producer),
        producer_run_digest: Some(author.receipt_digest()),
        observation_digest: Some(dgcl_implementation_binding_payload(
            pipeline, task_id, snapshot,
        )),
        lifecycle_lease: Some(lease),
        attestation: None,
    };
    claim.attestation = Some(
        context
            .issuer
            .issue(
                ReceiptClass::Closure,
                SubjectRevision::checked(&claim.source_revision).map_err(|_| Error::Receipt)?,
                ReceiptScope::checked(coding_evidence_scope(&claim)).map_err(|_| Error::Receipt)?,
                coding_evidence_payload_digest(&claim),
                context.now_epoch,
                Some(context.now_epoch + lease_seconds),
                None,
            )
            .map_err(|_| Error::Receipt)?,
    );
    Ok(claim)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum DgclValidationProducerError {
    InvalidPlan,
    UnsupportedKind,
    InvalidArtifact,
    Unauthorized,
    UnsafePath,
    StaleInput,
    ProcessFailed,
    TestsNotObserved,
    InvalidLifecycle,
    IssuerMismatch,
    Receipt,
    UnregisteredTool,
}

/// Fields intentionally private, and no Deserialize implementation. Public
/// BoundedToolResult/report objects cannot manufacture a checked observation.
pub struct CheckedDgclCargoObservation {
    source_revision: String,
    task: PlannedEvidenceTask,
    plan_digest: String,
    lifecycle: DgclExecutionSnapshot,
    run_digest: String,
    observation_digest: String,
    process: BoundedToolResult,
    observed_at_epoch: u64,
    valid_until_epoch: u64,
}

impl CheckedDgclCargoObservation {
    pub fn process(&self) -> &BoundedToolResult {
        &self.process
    }
}

pub fn check_cargo_test_result(kind: CodingEvidenceKind, stdout: &[u8]) -> bool {
    if !matches!(
        kind,
        CodingEvidenceKind::ProductionConnection
            | CodingEvidenceKind::RuntimeValidation
            | CodingEvidenceKind::AcceptanceTest
    ) {
        return false;
    }
    let Ok(text) = std::str::from_utf8(stdout) else {
        return false;
    };
    let mut positive = false;
    for line in text.lines() {
        if line.starts_with("test result: FAILED") {
            return false;
        }
        if let Some(summary) = line.strip_prefix("test result: ok. ") {
            let mut fields = summary.split(';');
            let passed = fields
                .next()
                .and_then(|value| value.trim().strip_suffix(" passed"))
                .and_then(|count| count.parse::<usize>().ok());
            let failed = fields.next().map(str::trim);
            if passed.is_none_or(|count| count == 0) || failed != Some("0 failed") {
                return false;
            }
            positive = true;
        }
    }
    positive
}

pub fn observe_dgcl_cargo_validation(
    root: &Path,
    source: &str,
    pipeline: &DgclPipelineReport,
    plan: &DgclCargoValidationPlan,
    context: DgclCargoExecutionContext<'_>,
) -> Result<CheckedDgclCargoObservation, DgclValidationProducerError> {
    use DgclValidationProducerError as Error;
    let DgclCargoExecutionContext {
        lifecycle,
        permit,
        authority,
        tool_verifier,
        max_duration,
    } = context;
    let timeout_ms = plan
        .timeout_ms
        .min(max_duration.as_millis().min(u128::from(u64::MAX)) as u64);
    if timeout_ms == 0 {
        return Err(Error::InvalidPlan);
    }
    if !crate::verify_dgcl_pipeline_identity(source, pipeline) {
        return Err(Error::InvalidArtifact);
    }
    preflight_dgcl_cargo_plan(root, pipeline, plan)?;
    if plan.schema_version != "epistesys-dgcl-cargo-validation-plan.v1"
        || plan.timeout_ms == 0
        || plan.timeout_ms > 120_000
        || plan.source_revision != pipeline.source_revision
        || plan.completion_plan_digest != pipeline.completion_plan.plan_digest
    {
        return Err(Error::InvalidPlan);
    }
    // The exact immutable artifact is checked before any process can start.
    verify_dgcl_completion_plan(
        source,
        &pipeline.program_ir,
        &pipeline.translation_projection,
        &pipeline.completion_plan,
    )
    .map_err(|_| Error::InvalidArtifact)?;
    let task = pipeline
        .completion_plan
        .tasks
        .iter()
        .find(|task| {
            task.task_id == plan.task_id
                && task.requirement_id == plan.requirement_id
                && task.evidence_kind == plan.kind
                && task.applicability == TaskApplicability::Required
        })
        .ok_or(Error::InvalidPlan)?
        .clone();
    if authority.action != Action::Test
        || authority.scope != dgcl_cargo_validation_scope(plan)
        || authority.source_revision != plan.source_revision
        || permit.validate(authority) != PermitValidation::ValidGrant
    {
        return Err(Error::Unauthorized);
    }
    if tool_verifier.key_fingerprint() != authority.trusted_host_fingerprint {
        return Err(Error::UnregisteredTool);
    }
    verify_dgcl_cargo_registration(plan, tool_verifier, authority.now_epoch)?;
    if dgcl_repository_root_digest(root)? != plan.repository_root_digest {
        return Err(Error::UnsafePath);
    }
    if lifecycle.source_revision != plan.source_revision
        || !crate::lifecycle_binds_target(lifecycle, &task.target_ref, &task.target_digest)
    {
        return Err(Error::InvalidLifecycle);
    }
    let root = root.canonicalize().map_err(|_| Error::UnsafePath)?;
    let before = capture_dgcl_cargo_snapshot(&root, source, pipeline, authority)?;
    if before.snapshot_digest != lifecycle.snapshot_digest {
        return Err(Error::StaleInput);
    }
    let manifest = checked_validation_path(&root, &plan.manifest_path)?;
    let cargo = Path::new(&plan.cargo_path)
        .canonicalize()
        .map_err(|_| Error::UnsafePath)?;
    check_real_file(&cargo)?;
    if digest_validation_file(&manifest)? != plan.manifest_digest
        || digest_validation_file(&cargo)? != plan.cargo_digest
    {
        return Err(Error::StaleInput);
    }
    let manifest_text = manifest.to_str().ok_or(Error::UnsafePath)?;
    let mut args = match plan.kind {
        CodingEvidenceKind::StaticValidation => vec!["check", "--all-targets"],
        CodingEvidenceKind::RuntimeValidation | CodingEvidenceKind::ProductionConnection => {
            vec!["test", "--lib"]
        }
        CodingEvidenceKind::AcceptanceTest => vec![
            "test",
            "--test",
            plan.acceptance_target
                .as_deref()
                .filter(|value| safe_test_name(value))
                .ok_or(Error::InvalidPlan)?,
        ],
        _ => return Err(Error::UnsupportedKind),
    };
    if plan.kind != CodingEvidenceKind::AcceptanceTest
        && (plan.acceptance_target.is_some() || plan.acceptance_test.is_some())
    {
        return Err(Error::InvalidPlan);
    }
    args.extend(["--manifest-path", manifest_text, "--locked", "--offline"]);
    if plan.kind == CodingEvidenceKind::AcceptanceTest {
        args.push(
            plan.acceptance_test
                .as_deref()
                .filter(|value| safe_test_name(value))
                .ok_or(Error::InvalidPlan)?,
        );
        args.extend(["--", "--exact", "--test-threads=1"]);
    } else if matches!(
        plan.kind,
        CodingEvidenceKind::RuntimeValidation | CodingEvidenceKind::ProductionConnection
    ) {
        args.extend(["--", "--test-threads=1"]);
    }
    let start = Instant::now();
    let process = run_bounded_tool(BoundedToolRequest {
        program: cargo.to_str().ok_or(Error::UnsafePath)?,
        args: &args,
        stdin: None,
        timeout_ms,
        stdout_limit: 1_048_576,
        stderr_limit: 1_048_576,
    });
    if process.state != ToolRunState::Observed
        || process.exit_code != Some(0)
        || process.timed_out
        || process.stdout_truncated
        || process.stderr_truncated
    {
        return Err(Error::ProcessFailed);
    }
    if plan.kind != CodingEvidenceKind::StaticValidation
        && !check_cargo_test_result(plan.kind, &process.stdout)
    {
        return Err(Error::TestsNotObserved);
    }
    if digest_validation_file(&manifest)? != plan.manifest_digest
        || digest_validation_file(&cargo)? != plan.cargo_digest
        || process.executable_sha256.as_deref() != Some(plan.cargo_digest.as_str())
    {
        return Err(Error::StaleInput);
    }
    let observed_at_epoch = authority
        .now_epoch
        .checked_add(start.elapsed().as_secs())
        .ok_or(Error::InvalidPlan)?;
    let after = capture_dgcl_cargo_snapshot(&root, source, pipeline, authority)?;
    if after.snapshot_digest != before.snapshot_digest {
        return Err(Error::StaleInput);
    }
    let after_authority = ExecutionPermitContext {
        now_epoch: observed_at_epoch,
        ..*authority
    };
    if permit.validate(&after_authority) != PermitValidation::ValidGrant {
        return Err(Error::Unauthorized);
    }
    verify_dgcl_cargo_registration(plan, tool_verifier, observed_at_epoch)?;
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Error::InvalidPlan)?
        .as_nanos();
    let plan_digest = stable_sha256(&serde_json::to_string(plan).map_err(|_| Error::InvalidPlan)?);
    let run_digest = stable_sha256(&format!(
        "cargo-validation\0{}\0{unique}\0{}\0{}",
        std::process::id(),
        plan_digest,
        process.args_digest
    ));
    let observation_digest = stable_sha256(
        &serde_json::to_string(&(
            &run_digest,
            &plan_digest,
            &process,
            &lifecycle.snapshot_digest,
        ))
        .map_err(|_| Error::InvalidPlan)?,
    );
    Ok(CheckedDgclCargoObservation {
        source_revision: plan.source_revision.clone(),
        task,
        plan_digest,
        lifecycle: lifecycle.clone(),
        run_digest,
        observation_digest,
        process,
        observed_at_epoch,
        valid_until_epoch: permit.expires_at_epoch(),
    })
}

/// Capture actual declared Rust/Cargo source inputs, never a caller's claimed
/// build digest. Generated targets/caches are excluded; external dependencies,
/// build-script effects and arbitrary non-Rust include_bytes inputs are not a
/// complete hermetic build world and remain outside this profile.
pub fn capture_dgcl_cargo_snapshot(
    root: &Path,
    source: &str,
    pipeline: &DgclPipelineReport,
    authority: &ExecutionPermitContext<'_>,
) -> Result<DgclExecutionSnapshot, DgclValidationProducerError> {
    let mut pending = vec![root.to_path_buf()];
    let mut inputs = BTreeMap::new();
    let mut visited = 0usize;
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).map_err(|_| DgclValidationProducerError::UnsafePath)? {
            visited += 1;
            if visited > 16384 {
                return Err(DgclValidationProducerError::InvalidLifecycle);
            }
            let entry = entry.map_err(|_| DgclValidationProducerError::UnsafePath)?;
            let name = entry.file_name();
            if matches!(name.to_str(), Some("target" | ".git" | ".cache")) {
                continue;
            }
            let path = entry.path();
            let metadata =
                fs::symlink_metadata(&path).map_err(|_| DgclValidationProducerError::UnsafePath)?;
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if metadata.file_attributes() & 0x400 != 0 {
                    return Err(DgclValidationProducerError::UnsafePath);
                }
            }
            if metadata.file_type().is_symlink() {
                return Err(DgclValidationProducerError::UnsafePath);
            }
            if metadata.is_dir() {
                pending.push(path);
                continue;
            }
            if metadata.is_file()
                && (name == "Cargo.toml"
                    || name == "Cargo.lock"
                    || path.extension().and_then(|value| value.to_str()) == Some("rs"))
            {
                if inputs.len() >= 4096 {
                    return Err(DgclValidationProducerError::InvalidLifecycle);
                }
                inputs.insert(
                    path.strip_prefix(root)
                        .map_err(|_| DgclValidationProducerError::UnsafePath)?
                        .to_string_lossy()
                        .replace('\\', "/"),
                    digest_validation_file(&path)?,
                );
            }
        }
    }
    let build =
        serde_json::to_vec(&inputs).map_err(|_| DgclValidationProducerError::InvalidLifecycle)?;
    let mut targets_by_ref = inputs;
    for task in &pipeline.completion_plan.tasks {
        targets_by_ref.insert(task.target_ref.clone(), task.target_digest.clone());
    }
    let targets = targets_by_ref
        .into_iter()
        .map(|(target_ref, target_digest)| crate::DgclTargetIdentity {
            target_ref,
            target_digest,
        })
        .collect();
    crate::build_dgcl_execution_snapshot(
        source,
        &build,
        &format!(
            "dgcl-cargo-offline-check-test.v1/{}",
            dgcl_repository_root_digest(root)?
        ),
        &dgcl_validator_revision(),
        &format!("authority-revision:{}", authority.authority_revision.0),
        targets,
    )
    .map_err(|_| DgclValidationProducerError::InvalidLifecycle)
}

/// Predict one declared file delta for independent intent signing. This is a
/// candidate snapshot, not evidence of a write; the file driver later captures
/// and compares real bytes. All other build inputs and task identities survive.
pub fn project_dgcl_cargo_target_snapshot(
    root: &Path,
    source: &str,
    pipeline: &DgclPipelineReport,
    before: &DgclExecutionSnapshot,
    target: &str,
    after_digest: &str,
    authority_revision: u64,
) -> Result<DgclExecutionSnapshot, DgclValidationProducerError> {
    use DgclValidationProducerError as Error;
    if !crate::verify_dgcl_pipeline_identity(source, pipeline) {
        return Err(Error::InvalidArtifact);
    }
    let virtual_refs = pipeline
        .completion_plan
        .tasks
        .iter()
        .map(|task| task.target_ref.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let mut inputs = before
        .targets
        .iter()
        .filter(|target| !virtual_refs.contains(target.target_ref.as_str()))
        .map(|target| (target.target_ref.clone(), target.target_digest.clone()))
        .collect::<BTreeMap<_, _>>();
    let prior = inputs.get_mut(target).ok_or(Error::InvalidPlan)?;
    *prior = after_digest.into();
    let mut targets = before.targets.clone();
    targets
        .iter_mut()
        .find(|entry| entry.target_ref == target)
        .ok_or(Error::InvalidPlan)?
        .target_digest = after_digest.into();
    crate::build_dgcl_execution_snapshot(
        source,
        &serde_json::to_vec(&inputs).map_err(|_| Error::InvalidLifecycle)?,
        &format!(
            "dgcl-cargo-offline-check-test.v1/{}",
            dgcl_repository_root_digest(root)?
        ),
        &dgcl_validator_revision(),
        &format!("authority-revision:{authority_revision}"),
        targets,
    )
    .map_err(|_| Error::InvalidLifecycle)
}

pub fn issue_dgcl_cargo_evidence(
    observation: CheckedDgclCargoObservation,
    issuer: &ReceiptIssuer,
    verifier: &ReceiptVerifier,
) -> Result<CodingEvidenceClaim, DgclValidationProducerError> {
    use DgclValidationProducerError as Error;
    if issuer.key_fingerprint() != verifier.key_fingerprint() {
        return Err(Error::IssuerMismatch);
    }
    let now = observation.observed_at_epoch;
    let lease_seconds = observation.valid_until_epoch.saturating_sub(now).min(300);
    let lease = issue_dgcl_evidence_lease(&observation.lifecycle, now, lease_seconds)
        .map_err(|_| Error::InvalidLifecycle)?;
    let mut claim = CodingEvidenceClaim {
        requirement_id: 1,
        coding_requirement_id: Some(observation.task.requirement_id),
        coding_task_id: Some(observation.task.task_id),
        kind: observation.task.evidence_kind,
        source_revision: observation.source_revision,
        target: observation.task.target_ref,
        target_digest: observation.task.target_digest,
        producer: Some(observation.task.producer),
        producer_run_digest: Some(observation.run_digest),
        observation_digest: Some(stable_sha256(&format!(
            "{}\0{}",
            observation.plan_digest, observation.observation_digest
        ))),
        lifecycle_lease: Some(lease),
        attestation: None,
    };
    claim.attestation = Some(
        issuer
            .issue(
                ReceiptClass::Closure,
                SubjectRevision::checked(&claim.source_revision).map_err(|_| Error::Receipt)?,
                ReceiptScope::checked(coding_evidence_scope(&claim)).map_err(|_| Error::Receipt)?,
                coding_evidence_payload_digest(&claim),
                now,
                Some(now + lease_seconds),
                None,
            )
            .map_err(|_| Error::Receipt)?,
    );
    Ok(claim)
}

fn safe_test_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && !value.starts_with('-')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b':'))
}

fn check_real_file(path: &Path) -> Result<(), DgclValidationProducerError> {
    for part in path.ancestors() {
        let metadata =
            fs::symlink_metadata(part).map_err(|_| DgclValidationProducerError::UnsafePath)?;
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                return Err(DgclValidationProducerError::UnsafePath);
            }
        }
        if metadata.file_type().is_symlink() {
            return Err(DgclValidationProducerError::UnsafePath);
        }
    }
    if !path.is_file() {
        return Err(DgclValidationProducerError::UnsafePath);
    }
    Ok(())
}

fn checked_validation_path(
    root: &Path,
    relative: &str,
) -> Result<std::path::PathBuf, DgclValidationProducerError> {
    let path = Path::new(relative);
    if path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
        || relative.is_empty()
    {
        return Err(DgclValidationProducerError::UnsafePath);
    }
    let joined = root.join(path);
    check_real_file(&joined)?;
    let canonical = joined
        .canonicalize()
        .map_err(|_| DgclValidationProducerError::UnsafePath)?;
    if !canonical.starts_with(root) {
        return Err(DgclValidationProducerError::UnsafePath);
    }
    Ok(canonical)
}

fn digest_validation_file(path: &Path) -> Result<String, DgclValidationProducerError> {
    let file = File::open(path).map_err(|_| DgclValidationProducerError::StaleInput)?;
    if file
        .metadata()
        .map_err(|_| DgclValidationProducerError::StaleInput)?
        .len()
        > 128 * 1024 * 1024
    {
        return Err(DgclValidationProducerError::StaleInput);
    }
    let mut reader = file.take(128 * 1024 * 1024 + 1);
    let mut hasher = Sha256::new();
    let mut buffer = [0; 8192];
    let mut bytes = 0usize;
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|_| DgclValidationProducerError::StaleInput)?;
        if read == 0 {
            break;
        }
        bytes += read;
        if bytes > 128 * 1024 * 1024 {
            return Err(DgclValidationProducerError::StaleInput);
        }
        hasher.update(&buffer[..read]);
    }
    let hex = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(format!("sha256:{hex}"))
}
