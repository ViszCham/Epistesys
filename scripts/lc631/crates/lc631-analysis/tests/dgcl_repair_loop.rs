#![forbid(unsafe_code)]

use lc631_analysis::{
    advance_dgcl_target_snapshot, analyze_connection_graph_sources, build_dgcl_execution_snapshot,
    build_dgcl_pipeline, dgcl_repair_application_payload_digest, dgcl_repair_application_scope,
    dgcl_repair_request_digest, dgcl_repair_validation_payload_digest,
    dgcl_repair_validation_scope, issue_dgcl_evidence_lease, repair_authorization_scope,
    run_dgcl_repair_loop, DgclRepairApplication, DgclRepairAuthorization, DgclRepairDriver,
    DgclRepairDriverError, DgclRepairLoopInput, DgclRepairRequest, DgclRepairStatus,
    DgclRepairValidation, DgclRepairValidationOutcome, MAX_DGCL_REPAIR_ATTEMPTS,
};
use lc631_core::{
    authority_event_payload_digest, stable_sha256, Action, AuthorityEvent, AuthorityRevision,
    AuthoritySourceKind, DeonticPolarity, ExecutionPermit, SourceSpan, VerifiedAuthorityEvent,
};
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptIssuer, ReceiptPolicy, ReceiptScope, ReceiptVerifier, ReplayGuard,
    SubjectRevision, UntrustedReceipt,
};
use serde_json::Value;
use std::time::Duration;

const USER_SOURCE: &str = "Please repair the authorized target in this repository.";
const CONNECTED_IMPLEMENTATION: &str = "pub fn analyze_dg1() {}\n";
const CONNECTED_ROUTER: &str = r#"
use lc631_tldg::analyze_dg1;
pub fn dispatch(command: &str) {
    match command.as_str() {
        "lc631-dg1-doctor" => { analyze_dg1(); }
        _ => {}
    }
}
"#;
const DISCONNECTED_ROUTER: &str = r#"
use lc631_tldg::analyze_dg1;
pub fn dispatch() { analyze_dg1(); }
"#;

#[derive(Clone, Copy)]
enum RepairFixture {
    Route,
    ConditionProjection,
    NoProgress,
    Oscillation,
    ApplyTimeout,
    RevalidationTimeout,
}

struct MemoryRepairDriver {
    source_revision: String,
    plan_digest: String,
    target_ref: String,
    current_snapshot: lc631_analysis::DgclExecutionSnapshot,
    current_target: String,
    repaired_target: String,
    fixture: RepairFixture,
    initial_gaps: Vec<String>,
    issuer: ReceiptIssuer,
    now_epoch: u64,
}

impl DgclRepairDriver for MemoryRepairDriver {
    fn apply(
        &mut self,
        request: &DgclRepairRequest,
        permit: &ExecutionPermit,
        _remaining_time: Duration,
        remaining_delta_bytes: u64,
    ) -> Result<DgclRepairApplication, DgclRepairDriverError> {
        if matches!(self.fixture, RepairFixture::ApplyTimeout) {
            return Err(DgclRepairDriverError::TimedOut);
        }
        let before_digest = stable_sha256(&self.current_target);
        let desired_after = match self.fixture {
            RepairFixture::NoProgress => self.current_target.as_str(),
            RepairFixture::Oscillation if request.attempt == 1 => CONNECTED_ROUTER,
            RepairFixture::Oscillation => DISCONNECTED_ROUTER,
            _ => self.repaired_target.as_str(),
        };
        if request.target_ref != self.target_ref
            || request.target_digest_before != before_digest
            || request.candidate_digest != stable_sha256(desired_after)
        {
            return Err(DgclRepairDriverError::Failed);
        }
        let after = desired_after.to_string();
        let delta_bytes = if after == self.current_target {
            0
        } else {
            after.len() as u64
        };
        if delta_bytes > remaining_delta_bytes {
            return Err(DgclRepairDriverError::Failed);
        }
        let after_digest = stable_sha256(&after);
        let after_snapshot = if after_digest == before_digest {
            self.current_snapshot.clone()
        } else {
            advance_dgcl_target_snapshot(
                &self.current_snapshot,
                &self.target_ref,
                &before_digest,
                &after_digest,
            )
            .map_err(|_| DgclRepairDriverError::Failed)?
        };
        let after_lease = issue_dgcl_evidence_lease(&after_snapshot, self.now_epoch, 120)
            .map_err(|_| DgclRepairDriverError::Failed)?;
        let mut application = DgclRepairApplication {
            schema_version: "epistesys-dgcl-repair-application.v1",
            attempt: request.attempt,
            gap_id: request.gap_id.clone(),
            request_digest: dgcl_repair_request_digest(request),
            source_revision: self.source_revision.clone(),
            plan_digest: self.plan_digest.clone(),
            target_ref: self.target_ref.clone(),
            target_digest_before: before_digest,
            target_digest_after: after_digest,
            candidate_digest: request.candidate_digest.clone(),
            delta_bytes,
            run_digest: stable_sha256(&format!("repair-run:{}", request.attempt)),
            permit_digest: permit.digest().to_string(),
            applied: after != self.current_target,
            after_snapshot: after_snapshot.clone(),
            after_lease,
            attestation: None,
        };
        application.attestation = Some(issue_receipt(
            &self.issuer,
            ReceiptClass::ToolExecution,
            &self.source_revision,
            dgcl_repair_application_scope(request),
            dgcl_repair_application_payload_digest(&application),
            self.now_epoch,
        ));
        self.current_target = after;
        self.current_snapshot = after_snapshot;
        Ok(application)
    }

    fn revalidate(
        &mut self,
        request: &DgclRepairRequest,
        application: &DgclRepairApplication,
        _remaining_time: Duration,
    ) -> Result<DgclRepairValidation, DgclRepairDriverError> {
        if matches!(self.fixture, RepairFixture::RevalidationTimeout) {
            return Err(DgclRepairDriverError::TimedOut);
        }
        let (outcome, remaining_gap_ids, closure_digest) = match self.fixture {
            RepairFixture::Route => {
                let connected = route_is_connected(&self.current_target);
                if connected {
                    (
                        DgclRepairValidationOutcome::Pending,
                        self.initial_gaps
                            .iter()
                            .filter(|gap| **gap != request.gap_id)
                            .cloned()
                            .collect(),
                        stable_sha256("route-static-graph-connected"),
                    )
                } else {
                    (
                        DgclRepairValidationOutcome::Pending,
                        self.initial_gaps.clone(),
                        stable_sha256("route-static-graph-disconnected"),
                    )
                }
            }
            RepairFixture::Oscillation => (
                DgclRepairValidationOutcome::Pending,
                self.initial_gaps.clone(),
                stable_sha256("oscillating-route-stays-open"),
            ),
            RepairFixture::ApplyTimeout | RepairFixture::RevalidationTimeout => {
                unreachable!("fixture exits through a timeout before validation")
            }
            RepairFixture::ConditionProjection => {
                let report = build_dgcl_pipeline(
                    "Please run tests if the package builds.",
                    lc631_analysis::DgclBackendSet {
                        english: None,
                        japanese: None,
                    },
                )
                .map_err(|_| DgclRepairDriverError::Failed)?;
                let condition_survived = report.program_ir.requirements.iter().any(|requirement| {
                    requirement
                        .alternatives
                        .iter()
                        .any(|alternative| !alternative.ast.conditions.is_empty())
                });
                let open_condition = stable_sha256("condition-truth-remains-unknown");
                if condition_survived {
                    let mut remaining = self
                        .initial_gaps
                        .iter()
                        .filter(|gap| **gap != request.gap_id)
                        .cloned()
                        .collect::<Vec<_>>();
                    remaining.push(open_condition);
                    remaining.sort();
                    remaining.dedup();
                    (
                        DgclRepairValidationOutcome::Pending,
                        remaining,
                        stable_sha256(
                            &serde_json::to_string(&report.translation_projection).unwrap(),
                        ),
                    )
                } else {
                    (
                        DgclRepairValidationOutcome::Conflict,
                        vec![request.gap_id.clone()],
                        stable_sha256("condition-projection-not-preserved"),
                    )
                }
            }
            RepairFixture::NoProgress => unreachable!("no-progress exits before validation"),
        };
        let mut validation = DgclRepairValidation {
            closure_claims: None,
            schema_version: "epistesys-dgcl-repair-validation.v1",
            attempt: request.attempt,
            gap_id: request.gap_id.clone(),
            request_digest: application.request_digest.clone(),
            source_revision: self.source_revision.clone(),
            plan_digest: self.plan_digest.clone(),
            run_digest: application.run_digest.clone(),
            snapshot_digest: application.after_snapshot.snapshot_digest.clone(),
            validator_digest: application.after_snapshot.validator_digest.clone(),
            closure_digest,
            outcome,
            remaining_gap_ids,
            attestation: None,
        };
        validation.attestation = Some(issue_receipt(
            &self.issuer,
            ReceiptClass::Validation,
            &self.source_revision,
            dgcl_repair_validation_scope(request),
            dgcl_repair_validation_payload_digest(&validation),
            self.now_epoch,
        ));
        Ok(validation)
    }
}

fn issue_receipt(
    issuer: &ReceiptIssuer,
    class: ReceiptClass,
    source_revision: &str,
    scope: String,
    payload: String,
    now_epoch: u64,
) -> UntrustedReceipt {
    issuer
        .issue(
            class,
            SubjectRevision::checked(source_revision).unwrap(),
            ReceiptScope::checked(scope).unwrap(),
            payload,
            now_epoch,
            Some(now_epoch + 120),
            None,
        )
        .unwrap()
}

fn permit_for(scope: &str) -> (ExecutionPermit, String, [u8; 32]) {
    let key = [171_u8; 32];
    let issuer = ReceiptIssuer::from_key_bytes("repair-host", &key).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("repair-host", &key).unwrap();
    let seed_receipt_digest = stable_sha256("verified-seed-receipt");
    let event = AuthorityEvent {
        action: Action::Edit,
        source: AuthoritySourceKind::VerifiedHostUserSpan,
        polarity: DeonticPolarity::Grant,
        span: SourceSpan {
            start: 0,
            end: USER_SOURCE.len(),
        },
        scope: scope.to_string(),
    };
    let source_revision = stable_sha256(USER_SOURCE);
    let subject = SubjectRevision::checked(source_revision).unwrap();
    let receipt_scope = ReceiptScope::checked(format!("authority/edit/{scope}")).unwrap();
    let payload = authority_event_payload_digest(&event);
    let receipt = issuer
        .issue(
            ReceiptClass::Authority,
            subject.clone(),
            receipt_scope.clone(),
            payload.clone(),
            100,
            Some(300),
            Some(seed_receipt_digest.clone()),
        )
        .unwrap();
    let verified = verifier
        .verify(
            receipt,
            &ReceiptPolicy::exact(
                ReceiptClass::Authority,
                subject,
                receipt_scope,
                payload,
                100,
            )
            .with_parent(&seed_receipt_digest),
            &mut ReplayGuard::default(),
        )
        .unwrap();
    let event = VerifiedAuthorityEvent::checked(event, verified).unwrap();
    let permit = ExecutionPermit::from_verified_user_event(
        event,
        USER_SOURCE,
        AuthorityRevision(7),
        100,
        300,
        1,
    )
    .unwrap();
    let fingerprint = verifier.key_fingerprint().to_string();
    (permit, fingerprint, key)
}

fn input_parts(
    target_ref: &str,
    before: &str,
    candidates: &[&str],
    permit: &ExecutionPermit,
    fingerprint: &str,
    max_attempts: u8,
    fixture: RepairFixture,
) -> (
    lc631_analysis::DgclPipelineReport,
    String,
    String,
    lc631_analysis::DgclExecutionSnapshot,
    lc631_analysis::DgclEvidenceLease,
    Vec<String>,
    Vec<DgclRepairRequest>,
) {
    let source_revision = stable_sha256(USER_SOURCE);
    let pipeline = build_dgcl_pipeline(
        USER_SOURCE,
        lc631_analysis::DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    assert!(!pipeline.completion_plan.tasks.is_empty());
    let plan_digest = pipeline.completion_plan.plan_digest.clone();
    let before_digest = stable_sha256(before);
    let snapshot = build_dgcl_execution_snapshot(
        USER_SOURCE,
        b"manifest+lockfile-v1",
        "repair-test-profile.v1",
        "repair-test-validator.v1",
        "authority-revision-7",
        vec![lc631_analysis::DgclTargetIdentity {
            target_ref: target_ref.to_string(),
            target_digest: before_digest.clone(),
        }],
    )
    .unwrap();
    let lease = issue_dgcl_evidence_lease(&snapshot, 100, 180).unwrap();
    let selected_task_kind = match fixture {
        RepairFixture::ConditionProjection => lc631_analysis::CodingEvidenceKind::StaticValidation,
        _ => lc631_analysis::CodingEvidenceKind::ProductionConnection,
    };
    let gap_id = pipeline
        .completion_plan
        .tasks
        .iter()
        .find(|task| task.evidence_kind == selected_task_kind)
        .unwrap()
        .task_id
        .clone();
    let baseline_closure = lc631_analysis::build_dgcl_implementation_closure(
        USER_SOURCE,
        &pipeline.program_ir,
        &pipeline.translation_projection,
        &pipeline.completion_plan,
        &[],
        lc631_analysis::DgclImplementationClosureContext {
            current_lifecycle: Some(&snapshot),
            verifier: None,
            replay: &mut ReplayGuard::default(),
            now_epoch: 110,
        },
    )
    .unwrap();
    let gaps = baseline_closure
        .gaps
        .iter()
        .filter(|gap| {
            !matches!(
                gap.state,
                lc631_analysis::ImplementationGapState::VerifiedReceipt
                    | lc631_analysis::ImplementationGapState::NotApplicable
            )
        })
        .map(|gap| gap.gap_id.clone())
        .collect::<Vec<_>>();
    let requests = candidates
        .iter()
        .enumerate()
        .map(|(offset, candidate)| {
            let (target_digest_before, candidate_digest) = if offset == 0 {
                (before_digest.clone(), stable_sha256(candidate))
            } else {
                (
                    stable_sha256(candidates[offset - 1]),
                    stable_sha256(candidate),
                )
            };
            DgclRepairRequest {
                repository_root_digest: None,
                attempt: (offset + 1) as u8,
                gap_id: gap_id.clone(),
                source_revision: source_revision.clone(),
                plan_digest: plan_digest.clone(),
                target_ref: target_ref.to_string(),
                target_digest_before,
                candidate_digest,
                authorization_scope: repair_authorization_scope(target_ref),
                authorization_span: permit.source_span(),
            }
        })
        .collect::<Vec<_>>();
    let _ = (fingerprint, max_attempts);
    (
        pipeline,
        source_revision,
        plan_digest,
        snapshot,
        lease,
        gaps,
        requests,
    )
}

fn make_plan(router: &str) -> lc631_analysis::CodingConnectionPlan {
    let source = "Please parse this source.";
    lc631_analysis::CodingConnectionPlan {
        schema_version: "epistesys-coding-connection-plan.v1".into(),
        source: source.into(),
        source_revision: stable_sha256(source),
        requirement_id: 1,
        coding_requirement_id: None,
        coding_task_id: None,
        coding_target_ref: None,
        coding_target_digest: None,
        expected_polarity: "positive".into(),
        expected_condition_kind: None,
        expected_scope: None,
        implementation_path: "crates/lc631-tldg/src/dg1.rs".into(),
        implementation_digest: stable_sha256(CONNECTED_IMPLEMENTATION),
        implementation_symbol: "analyze_dg1".into(),
        router_path: "crates/lc631-cli/src/lib.rs".into(),
        router_digest: stable_sha256(router),
        executable_path: "target/debug/lc631.exe".into(),
        executable_digest: stable_sha256("binary"),
        command: "lc631-dg1-doctor".into(),
        arguments: vec!["--prompt".into(), source.into()],
        expected_json_pointer: "/payload/schema_version".into(),
        expected_json_value: Value::String("epistesys-dg1-parse.v1".into()),
    }
}

fn route_is_connected(router: &str) -> bool {
    analyze_connection_graph_sources(&make_plan(router), CONNECTED_IMPLEMENTATION, router)
        .is_ok_and(|graph| graph.state == lc631_analysis::StaticConnectionState::Connected)
}

fn execute_fixture(
    target_ref: &str,
    before: &str,
    candidates: &[&str],
    fixture: RepairFixture,
    permit_scope_override: Option<&str>,
    max_attempts: u8,
    omit_one_open_gap: bool,
) -> Result<lc631_analysis::DgclRepairLoopReport, lc631_analysis::DgclRepairConfigError> {
    let actual_scope = repair_authorization_scope(target_ref);
    let (permit, fingerprint, key) = permit_for(permit_scope_override.unwrap_or(&actual_scope));
    let (pipeline, source_revision, plan_digest, snapshot, lease, mut gap_ids, requests) =
        input_parts(
            target_ref,
            before,
            candidates,
            &permit,
            &fingerprint,
            max_attempts,
            fixture,
        );
    if omit_one_open_gap {
        gap_ids.pop();
    }
    let verifier = ReceiptVerifier::from_key_bytes("repair-host", &key).unwrap();
    let issuer = ReceiptIssuer::from_key_bytes("repair-host", &key).unwrap();
    let mut driver = MemoryRepairDriver {
        source_revision: source_revision.clone(),
        plan_digest: plan_digest.clone(),
        target_ref: target_ref.to_string(),
        current_snapshot: snapshot.clone(),
        current_target: before.to_string(),
        repaired_target: candidates.last().copied().unwrap_or(before).to_string(),
        fixture,
        initial_gaps: gap_ids.clone(),
        issuer,
        now_epoch: 110,
    };
    run_dgcl_repair_loop(
        DgclRepairLoopInput {
            source: USER_SOURCE,
            source_revision: &source_revision,
            plan_digest: &plan_digest,
            program_ir: &pipeline.program_ir,
            translation: &pipeline.translation_projection,
            completion_plan: &pipeline.completion_plan,
            initial_snapshot: &snapshot,
            initial_lease: &lease,
            initial_gap_ids: &gap_ids,
            requests: &requests,
            authorization: DgclRepairAuthorization {
                permit: &permit,
                authority_revision: AuthorityRevision(7),
                trusted_host_fingerprint: &fingerprint,
                now_epoch: 110,
                revocation_revision: 1,
                revoked_permit_digests: &[],
            },
            verifier: &verifier,
            replay: &mut ReplayGuard::default(),
            max_attempts,
            max_wall_time: Duration::from_secs(2),
            max_delta_bytes: 64 * 1024,
        },
        &mut driver,
    )
}

#[test]
fn repair_budget_is_hard_capped_at_four_attempts() {
    assert_eq!(MAX_DGCL_REPAIR_ATTEMPTS, 4);
    let report = execute_fixture(
        "src/router.rs",
        DISCONNECTED_ROUTER,
        &[CONNECTED_ROUTER],
        RepairFixture::Route,
        None,
        MAX_DGCL_REPAIR_ATTEMPTS,
        false,
    )
    .unwrap();
    assert_eq!(report.status, DgclRepairStatus::Hold);
    assert_eq!(report.attempts.len(), 1);
    assert!(!report.remaining_gap_ids.is_empty());
    assert!(!report
        .remaining_gap_ids
        .contains(&report.attempts[0].gap_id));
    assert!(!report.authority_created);
    assert!(!report.commit_push_merge_performed);
}

#[test]
fn condition_projection_repair_is_revalidated_but_unknown_truth_remains_open() {
    let report = execute_fixture(
        "src/condition_projection.rs",
        "// missing condition projection adapter",
        &["wire condition AST to TL obligation graph"],
        RepairFixture::ConditionProjection,
        None,
        MAX_DGCL_REPAIR_ATTEMPTS,
        false,
    )
    .unwrap();
    assert_eq!(report.status, DgclRepairStatus::Hold);
    assert_eq!(report.attempts.len(), 1);
    assert_eq!(report.attempts[0].outcome, DgclRepairStatus::Hold);
    assert!(report.remaining_gap_ids.len() > 1);
    assert!(report
        .remaining_gap_ids
        .contains(&stable_sha256("condition-truth-remains-unknown")));
    assert_ne!(report.initial_snapshot_digest, report.final_snapshot_digest);
}

#[test]
fn no_progress_cycle_scope_and_attempt_budget_fail_closed() {
    let no_progress = execute_fixture(
        "src/router.rs",
        DISCONNECTED_ROUTER,
        &[DISCONNECTED_ROUTER],
        RepairFixture::NoProgress,
        None,
        MAX_DGCL_REPAIR_ATTEMPTS,
        false,
    )
    .unwrap();
    assert_eq!(no_progress.status, DgclRepairStatus::NoProgress);
    assert_eq!(
        no_progress.initial_snapshot_digest,
        no_progress.final_snapshot_digest
    );

    let cycle = execute_fixture(
        "src/router.rs",
        DISCONNECTED_ROUTER,
        &[CONNECTED_ROUTER, DISCONNECTED_ROUTER],
        RepairFixture::Oscillation,
        None,
        MAX_DGCL_REPAIR_ATTEMPTS,
        false,
    )
    .unwrap();
    assert_eq!(cycle.status, DgclRepairStatus::CycleDetected, "{cycle:?}");
    assert_eq!(cycle.attempts.len(), 2);

    let unauthorized = execute_fixture(
        "src/router.rs",
        DISCONNECTED_ROUTER,
        &[CONNECTED_ROUTER],
        RepairFixture::Route,
        Some("dgcl/repair/another/file.rs"),
        MAX_DGCL_REPAIR_ATTEMPTS,
        false,
    )
    .unwrap();
    assert_eq!(unauthorized.status, DgclRepairStatus::Unauthorized);
    assert_eq!(unauthorized.attempts.len(), 1);
    assert_eq!(
        unauthorized.attempts[0].reason,
        "edit_permit_missing_expired_denied_or_scope_mismatched"
    );

    assert_eq!(
        execute_fixture(
            "src/router.rs",
            DISCONNECTED_ROUTER,
            &[CONNECTED_ROUTER],
            RepairFixture::Route,
            None,
            MAX_DGCL_REPAIR_ATTEMPTS + 1,
            false,
        ),
        Err(lc631_analysis::DgclRepairConfigError::InvalidAttemptLimit)
    );

    assert_eq!(
        execute_fixture(
            "src/router.rs",
            DISCONNECTED_ROUTER,
            &[CONNECTED_ROUTER],
            RepairFixture::Route,
            None,
            MAX_DGCL_REPAIR_ATTEMPTS,
            true,
        ),
        Err(lc631_analysis::DgclRepairConfigError::InvalidGapSet)
    );

    let apply_timeout = execute_fixture(
        "src/router.rs",
        DISCONNECTED_ROUTER,
        &[CONNECTED_ROUTER],
        RepairFixture::ApplyTimeout,
        None,
        MAX_DGCL_REPAIR_ATTEMPTS,
        false,
    )
    .unwrap();
    assert_eq!(apply_timeout.status, DgclRepairStatus::Hold);
    assert_eq!(
        apply_timeout.attempts[0].effect_state,
        lc631_analysis::RepairEffectState::MayHaveApplied
    );
    assert_eq!(
        apply_timeout.initial_snapshot_digest,
        apply_timeout.final_snapshot_digest
    );

    let revalidation_timeout = execute_fixture(
        "src/router.rs",
        DISCONNECTED_ROUTER,
        &[CONNECTED_ROUTER],
        RepairFixture::RevalidationTimeout,
        None,
        MAX_DGCL_REPAIR_ATTEMPTS,
        false,
    )
    .unwrap();
    assert_eq!(revalidation_timeout.status, DgclRepairStatus::Hold);
    assert_ne!(
        revalidation_timeout.initial_snapshot_digest,
        revalidation_timeout.final_snapshot_digest
    );
    assert_eq!(
        revalidation_timeout.attempts[0].effect_state,
        lc631_analysis::RepairEffectState::ReceiptVerified
    );
    assert!(!revalidation_timeout.remaining_gap_ids.is_empty());
}
