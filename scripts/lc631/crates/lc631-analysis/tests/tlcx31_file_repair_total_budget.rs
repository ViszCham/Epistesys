//! Actual file/backup/authority/Cargo paths. Not a durable-resume profile.
#![forbid(unsafe_code)]
#[path = "tlcx31_file_repair_fixture/mod.rs"]
mod fixture;

use fixture::authority::permit;
use fixture::validation::{PartialFileValidator, ValidationTrace};
use fixture::{CountingFileDriver, SOURCE};
use lc631_analysis::*;
use lc631_core::{stable_sha256, Action, AuthorityRevision, CallerOrigin, ExecutionPermitContext};
use lc631_receipt_kernel::{ReceiptIssuer, ReceiptVerifier, ReplayGuard};
use std::time::Duration;

enum Case {
    Full,
    Capped,
    SmallAllowance,
    AmbiguousAfterApply,
}

struct ActualCase {
    report: DgclRepairTotalBudgetReport,
    trace: ValidationTrace,
    actual: String,
    initial: String,
    intermediate: String,
    correct: String,
    backups: Vec<String>,
    applies: usize,
    validations: usize,
    d1: u64,
    d2: u64,
    per_attempt: u64,
}

fn actual_case(case: Case) -> ActualCase {
    let root = fixture::fixture_package();
    let correct = std::fs::read_to_string(root.join("src/lib.rs")).unwrap();
    let initial = correct.replace("a.checked_add(b)", "None");
    let intermediate = correct.replace("a.checked_add(b)", "Some(a.wrapping_add(b))");
    std::fs::write(root.join("src/lib.rs"), &initial).unwrap();
    let pipeline = build_dgcl_pipeline(
        SOURCE,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    let issuer = ReceiptIssuer::from_key_bytes("tlcx31-real-tool", &[91; 32]).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("tlcx31-real-tool", &[91; 32]).unwrap();
    let author = ReceiptIssuer::from_key_bytes("tlcx31-intent", &[92; 32]).unwrap();
    let author_verifier = ReceiptVerifier::from_key_bytes("tlcx31-intent", &[92; 32]).unwrap();
    let scope = format!(
        "dgcl/repair/{}/src/lib.rs",
        dgcl_repository_root_digest(&root).unwrap()
    );
    let edit = permit(SOURCE, &scope, &issuer, &verifier, Action::Edit);
    let authority = ExecutionPermitContext {
        principal: edit.principal_binding(),
        action: Action::Edit,
        scope: &scope,
        source_revision: &pipeline.source_revision,
        authority_revision: AuthorityRevision(1),
        now_epoch: 100,
        revocation_revision: 0,
        revoked_permit_digests: &[],
        caller_origin: CallerOrigin::HostVerifiedUser,
        trusted_host_fingerprint: verifier.key_fingerprint(),
    };
    let snapshot = capture_dgcl_cargo_snapshot(&root, SOURCE, &pipeline, &authority).unwrap();
    let lease = issue_dgcl_evidence_lease(&snapshot, 100, 300).unwrap();
    let requests = [
        fixture::request(&pipeline, &edit, &root, 1, &initial, &intermediate),
        fixture::request(&pipeline, &edit, &root, 2, &intermediate, &correct),
    ];
    let gaps = pipeline
        .implementation_closure
        .gaps
        .iter()
        .map(|gap| gap.gap_id.clone())
        .collect::<Vec<_>>();
    let authorization = DgclRepairAuthorization {
        permit: &edit,
        authority_revision: AuthorityRevision(1),
        trusted_host_fingerprint: verifier.key_fingerprint(),
        now_epoch: 100,
        revocation_revision: 0,
        revoked_permit_digests: &[],
    };
    let cargo = fixture::cargo_path();
    let mut trace = ValidationTrace::default();
    let mut validator = PartialFileValidator {
        root: &root,
        source: SOURCE,
        pipeline: &pipeline,
        issuer: &issuer,
        verifier: &verifier,
        author: &author,
        author_verifier: &author_verifier,
        cargo: &cargo,
        trace: &mut trace,
    };
    let inner = DgclFileRepairDriver::new(DgclFileRepairConfig {
        root: &root,
        source: SOURCE,
        pipeline: &pipeline,
        initial_snapshot: &snapshot,
        authorization: &authorization,
        issuer: &issuer,
        verifier: &verifier,
        candidates: std::collections::BTreeMap::from([
            (stable_sha256(&intermediate), intermediate.clone()),
            (stable_sha256(&correct), correct.clone()),
        ]),
        validator: &mut validator,
        epoch_clock: None,
    })
    .unwrap();
    let mut driver = CountingFileDriver {
        inner,
        applies: 0,
        validations: 0,
        fail_after_actual_apply: matches!(case, Case::AmbiguousAfterApply),
    };
    let d1 = initial.len().max(intermediate.len()) as u64;
    let d2 = intermediate.len().max(correct.len()) as u64;
    let per_attempt = d1.max(d2) + 1;
    let total = match case {
        Case::Capped => d1,
        Case::SmallAllowance => d1 + 1,
        Case::Full | Case::AmbiguousAfterApply => d1 + d2,
    };
    let report = run_dgcl_repair_loop_with_total_budget(
        DgclRepairLoopInput {
            source: SOURCE,
            source_revision: &pipeline.source_revision,
            plan_digest: &pipeline.completion_plan.plan_digest,
            program_ir: &pipeline.program_ir,
            translation: &pipeline.translation_projection,
            completion_plan: &pipeline.completion_plan,
            initial_snapshot: &snapshot,
            initial_lease: &lease,
            initial_gap_ids: &gaps,
            requests: &requests,
            authorization: DgclRepairAuthorization {
                permit: &edit,
                authority_revision: AuthorityRevision(1),
                trusted_host_fingerprint: verifier.key_fingerprint(),
                now_epoch: 100,
                revocation_revision: 0,
                revoked_permit_digests: &[],
            },
            verifier: &verifier,
            replay: &mut ReplayGuard::default(),
            max_attempts: 4,
            max_wall_time: Duration::from_secs(180),
            max_delta_bytes: per_attempt,
        },
        &mut driver,
        total,
    )
    .unwrap();
    let applies = driver.applies;
    let validations = driver.validations;
    drop(driver);
    let actual = std::fs::read_to_string(root.join("src/lib.rs")).unwrap();
    let backups = std::fs::read_dir(root.join(".dgcl-repair-backups"))
        .unwrap()
        .map(|entry| std::fs::read_to_string(entry.unwrap().path()).unwrap())
        .collect();
    ActualCase {
        report,
        trace,
        actual,
        initial,
        intermediate,
        correct,
        backups,
        applies,
        validations,
        d1,
        d2,
        per_attempt,
    }
}

fn diagnostic(label: &str, case: &ActualCase) {
    let class = if case.actual == case.correct {
        "B"
    } else if case.actual == case.intermediate {
        "A"
    } else {
        "other"
    };
    eprintln!("TLCX31 {label}: final={class}; backups={}; applies={}; validations={}; status={:?}; attempts={:?}; charged={}; reserved={}; validation_successes={}; validation_errors={:?}",
        case.backups.len(), case.applies, case.validations, case.report.repair.status,
        case.report.repair.attempts.iter().map(|a| (a.attempt, a.outcome, a.effect_state)).collect::<Vec<_>>(),
        case.report.budget.charged_bytes, case.report.budget.reserved_bytes,
        case.trace.successes.len(), case.trace.failures.iter().map(|(_, kind, error)| (kind, error)).collect::<Vec<_>>());
}

fn assert_first_real_pending(case: &ActualCase) {
    let first = &case.report.repair.attempts[0];
    assert_eq!(first.reason, "repair_revalidated_but_gaps_remain");
    assert_eq!(first.effect_state, RepairEffectState::ReceiptVerified);
    assert!(first.application_receipt_digest.is_some());
    assert!(first.validation_receipt_digest.is_some());
    assert!(case
        .trace
        .failures
        .iter()
        .any(
            |(snapshot, kind, error)| Some(snapshot) == first.after_snapshot_digest.as_ref()
                && *kind == CodingEvidenceKind::RuntimeValidation
                && *error == DgclValidationProducerError::ProcessFailed
        ));
    assert!(case
        .trace
        .intent_snapshots
        .contains(first.after_snapshot_digest.as_ref().unwrap()));
}

#[test]
fn tlcx31_actual_two_writes_control_and_total_cap_before_second_apply() {
    let control = actual_case(Case::Full);
    diagnostic("control", &control);
    assert_first_real_pending(&control);
    assert_eq!(control.applies, 2);
    assert_eq!(control.validations, 2);
    assert_eq!(control.actual, control.correct);
    assert_eq!(control.backups.len(), 2);
    assert!(control.backups.contains(&control.initial));
    assert!(control.backups.contains(&control.intermediate));
    assert_eq!(control.report.repair.status, DgclRepairStatus::Completed);
    assert_eq!(control.report.budget.charged_bytes, control.d1 + control.d2);
    assert_eq!(control.report.budget.reserved_bytes, 0);
    assert_eq!(control.trace.snapshots.len(), 2);
    assert_ne!(control.trace.snapshots[0], control.trace.snapshots[1]);
    assert!(control
        .trace
        .intent_snapshots
        .contains(&control.trace.snapshots[1]));
    assert!(!control
        .trace
        .failures
        .iter()
        .any(|(snapshot, _, _)| snapshot == &control.trace.snapshots[1]));
    assert!(control.report.repair.remaining_gap_ids.is_empty());

    let capped = actual_case(Case::Capped);
    // Must precede assertions so the parent's temporary cap-removal mutant
    // exposes the real two-write/two-backup result before the expected red.
    diagnostic("capped", &capped);
    assert_first_real_pending(&capped);
    assert!(capped.d1 < capped.per_attempt && capped.d2 < capped.per_attempt);
    assert_eq!(
        capped.applies, 1,
        "total cap must stop before the actual second driver call"
    );
    assert_eq!(capped.validations, 1);
    assert_eq!(capped.actual, capped.intermediate);
    assert_eq!(capped.backups, vec![capped.initial]);
    assert_eq!(
        capped.report.repair.status,
        DgclRepairStatus::BudgetExceeded
    );
    assert_eq!(capped.report.repair.attempts.len(), 2);
    assert_eq!(
        capped.report.repair.attempts[1].effect_state,
        RepairEffectState::NotAttempted
    );
    assert_eq!(
        capped.report.repair.attempts[1].reason,
        "total_delta_budget_exhausted_before_apply"
    );
    assert_eq!(capped.report.budget.charged_bytes, capped.d1);
    assert_eq!(capped.report.budget.reserved_bytes, 0);
    assert_eq!(
        capped.report.budget.unit,
        DgclRepairDeltaUnit::ConservativeWriteSizeBytes
    );
    assert!(capped.report.budget.ephemeral);
    assert!(!capped.report.repair.authority_created);
    assert!(!capped.report.repair.commit_push_merge_performed);
}

#[test]
fn tlcx31_actual_write_receipt_return_cut_retains_full_uncertain_reservation() {
    let case = actual_case(Case::AmbiguousAfterApply);
    diagnostic("actual-write-return-cut", &case);
    assert_eq!(case.applies, 1);
    assert_eq!(case.validations, 0);
    assert_eq!(case.actual, case.intermediate);
    assert_eq!(case.backups, vec![case.initial]);
    assert_eq!(case.report.repair.status, DgclRepairStatus::Hold);
    assert_eq!(
        case.report.repair.attempts[0].effect_state,
        RepairEffectState::MayHaveApplied
    );
    assert!(case.report.repair.attempts[0]
        .application_receipt_digest
        .is_none());
    assert_eq!(case.report.budget.charged_bytes, 0);
    assert_eq!(case.report.budget.reserved_bytes, case.per_attempt);
}

#[test]
fn tlcx31_actual_driver_rejects_small_remaining_allowance_before_second_backup() {
    let case = actual_case(Case::SmallAllowance);
    diagnostic("small-remaining-allowance", &case);
    assert_first_real_pending(&case);
    assert_eq!(case.applies, 2);
    assert_eq!(case.validations, 1);
    assert_eq!(case.actual, case.intermediate);
    assert_eq!(case.backups, vec![case.initial]);
    assert_eq!(case.report.repair.status, DgclRepairStatus::Hold);
    assert_eq!(
        case.report.repair.attempts[1].effect_state,
        RepairEffectState::MayHaveApplied
    );
    assert_eq!(case.report.budget.charged_bytes, case.d1);
    assert_eq!(case.report.budget.reserved_bytes, 1);
}
