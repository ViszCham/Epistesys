//! Checked bridge from a validated binary segment to its owned storage handle.

use super::segment::{validate_segment_streaming, StreamingSegmentSummary};
use super::types::{JournalCursor, JournalError, JournalHead};
use super::JournalChainLimits;
use lc631_host::{BinaryCheckpointSegmentWriteError, HostError, HostPersistentStoreOwner};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JournalPersistenceError {
    InvalidSegment(JournalError),
    PredecessorMismatch(JournalError),
    StorageRejectedBeforeWrite(HostError),
    StorageMayHaveApplied(HostError),
}

/// A local storage receipt. Its head/digest are not an external anchor,
/// signature, authority grant, or rollback-resistance witness.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedJournalSegment {
    head: JournalHead,
    storage_digest: String,
    record_count: usize,
}

impl PersistedJournalSegment {
    pub const fn head(&self) -> JournalHead {
        self.head
    }

    pub fn storage_digest(&self) -> &str {
        &self.storage_digest
    }

    pub const fn record_count(&self) -> usize {
        self.record_count
    }

    pub const fn claim_boundary(&self) -> &'static str {
        "validated local frame plus owner-reported byte handoff; no external anchor, authentication, authority, or delivery"
    }
}

/// Validate one segment and its exact prior cursor before owner-managed write.
/// A bounded scan of the locked local root rejects corrupt histories and forks
/// before admitting the new write. This interim scan reparses prior segments;
/// it is not the indexed append path required for the TLCX-07 slice exit.
/// The owner lock coordinates cooperating writers, not privileged external
/// actors; local agreement is not an authenticated or rollback-resistant head.
/// Errors after admission remain `StorageMayHaveApplied`; no retry or deletion
/// is attempted. `StorageRejectedBeforeWrite` refers to the segment target:
/// creating/acquiring the host owner or its lock may already have had effects.
pub fn persist_segment(
    owner: &mut HostPersistentStoreOwner<'_>,
    bytes: &[u8],
    expected_predecessor: JournalCursor,
) -> Result<PersistedJournalSegment, JournalPersistenceError> {
    let segment = validate_for_append(bytes, expected_predecessor)?;
    validate_locked_root(owner, expected_predecessor, bytes.len())?;
    let head = segment.local_head;
    let record_count = segment.record_count;
    let storage_digest =
        owner
            .write_binary_checkpoint_segment(bytes)
            .map_err(|error| match error {
                BinaryCheckpointSegmentWriteError::RejectedBeforeWrite(error) => {
                    JournalPersistenceError::StorageRejectedBeforeWrite(error)
                }
                BinaryCheckpointSegmentWriteError::MayHaveApplied(error) => {
                    JournalPersistenceError::StorageMayHaveApplied(error)
                }
            })?;
    Ok(PersistedJournalSegment {
        head,
        storage_digest,
        record_count,
    })
}

fn validate_locked_root(
    owner: &HostPersistentStoreOwner<'_>,
    expected_predecessor: JournalCursor,
    candidate_len: usize,
) -> Result<(), JournalPersistenceError> {
    let target_sequence = owner
        .binary_checkpoint_target_sequence()
        .map_err(JournalPersistenceError::StorageRejectedBeforeWrite)?;
    let inventory = owner
        .binary_checkpoint_segment_inventory()
        .map_err(JournalPersistenceError::StorageRejectedBeforeWrite)?;
    let limits = JournalChainLimits::default();
    if inventory
        .len()
        .checked_add(1)
        .is_none_or(|count| count > limits.max_segments())
    {
        return Err(root_limit("binary_checkpoint_root_segment_limit"));
    }
    let mut total_bytes = candidate_len;
    for descriptor in inventory.iter() {
        let size = usize::try_from(descriptor.byte_len())
            .map_err(|_| root_limit("binary_checkpoint_root_byte_limit"))?;
        total_bytes = total_bytes
            .checked_add(size)
            .ok_or_else(|| root_limit("binary_checkpoint_root_byte_limit"))?;
        if total_bytes > limits.max_input_bytes() {
            return Err(root_limit("binary_checkpoint_root_byte_limit"));
        }
    }
    if total_bytes > limits.max_input_bytes() {
        return Err(root_limit("binary_checkpoint_root_byte_limit"));
    }

    let mut cursor = JournalCursor::genesis();
    let mut previous_file_sequence: Option<u64> = None;
    for descriptor in inventory.iter() {
        if let Some(previous) = previous_file_sequence {
            let expected =
                previous
                    .checked_add(1)
                    .ok_or(JournalPersistenceError::PredecessorMismatch(
                        JournalError::SequenceOverflow,
                    ))?;
            if descriptor.sequence() != expected {
                return Err(JournalPersistenceError::PredecessorMismatch(
                    JournalError::SequenceGap,
                ));
            }
        }
        let stored = owner
            .read_binary_checkpoint_segment_descriptor(descriptor)
            .map_err(JournalPersistenceError::StorageRejectedBeforeWrite)?;
        let segment = validate_for_append(stored.as_bytes(), cursor)?;
        cursor = segment.cursor;
        previous_file_sequence = Some(descriptor.sequence());
    }
    if cursor != expected_predecessor {
        return Err(JournalPersistenceError::PredecessorMismatch(
            JournalError::StalePredecessor,
        ));
    }
    if let Some(previous) = previous_file_sequence {
        let next = previous
            .checked_add(1)
            .ok_or(JournalPersistenceError::PredecessorMismatch(
                JournalError::SequenceOverflow,
            ))?;
        if target_sequence != next {
            return Err(JournalPersistenceError::PredecessorMismatch(
                JournalError::SequenceGap,
            ));
        }
    }
    Ok(())
}

fn validate_for_append(
    bytes: &[u8],
    predecessor: JournalCursor,
) -> Result<StreamingSegmentSummary, JournalPersistenceError> {
    validate_segment_streaming(bytes, predecessor).map_err(|error| match error {
        JournalError::StalePredecessor => JournalPersistenceError::PredecessorMismatch(error),
        _ => JournalPersistenceError::InvalidSegment(error),
    })
}

fn root_limit(code: &'static str) -> JournalPersistenceError {
    JournalPersistenceError::StorageRejectedBeforeWrite(HostError::Io(code.into()))
}
