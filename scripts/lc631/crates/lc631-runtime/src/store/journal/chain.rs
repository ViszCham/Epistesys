//! Read-only bounded inspection of ordered journal segments.
//!
//! The inspector validates each frame without materializing per-record views.
//! The returned head is local byte continuity only, never a trusted rollback
//! anchor, resumable completion, Authority, proof, or evidence grant.

use super::segment::validate_segment_streaming;
use super::types::{JournalCursor, JournalError, JournalHead};

pub const MAX_JOURNAL_CHAIN_SEGMENTS: usize = 1_000;
pub const MAX_JOURNAL_CHAIN_INPUT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JournalChainLimitError {
    ZeroLimit,
    LimitTooLarge,
}

/// Per-call limits cannot exceed the registered checkpoint-store profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JournalChainLimits {
    max_segments: usize,
    max_input_bytes: usize,
}

impl JournalChainLimits {
    pub fn checked(
        max_segments: usize,
        max_input_bytes: usize,
    ) -> Result<Self, JournalChainLimitError> {
        if max_segments == 0 || max_input_bytes == 0 {
            return Err(JournalChainLimitError::ZeroLimit);
        }
        if max_segments > MAX_JOURNAL_CHAIN_SEGMENTS
            || max_input_bytes > MAX_JOURNAL_CHAIN_INPUT_BYTES
        {
            return Err(JournalChainLimitError::LimitTooLarge);
        }
        Ok(Self {
            max_segments,
            max_input_bytes,
        })
    }

    pub const fn max_segments(self) -> usize {
        self.max_segments
    }

    pub const fn max_input_bytes(self) -> usize {
        self.max_input_bytes
    }
}

impl Default for JournalChainLimits {
    fn default() -> Self {
        Self {
            max_segments: MAX_JOURNAL_CHAIN_SEGMENTS,
            max_input_bytes: MAX_JOURNAL_CHAIN_INPUT_BYTES,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JournalChainHeadState {
    /// No expected head was supplied; the chain has only local byte validation.
    Unanchored,
    /// The supplied local head matches, but is not an external anchor.
    LocalMatchUnanchored,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JournalChainHoldReason {
    EmptyChain,
    SegmentCountExceeded {
        limit: usize,
    },
    AggregateInputBytesExceeded {
        limit: usize,
    },
    InvalidSegment {
        segment_index: usize,
        error: JournalError,
    },
    PredecessorMismatch {
        segment_index: usize,
    },
    RecordCountOverflow,
    StaleHead,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JournalChainDecision {
    Inspected(JournalChainSummary),
    Hold(JournalChainHoldReason),
}

/// Bounded summary only: no per-segment/per-record index is retained.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JournalChainSummary {
    segment_count: usize,
    record_count: usize,
    total_input_bytes: usize,
    cursor: JournalCursor,
    local_head: JournalHead,
    head_state: JournalChainHeadState,
}

impl JournalChainSummary {
    pub const fn segment_count(self) -> usize {
        self.segment_count
    }

    pub const fn record_count(self) -> usize {
        self.record_count
    }

    pub const fn total_input_bytes(self) -> usize {
        self.total_input_bytes
    }

    pub const fn cursor(self) -> JournalCursor {
        self.cursor
    }

    pub const fn local_head(self) -> JournalHead {
        self.local_head
    }

    pub const fn head_state(self) -> JournalChainHeadState {
        self.head_state
    }

    pub const fn claim_boundary(self) -> &'static str {
        "bounded ordered-segment inspection only; no resume or completion, no external rollback protection, no Authority, proof, or evidence promotion"
    }
}

/// Rebuild a local chain summary from an iterator of borrowed segment bytes.
///
/// Each frame is decoded once, checked against genesis or the preceding cursor,
/// summarized, and dropped before the next frame. A missing head is Unanchored;
/// a matching local head remains Unanchored; a stale head or malformed chain
/// returns Hold. This function never restores checkpoint completion or emits a
/// trusted-head receipt.
pub fn inspect_chain<'bytes, Segments>(
    segments: Segments,
    expected_local_head: Option<&JournalHead>,
    limits: JournalChainLimits,
) -> JournalChainDecision
where
    Segments: IntoIterator<Item = &'bytes [u8]>,
{
    let genesis = JournalCursor::genesis();
    let mut predecessor = genesis;
    let mut segment_count = 0_usize;
    let mut record_count = 0_usize;
    let mut total_input_bytes = 0_usize;
    let mut local_head = None;

    for (segment_index, bytes) in segments.into_iter().enumerate() {
        if segment_count >= limits.max_segments {
            return JournalChainDecision::Hold(JournalChainHoldReason::SegmentCountExceeded {
                limit: limits.max_segments,
            });
        }

        let Some(next_total_bytes) = total_input_bytes.checked_add(bytes.len()) else {
            return JournalChainDecision::Hold(
                JournalChainHoldReason::AggregateInputBytesExceeded {
                    limit: limits.max_input_bytes,
                },
            );
        };
        if next_total_bytes > limits.max_input_bytes {
            return JournalChainDecision::Hold(
                JournalChainHoldReason::AggregateInputBytesExceeded {
                    limit: limits.max_input_bytes,
                },
            );
        }

        let decoded = match validate_segment_streaming(bytes, predecessor) {
            Ok(decoded) => decoded,
            Err(JournalError::StalePredecessor) => {
                return JournalChainDecision::Hold(JournalChainHoldReason::PredecessorMismatch {
                    segment_index,
                });
            }
            Err(error) => {
                return JournalChainDecision::Hold(JournalChainHoldReason::InvalidSegment {
                    segment_index,
                    error,
                });
            }
        };
        let next_cursor = decoded.cursor;
        let next_head = decoded.local_head;
        let segment_records = decoded.record_count;

        let Some(next_record_count) = record_count.checked_add(segment_records) else {
            return JournalChainDecision::Hold(JournalChainHoldReason::RecordCountOverflow);
        };
        segment_count += 1;
        record_count = next_record_count;
        total_input_bytes = next_total_bytes;
        predecessor = next_cursor;
        local_head = Some(next_head);
    }

    let Some(local_head) = local_head else {
        return JournalChainDecision::Hold(JournalChainHoldReason::EmptyChain);
    };
    let head_state = match expected_local_head {
        None => JournalChainHeadState::Unanchored,
        Some(expected) if *expected == local_head => JournalChainHeadState::LocalMatchUnanchored,
        Some(_) => return JournalChainDecision::Hold(JournalChainHoldReason::StaleHead),
    };

    JournalChainDecision::Inspected(JournalChainSummary {
        segment_count,
        record_count,
        total_input_bytes,
        cursor: predecessor,
        local_head,
        head_state,
    })
}
