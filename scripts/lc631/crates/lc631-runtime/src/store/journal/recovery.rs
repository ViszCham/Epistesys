//! Read-only recovery inspection for one owner-held binary checkpoint segment.
//!
//! A candidate is only a local structural summary. It never restores a task's
//! completion, fresh authority, sink delivery or external rollback resistance.
use super::segment::{decode_segment, validate_segment_streaming};
use super::types::{JournalCursor, JournalError, JournalHead, JournalRecordKind};
use lc631_analysis::{
    inspect_checkpoint_record_bytes, CheckpointReadIdentity, ResumeError, ResumeObservation,
};
use lc631_host::{HostError, HostPersistentStoreOwner};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PersistedSegmentHold {
    StorageRead(HostError),
    Decode(JournalError),
    PredecessorMismatch(JournalError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PersistedSegmentInspection {
    CandidateUnanchored {
        head: JournalHead,
        cursor: JournalCursor,
        record_count: usize,
    },
    Hold(PersistedSegmentHold),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PersistedCheckpointResumeHold {
    StorageRead(HostError),
    Decode(JournalError),
    PredecessorMismatch(JournalError),
    UnexpectedRecordKind {
        record_index: usize,
        kind: JournalRecordKind,
    },
    AllocationFailed,
    Analysis(ResumeError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PersistedCheckpointResume {
    Candidate(ResumeObservation),
    Hold(PersistedCheckpointResumeHold),
}

impl PersistedCheckpointResume {
    pub const fn claim_boundary(&self) -> &'static str {
        "owner-read binary checkpoint bytes plus current source/plan/authority checks; observation stays unanchored and never restores completion or grants Authority"
    }
}

impl PersistedSegmentInspection {
    pub const fn claim_boundary(&self) -> &'static str {
        "read-only structural segment summary; does not restore completion, current authority, sink delivery or external rollback resistance"
    }
}

/// Read one already owner-selected binary segment, decode and check its prior
/// cursor, then drop the owner read view before returning the small summary.
/// The owner holds the RAM/output reservation for the byte buffer until the
/// returned read view is dropped. This function never writes, repairs, deletes
/// or authenticates the segment.
pub fn inspect_persisted_segment(
    owner: &HostPersistentStoreOwner<'_>,
    expected_predecessor: JournalCursor,
) -> PersistedSegmentInspection {
    match owner.read_binary_checkpoint_segment() {
        Err(error) => PersistedSegmentInspection::Hold(PersistedSegmentHold::StorageRead(error)),
        Ok(read_view) => {
            let inspection =
                match validate_segment_streaming(read_view.as_bytes(), expected_predecessor) {
                    Ok(segment) => PersistedSegmentInspection::CandidateUnanchored {
                        head: segment.local_head,
                        cursor: segment.cursor,
                        record_count: segment.record_count,
                    },
                    Err(JournalError::StalePredecessor) => PersistedSegmentInspection::Hold(
                        PersistedSegmentHold::PredecessorMismatch(JournalError::StalePredecessor),
                    ),
                    Err(error) => {
                        PersistedSegmentInspection::Hold(PersistedSegmentHold::Decode(error))
                    }
                };
            // No raw segment bytes escape after the read claim is released.
            inspection
        }
    }
}

/// Inspect a persisted binary segment containing only legacy checkpoint
/// payloads, then run the shared read-only checkpoint-chain validator against
/// the caller's current source/plan/authority identity. The owner-held byte
/// claim remains live until parsing and all checks finish.
pub fn inspect_persisted_checkpoint_resume(
    owner: &HostPersistentStoreOwner<'_>,
    expected_predecessor: JournalCursor,
    identity: CheckpointReadIdentity<'_>,
) -> PersistedCheckpointResume {
    let read_view = match owner.read_binary_checkpoint_segment() {
        Ok(read_view) => read_view,
        Err(error) => {
            return PersistedCheckpointResume::Hold(PersistedCheckpointResumeHold::StorageRead(
                error,
            ));
        }
    };
    // Enforce the checkpoint-specific count before materializing the decoder's
    // per-record views. This validates the frame twice for admitted segments;
    // aggregate metadata accounting remains a separate requirement.
    let preflight = match validate_segment_streaming(read_view.as_bytes(), expected_predecessor) {
        Ok(summary) => summary,
        Err(JournalError::StalePredecessor) => {
            return PersistedCheckpointResume::Hold(
                PersistedCheckpointResumeHold::PredecessorMismatch(JournalError::StalePredecessor),
            );
        }
        Err(error) => {
            return PersistedCheckpointResume::Hold(PersistedCheckpointResumeHold::Decode(error));
        }
    };
    if preflight.record_count > lc631_analysis::MAX_CHECKPOINT_RECORDS {
        return PersistedCheckpointResume::Hold(PersistedCheckpointResumeHold::Analysis(
            ResumeError::BudgetExceeded,
        ));
    }
    let segment = match decode_segment(read_view.as_bytes()) {
        Ok(segment) => segment,
        Err(error) => {
            return PersistedCheckpointResume::Hold(PersistedCheckpointResumeHold::Decode(error));
        }
    };
    if let Err(error) = segment.verify_predecessor(expected_predecessor) {
        return PersistedCheckpointResume::Hold(
            PersistedCheckpointResumeHold::PredecessorMismatch(error),
        );
    }
    let mut checkpoint_payloads = Vec::new();
    if checkpoint_payloads
        .try_reserve_exact(segment.records().len())
        .is_err()
    {
        return PersistedCheckpointResume::Hold(PersistedCheckpointResumeHold::AllocationFailed);
    }
    for (record_index, record) in segment.records().iter().enumerate() {
        if record.kind() != JournalRecordKind::Checkpoint {
            return PersistedCheckpointResume::Hold(
                PersistedCheckpointResumeHold::UnexpectedRecordKind {
                    record_index,
                    kind: record.kind(),
                },
            );
        }
        checkpoint_payloads.push(record.payload());
    }

    match inspect_checkpoint_record_bytes(&checkpoint_payloads, identity) {
        Ok(observation) => PersistedCheckpointResume::Candidate(observation),
        Err(error) => {
            PersistedCheckpointResume::Hold(PersistedCheckpointResumeHold::Analysis(error))
        }
    }
}
