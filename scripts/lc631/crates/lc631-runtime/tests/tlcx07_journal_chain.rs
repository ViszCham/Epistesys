use lc631_runtime::journal::{
    decode_segment, inspect_chain, JournalChainDecision, JournalChainHeadState,
    JournalChainHoldReason, JournalChainLimits, JournalCursor, JournalRecordKind,
    JournalSegmentBuilder,
};
use lc631_runtime::resource::Digest32;

fn digest(byte: u8) -> Digest32 {
    Digest32::from_bytes([byte; 32])
}

fn first_segment() -> Vec<u8> {
    let mut builder = JournalSegmentBuilder::genesis();
    builder
        .append_at(1, JournalRecordKind::Evidence, digest(1), b"first-evidence")
        .unwrap();
    builder.finish().unwrap()
}

fn second_segment(first: &[u8]) -> Vec<u8> {
    let cursor = decode_segment(first).unwrap().cursor();
    let sequence = cursor.sequence().checked_add(1).unwrap();
    let mut builder = JournalSegmentBuilder::after(cursor);
    builder
        .append_at(
            sequence,
            JournalRecordKind::Checkpoint,
            digest(2),
            b"second-checkpoint",
        )
        .unwrap();
    builder.finish().unwrap()
}

fn limits(max_segments: usize, max_input_bytes: usize) -> JournalChainLimits {
    JournalChainLimits::checked(max_segments, max_input_bytes).unwrap()
}

#[test]
fn tlcx07_empty_chain_holds_instead_of_claiming_a_genesis_result() {
    let result = inspect_chain(
        std::iter::empty::<&[u8]>(),
        None,
        JournalChainLimits::default(),
    );

    assert!(matches!(
        result,
        JournalChainDecision::Hold(JournalChainHoldReason::EmptyChain)
    ));
}

#[test]
fn tlcx07_valid_two_segment_chain_distinguishes_absent_and_local_matching_heads() {
    let first = first_segment();
    let second = second_segment(&first);
    let second_head = decode_segment(&second).unwrap().local_head();
    let input_bytes = first.len() + second.len();
    let chain = [first.as_slice(), second.as_slice()];

    let unanchored = match inspect_chain(chain.iter().copied(), None, limits(2, input_bytes)) {
        JournalChainDecision::Inspected(summary) => summary,
        other => panic!("well-ordered segments should inspect locally: {other:?}"),
    };
    assert_eq!(unanchored.segment_count(), 2);
    assert_eq!(unanchored.record_count(), 2);
    assert_eq!(unanchored.total_input_bytes(), input_bytes);
    assert_eq!(unanchored.local_head(), second_head);
    assert_eq!(unanchored.head_state(), JournalChainHeadState::Unanchored);
    assert!(unanchored
        .claim_boundary()
        .contains("no external rollback protection"));

    let locally_matched = match inspect_chain(
        chain.iter().copied(),
        Some(&second_head),
        limits(2, input_bytes),
    ) {
        JournalChainDecision::Inspected(summary) => summary,
        other => panic!("exact local head should remain inspectable: {other:?}"),
    };
    assert_eq!(
        locally_matched.head_state(),
        JournalChainHeadState::LocalMatchUnanchored
    );
}

#[test]
fn tlcx07_swapped_or_misordered_segments_hold_at_the_first_bad_predecessor() {
    let first = first_segment();
    let second = second_segment(&first);
    let limits = limits(2, first.len() + second.len());

    assert!(matches!(
        inspect_chain([second.as_slice(), first.as_slice()], None, limits),
        JournalChainDecision::Hold(JournalChainHoldReason::PredecessorMismatch {
            segment_index: 0
        })
    ));
    assert!(matches!(
        inspect_chain([first.as_slice(), first.as_slice()], None, limits),
        JournalChainDecision::Hold(JournalChainHoldReason::PredecessorMismatch {
            segment_index: 1
        })
    ));
}

#[test]
fn tlcx07_stale_local_head_holds_and_is_never_reported_as_external_anchor() {
    let first = first_segment();
    let second = second_segment(&first);
    let old_head = decode_segment(&first).unwrap().local_head();
    let limits = limits(2, first.len() + second.len());

    assert!(matches!(
        inspect_chain(
            [first.as_slice(), second.as_slice()],
            Some(&old_head),
            limits
        ),
        JournalChainDecision::Hold(JournalChainHoldReason::StaleHead)
    ));
}

#[test]
fn tlcx07_torn_segment_holds_without_publishing_a_partial_summary() {
    let valid = first_segment();
    let torn = &valid[..valid.len() - 1];

    assert!(matches!(
        inspect_chain([torn], None, limits(1, valid.len()),),
        JournalChainDecision::Hold(JournalChainHoldReason::InvalidSegment {
            segment_index: 0,
            ..
        })
    ));
}

#[test]
fn tlcx07_segment_count_and_aggregate_bytes_fail_before_overreading_chain() {
    let first = first_segment();
    let second = second_segment(&first);
    let segments = [first.as_slice(), second.as_slice()];

    assert!(matches!(
        inspect_chain(
            segments.iter().copied(),
            None,
            limits(1, first.len() + second.len())
        ),
        JournalChainDecision::Hold(JournalChainHoldReason::SegmentCountExceeded { limit: 1 })
    ));
    assert!(matches!(
        inspect_chain(
            segments.iter().copied(),
            None,
            limits(2, first.len() + second.len() - 1),
        ),
        JournalChainDecision::Hold(
            JournalChainHoldReason::AggregateInputBytesExceeded { limit }
        ) if limit == first.len() + second.len() - 1
    ));
}

#[test]
fn tlcx07_chain_limits_are_checked_and_zero_is_not_a_successful_empty_scan() {
    assert!(JournalChainLimits::checked(0, 1).is_err());
    assert!(JournalChainLimits::checked(1, 0).is_err());
    assert!(JournalChainLimits::checked(1025, 1).is_err());
    assert!(JournalChainLimits::checked(1, 16 * 1024 * 1024 + 1).is_err());
    assert_eq!(JournalCursor::genesis().sequence(), 0);
}
