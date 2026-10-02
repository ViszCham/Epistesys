use lc631_analysis::{
    build_dgcl_pipeline, build_verified_dgcl_completion_candidate, capture_dgcl_cargo_snapshot,
    dgcl_cargo_validation_scope, issue_dgcl_cargo_evidence, issue_dgcl_implementation_binding,
    observe_dgcl_cargo_validation, DgclBackendSet, DgclBindingIssuerContext,
    DgclCargoValidationPlan, DgclImplementationClosureContext, DgclPipelineReport,
};
use lc631_core::{
    Action, ArtifactId, AuthorityRevision, CallerOrigin, ExecutionPermitContext, SourceSpan, TurnId,
};
use lc631_host::{HostReceiptContext, HostSeedEnvelope};
use lc631_receipt_kernel::{ReplayGuard, UntrustedReceipt};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageSeedReceipt {
    pub artifact_id: u64,
    pub turn_id: u64,
    pub user_start: usize,
    pub user_end: usize,
    pub receipt: UntrustedReceipt,
    #[serde(default)]
    pub authority_receipt: Option<UntrustedReceipt>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageValidationBundle {
    pub schema_version: String,
    pub authority_revision: u64,
    pub revocation_revision: u64,
    pub plans: Vec<DgclCargoValidationPlan>,
    pub seeds: Vec<PackageSeedReceipt>,
    pub binding_receipt: UntrustedReceipt,
    #[serde(default)]
    pub binding_receipts: Vec<UntrustedReceipt>,
}

pub fn finalize_package(
    root: &Path,
    source: &str,
    bundle: PackageValidationBundle,
    context: &HostReceiptContext,
    author: &HostReceiptContext,
) -> Result<Value, String> {
    let pipeline = build_dgcl_pipeline(
        source,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .map_err(|error| format!("dgcl_package_pipeline:{error:?}"))?;
    finalize_with_pipeline(
        root,
        source,
        bundle,
        context,
        author,
        &pipeline,
        std::time::Duration::from_secs(120),
    )
}

pub(super) fn finalize_with_pipeline(
    root: &Path,
    source: &str,
    bundle: PackageValidationBundle,
    context: &HostReceiptContext,
    author: &HostReceiptContext,
    pipeline: &DgclPipelineReport,
    budget: std::time::Duration,
) -> Result<Value, String> {
    let count = pipeline.completion_plan.requirements.len();
    if bundle.schema_version != "epistesys-dgcl-package-validation-bundle.v1"
        || count == 0
        || count > 8
        || bundle.plans.len() != count * 4
        || bundle.seeds.len() != bundle.plans.len()
        || bundle.binding_receipts.len() != count - 1
    {
        return Err("dgcl_package_bundle_shape".into());
    }
    if !lc631_analysis::verify_dgcl_pipeline_identity(source, pipeline)
        || !pipeline.completion_plan.blockers.is_empty()
        || pipeline
            .completion_plan
            .tasks
            .iter()
            .any(|task| task.applicability != lc631_analysis::TaskApplicability::Required)
    {
        return Err("dgcl_package_profile_requires_unconditional_required_tasks".into());
    }
    let mut tasks = BTreeSet::new();
    for plan in &bundle.plans {
        lc631_analysis::preflight_dgcl_cargo_plan(root, pipeline, plan)
            .map_err(|error| format!("dgcl_package_plan_preflight:{error:?}"))?;
        if lc631_analysis::dgcl_repository_root_digest(root)
            .map_err(|error| format!("dgcl_package_root:{error:?}"))?
            != plan.repository_root_digest
        {
            return Err("dgcl_package_repository_scope_mismatch".into());
        }
        lc631_analysis::verify_dgcl_cargo_registration(
            plan,
            context.verifier(),
            super::current_epoch_seconds()?,
        )
        .map_err(|error| format!("dgcl_package_tool_registration:{error:?}"))?;
        if !tasks.insert(plan.task_id.clone())
            || plan.source_revision != pipeline.source_revision
            || plan.completion_plan_digest != pipeline.completion_plan.plan_digest
        {
            return Err("dgcl_package_plan_identity".into());
        }
    }
    let expected_tasks = pipeline
        .completion_plan
        .tasks
        .iter()
        .filter(|task| {
            task.evidence_kind != lc631_analysis::CodingEvidenceKind::ImplementationBinding
        })
        .map(|task| task.task_id.clone())
        .collect::<BTreeSet<_>>();
    if tasks != expected_tasks {
        return Err("dgcl_package_task_set".into());
    }
    let mut replay = ReplayGuard::default();
    // Verify every host-origin capability before starting any child. A signed
    // seed is not itself a global grant; each scope is the whole command digest.
    let mut authorized = Vec::new();
    for (plan, seed_input) in bundle.plans.iter().zip(bundle.seeds) {
        let now = super::current_epoch_seconds()?;
        let scope = dgcl_cargo_validation_scope(plan);
        seed_input
            .receipt
            .claims()
            .expires_at_epoch()
            .ok_or("dgcl_package_ingress_expiry_required")?;
        let seed = HostSeedEnvelope::verify_attested_scoped(
            ArtifactId(seed_input.artifact_id),
            TurnId(seed_input.turn_id),
            source.to_string(),
            SourceSpan {
                start: seed_input.user_start,
                end: seed_input.user_end,
            },
            scope.clone(),
            seed_input.receipt,
            context.verifier(),
            &mut replay,
            now,
        )
        .map_err(|error| format!("dgcl_package_ingress:{error:?}"))?;
        let permit = context
            .admit_execution_permit(
                &seed,
                lc631_host::HostExecutionPermitAdmission {
                    action: Action::Test,
                    scope,
                    span: seed.user_span,
                    authority_revision: AuthorityRevision(bundle.authority_revision),
                    revocation_revision: bundle.revocation_revision,
                    now_epoch: now,
                    attestation: seed_input
                        .authority_receipt
                        .ok_or("dgcl_package_authority_receipt_missing")?,
                },
                &mut replay,
            )
            .map_err(|error| format!("dgcl_package_permit:{error:?}"))?;
        authorized.push((seed, permit));
    }
    let snapshot = capture(
        pipeline,
        root,
        source,
        &bundle.plans[0],
        &authorized[0].0,
        context,
        bundle.authority_revision,
        bundle.revocation_revision,
    )?;
    let binding_tasks = pipeline
        .completion_plan
        .tasks
        .iter()
        .filter(|task| {
            task.evidence_kind == lc631_analysis::CodingEvidenceKind::ImplementationBinding
        })
        .collect::<Vec<_>>();
    let mut intent_receipts = vec![bundle.binding_receipt];
    intent_receipts.extend(bundle.binding_receipts);
    let mut claims = Vec::new();
    for (binding_task, receipt) in binding_tasks.into_iter().zip(intent_receipts) {
        claims.push(
            issue_dgcl_implementation_binding(
                source,
                pipeline,
                &binding_task.task_id,
                &snapshot,
                DgclBindingIssuerContext {
                    author_receipt: receipt,
                    author_verifier: author.verifier(),
                    issuer: context.issuer(),
                    verifier: context.verifier(),
                    now_epoch: super::current_epoch_seconds()?,
                    replay: &mut replay,
                },
            )
            .map_err(|error| format!("dgcl_package_intent:{error:?}"))?,
        );
    }
    let mut processes = Vec::new();
    let execution_started = std::time::Instant::now();
    for (plan, (seed, permit)) in bundle.plans.iter().zip(&authorized) {
        let principal = seed
            .principal_binding()
            .map_err(|error| format!("dgcl_package_principal:{error:?}"))?;
        let scope = dgcl_cargo_validation_scope(plan);
        let fingerprint = context.key_fingerprint();
        let authority = ExecutionPermitContext {
            principal: &principal,
            action: Action::Test,
            scope: &scope,
            source_revision: &pipeline.source_revision,
            authority_revision: AuthorityRevision(bundle.authority_revision),
            now_epoch: super::current_epoch_seconds()?,
            revocation_revision: bundle.revocation_revision,
            revoked_permit_digests: &[],
            caller_origin: CallerOrigin::HostVerifiedUser,
            trusted_host_fingerprint: &fingerprint,
        };
        let observation = observe_dgcl_cargo_validation(
            root,
            source,
            pipeline,
            plan,
            lc631_analysis::DgclCargoExecutionContext {
                lifecycle: &snapshot,
                permit,
                authority: &authority,
                tool_verifier: context.verifier(),
                max_duration: budget
                    .min(std::time::Duration::from_secs(120))
                    .saturating_sub(execution_started.elapsed()),
            },
        )
        .map_err(|error| format!("dgcl_package_observation:{error:?}"))?;
        processes.push(observation.process().clone());
        claims.push(
            issue_dgcl_cargo_evidence(observation, context.issuer(), context.verifier())
                .map_err(|error| format!("dgcl_package_evidence:{error:?}"))?,
        );
    }
    let current = capture(
        pipeline,
        root,
        source,
        &bundle.plans[0],
        &authorized[0].0,
        context,
        bundle.authority_revision,
        bundle.revocation_revision,
    )?;
    let candidate = build_verified_dgcl_completion_candidate(
        source,
        pipeline,
        &claims,
        DgclImplementationClosureContext {
            current_lifecycle: Some(&current),
            verifier: Some(context.verifier()),
            replay: &mut ReplayGuard::default(),
            now_epoch: super::current_epoch_seconds()?,
        },
    )
    .map_err(|error| format!("dgcl_package_candidate:{error:?}"))?;
    let legacy_view = lc631_analysis::project_dgcl_consumer_view(
        &candidate,
        lc631_analysis::DgclConsumerVersion::LegacyV1,
        "epistesys-dgcl-consumer-view.v1",
    )?;
    let modern_view = lc631_analysis::project_dgcl_consumer_view(
        &candidate,
        lc631_analysis::DgclConsumerVersion::BoundedV2,
        "epistesys-dgcl-consumer-view.v2",
    )?;
    Ok(
        serde_json::json!({ "schema_version": "epistesys-dgcl-package-finalization.v1",
        "source_revision": pipeline.source_revision, "completion_plan_digest": pipeline.completion_plan.plan_digest,
        "candidate": candidate, "claims": claims, "processes": processes, "snapshot": current,
        "consumer_views": { "legacy_v1": legacy_view, "bounded_v2": modern_view },
        "authority_created": false, "host_send_authorized": false, "output_commit_allowed": false,
        "host_observation": "pending", "research_evaluation": "pending_no_corpus",
        "claim_boundary": "bounded offline Cargo profile; trusted host-origin Test scopes and independent intent binding; actual check, library-test, separate connection invocation and acceptance-test runs; no process sandbox, live host revocation feed, arbitrary build-world completeness, general semantics, real Codex callback or research proof" }),
    )
}

#[allow(clippy::too_many_arguments)]
fn capture(
    pipeline: &DgclPipelineReport,
    root: &Path,
    source: &str,
    plan: &DgclCargoValidationPlan,
    seed: &HostSeedEnvelope,
    host: &HostReceiptContext,
    revision: u64,
    revocation: u64,
) -> Result<lc631_analysis::DgclExecutionSnapshot, String> {
    let principal = seed
        .principal_binding()
        .map_err(|error| format!("dgcl_package_principal:{error:?}"))?;
    let scope = dgcl_cargo_validation_scope(plan);
    let fingerprint = host.key_fingerprint();
    capture_dgcl_cargo_snapshot(
        root,
        source,
        pipeline,
        &ExecutionPermitContext {
            principal: &principal,
            action: Action::Test,
            scope: &scope,
            source_revision: &pipeline.source_revision,
            authority_revision: AuthorityRevision(revision),
            now_epoch: super::current_epoch_seconds()?,
            revocation_revision: revocation,
            revoked_permit_digests: &[],
            caller_origin: CallerOrigin::HostVerifiedUser,
            trusted_host_fingerprint: &fingerprint,
        },
    )
    .map_err(|error| format!("dgcl_package_snapshot:{error:?}"))
}
