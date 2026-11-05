//! 実保存の限定試験。未実行を成功としない。
//! Bounded persistence tests; unrun tests are not passes.
use lc631_runtime::contextual_resume::budget;
use lc631_runtime::journal;
#[path = "tlcx32_budget_support/mod.rs"]
mod support;
use budget::*;
use lc631_substrate::Digest32;
use sha2::{Digest, Sha256};
use std::fs;
use support::*;

#[test]
fn tlcx32_budget_valid_journal_wrong_counter_and_scope_caps_swaps_are_rejected() {
    let a = root("counter-origin");
    let b = root("counter-mutant");
    let (_broker, mut run, mut reservation) = resources(3204);
    let head_a;
    {
        let lease = reservation.lease();
        let mut sa = BudgetSession::initialize_new(
            &a,
            binding(),
            caps(),
            65536,
            limits(),
            journal::JournalChainLimits::default(),
            &lease,
        )
        .unwrap();
        let _ticket = sa.reserve_before_apply(proposal(40)).unwrap();
        head_a = sa.head().unwrap();
        let _sb = BudgetSession::initialize_new(
            &b,
            binding(),
            caps(),
            65536,
            limits(),
            journal::JournalChainLimits::default(),
            &lease,
        )
        .unwrap();
    }
    let bytes = fs::read(part(&a, "segment", 2)).unwrap();
    let decoded = journal::decode_segment(&bytes).unwrap();
    let mut payload = decoded.records()[0].payload().to_vec();
    assert_eq!(payload.len(), 648);
    // Independent schema mutation: proposal remains40, charged total becomes41.
    // The journal/frame/index/head and payload object digests remain valid.
    payload[640..648].copy_from_slice(&41_u64.to_le_bytes());
    let mut hash = Sha256::new();
    hash.update(b"Epistesys/TLCX/RepairBudget/Evidence/v1\0");
    hash.update(&payload);
    let object = Digest32::from_bytes(hash.finalize().into());
    {
        let lease = reservation.lease();
        let mut writer = journal::JournalBundleLiveSession::open(
            &b,
            65536,
            limits(),
            &lease,
            journal::JournalChainLimits::default(),
        )
        .unwrap();
        let cursor = writer.cursor().unwrap();
        let mut builder = journal::JournalSegmentBuilder::after(cursor);
        builder
            .append_at(2, journal::JournalRecordKind::Evidence, object, &payload)
            .unwrap();
        writer.append(cursor, &builder.finish().unwrap()).unwrap();
    }
    {
        let lease = reservation.lease();
        assert!(matches!(
            BudgetSession::resume_existing(
                &b,
                binding(),
                caps(),
                actual_head(&b, 2),
                65536,
                limits(),
                journal::JournalChainLimits::default(),
                &lease
            ),
            Err(BudgetError::CountersMismatch)
        ));
        let policy_swap = BudgetBinding {
            policy: d(88),
            ..binding()
        };
        assert!(matches!(
            BudgetSession::resume_existing(
                &a,
                policy_swap,
                caps(),
                head_a,
                65536,
                limits(),
                journal::JournalChainLimits::default(),
                &lease
            ),
            Err(BudgetError::BindingMismatch)
        ));
        let source_swap = BudgetBinding {
            source: d(89),
            ..binding()
        };
        assert!(matches!(
            BudgetSession::resume_existing(
                &a,
                source_swap,
                caps(),
                head_a,
                65536,
                limits(),
                journal::JournalChainLimits::default(),
                &lease
            ),
            Err(BudgetError::BindingMismatch)
        ));
        let caps_swap = BudgetCaps {
            attempts: 3,
            ..caps()
        };
        assert!(matches!(
            BudgetSession::resume_existing(
                &a,
                binding(),
                caps_swap,
                head_a,
                65536,
                limits(),
                journal::JournalChainLimits::default(),
                &lease
            ),
            Err(BudgetError::CapsMismatch)
        ));
    }
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}

#[test]
fn tlcx32_budget_duplicate_settlement_and_changed_local_head_replay_are_rejected() {
    let r = root("duplicate");
    let (_broker, mut run, mut reservation) = resources(3203);
    let settled_head;
    {
        let lease = reservation.lease();
        let mut session = BudgetSession::initialize_new(
            &r,
            binding(),
            caps(),
            65536,
            limits(),
            journal::JournalChainLimits::default(),
            &lease,
        )
        .unwrap();
        let ticket = session.reserve_before_apply(proposal(40)).unwrap();
        session
            .record_reported_settlement(ticket, report())
            .unwrap();
        settled_head = session.head().unwrap();
    }
    let bytes = fs::read(part(&r, "segment", 3)).unwrap();
    let segment = journal::decode_segment(&bytes).unwrap();
    let mut payload = segment.records()[0].payload().to_vec();
    // Independent byte-level model: fixed profile is 648 bytes; prior cursor/head
    // offsets are specified in the document, not obtained from production codec.
    assert_eq!(payload.len(), 648);
    payload[296..304].copy_from_slice(&3_u64.to_le_bytes());
    payload[304..336].copy_from_slice(settled_head.record_digest.as_bytes());
    payload[336..368].copy_from_slice(settled_head.local_head_frame_digest.as_bytes());
    let mut hasher = Sha256::new();
    hasher.update(b"Epistesys/TLCX/RepairBudget/Evidence/v1\0");
    hasher.update(&payload);
    let digest = Digest32::from_bytes(hasher.finalize().into());
    {
        let lease = reservation.lease();
        let mut writer = journal::JournalBundleLiveSession::open(
            &r,
            65536,
            limits(),
            &lease,
            journal::JournalChainLimits::default(),
        )
        .unwrap();
        let cursor = writer.cursor().unwrap();
        let mut builder = journal::JournalSegmentBuilder::after(cursor);
        builder
            .append_at(4, journal::JournalRecordKind::Evidence, digest, &payload)
            .unwrap();
        writer.append(cursor, &builder.finish().unwrap()).unwrap();
    }
    {
        let lease = reservation.lease();
        assert!(matches!(
            BudgetSession::resume_existing(
                &r,
                binding(),
                caps(),
                actual_head(&r, 4),
                65536,
                limits(),
                journal::JournalChainLimits::default(),
                &lease
            ),
            Err(BudgetError::DuplicateSettlement)
        ));
    }
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}

#[test]
fn tlcx32_budget_real_handoff_owner_replay_and_no_report_based_follow_on_apply() {
    let a = root("owner-a");
    let b = root("owner-b");
    let (_broker, mut run, mut reservation) = resources(3201);
    let (head_a, head_b);
    {
        let lease = reservation.lease();
        let mut sa = BudgetSession::initialize_new(
            &a,
            binding(),
            caps(),
            65536,
            limits(),
            journal::JournalChainLimits::default(),
            &lease,
        )
        .unwrap();
        let mut sb = BudgetSession::initialize_new(
            &b,
            binding(),
            caps(),
            65536,
            limits(),
            journal::JournalChainLimits::default(),
            &lease,
        )
        .unwrap();
        assert_eq!(sa.summary().unwrap().charged_attempts, 0);
        assert!(matches!(
            sa.reserve_before_apply(proposal(51)),
            Err(BudgetError::BudgetExhausted)
        ));
        assert!(matches!(
            sa.reserve_before_apply(proposal(0)),
            Err(BudgetError::InvalidProposal)
        ));
        let ta = sa.reserve_before_apply(proposal(40)).unwrap();
        let tb = sb.reserve_before_apply(proposal(40)).unwrap();
        assert_eq!(
            ta.reservation_record_digest(),
            tb.reservation_record_digest()
        );
        assert_eq!(ta.proposal(), proposal(40));
        assert!(ta.claim_boundary().contains("not application permission"));
        assert!(matches!(
            sb.record_reported_settlement(ta, report()),
            Err(BudgetError::ForeignTicket)
        ));
        assert_eq!(
            sa.summary().unwrap().phase,
            BudgetPhase::PendingMayHaveApplied
        );
        let settled = sb.record_reported_settlement(tb, report()).unwrap();
        assert_eq!(settled.phase, BudgetPhase::SettledNeedsFreshValidation);
        assert_eq!(
            (settled.charged_attempts, settled.charged_delta_bytes),
            (1, 40)
        );
        assert!(settled
            .claim_boundary()
            .contains("RollbackResistanceUnavailable"));
        assert!(matches!(
            sb.reserve_before_apply(proposal(1)),
            Err(BudgetError::ReconciliationRequired)
        ));
        head_a = sa.head().unwrap();
        head_b = sb.head().unwrap();
    }
    {
        let lease = reservation.lease();
        let mut resumed = BudgetSession::resume_existing(
            &a,
            binding(),
            caps(),
            head_a,
            65536,
            limits(),
            journal::JournalChainLimits::default(),
            &lease,
        )
        .unwrap();
        assert_eq!(
            resumed.summary().unwrap().phase,
            BudgetPhase::PendingMayHaveApplied
        );
        assert!(matches!(
            resumed.reserve_before_apply(proposal(1)),
            Err(BudgetError::ReconciliationRequired)
        ));
    }
    {
        let lease = reservation.lease();
        let resumed = BudgetSession::resume_existing(
            &b,
            binding(),
            caps(),
            head_b,
            65536,
            limits(),
            journal::JournalChainLimits::default(),
            &lease,
        )
        .unwrap();
        assert_eq!(
            resumed.summary().unwrap().phase,
            BudgetPhase::SettledNeedsFreshValidation
        );
        assert_eq!(resumed.summary().unwrap().charged_delta_bytes, 40);
    }
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}

#[test]
fn tlcx32_budget_missing_legacy_stale_head_and_unknown_payload_never_reset() {
    let r = root("invalid");
    let missing = r.join("absent");
    let expected = BudgetHead {
        sequence: 1,
        record_digest: d(20),
        local_head_frame_digest: d(21),
    };
    let (_broker, mut run, mut reservation) = resources(3202);
    let head;
    {
        let lease = reservation.lease();
        assert!(BudgetSession::resume_existing(
            &missing,
            binding(),
            caps(),
            expected,
            65536,
            limits(),
            journal::JournalChainLimits::default(),
            &lease
        )
        .is_err());
        assert!(!missing.exists());
        let legacy = root("legacy");
        let legacy_path = legacy.join("old-checkpoint.json");
        fs::write(&legacy_path, b"legacy-unmigrated-history").unwrap();
        assert!(BudgetSession::resume_existing(
            &legacy,
            binding(),
            caps(),
            expected,
            65536,
            limits(),
            journal::JournalChainLimits::default(),
            &lease
        )
        .is_err());
        assert!(BudgetSession::initialize_new(
            &legacy,
            binding(),
            caps(),
            65536,
            limits(),
            journal::JournalChainLimits::default(),
            &lease
        )
        .is_err());
        assert_eq!(fs::read(legacy_path).unwrap(), b"legacy-unmigrated-history");
        let session = BudgetSession::initialize_new(
            &r,
            binding(),
            caps(),
            65536,
            limits(),
            journal::JournalChainLimits::default(),
            &lease,
        )
        .unwrap();
        head = session.head().unwrap();
    }
    {
        let lease = reservation.lease();
        assert!(matches!(
            BudgetSession::initialize_new(
                &r,
                binding(),
                caps(),
                65536,
                limits(),
                journal::JournalChainLimits::default(),
                &lease
            ),
            Err(BudgetError::ExistingHistory)
        ));
        let wrong = BudgetHead {
            local_head_frame_digest: d(99),
            ..head
        };
        assert!(matches!(
            BudgetSession::resume_existing(
                &r,
                binding(),
                caps(),
                wrong,
                65536,
                limits(),
                journal::JournalChainLimits::default(),
                &lease
            ),
            Err(BudgetError::HeadMismatch)
        ));
        let mut writer = journal::JournalBundleLiveSession::open(
            &r,
            65536,
            limits(),
            &lease,
            journal::JournalChainLimits::default(),
        )
        .unwrap();
        let cursor = writer.cursor().unwrap();
        let mut builder = journal::JournalSegmentBuilder::after(cursor);
        builder
            .append_at(
                cursor.sequence() + 1,
                journal::JournalRecordKind::Evidence,
                d(22),
                b"unknown-counter-profile",
            )
            .unwrap();
        writer.append(cursor, &builder.finish().unwrap()).unwrap();
    }
    {
        let lease = reservation.lease();
        assert!(matches!(
            BudgetSession::resume_existing(
                &r,
                binding(),
                caps(),
                actual_head(&r, 2),
                65536,
                limits(),
                journal::JournalChainLimits::default(),
                &lease
            ),
            Err(BudgetError::UnsupportedPayload)
        ));
    }
    // Legacy checkpoint inspection must reject this explicit Evidence profile.
    let segment = fs::read(part(&r, "segment", 1)).unwrap();
    let id = lc631_analysis::CheckpointReadIdentity {
        source_revision: "source",
        plan_digest: "plan",
        authority_revision: "authority",
    };
    assert!(matches!(
        journal::inspect_checkpoint_chain_resume(
            &[&segment],
            None,
            journal::JournalChainLimits::default(),
            id
        ),
        journal::CheckpointChainResumeDecision::Hold(
            journal::CheckpointChainResumeHold::NonCheckpointRecord { .. }
        )
    ));
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}
