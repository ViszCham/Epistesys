use lc631_analysis::{
    CheckpointBody, CheckpointEventKind, CheckpointIntegrityState, CheckpointReadIdentity,
    DeliveryState, ResumeError,
};
use lc631_core::stable_sha256;
use lc631_runtime::journal::{
    decode_segment, inspect_checkpoint_chain_resume, CheckpointChainResumeDecision,
    CheckpointChainResumeHold, JournalChainHeadState, JournalChainLimits, JournalHead,
    JournalRecordKind, JournalSegmentBuilder,
};
use lc631_substrate::Digest32;

fn digest(byte: u8) -> Digest32 {
    Digest32::from_bytes([byte; 32])
}

fn identity<'a>(source: &'a str, plan: &'a str, authority: &'a str) -> CheckpointReadIdentity<'a> {
    CheckpointReadIdentity {
        source_revision: source,
        plan_digest: plan,
        authority_revision: authority,
    }
}

fn checkpoint_record(
    sequence: u64,
    source: &str,
    plan: &str,
    authority: &str,
    parent_digest: Option<String>,
) -> (Vec<u8>, String) {
    let body = CheckpointBody {
        schema_version: "epistesys-dgcl-checkpoint.v1".to_owned(),
        sequence,
        source_revision: source.to_owned(),
        plan_digest: plan.to_owned(),
        authority_revision: authority.to_owned(),
        action_key: format!("action-{sequence}"),
        event_kind: CheckpointEventKind::LegacyUnclassified,
        delivery: DeliveryState::ConfirmedSuccess,
        result_digest: Some(stable_sha256("observed-result")),
        parent_digest,
    };
    let canonical_body = serde_json::to_string(&body).unwrap();
    let digest = stable_sha256(&canonical_body);
    let stored = serde_json::json!({ "body": body, "digest": digest });
    (serde_json::to_vec(&stored).unwrap(), digest)
}

fn checkpoint_segments(source: &str, plan: &str, authority: &str) -> (Vec<u8>, Vec<u8>, String) {
    let (first_payload, first_digest) = checkpoint_record(0, source, plan, authority, None);
    let (second_payload, second_digest) =
        checkpoint_record(1, source, plan, authority, Some(first_digest));

    let mut first = JournalSegmentBuilder::genesis();
    first
        .append_at(1, JournalRecordKind::Checkpoint, digest(1), &first_payload)
        .unwrap();
    let first_segment = first.finish().unwrap();
    let first_cursor = decode_segment(&first_segment).unwrap().cursor();

    let mut second = JournalSegmentBuilder::after(first_cursor);
    second
        .append_at(2, JournalRecordKind::Checkpoint, digest(2), &second_payload)
        .unwrap();
    (first_segment, second.finish().unwrap(), second_digest)
}

fn valid_limits(segments: &[&[u8]]) -> JournalChainLimits {
    let total_bytes = segments.iter().map(|segment| segment.len()).sum();
    JournalChainLimits::checked(segments.len(), total_bytes).unwrap()
}

#[test]
fn tlcx07_cross_segment_checkpoint_chain_revalidates_current_identity_unanchored() {
    let source = stable_sha256("source");
    let plan = stable_sha256("plan");
    let authority = "authority-r1";
    let (first, second, second_digest) = checkpoint_segments(&source, &plan, authority);
    let segments = [first.as_slice(), second.as_slice()];
    let expected_head = decode_segment(&second).unwrap().local_head();

    let candidate = inspect_checkpoint_chain_resume(
        &segments,
        Some(&expected_head),
        valid_limits(&segments),
        identity(&source, &plan, authority),
    );
    let (chain, observation) = match candidate {
        CheckpointChainResumeDecision::Candidate { chain, observation } => (chain, observation),
        other => panic!("valid ordered checkpoint chain should remain inspectable: {other:?}"),
    };
    assert_eq!(chain.segment_count(), 2);
    assert_eq!(
        chain.head_state(),
        JournalChainHeadState::LocalMatchUnanchored
    );
    assert_eq!(
        observation.integrity_state,
        CheckpointIntegrityState::Unanchored
    );
    assert_eq!(observation.latest_sequence, Some(1));
    assert_eq!(
        observation.latest_digest.as_deref(),
        Some(second_digest.as_str())
    );
    assert!(observation.fresh_revalidation_required);
    assert!(!observation.authority_created);
    assert!(!observation.completion_restored);
}

#[test]
fn tlcx07_cross_segment_stale_head_or_segment_order_holds_without_selection() {
    let source = stable_sha256("source");
    let plan = stable_sha256("plan");
    let authority = "authority-r1";
    let (first, second, _) = checkpoint_segments(&source, &plan, authority);
    let segments = [first.as_slice(), second.as_slice()];
    let limits = valid_limits(&segments);
    let stale_head: JournalHead = decode_segment(&first).unwrap().local_head();

    assert!(matches!(
        inspect_checkpoint_chain_resume(
            &segments,
            Some(&stale_head),
            limits,
            identity(&source, &plan, authority),
        ),
        CheckpointChainResumeDecision::Hold(CheckpointChainResumeHold::Chain(
            lc631_runtime::journal::JournalChainHoldReason::StaleHead
        ))
    ));
    assert!(matches!(
        inspect_checkpoint_chain_resume(
            &[second.as_slice(), first.as_slice()],
            None,
            limits,
            identity(&source, &plan, authority),
        ),
        CheckpointChainResumeDecision::Hold(CheckpointChainResumeHold::Chain(
            lc631_runtime::journal::JournalChainHoldReason::PredecessorMismatch {
                segment_index: 0
            }
        ))
    ));
}

#[test]
fn tlcx07_cross_segment_analysis_mismatch_is_a_hold_not_a_resume() {
    let source = stable_sha256("source");
    let plan = stable_sha256("plan");
    let authority = "authority-r1";
    let (first, second, _) = checkpoint_segments(&source, &plan, authority);
    let segments = [first.as_slice(), second.as_slice()];

    assert!(matches!(
        inspect_checkpoint_chain_resume(
            &segments,
            None,
            valid_limits(&segments),
            identity("sha256:stale-source", &plan, authority),
        ),
        CheckpointChainResumeDecision::Hold(CheckpointChainResumeHold::Analysis(
            ResumeError::StaleSource
        ))
    ));
}

#[test]
fn tlcx07_noncheckpoint_records_do_not_enter_analysis_resume() {
    let mut segment = JournalSegmentBuilder::genesis();
    segment
        .append_at(
            1,
            JournalRecordKind::Evidence,
            digest(1),
            b"not-a-checkpoint",
        )
        .unwrap();
    let bytes = segment.finish().unwrap();
    let segments = [bytes.as_slice()];
    let source = stable_sha256("source");
    let plan = stable_sha256("plan");
    let authority = "authority-r1";

    assert!(matches!(
        inspect_checkpoint_chain_resume(
            &segments,
            None,
            valid_limits(&segments),
            identity(&source, &plan, authority),
        ),
        CheckpointChainResumeDecision::Hold(CheckpointChainResumeHold::NonCheckpointRecord {
            segment_index: 0,
            record_index: 0,
            kind: JournalRecordKind::Evidence,
        })
    ));
}

#[test]
fn tlcx07_chain_limits_hold_before_checkpoint_payload_materialization() {
    let source = stable_sha256("source");
    let plan = stable_sha256("plan");
    let authority = "authority-r1";
    let (first, second, _) = checkpoint_segments(&source, &plan, authority);
    let segments = [first.as_slice(), second.as_slice()];
    let count_limit = JournalChainLimits::checked(1, first.len() + second.len()).unwrap();

    assert!(matches!(
        inspect_checkpoint_chain_resume(
            &segments,
            None,
            count_limit,
            identity(&source, &plan, authority),
        ),
        CheckpointChainResumeDecision::Hold(CheckpointChainResumeHold::Chain(
            lc631_runtime::journal::JournalChainHoldReason::SegmentCountExceeded { limit: 1 }
        ))
    ));
}
