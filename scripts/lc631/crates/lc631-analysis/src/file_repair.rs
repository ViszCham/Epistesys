use crate::{
    build_dgcl_implementation_closure, capture_dgcl_cargo_snapshot,
    dgcl_repair_application_payload_digest, dgcl_repair_application_scope,
    dgcl_repair_request_digest, dgcl_repair_validation_payload_digest,
    dgcl_repair_validation_scope, issue_dgcl_evidence_lease, repair_request_authorization_scope,
    CodingEvidenceClaim, DgclExecutionSnapshot, DgclImplementationClosureContext,
    DgclImplementationStatus, DgclPipelineReport, DgclRepairApplication, DgclRepairAuthorization,
    DgclRepairDriver, DgclRepairDriverError, DgclRepairRequest, DgclRepairValidation,
    DgclRepairValidationOutcome, ImplementationGapState,
};
use lc631_core::{
    stable_sha256, Action, CallerOrigin, ExecutionPermit, ExecutionPermitContext, PermitValidation,
};
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptIssuer, ReceiptScope, ReceiptVerifier, ReplayGuard, SubjectRevision,
};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, Path};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub trait DgclFileEvidenceValidator {
    /// Return newly observed receipts, not a boolean or a list of supposedly
    /// closed gaps. The driver and coordinator recompute the entire closure.
    fn observe(
        &mut self,
        snapshot: &DgclExecutionSnapshot,
        budget: Duration,
    ) -> Result<Vec<CodingEvidenceClaim>, DgclRepairDriverError>;
}

pub struct DgclFileRepairConfig<'a> {
    pub root: &'a Path,
    pub source: &'a str,
    pub pipeline: &'a DgclPipelineReport,
    pub initial_snapshot: &'a DgclExecutionSnapshot,
    pub authorization: &'a DgclRepairAuthorization<'a>,
    pub issuer: &'a ReceiptIssuer,
    pub verifier: &'a ReceiptVerifier,
    pub candidates: BTreeMap<String, String>,
    pub validator: &'a mut dyn DgclFileEvidenceValidator,
    pub epoch_clock: Option<fn() -> u64>,
}

pub struct DgclFileRepairDriver<'a> {
    config: DgclFileRepairConfig<'a>,
    snapshot: DgclExecutionSnapshot,
    started: Instant,
}

impl<'a> DgclFileRepairDriver<'a> {
    pub fn new(config: DgclFileRepairConfig<'a>) -> Result<Self, DgclRepairDriverError> {
        if !crate::verify_dgcl_pipeline_identity(config.source, config.pipeline)
            || config.issuer.key_fingerprint() != config.verifier.key_fingerprint()
            || config.candidates.len() > 4
            || config
                .candidates
                .iter()
                .any(|(digest, value)| value.len() > 1024 * 1024 || digest != &stable_sha256(value))
        {
            return Err(DgclRepairDriverError::Failed);
        }
        Ok(Self {
            snapshot: config.initial_snapshot.clone(),
            config,
            started: Instant::now(),
        })
    }

    fn now(&self) -> u64 {
        let logical = self
            .config
            .authorization
            .now_epoch
            .saturating_add(self.started.elapsed().as_secs());
        self.config
            .epoch_clock
            .map_or(logical, |clock| clock().max(logical))
    }

    fn authority<'b>(
        &'b self,
        permit: &'b ExecutionPermit,
        scope: &'b str,
    ) -> ExecutionPermitContext<'b> {
        ExecutionPermitContext {
            principal: permit.principal_binding(),
            action: Action::Edit,
            scope,
            source_revision: &self.config.pipeline.source_revision,
            authority_revision: self.config.authorization.authority_revision,
            now_epoch: self.now(),
            revocation_revision: self.config.authorization.revocation_revision,
            revoked_permit_digests: self.config.authorization.revoked_permit_digests,
            caller_origin: CallerOrigin::HostVerifiedUser,
            trusted_host_fingerprint: self.config.authorization.trusted_host_fingerprint,
        }
    }

    fn capture(
        &self,
        permit: &ExecutionPermit,
        scope: &str,
    ) -> Result<DgclExecutionSnapshot, DgclRepairDriverError> {
        capture_dgcl_cargo_snapshot(
            self.config.root,
            self.config.source,
            self.config.pipeline,
            &self.authority(permit, scope),
        )
        .map_err(|_| DgclRepairDriverError::Failed)
    }
}

impl DgclRepairDriver for DgclFileRepairDriver<'_> {
    fn observation_epoch(&self) -> Option<u64> {
        self.config.epoch_clock.map(|_| self.now())
    }
    fn apply(
        &mut self,
        request: &DgclRepairRequest,
        permit: &ExecutionPermit,
        remaining_time: Duration,
        remaining_delta_bytes: u64,
    ) -> Result<DgclRepairApplication, DgclRepairDriverError> {
        let start = Instant::now();
        let fail = DgclRepairDriverError::Failed;
        if remaining_time.is_zero()
            || request.source_revision != self.config.pipeline.source_revision
            || request.plan_digest != self.config.pipeline.completion_plan.plan_digest
            || request.authorization_scope != repair_request_authorization_scope(request)
            || request.repository_root_digest.as_deref()
                != Some(
                    crate::dgcl_repository_root_digest(self.config.root)
                        .map_err(|_| fail)?
                        .as_str(),
                )
            || request.authorization_span != permit.source_span()
            || permit.validate(&self.authority(permit, &request.authorization_scope))
                != PermitValidation::ValidGrant
        {
            return Err(fail);
        }
        let root = checked_root(self.config.root)?;
        let relative = Path::new(&request.target_ref);
        if relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
            || relative.extension().and_then(|value| value.to_str()) != Some("rs")
        {
            return Err(fail);
        }
        let target = root.join(relative);
        check_path(&target)?;
        if !target.canonicalize().map_err(|_| fail)?.starts_with(&root) {
            return Err(fail);
        }
        let candidate = self
            .config
            .candidates
            .get(&request.candidate_digest)
            .ok_or(fail)?
            .clone();
        let parsed = lc631_tldg::analyze_dg1(&candidate, lc631_tldg::Dg1Budget::default())
            .map_err(|_| fail)?;
        if parsed
            .rust_trees
            .first()
            .is_none_or(|tree| tree.root_has_error)
        {
            return Err(fail);
        }
        let current = self.capture(permit, &request.authorization_scope)?;
        if current.snapshot_digest != self.snapshot.snapshot_digest {
            return Err(fail);
        }
        let mut options = OpenOptions::new();
        options.read(true).write(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(0);
        }
        let mut file = options.open(&target).map_err(|_| fail)?;
        file.try_lock().map_err(|_| fail)?;
        if file.metadata().map_err(|_| fail)?.len() > 1024 * 1024 {
            return Err(fail);
        }
        let mut before = String::new();
        (&mut file)
            .take(1024 * 1024 + 1)
            .read_to_string(&mut before)
            .map_err(|_| fail)?;
        if before.len() > 1024 * 1024 || stable_sha256(&before) != request.target_digest_before {
            return Err(fail);
        }
        let delta = before.len().max(candidate.len()) as u64;
        if delta > remaining_delta_bytes {
            return Err(fail);
        }
        // Preserve recoverable preimage bytes. Never delete/overwrite backups.
        let backups = root.join(".dgcl-repair-backups");
        if !backups.exists() {
            fs::create_dir(&backups).map_err(|_| fail)?;
        }
        checked_root(&backups)?;
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| fail)?
            .as_nanos();
        let run = stable_sha256(&format!(
            "file-repair:{}:{unique}:{}",
            std::process::id(),
            dgcl_repair_request_digest(request)
        ));
        let mut backup = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(backups.join(format!("{}-{unique}.backup", request.attempt)))
            .map_err(|_| fail)?;
        backup
            .write_all(before.as_bytes())
            .and_then(|_| backup.sync_all())
            .map_err(|_| fail)?;
        if start.elapsed() >= remaining_time
            || permit.validate(&self.authority(permit, &request.authorization_scope))
                != PermitValidation::ValidGrant
        {
            return Err(fail);
        }
        file.seek(SeekFrom::Start(0))
            .and_then(|_| file.write_all(candidate.as_bytes()))
            .and_then(|_| file.set_len(candidate.len() as u64))
            .and_then(|_| file.sync_all())
            .map_err(|_| fail)?;
        drop(file);
        // A post-write failure is explicitly MayHaveApplied in the coordinator.
        if start.elapsed() >= remaining_time {
            return Err(DgclRepairDriverError::TimedOut);
        }
        if permit.validate(&self.authority(permit, &request.authorization_scope))
            != PermitValidation::ValidGrant
        {
            return Err(fail);
        }
        let after = self.capture(permit, &request.authorization_scope)?;
        let now = self.now();
        let lease = issue_dgcl_evidence_lease(&after, now, 300).map_err(|_| fail)?;
        let mut application = DgclRepairApplication {
            schema_version: "epistesys-dgcl-repair-application.v1",
            attempt: request.attempt,
            gap_id: request.gap_id.clone(),
            request_digest: dgcl_repair_request_digest(request),
            source_revision: request.source_revision.clone(),
            plan_digest: request.plan_digest.clone(),
            target_ref: request.target_ref.clone(),
            target_digest_before: request.target_digest_before.clone(),
            target_digest_after: stable_sha256(&candidate),
            candidate_digest: request.candidate_digest.clone(),
            delta_bytes: delta,
            run_digest: run,
            permit_digest: permit.digest().into(),
            applied: before != candidate,
            after_snapshot: after.clone(),
            after_lease: lease,
            attestation: None,
        };
        application.attestation = Some(
            self.config
                .issuer
                .issue(
                    ReceiptClass::ToolExecution,
                    SubjectRevision::checked(&request.source_revision).map_err(|_| fail)?,
                    ReceiptScope::checked(dgcl_repair_application_scope(request))
                        .map_err(|_| fail)?,
                    dgcl_repair_application_payload_digest(&application),
                    now,
                    Some(now.checked_add(300).ok_or(fail)?),
                    None,
                )
                .map_err(|_| fail)?,
        );
        self.snapshot = after;
        Ok(application)
    }

    fn revalidate(
        &mut self,
        request: &DgclRepairRequest,
        application: &DgclRepairApplication,
        remaining_time: Duration,
    ) -> Result<DgclRepairValidation, DgclRepairDriverError> {
        let start = Instant::now();
        let fail = DgclRepairDriverError::Failed;
        let claims = self
            .config
            .validator
            .observe(&self.snapshot, remaining_time)?;
        if start.elapsed() >= remaining_time {
            return Err(DgclRepairDriverError::TimedOut);
        }
        let now = self.now();
        let closure = build_dgcl_implementation_closure(
            self.config.source,
            &self.config.pipeline.program_ir,
            &self.config.pipeline.translation_projection,
            &self.config.pipeline.completion_plan,
            &claims,
            DgclImplementationClosureContext {
                current_lifecycle: Some(&self.snapshot),
                verifier: Some(self.config.verifier),
                replay: &mut ReplayGuard::default(),
                now_epoch: now,
            },
        )
        .map_err(|_| fail)?;
        let remaining = closure
            .gaps
            .iter()
            .filter(|gap| {
                !matches!(
                    gap.state,
                    ImplementationGapState::VerifiedReceipt | ImplementationGapState::NotApplicable
                )
            })
            .map(|gap| gap.gap_id.clone())
            .collect();
        let mut validation = DgclRepairValidation {
            schema_version: "epistesys-dgcl-repair-validation.v1",
            attempt: request.attempt,
            gap_id: request.gap_id.clone(),
            request_digest: application.request_digest.clone(),
            source_revision: request.source_revision.clone(),
            plan_digest: request.plan_digest.clone(),
            run_digest: application.run_digest.clone(),
            snapshot_digest: self.snapshot.snapshot_digest.clone(),
            validator_digest: self.snapshot.validator_digest.clone(),
            closure_digest: stable_sha256(&serde_json::to_string(&closure).map_err(|_| fail)?),
            outcome: if closure.status == DgclImplementationStatus::ImplementationClosed {
                DgclRepairValidationOutcome::ImplementationClosed
            } else if closure.status == DgclImplementationStatus::Conflict {
                DgclRepairValidationOutcome::Conflict
            } else {
                DgclRepairValidationOutcome::Pending
            },
            remaining_gap_ids: remaining,
            attestation: None,
            closure_claims: Some(claims),
        };
        validation.attestation = Some(
            self.config
                .issuer
                .issue(
                    ReceiptClass::Validation,
                    SubjectRevision::checked(&request.source_revision).map_err(|_| fail)?,
                    ReceiptScope::checked(dgcl_repair_validation_scope(request))
                        .map_err(|_| fail)?,
                    dgcl_repair_validation_payload_digest(&validation),
                    now,
                    Some(now.checked_add(300).ok_or(fail)?),
                    None,
                )
                .map_err(|_| fail)?,
        );
        Ok(validation)
    }
}

fn checked_root(root: &Path) -> Result<std::path::PathBuf, DgclRepairDriverError> {
    check_path(root)?;
    if !root.is_dir() {
        return Err(DgclRepairDriverError::Failed);
    }
    root.canonicalize()
        .map_err(|_| DgclRepairDriverError::Failed)
}

fn check_path(path: &Path) -> Result<(), DgclRepairDriverError> {
    let absolute = std::path::absolute(path).map_err(|_| DgclRepairDriverError::Failed)?;
    for ancestor in absolute.ancestors() {
        let metadata = fs::symlink_metadata(ancestor).map_err(|_| DgclRepairDriverError::Failed)?;
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                return Err(DgclRepairDriverError::Failed);
            }
        }
        if metadata.file_type().is_symlink() {
            return Err(DgclRepairDriverError::Failed);
        }
    }
    Ok(())
}
