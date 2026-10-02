use super::dgcl_package::{finalize_with_pipeline, PackageSeedReceipt, PackageValidationBundle};
use lc631_analysis::*;
use lc631_core::{
    Action, ArtifactId, AuthorityRevision, CallerOrigin, ExecutionPermitContext, SourceSpan, TurnId,
};
use lc631_host::{HostExecutionPermitAdmission, HostReceiptContext, HostSeedEnvelope};
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptPolicy, ReceiptScope, ReplayGuard, SubjectRevision,
};
use serde::Deserialize;
use std::path::Path;
use std::time::Duration;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageRepairBundle {
    pub schema_version: String,
    pub request: DgclRepairRequest,
    pub replacement_utf8: String,
    pub edit_seed: PackageSeedReceipt,
    pub validation: PackageValidationBundle,
}

struct PackageRevalidator<'a> {
    root: &'a Path,
    source: &'a str,
    pipeline: &'a DgclPipelineReport,
    bundle: PackageValidationBundle,
    host: &'a HostReceiptContext,
    author: &'a HostReceiptContext,
}

fn receipt_epoch() -> u64 {
    // A failed wall clock expires permits rather than extending their lifetime.
    super::current_epoch_seconds().unwrap_or(u64::MAX)
}

impl DgclFileEvidenceValidator for PackageRevalidator<'_> {
    fn observe(
        &mut self,
        _snapshot: &DgclExecutionSnapshot,
        budget: Duration,
    ) -> Result<Vec<CodingEvidenceClaim>, DgclRepairDriverError> {
        let report = finalize_with_pipeline(
            self.root,
            self.source,
            self.bundle.clone(),
            self.host,
            self.author,
            self.pipeline,
            budget,
        )
        .map_err(|_| DgclRepairDriverError::Failed)?;
        serde_json::from_value(report["claims"].clone()).map_err(|_| DgclRepairDriverError::Failed)
    }
}

#[allow(clippy::too_many_arguments)]
pub fn repair_package(
    root: &Path,
    source: &str,
    bundle: PackageRepairBundle,
    host: &HostReceiptContext,
    author: &HostReceiptContext,
    journal_roots: Option<(&Path, &Path)>,
) -> Result<serde_json::Value, String> {
    if bundle.schema_version != "epistesys-dgcl-package-repair-bundle.v1"
        || bundle.request.attempt != 1
        || bundle.replacement_utf8.len() > 1024 * 1024
        || lc631_core::stable_sha256(&bundle.replacement_utf8) != bundle.request.candidate_digest
        || host.key_fingerprint() == author.key_fingerprint()
    {
        return Err("dgcl_package_repair_shape_or_issuer".into());
    }
    let pipeline = build_dgcl_pipeline(
        source,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .map_err(|error| format!("dgcl_package_repair_pipeline:{error:?}"))?;
    let count = pipeline.completion_plan.requirements.len();
    if count == 0
        || count > 8
        || !pipeline.completion_plan.blockers.is_empty()
        || bundle.validation.plans.len() != count * 4
        || bundle.validation.seeds.len() != count * 4
        || bundle.validation.binding_receipts.len() != count - 1
        || pipeline
            .completion_plan
            .tasks
            .iter()
            .any(|task| task.applicability != TaskApplicability::Required)
    {
        return Err("dgcl_repair_validation_shape".into());
    }
    let scope = repair_request_authorization_scope(&bundle.request);
    let now = super::current_epoch_seconds()?;
    let edit_authority = bundle
        .edit_seed
        .authority_receipt
        .ok_or("dgcl_repair_external_edit_authority_missing")?;
    let seed = HostSeedEnvelope::verify_attested_scoped(
        ArtifactId(bundle.edit_seed.artifact_id),
        TurnId(bundle.edit_seed.turn_id),
        source.into(),
        SourceSpan {
            start: bundle.edit_seed.user_start,
            end: bundle.edit_seed.user_end,
        },
        scope.clone(),
        bundle.edit_seed.receipt,
        host.verifier(),
        &mut ReplayGuard::default(),
        now,
    )
    .map_err(|error| format!("dgcl_package_repair_ingress:{error:?}"))?;
    let permit = host
        .admit_execution_permit(
            &seed,
            HostExecutionPermitAdmission {
                action: Action::Edit,
                scope: scope.clone(),
                span: bundle.request.authorization_span,
                authority_revision: AuthorityRevision(bundle.validation.authority_revision),
                revocation_revision: bundle.validation.revocation_revision,
                now_epoch: now,
                attestation: edit_authority,
            },
            &mut ReplayGuard::default(),
        )
        .map_err(|error| format!("dgcl_package_repair_permit:{error:?}"))?;
    let principal = seed
        .principal_binding()
        .map_err(|error| format!("dgcl_repair_principal:{error:?}"))?;
    let fingerprint = host.key_fingerprint();
    let authority = ExecutionPermitContext {
        principal: &principal,
        action: Action::Edit,
        scope: &scope,
        source_revision: &pipeline.source_revision,
        authority_revision: AuthorityRevision(bundle.validation.authority_revision),
        now_epoch: now,
        revocation_revision: bundle.validation.revocation_revision,
        revoked_permit_digests: &[],
        caller_origin: CallerOrigin::HostVerifiedUser,
        trusted_host_fingerprint: &fingerprint,
    };
    let snapshot = capture_dgcl_cargo_snapshot(root, source, &pipeline, &authority)
        .map_err(|error| format!("dgcl_repair_snapshot:{error:?}"))?;
    // Prediction supports independent intent signing, not evidence of a write.
    let predicted = project_dgcl_cargo_target_snapshot(
        root,
        source,
        &pipeline,
        &snapshot,
        &bundle.request.target_ref,
        &bundle.request.candidate_digest,
        bundle.validation.authority_revision,
    )
    .map_err(|error| format!("dgcl_repair_prediction:{error:?}"))?;
    let binding_tasks = pipeline
        .completion_plan
        .tasks
        .iter()
        .filter(|task| task.evidence_kind == CodingEvidenceKind::ImplementationBinding)
        .collect::<Vec<_>>();
    let mut bindings = vec![bundle.validation.binding_receipt.clone()];
    bindings.extend(bundle.validation.binding_receipts.clone());
    for (task, receipt) in binding_tasks.into_iter().zip(bindings) {
        author
            .verifier()
            .verify(
                receipt,
                &ReceiptPolicy::exact(
                    ReceiptClass::Closure,
                    SubjectRevision::checked(&pipeline.source_revision)
                        .map_err(|_| "dgcl_repair_subject")?,
                    ReceiptScope::checked(format!("dgcl/implementation-binding/{}", task.task_id))
                        .map_err(|_| "dgcl_repair_binding_scope")?,
                    dgcl_implementation_binding_payload(&pipeline, &task.task_id, &predicted),
                    now,
                ),
                &mut ReplayGuard::default(),
            )
            .map_err(|error| format!("dgcl_repair_predicted_binding:{error:?}"))?;
    }
    let expected = pipeline
        .completion_plan
        .tasks
        .iter()
        .filter(|task| task.evidence_kind != CodingEvidenceKind::ImplementationBinding)
        .map(|task| task.task_id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let supplied = bundle
        .validation
        .plans
        .iter()
        .map(|plan| plan.task_id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    if supplied != expected || supplied.len() != bundle.validation.plans.len() {
        return Err("dgcl_repair_validation_task_set".into());
    }
    let mut preflight_replay = ReplayGuard::default();
    for (plan, seed_input) in bundle.validation.plans.iter().zip(&bundle.validation.seeds) {
        preflight_dgcl_cargo_plan(root, &pipeline, plan)
            .map_err(|error| format!("dgcl_repair_plan_preflight:{error:?}"))?;
        if plan.source_revision != pipeline.source_revision
            || plan.completion_plan_digest != pipeline.completion_plan.plan_digest
            || plan.repository_root_digest
                != dgcl_repository_root_digest(root).map_err(|_| "dgcl_repair_root")?
        {
            return Err("dgcl_repair_validation_identity".into());
        }
        verify_dgcl_cargo_registration(plan, host.verifier(), now)
            .map_err(|error| format!("dgcl_repair_tool:{error:?}"))?;
        let test_scope = dgcl_cargo_validation_scope(plan);
        let test_seed = HostSeedEnvelope::verify_attested_scoped(
            ArtifactId(seed_input.artifact_id),
            TurnId(seed_input.turn_id),
            source.into(),
            SourceSpan {
                start: seed_input.user_start,
                end: seed_input.user_end,
            },
            test_scope.clone(),
            seed_input.receipt.clone(),
            host.verifier(),
            &mut preflight_replay,
            now,
        )
        .map_err(|error| format!("dgcl_repair_test_ingress:{error:?}"))?;
        host.admit_execution_permit(
            &test_seed,
            HostExecutionPermitAdmission {
                action: Action::Test,
                scope: test_scope,
                span: test_seed.user_span,
                authority_revision: AuthorityRevision(bundle.validation.authority_revision),
                revocation_revision: bundle.validation.revocation_revision,
                now_epoch: now,
                attestation: seed_input
                    .authority_receipt
                    .clone()
                    .ok_or("dgcl_repair_external_test_authority_missing")?,
            },
            &mut preflight_replay,
        )
        .map_err(|error| format!("dgcl_repair_test_permit:{error:?}"))?;
    }
    let initial_gaps = pipeline
        .implementation_closure
        .gaps
        .iter()
        .filter(|gap| {
            !matches!(
                gap.state,
                ImplementationGapState::VerifiedReceipt | ImplementationGapState::NotApplicable
            )
        })
        .map(|gap| gap.gap_id.clone())
        .collect::<Vec<_>>();
    let execution_now = super::current_epoch_seconds()?;
    let authorization = DgclRepairAuthorization {
        permit: &permit,
        authority_revision: AuthorityRevision(bundle.validation.authority_revision),
        trusted_host_fingerprint: &fingerprint,
        now_epoch: execution_now,
        revocation_revision: bundle.validation.revocation_revision,
        revoked_permit_digests: &[],
    };
    let mut revalidator = PackageRevalidator {
        root,
        source,
        pipeline: &pipeline,
        bundle: bundle.validation,
        host,
        author,
    };
    let mut candidates = std::collections::BTreeMap::new();
    candidates.insert(
        bundle.request.candidate_digest.clone(),
        bundle.replacement_utf8,
    );
    let initial_lease = issue_dgcl_evidence_lease(&snapshot, execution_now, 300)
        .map_err(|error| format!("dgcl_repair_lease:{error:?}"))?;
    let mut driver = DgclFileRepairDriver::new(DgclFileRepairConfig {
        root,
        source,
        pipeline: &pipeline,
        initial_snapshot: &snapshot,
        authorization: &authorization,
        issuer: host.issuer(),
        verifier: host.verifier(),
        candidates,
        validator: &mut revalidator,
        epoch_clock: Some(receipt_epoch),
    })
    .map_err(|error| format!("dgcl_repair_driver:{error:?}"))?;
    let journal = journal_roots
        .map(|(ledger, heads)| {
            super::dgcl_journal::RepairJournal::new(
                ledger,
                heads,
                host,
                &bundle.request,
                authorization.authority_revision.0,
            )
        })
        .transpose()?;
    if let Some(journal) = &journal {
        journal.start()?;
    }
    let report = run_dgcl_repair_loop(
        DgclRepairLoopInput {
            source,
            source_revision: &pipeline.source_revision,
            plan_digest: &pipeline.completion_plan.plan_digest,
            program_ir: &pipeline.program_ir,
            translation: &pipeline.translation_projection,
            completion_plan: &pipeline.completion_plan,
            initial_snapshot: &snapshot,
            initial_lease: &initial_lease,
            initial_gap_ids: &initial_gaps,
            requests: &[bundle.request],
            authorization: DgclRepairAuthorization {
                permit: &permit,
                authority_revision: authorization.authority_revision,
                trusted_host_fingerprint: &fingerprint,
                now_epoch: execution_now,
                revocation_revision: authorization.revocation_revision,
                revoked_permit_digests: &[],
            },
            verifier: host.verifier(),
            replay: &mut ReplayGuard::default(),
            max_attempts: 4,
            max_wall_time: Duration::from_secs(120),
            max_delta_bytes: 1024 * 1024,
        },
        &mut driver,
    )
    .map_err(|error| format!("dgcl_package_repair:{error:?}"))?;
    let mut output = serde_json::json!({ "schema_version": "epistesys-dgcl-package-repair.v1", "repair": report,
        "authority_created": false, "host_send_authorized": false, "output_commit_allowed": false,
        "host_observation": "pending", "research_evaluation": "pending_no_corpus" });
    if let Some(journal) = &journal {
        journal.finish(&output, report.status == DgclRepairStatus::Completed)?;
        output["journal"] =
            serde_json::to_value(journal.inspect()?).map_err(|_| "repair_journal_encode")?;
    }
    Ok(output)
}
