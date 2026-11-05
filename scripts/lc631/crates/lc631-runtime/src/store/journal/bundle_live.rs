//! Live bundle continuation under a cooperative host session.
use super::bundle_head::{encode_head, frame_digest, LocalHead};
use super::bundle_validation::{validate_existing_bundle, work_claim, State};
use super::segment::{decode_segment, validate_segment_streaming};
use super::{
    JournalChainLimits, JournalCursor, JournalError, JournalHead, JournalIndex, JournalIndexError,
};
use lc631_host::{
    BundlePart, HostError, HostJournalBundleSession, JournalBundleLimits, JournalBundleOpenError,
    JournalBundleWriteError,
};
use lc631_process_supervisor::{ResourceError, ResourceReservationView};
use lc631_substrate::codec::CodecError;
use lc631_substrate::Digest32;
use std::path::Path;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BundleLiveError {
    /// Opening can have created root/lock artifacts even if it fails.
    HostOpen(JournalBundleOpenError),
    Host(HostError),
    Resource(ResourceError),
    /// An incomplete, corrupt or inconsistent existing bundle was observed.
    RebuildRequired,
    ChainLimit,
    StaleCursor,
    InvalidCandidate(JournalError),
    Index(JournalIndexError),
    Codec(CodecError),
    StorageRejectedBeforeWrite(HostError),
    StorageMayHaveApplied(HostError),
    Poisoned,
}

/// Owns one real host lock; reopening always validates the entire bundle chain.
pub struct JournalBundleLiveSession<'a> {
    host: HostJournalBundleSession<'a>,
    resources: &'a ResourceReservationView<'a>,
    limits: JournalChainLimits,
    state: Option<State>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JournalBundleReceipt {
    head: JournalHead,
    file_sequence: u64,
    head_frame_digest: Digest32,
    record_count: usize,
}

impl JournalBundleReceipt {
    pub const fn head(&self) -> JournalHead {
        self.head
    }
    pub const fn file_sequence(&self) -> u64 {
        self.file_sequence
    }
    pub const fn head_frame_digest(&self) -> Digest32 {
        self.head_frame_digest
    }
    pub const fn record_count(&self) -> usize {
        self.record_count
    }
    pub const fn claim_boundary(&self) -> &'static str {
        "local chain and ordered handoff only; no authentication, anchor, authority, crash durability, or delivery"
    }
}

impl<'a> JournalBundleLiveSession<'a> {
    /// A failure can follow creation of root/lock coordination artifacts.
    /// Incomplete or inconsistent existing data is held, not rebuilt or erased.
    pub fn open(
        root: &Path,
        planned_total_bytes: u64,
        store_limits: JournalBundleLimits,
        resources: &'a ResourceReservationView<'a>,
        limits: JournalChainLimits,
    ) -> Result<Self, BundleLiveError> {
        let mut host =
            HostJournalBundleSession::open(root, planned_total_bytes, store_limits, resources)
                .map_err(BundleLiveError::HostOpen)?;
        let state = validate_existing_bundle(&mut host, resources, limits)?;
        Ok(Self {
            host,
            resources,
            limits,
            state: Some(state),
        })
    }

    /// Snapshot only; no external freshness or authority.
    pub fn cursor(&mut self) -> Result<JournalCursor, BundleLiveError> {
        let state = self.state.as_ref().ok_or(BundleLiveError::Poisoned)?;
        let cursor = state.cursor;
        let next = state.next_file;
        match self.host.next_sequence() {
            Ok(value) if value == next => Ok(cursor),
            Ok(_) => {
                self.state = None;
                Err(BundleLiveError::RebuildRequired)
            }
            Err(error) => {
                self.state = None;
                Err(BundleLiveError::Host(error))
            }
        }
    }

    /// All errors discard live state; caller must drop then open/replay afresh.
    pub fn append(
        &mut self,
        expected_predecessor: JournalCursor,
        bytes: &[u8],
    ) -> Result<JournalBundleReceipt, BundleLiveError> {
        self.append_observed(expected_predecessor, bytes, |_| Ok(()))
    }

    /// Runs the observer after each actual write/sync/close. Observer errors
    /// are post-admission and therefore retain MayHaveApplied; this callback
    /// is a local fault seam, not external crash or durability evidence.
    pub fn append_observed<F>(
        &mut self,
        expected_predecessor: JournalCursor,
        bytes: &[u8],
        mut after_part: F,
    ) -> Result<JournalBundleReceipt, BundleLiveError>
    where
        F: FnMut(BundlePart) -> Result<(), HostError>,
    {
        let state = self.state.take().ok_or(BundleLiveError::Poisoned)?;
        let result = (|| {
            if state.cursor != expected_predecessor {
                return Err(BundleLiveError::StaleCursor);
            }
            let segments = state
                .segments
                .checked_add(1)
                .ok_or(BundleLiveError::ChainLimit)?;
            let segment_bytes = state
                .segment_bytes
                .checked_add(bytes.len())
                .ok_or(BundleLiveError::ChainLimit)?;
            if segments > self.limits.max_segments()
                || segment_bytes > self.limits.max_input_bytes()
            {
                return Err(BundleLiveError::ChainLimit);
            }
            let summary = validate_segment_streaming(bytes, state.cursor)
                .map_err(BundleLiveError::InvalidCandidate)?;
            let total_records = state
                .total_records
                .checked_add(summary.record_count)
                .ok_or(BundleLiveError::ChainLimit)?;
            let next_file = state
                .next_file
                .checked_add(1)
                .ok_or(BundleLiveError::ChainLimit)?;
            if self.host.next_sequence().map_err(BundleLiveError::Host)? != state.next_file {
                return Err(BundleLiveError::RebuildRequired);
            }
            let _work = work_claim(self.resources)?;
            let segment = decode_segment(bytes).map_err(BundleLiveError::InvalidCandidate)?;
            segment
                .verify_predecessor(state.cursor)
                .map_err(BundleLiveError::InvalidCandidate)?;
            let index = JournalIndex::from_segment(&segment).map_err(BundleLiveError::Index)?;
            let index_bytes = index.encode().map_err(BundleLiveError::Index)?;
            let index_frame = frame_digest(&index_bytes)?;
            let count =
                u32::try_from(summary.record_count).map_err(|_| BundleLiveError::ChainLimit)?;
            let head = LocalHead {
                file_sequence: state.next_file,
                record_sequence: summary.cursor.sequence(),
                record_digest: summary.cursor.record_digest(),
                segment_frame_digest: summary.local_head.segment_digest(),
                index_frame_digest: index_frame,
                previous_head_frame_digest: state.previous_head,
                record_count: count,
            };
            let head_bytes = encode_head(head)?;
            let head_digest = frame_digest(&head_bytes)?;
            self.host
                .append_bundle_observed(
                    state.next_file,
                    bytes,
                    &index_bytes,
                    &head_bytes,
                    &mut after_part,
                )
                .map_err(|error| match error {
                    JournalBundleWriteError::RejectedBeforeWrite(e) => {
                        BundleLiveError::StorageRejectedBeforeWrite(e)
                    }
                    JournalBundleWriteError::MayHaveApplied(e) => {
                        BundleLiveError::StorageMayHaveApplied(e)
                    }
                })?;
            Ok((
                State {
                    cursor: summary.cursor,
                    next_file,
                    segments,
                    segment_bytes,
                    total_records,
                    previous_head: head_digest,
                },
                JournalBundleReceipt {
                    head: summary.local_head,
                    file_sequence: state.next_file,
                    head_frame_digest: head_digest,
                    record_count: summary.record_count,
                },
            ))
        })();
        match result {
            Ok((state, receipt)) => {
                self.state = Some(state);
                Ok(receipt)
            }
            Err(error) => Err(error),
        }
    }
}
