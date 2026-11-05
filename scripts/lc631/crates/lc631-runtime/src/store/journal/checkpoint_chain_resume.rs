//! Read-only bridge from a bounded binary segment chain to legacy checkpoint
//! revalidation. No completion, Authority, or external-head trust is restored.

use super::chain::{
    inspect_chain, JournalChainDecision, JournalChainHoldReason, JournalChainLimits,
    JournalChainSummary,
};
use super::segment::decode_segment;
use super::types::{JournalCursor, JournalError, JournalHead, JournalRecordKind};
use lc631_analysis::{
    inspect_checkpoint_record_bytes, CheckpointReadIdentity, ResumeError, ResumeObservation,
    MAX_CHECKPOINT_RECORDS,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckpointChainResumeHold {
    Chain(JournalChainHoldReason),
    InvalidSegment {
        segment_index: usize,
        error: JournalError,
    },
    PredecessorMismatch {
        segment_index: usize,
    },
    NonCheckpointRecord {
        segment_index: usize,
        record_index: usize,
        kind: JournalRecordKind,
    },
    CheckpointLimitExceeded {
        limit: usize,
    },
    AllocationFailed,
    Analysis(ResumeError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckpointChainResumeDecision {
    Candidate {
        chain: JournalChainSummary,
        observation: ResumeObservation,
    },
    Hold(CheckpointChainResumeHold),
}

impl CheckpointChainResumeDecision {
    pub const fn claim_boundary(&self) -> &'static str {
        "ordered binary checkpoint chain plus source/plan/authority byte revalidation; candidate remains unanchored and never restores completion or Authority"
    }
}

/// Validate an ordered bounded segment chain, retain only borrowed checkpoint
/// payload slices, and pass those exact legacy bytes to the analysis-level
/// source/plan/authority validator. Every output remains unanchored; the
/// caller owns file acquisition and any external-head verification.
pub fn inspect_checkpoint_chain_resume(
    segments: &[&[u8]],
    expected_local_head: Option<&JournalHead>,
    limits: JournalChainLimits,
    identity: CheckpointReadIdentity<'_>,
) -> CheckpointChainResumeDecision {
    let chain = match inspect_chain(segments.iter().copied(), expected_local_head, limits) {
        JournalChainDecision::Inspected(chain) => chain,
        JournalChainDecision::Hold(reason) => {
            return CheckpointChainResumeDecision::Hold(CheckpointChainResumeHold::Chain(reason));
        }
    };

    if chain.record_count() > MAX_CHECKPOINT_RECORDS {
        return CheckpointChainResumeDecision::Hold(
            CheckpointChainResumeHold::CheckpointLimitExceeded {
                limit: MAX_CHECKPOINT_RECORDS,
            },
        );
    }

    let mut checkpoint_payloads: Vec<&[u8]> = Vec::new();
    if checkpoint_payloads
        .try_reserve_exact(chain.record_count())
        .is_err()
    {
        return CheckpointChainResumeDecision::Hold(CheckpointChainResumeHold::AllocationFailed);
    }

    let mut predecessor = JournalCursor::genesis();
    for (segment_index, bytes) in segments.iter().enumerate() {
        let segment = match decode_segment(bytes) {
            Ok(segment) => segment,
            Err(error) => {
                return CheckpointChainResumeDecision::Hold(
                    CheckpointChainResumeHold::InvalidSegment {
                        segment_index,
                        error,
                    },
                );
            }
        };
        if segment.verify_predecessor(predecessor).is_err() {
            return CheckpointChainResumeDecision::Hold(
                CheckpointChainResumeHold::PredecessorMismatch { segment_index },
            );
        }
        for (record_index, record) in segment.records().iter().enumerate() {
            if record.kind() != JournalRecordKind::Checkpoint {
                return CheckpointChainResumeDecision::Hold(
                    CheckpointChainResumeHold::NonCheckpointRecord {
                        segment_index,
                        record_index,
                        kind: record.kind(),
                    },
                );
            }
            checkpoint_payloads.push(record.payload());
        }
        predecessor = segment.cursor();
    }

    match inspect_checkpoint_record_bytes(&checkpoint_payloads, identity) {
        Ok(observation) => {
            // The analysis inspector is required to remain read-only and
            // unanchored; preserve that condition as a defensive boundary.
            if observation.integrity_state != lc631_analysis::CheckpointIntegrityState::Unanchored
                || !observation.fresh_revalidation_required
                || observation.authority_created
                || observation.completion_restored
            {
                return CheckpointChainResumeDecision::Hold(CheckpointChainResumeHold::Analysis(
                    ResumeError::CorruptCheckpoint,
                ));
            }
            CheckpointChainResumeDecision::Candidate { chain, observation }
        }
        Err(error) => {
            CheckpointChainResumeDecision::Hold(CheckpointChainResumeHold::Analysis(error))
        }
    }
}
