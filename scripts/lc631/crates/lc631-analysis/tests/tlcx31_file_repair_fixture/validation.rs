//! Partial claims come only from actual checked observations and fresh intent.
use super::authority::{connection_spec, file_hash, permit, registration};
use lc631_analysis::*;
use lc631_core::{Action, AuthorityRevision, CallerOrigin, ExecutionPermitContext};
use lc631_receipt_kernel::*;
use std::path::Path;
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct ValidationTrace {
    pub snapshots: Vec<String>,
    pub intent_snapshots: Vec<String>,
    pub failures: Vec<(String, CodingEvidenceKind, DgclValidationProducerError)>,
    pub successes: Vec<(String, CodingEvidenceKind)>,
}

pub struct PartialFileValidator<'a> {
    pub root: &'a Path,
    pub source: &'a str,
    pub pipeline: &'a DgclPipelineReport,
    pub issuer: &'a ReceiptIssuer,
    pub verifier: &'a ReceiptVerifier,
    pub author: &'a ReceiptIssuer,
    pub author_verifier: &'a ReceiptVerifier,
    pub cargo: &'a Path,
    pub trace: &'a mut ValidationTrace,
}

impl PartialFileValidator<'_> {
    fn binding(
        &self,
        task: &PlannedEvidenceTask,
        snapshot: &DgclExecutionSnapshot,
    ) -> Result<CodingEvidenceClaim, DgclRepairDriverError> {
        let fail = DgclRepairDriverError::Failed;
        let receipt = self
            .author
            .issue(
                ReceiptClass::Closure,
                SubjectRevision::checked(&self.pipeline.source_revision).map_err(|_| fail)?,
                ReceiptScope::checked(format!("dgcl/implementation-binding/{}", task.task_id))
                    .map_err(|_| fail)?,
                dgcl_implementation_binding_payload(self.pipeline, &task.task_id, snapshot),
                100,
                Some(400),
                None,
            )
            .map_err(|_| fail)?;
        issue_dgcl_implementation_binding(
            self.source,
            self.pipeline,
            &task.task_id,
            snapshot,
            DgclBindingIssuerContext {
                author_receipt: receipt,
                author_verifier: self.author_verifier,
                issuer: self.issuer,
                verifier: self.verifier,
                now_epoch: 100,
                replay: &mut ReplayGuard::default(),
            },
        )
        .map_err(|_| fail)
    }

    fn plan(&self, task: &PlannedEvidenceTask, remaining: Duration) -> DgclCargoValidationPlan {
        DgclCargoValidationPlan {
            schema_version: "epistesys-dgcl-cargo-validation-plan.v1".into(),
            source_revision: self.pipeline.source_revision.clone(),
            completion_plan_digest: self.pipeline.completion_plan.plan_digest.clone(),
            requirement_id: task.requirement_id.clone(),
            task_id: task.task_id.clone(),
            kind: task.evidence_kind,
            cargo_path: self.cargo.to_string_lossy().into_owned(),
            cargo_digest: file_hash(self.cargo),
            manifest_path: "Cargo.toml".into(),
            manifest_digest: file_hash(&self.root.join("Cargo.toml")),
            repository_root_digest: dgcl_repository_root_digest(self.root).unwrap(),
            timeout_ms: remaining.as_millis().min(60_000) as u64,
            tool_registration: Some(registration(self.issuer, self.cargo)),
            acceptance_target: (task.evidence_kind == CodingEvidenceKind::AcceptanceTest)
                .then(|| "acceptance".into()),
            acceptance_test: (task.evidence_kind == CodingEvidenceKind::AcceptanceTest)
                .then(|| "required_checked_arithmetic_contract".into()),
            connection: (task.evidence_kind == CodingEvidenceKind::ProductionConnection)
                .then(|| connection_spec(self.root, self.issuer)),
        }
    }
}

impl DgclFileEvidenceValidator for PartialFileValidator<'_> {
    fn observe(
        &mut self,
        snapshot: &DgclExecutionSnapshot,
        budget: Duration,
    ) -> Result<Vec<CodingEvidenceClaim>, DgclRepairDriverError> {
        let start = Instant::now();
        self.trace.snapshots.push(snapshot.snapshot_digest.clone());
        let mut claims = Vec::new();
        for task in &self.pipeline.completion_plan.tasks {
            let remaining = budget
                .checked_sub(start.elapsed())
                .filter(|n| !n.is_zero())
                .ok_or(DgclRepairDriverError::TimedOut)?;
            if task.evidence_kind == CodingEvidenceKind::ImplementationBinding {
                claims.push(self.binding(task, snapshot)?);
                self.trace
                    .intent_snapshots
                    .push(snapshot.snapshot_digest.clone());
                continue;
            }
            let plan = self.plan(task, remaining);
            let scope = dgcl_cargo_validation_scope(&plan);
            let authorization = permit(
                self.source,
                &scope,
                self.issuer,
                self.verifier,
                Action::Test,
            );
            let authority = ExecutionPermitContext {
                principal: authorization.principal_binding(),
                action: Action::Test,
                scope: &scope,
                source_revision: &self.pipeline.source_revision,
                authority_revision: AuthorityRevision(1),
                now_epoch: 100,
                revocation_revision: 0,
                revoked_permit_digests: &[],
                caller_origin: CallerOrigin::HostVerifiedUser,
                trusted_host_fingerprint: self.verifier.key_fingerprint(),
            };
            match observe_dgcl_cargo_validation(
                self.root,
                self.source,
                self.pipeline,
                &plan,
                DgclCargoExecutionContext {
                    lifecycle: snapshot,
                    permit: &authorization,
                    authority: &authority,
                    tool_verifier: self.verifier,
                    max_duration: remaining,
                },
            ) {
                Ok(observed) => {
                    let claim = issue_dgcl_cargo_evidence(observed, self.issuer, self.verifier)
                        .map_err(|_| DgclRepairDriverError::Failed)?;
                    self.trace
                        .successes
                        .push((snapshot.snapshot_digest.clone(), task.evidence_kind));
                    claims.push(claim);
                }
                Err(
                    error @ (DgclValidationProducerError::ProcessFailed
                    | DgclValidationProducerError::TestsNotObserved
                    | DgclValidationProducerError::ConnectionNotObserved),
                ) => {
                    // Only actual API errors; no invented process struct or success.
                    self.trace.failures.push((
                        snapshot.snapshot_digest.clone(),
                        task.evidence_kind,
                        error,
                    ));
                }
                Err(_) => return Err(DgclRepairDriverError::Failed),
            }
        }
        // The actual file driver computes Pending/Closed from this partial set.
        Ok(claims)
    }
}
