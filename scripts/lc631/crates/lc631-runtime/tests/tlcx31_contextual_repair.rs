//! Preparatory ledger checks only; these do not apply files or issue evidence.
use lc631_runtime::contextual_repair;

use contextual_repair::*;
use lc631_analysis::{repair_request_authorization_scope, DgclRepairRequest};
use lc631_core::context::{ContextRevision, ContextScope, SourceRevision};
use lc631_core::{stable_sha256, SourceSpan};
use lc631_substrate::Digest32;

const SOURCE: &str = "Implement the required function.";

fn digest(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}

fn binding() -> RepairRunBinding {
    RepairRunBinding {
        context: ContextRevision::original(SourceRevision::of(SOURCE)),
        scope: ContextScope::declared(digest(1), digest(2), digest(3), digest(4)),
        snapshot: digest(5),
    }
}

fn ledger(total: u64) -> ContextualRepairLedger {
    ContextualRepairLedger::new(
        SOURCE,
        binding(),
        RepairLimits::checked(4, 100, total).unwrap(),
        10,
        100,
    )
    .unwrap()
}

fn request(attempt: u8, replacement: &str) -> DgclRepairRequest {
    let mut request = DgclRepairRequest {
        attempt,
        gap_id: stable_sha256("gap"),
        source_revision: stable_sha256(SOURCE),
        plan_digest: stable_sha256("plan"),
        target_ref: "src/example.rs".into(),
        target_digest_before: stable_sha256("before"),
        candidate_digest: stable_sha256(replacement),
        authorization_scope: String::new(),
        authorization_span: SourceSpan {
            start: 0,
            end: SOURCE.len(),
        },
        repository_root_digest: Some(stable_sha256("root")),
    };
    request.authorization_scope = repair_request_authorization_scope(&request);
    request
}

#[test]
fn tlcx31_total_delta_is_distinct_from_per_attempt_and_never_refunded() {
    let mut run = ledger(70);
    let pending = run
        .prepare(request(1, "fn a() {}"), "fn a() {}", 40, binding(), 10)
        .unwrap();
    assert_eq!(pending.request().attempt, 1);
    assert_eq!(pending.replacement(), "fn a() {}");
    assert_eq!(pending.binding(), binding());
    assert_eq!(pending.charged_delta(), 40);
    assert_eq!(run.state(), RepairPlanState::AwaitingFreshAuthority);
    run.record_report(
        pending,
        ReportedRepairFeedback::RevalidatedPending {
            snapshot: digest(6),
        },
        11,
    )
    .unwrap();
    let current = RepairRunBinding {
        snapshot: digest(6),
        ..binding()
    };
    assert_eq!(
        run.prepare(request(2, "fn b() {}"), "fn b() {}", 40, current, 12)
            .err(),
        Some(RepairPlanError::DeltaLimit)
    );
    assert_eq!(run.charged_total_delta(), 40);
    assert_eq!(run.attempts(), 1);
}

#[test]
fn tlcx31_feedback_is_reported_and_cannot_close_implementation() {
    let mut run = ledger(100);
    let pending = run
        .prepare(request(1, "fn a() {}"), "fn a() {}", 10, binding(), 10)
        .unwrap();
    assert_eq!(
        run.record_report(pending, ReportedRepairFeedback::ClosureReported, 11)
            .unwrap(),
        RepairPlanState::NeedsEvidence
    );
    assert_eq!(
        run.last_feedback(),
        Some(ReportedRepairFeedback::ClosureReported)
    );
    assert_eq!(
        run.prepare(request(2, "fn b() {}"), "fn b() {}", 10, binding(), 12)
            .err(),
        Some(RepairPlanError::Stopped)
    );
}

#[test]
fn tlcx31_duplicate_and_snapshot_cycle_do_not_pad_attempts() {
    let mut run = ledger(400);
    let p = run
        .prepare(request(1, "fn a() {}"), "fn a() {}", 10, binding(), 10)
        .unwrap();
    run.record_report(
        p,
        ReportedRepairFeedback::RevalidatedPending {
            snapshot: digest(6),
        },
        11,
    )
    .unwrap();
    let current = RepairRunBinding {
        snapshot: digest(6),
        ..binding()
    };
    assert_eq!(
        run.prepare(request(2, "fn a() {}"), "fn a() {}", 10, current, 12)
            .err(),
        Some(RepairPlanError::DuplicateReplacement)
    );
    let p = run
        .prepare(request(2, "fn b() {}"), "fn b() {}", 10, current, 12)
        .unwrap();
    assert_eq!(
        run.record_report(
            p,
            ReportedRepairFeedback::RevalidatedPending {
                snapshot: digest(5)
            },
            13
        )
        .unwrap(),
        RepairPlanState::CycleReported
    );
}

#[test]
fn tlcx31_context_scope_clock_and_request_family_are_pinned() {
    let mut run = ledger(400);
    let changed = RepairRunBinding {
        context: binding().context.reconstructed([8; 32]),
        ..binding()
    };
    assert_eq!(
        run.prepare(request(1, "a"), "a", 1, changed, 10).err(),
        Some(RepairPlanError::BindingMismatch)
    );
    let p = run.prepare(request(1, "a"), "a", 1, binding(), 10).unwrap();
    run.record_report(
        p,
        ReportedRepairFeedback::RevalidatedPending {
            snapshot: digest(6),
        },
        11,
    )
    .unwrap();
    let current = RepairRunBinding {
        snapshot: digest(6),
        ..binding()
    };
    let mut next = request(2, "b");
    next.plan_digest = stable_sha256("other plan");
    assert_eq!(
        run.prepare(next, "b", 1, current, 12).err(),
        Some(RepairPlanError::BindingMismatch)
    );
    assert_eq!(
        run.prepare(request(2, "b"), "b", 1, current, 11).err(),
        Some(RepairPlanError::ClockRollback)
    );
}

#[test]
fn tlcx31_effect_ambiguity_no_progress_and_rejection_remain_distinct() {
    for (feedback, expected) in [
        (
            ReportedRepairFeedback::MayHaveApplied,
            RepairPlanState::MayHaveAppliedReported,
        ),
        (
            ReportedRepairFeedback::NoProgress,
            RepairPlanState::NoProgressReported,
        ),
        (
            ReportedRepairFeedback::Rejected,
            RepairPlanState::RejectedReported,
        ),
    ] {
        let mut run = ledger(100);
        let p = run
            .prepare(request(1, "a"), "a", 10, binding(), 10)
            .unwrap();
        assert_eq!(run.record_report(p, feedback, 11).unwrap(), expected);
        assert_eq!(run.charged_total_delta(), 10);
    }
}

#[test]
fn tlcx31_independent_small_budget_model_bounds_all_four_proposals() {
    // Independent arithmetic model: no production helper generates expected charges.
    for cap in 1..=80 {
        let mut run = ledger(cap);
        let mut expected_spent = 0;
        let mut current = binding();
        for attempt in 1..=4 {
            let text = ["a", "bb", "ccc", "dddd"][usize::from(attempt - 1)];
            let before = u64::from(attempt) * 7;
            let expected = before.max(text.len() as u64);
            let prepared = run.prepare(request(attempt, text), text, before, current, 10);
            if expected_spent + expected > cap {
                assert_eq!(prepared.err(), Some(RepairPlanError::DeltaLimit));
                break;
            }
            let prepared = prepared.unwrap();
            expected_spent += expected;
            current.snapshot = digest(attempt + 10);
            run.record_report(
                prepared,
                ReportedRepairFeedback::RevalidatedPending {
                    snapshot: current.snapshot,
                },
                10,
            )
            .unwrap();
            assert_eq!(run.charged_total_delta(), expected_spent);
        }
    }
}
