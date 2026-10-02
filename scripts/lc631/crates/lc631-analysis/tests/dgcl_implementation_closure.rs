#![forbid(unsafe_code)]

use lc631_analysis::{
    build_dgcl_implementation_closure, build_dgcl_pipeline, CodingEvidenceClaim, DgclBackendSet,
    DgclImplementationStatus, ImplementationGapState,
};
use lc631_core::stable_sha256;
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptIssuer, ReceiptScope, ReceiptVerifier, ReplayGuard, SubjectRevision,
};

fn report(source: &str) -> lc631_analysis::DgclPipelineReport {
    build_dgcl_pipeline(
        source,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap()
}

fn run_closure(
    source: &str,
    pipeline: &lc631_analysis::DgclPipelineReport,
    plan: &lc631_analysis::DgclCompletionPlan,
    claims: &[CodingEvidenceClaim],
    lifecycle: Option<&lc631_analysis::DgclExecutionSnapshot>,
    verifier: Option<&ReceiptVerifier>,
    now_epoch: u64,
) -> Result<
    lc631_analysis::DgclImplementationClosureReport,
    lc631_analysis::DgclImplementationClosureError,
> {
    build_dgcl_implementation_closure(
        source,
        &pipeline.program_ir,
        &pipeline.translation_projection,
        plan,
        claims,
        lc631_analysis::DgclImplementationClosureContext {
            current_lifecycle: lifecycle,
            verifier,
            replay: &mut ReplayGuard::default(),
            now_epoch,
        },
    )
}

fn claims_for(
    plan: &lc631_analysis::DgclCompletionPlan,
    issuer: &ReceiptIssuer,
    now: u64,
    lifecycle: &lc631_analysis::DgclExecutionSnapshot,
) -> Vec<CodingEvidenceClaim> {
    let lease = lc631_analysis::issue_dgcl_evidence_lease(lifecycle, now, 120).unwrap();
    plan.tasks
        .iter()
        .filter(|task| task.applicability != lc631_analysis::TaskApplicability::NotApplicable)
        .map(|task| {
            let mut claim = CodingEvidenceClaim {
                requirement_id: 1,
                coding_requirement_id: Some(task.requirement_id.clone()),
                coding_task_id: Some(task.task_id.clone()),
                kind: task.evidence_kind,
                source_revision: plan.source_revision.clone(),
                target: task.target_ref.clone(),
                target_digest: task.target_digest.clone(),
                producer: Some(task.producer),
                producer_run_digest: Some(stable_sha256(&format!("run:{}", task.task_id))),
                observation_digest: Some(stable_sha256(&format!("observation:{}", task.task_id))),
                lifecycle_lease: Some(lease.clone()),
                attestation: None,
            };
            claim.attestation = Some(
                issuer
                    .issue(
                        ReceiptClass::Closure,
                        SubjectRevision::checked(&claim.source_revision).unwrap(),
                        ReceiptScope::checked(lc631_analysis::coding_evidence_scope(&claim))
                            .unwrap(),
                        lc631_analysis::coding_evidence_payload_digest(&claim),
                        now,
                        Some(now + 300),
                        None,
                    )
                    .unwrap(),
            );
            claim
        })
        .collect()
}

fn lifecycle_snapshot(
    source: &str,
    plan: &lc631_analysis::DgclCompletionPlan,
) -> lc631_analysis::DgclExecutionSnapshot {
    lifecycle_snapshot_for(
        source,
        plan,
        "dgcl-test-profile.v1",
        "dgcl-test-validator.v1",
        "dgcl-test-authority.v1",
    )
}

fn lifecycle_snapshot_for(
    source: &str,
    plan: &lc631_analysis::DgclCompletionPlan,
    profile: &str,
    validator: &str,
    authority: &str,
) -> lc631_analysis::DgclExecutionSnapshot {
    let targets = plan
        .tasks
        .iter()
        .map(|task| (task.target_ref.clone(), task.target_digest.clone()))
        .collect::<std::collections::BTreeMap<_, _>>()
        .into_iter()
        .map(
            |(target_ref, target_digest)| lc631_analysis::DgclTargetIdentity {
                target_ref,
                target_digest,
            },
        )
        .collect();
    lc631_analysis::build_dgcl_execution_snapshot(
        source,
        b"Cargo.lock test profile",
        profile,
        validator,
        authority,
        targets,
    )
    .unwrap()
}

#[test]
fn exact_per_task_receipts_create_only_an_implementation_closed_candidate() {
    let source = "Please test the package.";
    let report = report(source);
    let key = [121_u8; 32];
    let issuer = ReceiptIssuer::from_key_bytes("completion-test", &key).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("completion-test", &key).unwrap();
    let lifecycle = lifecycle_snapshot(source, &report.completion_plan);
    let claims = claims_for(&report.completion_plan, &issuer, 100, &lifecycle);
    let closure = run_closure(
        source,
        &report,
        &report.completion_plan,
        &claims,
        Some(&lifecycle),
        Some(&verifier),
        110,
    )
    .unwrap();
    assert_eq!(
        closure.status,
        DgclImplementationStatus::ImplementationClosed
    );
    assert_eq!(closure.requirements.len(), 1);
    assert!(closure.gaps.iter().all(|gap| {
        gap.state == ImplementationGapState::VerifiedReceipt
            || gap.state == ImplementationGapState::NotApplicable
    }));
    assert!(closure.implementation_complete_candidate);
    assert!(!closure.authority_created);
    assert!(!closure.output_commit_allowed);
}

#[test]
fn one_requirement_failure_does_not_erase_an_independent_closed_requirement() {
    let source = "Please test the package and do not publish.";
    let report = report(source);
    assert_eq!(report.completion_plan.requirements.len(), 2);
    let key = [122_u8; 32];
    let issuer = ReceiptIssuer::from_key_bytes("completion-test", &key).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("completion-test", &key).unwrap();
    let first_id = &report.completion_plan.requirements[0].requirement_id;
    let lifecycle = lifecycle_snapshot(source, &report.completion_plan);
    let claims = claims_for(&report.completion_plan, &issuer, 200, &lifecycle)
        .into_iter()
        .filter(|claim| claim.coding_requirement_id.as_ref() == Some(first_id))
        .collect::<Vec<_>>();
    let closure = run_closure(
        source,
        &report,
        &report.completion_plan,
        &claims,
        Some(&lifecycle),
        Some(&verifier),
        210,
    )
    .unwrap();
    assert!(closure.requirements.iter().any(|requirement| {
        requirement.requirement_id == *first_id
            && requirement.status == DgclImplementationStatus::ImplementationClosed
    }));
    assert!(closure.requirements.iter().any(|requirement| {
        requirement.requirement_id != *first_id
            && requirement.status != DgclImplementationStatus::ImplementationClosed
    }));
    assert_ne!(
        closure.status,
        DgclImplementationStatus::ImplementationClosed
    );
}

#[test]
fn empty_instruction_holds_while_nonoperative_context_has_a_nonclosing_no_action_result() {
    let source = "";
    let empty_report = report(source);
    let closure = run_closure(
        source,
        &empty_report,
        &empty_report.completion_plan,
        &[],
        None,
        None,
        300,
    )
    .unwrap();
    assert_eq!(closure.status, DgclImplementationStatus::Clarify);
    assert!(!closure.implementation_complete_candidate);
    assert!(!closure.output_commit_allowed);

    let source = "> Please test the package.";
    let quoted_report = report(source);
    assert!(quoted_report.program_ir.requirements.is_empty());
    assert!(quoted_report.program_ir.residuals.is_empty());
    let closure = run_closure(
        source,
        &quoted_report,
        &quoted_report.completion_plan,
        &[],
        None,
        None,
        300,
    )
    .unwrap();
    assert_eq!(closure.status, DgclImplementationStatus::NoAction);
    assert!(!closure.implementation_complete_candidate);
    assert!(!closure.authority_created);
    assert!(!closure.output_commit_allowed);

    let source = "This is context only.";
    let context_report = report(source);
    let closure = run_closure(
        source,
        &context_report,
        &context_report.completion_plan,
        &[],
        None,
        None,
        300,
    )
    .unwrap();
    assert_eq!(closure.status, DgclImplementationStatus::Clarify);
    assert!(!closure.implementation_complete_candidate);

    let source = "Please publish if tests pass.";
    let report = report(source);
    let key = [123_u8; 32];
    let issuer = ReceiptIssuer::from_key_bytes("completion-test", &key).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("completion-test", &key).unwrap();
    let lifecycle = lifecycle_snapshot(source, &report.completion_plan);
    let claims = claims_for(&report.completion_plan, &issuer, 400, &lifecycle);
    let closure = run_closure(
        source,
        &report,
        &report.completion_plan,
        &claims,
        Some(&lifecycle),
        Some(&verifier),
        410,
    )
    .unwrap();
    assert_eq!(closure.status, DgclImplementationStatus::Hold);
    assert!(closure
        .gaps
        .iter()
        .any(|gap| gap.state == ImplementationGapState::ConditionalPending));
    assert!(!closure.output_commit_allowed);
}

#[test]
fn expired_duplicate_and_unknown_task_claims_remain_individual_rejections() {
    let source = "Please test the package.";
    let report = report(source);
    let key = [124_u8; 32];
    let issuer = ReceiptIssuer::from_key_bytes("completion-test", &key).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("completion-test", &key).unwrap();
    let lifecycle = lifecycle_snapshot(source, &report.completion_plan);
    let mut claims = claims_for(&report.completion_plan, &issuer, 500, &lifecycle);
    let original = claims[0].clone();
    let mut duplicate = original.clone();
    duplicate.attestation = Some(
        issuer
            .issue(
                ReceiptClass::Closure,
                SubjectRevision::checked(&duplicate.source_revision).unwrap(),
                ReceiptScope::checked(lc631_analysis::coding_evidence_scope(&duplicate)).unwrap(),
                lc631_analysis::coding_evidence_payload_digest(&duplicate),
                500,
                Some(800),
                None,
            )
            .unwrap(),
    );
    claims.push(duplicate);
    let mut unknown = original.clone();
    unknown.coding_task_id =
        Some("sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".into());
    claims.push(unknown);
    let closure = run_closure(
        source,
        &report,
        &report.completion_plan,
        &claims,
        Some(&lifecycle),
        Some(&verifier),
        900,
    )
    .unwrap();
    assert!(closure.rejected_claim_count >= 3);
    assert!(closure
        .rejected_claims
        .iter()
        .any(|claim| claim.reason.contains("expired")));
    assert!(closure
        .rejected_claims
        .iter()
        .any(|claim| claim.reason.contains("duplicate")));
    assert!(closure
        .rejected_claims
        .iter()
        .any(|claim| claim.reason.contains("unknown")));
    assert_ne!(
        closure.status,
        DgclImplementationStatus::ImplementationClosed
    );
}

#[test]
fn deleting_all_tasks_from_a_requirement_invalidates_the_plan_before_closure() {
    let source = "Please test the package.";
    let report = report(source);
    let mut plan = report.completion_plan.clone();
    plan.tasks.clear();
    for requirement in &mut plan.requirements {
        requirement.tasks.clear();
    }
    assert_eq!(
        run_closure(source, &report, &plan, &[], None, None, 1_100,),
        Err(lc631_analysis::DgclImplementationClosureError::CompletionPlanMismatch)
    );
}

#[test]
fn unresolved_polarity_blocker_is_bound_to_its_requirement_not_hidden_by_receipts() {
    let source = "Do not delete or publish.";
    let report = report(source);
    assert!(report.completion_plan.blockers.iter().any(|blocker| {
        blocker.kind == lc631_analysis::CompletionBlockerKind::UnknownPolarityOrModality
    }));
    let key = [125_u8; 32];
    let issuer = ReceiptIssuer::from_key_bytes("completion-test", &key).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("completion-test", &key).unwrap();
    let lifecycle = lifecycle_snapshot(source, &report.completion_plan);
    let claims = claims_for(&report.completion_plan, &issuer, 1_000, &lifecycle);
    let closure = run_closure(
        source,
        &report,
        &report.completion_plan,
        &claims,
        Some(&lifecycle),
        Some(&verifier),
        1_010,
    )
    .unwrap();
    assert!(closure
        .requirements
        .iter()
        .all(|requirement| { requirement.status == DgclImplementationStatus::Clarify }));
    assert!(closure.gaps.iter().any(|gap| {
        gap.state == ImplementationGapState::Unresolved
            && gap.reason.contains("polarity or modality")
    }));
    assert_ne!(
        closure.status,
        DgclImplementationStatus::ImplementationClosed
    );
}

#[test]
fn stale_profile_lease_and_missing_lease_cannot_close_evidence_gaps() {
    let source = "Please test the package.";
    let report = report(source);
    let key = [126_u8; 32];
    let issuer = ReceiptIssuer::from_key_bytes("completion-test", &key).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("completion-test", &key).unwrap();
    let issued_snapshot = lifecycle_snapshot(source, &report.completion_plan);
    let claims = claims_for(&report.completion_plan, &issuer, 1_200, &issued_snapshot);
    let changed_profile = lifecycle_snapshot_for(
        source,
        &report.completion_plan,
        "dgcl-test-profile.v2",
        "dgcl-test-validator.v1",
        "dgcl-test-authority.v1",
    );
    let stale = run_closure(
        source,
        &report,
        &report.completion_plan,
        &claims,
        Some(&changed_profile),
        Some(&verifier),
        1_210,
    )
    .unwrap();
    assert_eq!(stale.status, DgclImplementationStatus::Hold);
    assert!(stale
        .gaps
        .iter()
        .any(|gap| gap.state == ImplementationGapState::Stale));
    assert!(stale
        .rejected_claims
        .iter()
        .all(|claim| claim.reason == "evidence_lifecycle_profile_stale"));

    let mut unleased = claims[0].clone();
    unleased.lifecycle_lease = None;
    unleased.attestation = Some(
        issuer
            .issue(
                ReceiptClass::Closure,
                SubjectRevision::checked(&unleased.source_revision).unwrap(),
                ReceiptScope::checked(lc631_analysis::coding_evidence_scope(&unleased)).unwrap(),
                lc631_analysis::coding_evidence_payload_digest(&unleased),
                1_200,
                Some(1_320),
                None,
            )
            .unwrap(),
    );
    let missing = run_closure(
        source,
        &report,
        &report.completion_plan,
        &[unleased],
        Some(&issued_snapshot),
        Some(&verifier),
        1_210,
    )
    .unwrap();
    assert!(missing
        .rejected_claims
        .iter()
        .any(|claim| claim.reason == "evidence_lifecycle_lease_missing"));
    assert_ne!(
        missing.status,
        DgclImplementationStatus::ImplementationClosed
    );
}
