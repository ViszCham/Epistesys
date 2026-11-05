//! Live host-bound journal continuation; local consistency only.
use super::segment::validate_segment_streaming;
use super::{JournalChainLimits, JournalCursor, JournalError, JournalHead};
use lc631_host::{
    BinaryCheckpointSegmentWriteError, HostBinaryCheckpointLiveSession, HostError,
    PersistentStoreLimits,
};
use lc631_process_supervisor::ResourceReservationView;
use std::path::Path;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JournalLiveError {
    /// Open may already have created root/lock artifacts.
    Host(HostError),
    InvalidSegment(JournalError),
    StaleCursor,
    FileSequenceMismatch,
    ChainLimit,
    StorageRejectedBeforeWrite(HostError),
    StorageMayHaveApplied(HostError),
    Poisoned,
}

/// Local handoff plus validation of the appended segment only. Neither this
/// receipt nor its head authenticates history or proves rollback resistance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveJournalReceipt {
    head: JournalHead,
    storage_digest: String,
    record_count: usize,
}
impl LiveJournalReceipt {
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
        "same-live-session local chain validation and storage handoff; not authentication, rollback protection, completion, authority, or delivery"
    }
}

struct ValidatedState {
    cursor: JournalCursor,
    segments: usize,
    input_bytes: usize,
    record_count: usize,
}

/// A non-clone live capability. No constructor accepts a reported cursor, head,
/// boolean or serialized state and no method extracts the host owner.
pub struct JournalLiveSession<'a> {
    host: HostBinaryCheckpointLiveSession<'a>,
    limits: JournalChainLimits,
    state: Option<ValidatedState>,
}
impl<'a> JournalLiveSession<'a> {
    /// Acquire the actual host session, then replay its complete bounded raw
    /// inventory from genesis while the same cooperating lock is held. Open
    /// failures do not imply no host-side coordination artifacts were created.
    pub fn open(
        root: &Path,
        planned_total_bytes: u64,
        store_limits: PersistentStoreLimits,
        resources: &'a ResourceReservationView<'a>,
        limits: JournalChainLimits,
    ) -> Result<Self, JournalLiveError> {
        let mut host = HostBinaryCheckpointLiveSession::open(
            root,
            planned_total_bytes,
            store_limits,
            resources,
        )
        .map_err(JournalLiveError::Host)?;
        let inventory = host.inventory().map_err(JournalLiveError::Host)?;
        if inventory.len() > limits.max_segments() {
            return Err(JournalLiveError::ChainLimit);
        }
        let mut total = 0_usize;
        for descriptor in inventory.iter() {
            let len =
                usize::try_from(descriptor.byte_len()).map_err(|_| JournalLiveError::ChainLimit)?;
            total = total.checked_add(len).ok_or(JournalLiveError::ChainLimit)?;
            if total > limits.max_input_bytes() {
                return Err(JournalLiveError::ChainLimit);
            }
        }
        let mut cursor = JournalCursor::genesis();
        let mut record_count = 0_usize;
        let mut previous_file = None::<u64>;
        for descriptor in inventory.iter() {
            if let Some(previous) = previous_file {
                if previous.checked_add(1) != Some(descriptor.sequence()) {
                    return Err(JournalLiveError::FileSequenceMismatch);
                }
            }
            let raw = host
                .read_descriptor(descriptor)
                .map_err(JournalLiveError::Host)?;
            let summary = validate_segment_streaming(raw.as_bytes(), cursor)
                .map_err(JournalLiveError::InvalidSegment)?;
            record_count = record_count
                .checked_add(summary.record_count)
                .ok_or(JournalLiveError::ChainLimit)?;
            cursor = summary.cursor;
            previous_file = Some(descriptor.sequence());
        }
        let expected_next = match previous_file {
            Some(previous) => previous
                .checked_add(1)
                .ok_or(JournalLiveError::ChainLimit)?,
            None => 1,
        };
        if host.next_sequence().map_err(JournalLiveError::Host)? != expected_next {
            return Err(JournalLiveError::FileSequenceMismatch);
        }
        let segments = inventory.len();
        drop(inventory);
        Ok(Self {
            host,
            limits,
            state: Some(ValidatedState {
                cursor,
                segments,
                input_bytes: total,
                record_count,
            }),
        })
    }

    /// The current process-local cursor; this observation is not a freshness,
    /// authenticity, authority, or rollback-resistance receipt.
    pub fn cursor(&mut self) -> Result<JournalCursor, JournalLiveError> {
        let cursor = self
            .state
            .as_ref()
            .map(|state| state.cursor)
            .ok_or(JournalLiveError::Poisoned)?;
        if let Err(error) = self.host.next_sequence() {
            self.state = None;
            return Err(JournalLiveError::Host(error));
        }
        Ok(cursor)
    }

    /// Validate only the proposed segment against the private live cursor.
    /// Any error discards that cursor; reopening requires fresh full replay.
    pub fn append(
        &mut self,
        expected_predecessor: JournalCursor,
        bytes: &[u8],
    ) -> Result<LiveJournalReceipt, JournalLiveError> {
        let state = self.state.take().ok_or(JournalLiveError::Poisoned)?;
        let result = (|| {
            if expected_predecessor != state.cursor {
                return Err(JournalLiveError::StaleCursor);
            }
            let segments = state
                .segments
                .checked_add(1)
                .ok_or(JournalLiveError::ChainLimit)?;
            let input_bytes = state
                .input_bytes
                .checked_add(bytes.len())
                .ok_or(JournalLiveError::ChainLimit)?;
            if segments > self.limits.max_segments() || input_bytes > self.limits.max_input_bytes()
            {
                return Err(JournalLiveError::ChainLimit);
            }
            let summary = validate_segment_streaming(bytes, state.cursor)
                .map_err(JournalLiveError::InvalidSegment)?;
            let record_count = state
                .record_count
                .checked_add(summary.record_count)
                .ok_or(JournalLiveError::ChainLimit)?;
            let sequence = self.host.next_sequence().map_err(JournalLiveError::Host)?;
            let digest = self
                .host
                .append(sequence, bytes)
                .map_err(|error| match error {
                    BinaryCheckpointSegmentWriteError::RejectedBeforeWrite(error) => {
                        JournalLiveError::StorageRejectedBeforeWrite(error)
                    }
                    BinaryCheckpointSegmentWriteError::MayHaveApplied(error) => {
                        JournalLiveError::StorageMayHaveApplied(error)
                    }
                })?;
            Ok((
                ValidatedState {
                    cursor: summary.cursor,
                    segments,
                    input_bytes,
                    record_count,
                },
                LiveJournalReceipt {
                    head: summary.local_head,
                    storage_digest: digest,
                    record_count: summary.record_count,
                },
            ))
        })();
        match result {
            Ok((next, receipt)) => {
                self.state = Some(next);
                Ok(receipt)
            }
            Err(error) => Err(error),
        }
    }
}
