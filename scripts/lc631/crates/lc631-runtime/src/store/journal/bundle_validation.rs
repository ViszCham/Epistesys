//! Shared replay validation for real read-only and live bundle stores.
use super::bundle_head::{decode_head, frame_digest, zero_digest, JOURNAL_LOCAL_HEAD_FRAME_BYTES};
use super::bundle_live::BundleLiveError;
use super::segment::decode_segment;
use super::{
    decode_journal_index, JournalChainLimits, JournalCursor, JournalIndexEntry, JournalRecordView,
    MAX_JOURNAL_INDEX_ENTRIES, MAX_JOURNAL_INDEX_FRAME_BYTES, MAX_JOURNAL_SEGMENT_BYTES,
};
use lc631_host::{
    BundlePart, HostError, HostJournalBundleReader, HostJournalBundleSession,
    OwnedJournalBundleInventory, OwnedJournalBundlePart,
};
use lc631_process_supervisor::{
    ResourceAmount, ResourceKind, ResourceRequest, ResourceReservationView,
};
use lc631_substrate::Digest32;

const WORK_BYTES: u64 = 8 * 1024 * 1024;
pub(super) fn work_claim<'a>(
    resources: &'a ResourceReservationView<'a>,
) -> Result<lc631_process_supervisor::ResourceClaim<'a>, BundleLiveError> {
    // Account for record views, two index vectors, simultaneous payload/frame
    // buffers, and bounded decoder/head metadata. This logical bound is not RSS.
    let per_entry = std::mem::size_of::<JournalRecordView<'static>>()
        .checked_add(
            std::mem::size_of::<JournalIndexEntry>()
                .checked_mul(2)
                .ok_or(BundleLiveError::ChainLimit)?,
        )
        .ok_or(BundleLiveError::ChainLimit)?;
    let need = MAX_JOURNAL_INDEX_ENTRIES
        .checked_mul(per_entry)
        .and_then(|n| n.checked_add(MAX_JOURNAL_INDEX_FRAME_BYTES.checked_mul(2)?))
        .and_then(|n| n.checked_add(1024 * 1024))
        .ok_or(BundleLiveError::ChainLimit)?;
    if u64::try_from(need).map_err(|_| BundleLiveError::ChainLimit)? > WORK_BYTES {
        return Err(BundleLiveError::ChainLimit);
    }
    let amount = ResourceAmount::checked(WORK_BYTES).map_err(BundleLiveError::Resource)?;
    resources
        .claim_resources(&[
            ResourceRequest::new(ResourceKind::Ram, amount),
            ResourceRequest::new(ResourceKind::Output, amount),
        ])
        .map_err(BundleLiveError::Resource)
}

pub(super) struct State {
    pub(super) cursor: JournalCursor,
    pub(super) next_file: u64,
    pub(super) segments: usize,
    pub(super) segment_bytes: usize,
    pub(super) total_records: usize,
    pub(super) previous_head: Digest32,
}

// Private port: callers cannot attach arbitrary reports or implementations.
pub(super) trait BundleRead<'a> {
    fn inventory(&mut self) -> Result<OwnedJournalBundleInventory<'a>, HostError>;
    fn read_part(
        &mut self,
        sequence: u64,
        part: BundlePart,
    ) -> Result<OwnedJournalBundlePart<'a>, HostError>;
    fn validate_snapshot(&mut self) -> Result<(), HostError>;
    fn next_sequence(&mut self) -> Result<u64, HostError>;
}
impl<'a> BundleRead<'a> for HostJournalBundleSession<'a> {
    fn inventory(&mut self) -> Result<OwnedJournalBundleInventory<'a>, HostError> {
        HostJournalBundleSession::inventory(self)
    }
    fn read_part(
        &mut self,
        sequence: u64,
        part: BundlePart,
    ) -> Result<OwnedJournalBundlePart<'a>, HostError> {
        HostJournalBundleSession::read_part(self, sequence, part)
    }
    fn validate_snapshot(&mut self) -> Result<(), HostError> {
        HostJournalBundleSession::validate_snapshot(self)
    }
    fn next_sequence(&mut self) -> Result<u64, HostError> {
        HostJournalBundleSession::next_sequence(self)
    }
}

impl<'a> BundleRead<'a> for HostJournalBundleReader<'a> {
    fn inventory(&mut self) -> Result<OwnedJournalBundleInventory<'a>, HostError> {
        HostJournalBundleReader::inventory(self)
    }
    fn read_part(
        &mut self,
        sequence: u64,
        part: BundlePart,
    ) -> Result<OwnedJournalBundlePart<'a>, HostError> {
        HostJournalBundleReader::read_part(self, sequence, part)
    }
    fn validate_snapshot(&mut self) -> Result<(), HostError> {
        HostJournalBundleReader::validate_snapshot(self)
    }
    fn next_sequence(&mut self) -> Result<u64, HostError> {
        HostJournalBundleReader::next_sequence(self)
    }
}
pub(super) fn validate_existing_bundle<'a>(
    host: &mut impl BundleRead<'a>,
    resources: &'a ResourceReservationView<'a>,
    limits: JournalChainLimits,
) -> Result<State, BundleLiveError> {
    let inventory = host.inventory().map_err(BundleLiveError::Host)?;
    let (segment_bytes, expected) = preflight_inventory(inventory.items(), limits)?;
    let _work = work_claim(resources)?;
    let mut cursor = JournalCursor::genesis();
    let mut previous_head = zero_digest();
    let mut total_records = 0_usize;
    for item in inventory.items() {
        let checked = replay_bundle(host, item.sequence(), cursor, previous_head)?;
        total_records = total_records
            .checked_add(checked.records)
            .ok_or(BundleLiveError::ChainLimit)?;
        cursor = checked.cursor;
        previous_head = checked.head_digest;
    }
    // One final raw snapshot comparison, after the streaming replay.
    host.validate_snapshot().map_err(BundleLiveError::Host)?;
    if host.next_sequence().map_err(BundleLiveError::Host)? != expected {
        return Err(BundleLiveError::RebuildRequired);
    }
    let segments = inventory.items().len();
    drop(inventory);

    Ok(State {
        cursor,
        next_file: expected,
        segments,
        segment_bytes,
        total_records,
        previous_head,
    })
}

fn preflight_inventory(
    items: &[lc631_host::JournalBundleDescriptor],
    limits: JournalChainLimits,
) -> Result<(usize, u64), BundleLiveError> {
    if items.len() > limits.max_segments() {
        return Err(BundleLiveError::ChainLimit);
    }
    let mut segment_bytes = 0_usize;
    let mut expected = 1_u64;
    for item in items {
        if item.sequence() != expected {
            return Err(BundleLiveError::RebuildRequired);
        }
        expected = expected.checked_add(1).ok_or(BundleLiveError::ChainLimit)?;
        let segment = item
            .part(BundlePart::Segment)
            .ok_or(BundleLiveError::RebuildRequired)?;
        let index = item
            .part(BundlePart::Index)
            .ok_or(BundleLiveError::RebuildRequired)?;
        let head = item
            .part(BundlePart::LocalHead)
            .ok_or(BundleLiveError::RebuildRequired)?;
        if segment.byte_len() == 0
            || segment.byte_len() > MAX_JOURNAL_SEGMENT_BYTES as u64
            || index.byte_len() == 0
            || index.byte_len() > MAX_JOURNAL_INDEX_FRAME_BYTES as u64
            || head.byte_len() != JOURNAL_LOCAL_HEAD_FRAME_BYTES as u64
        {
            return Err(BundleLiveError::RebuildRequired);
        }
        let len = usize::try_from(segment.byte_len()).map_err(|_| BundleLiveError::ChainLimit)?;
        segment_bytes = segment_bytes
            .checked_add(len)
            .ok_or(BundleLiveError::ChainLimit)?;
        if segment_bytes > limits.max_input_bytes() {
            return Err(BundleLiveError::ChainLimit);
        }
    }

    Ok((segment_bytes, expected))
}

struct ReplayedBundle {
    cursor: JournalCursor,
    head_digest: Digest32,
    records: usize,
}

fn replay_bundle<'a>(
    host: &mut impl BundleRead<'a>,
    seq: u64,
    cursor: JournalCursor,
    previous_head: Digest32,
) -> Result<ReplayedBundle, BundleLiveError> {
    let raw_segment = host
        .read_part(seq, BundlePart::Segment)
        .map_err(BundleLiveError::Host)?;
    let raw_index = host
        .read_part(seq, BundlePart::Index)
        .map_err(BundleLiveError::Host)?;
    let raw_head = host
        .read_part(seq, BundlePart::LocalHead)
        .map_err(BundleLiveError::Host)?;
    let segment =
        decode_segment(raw_segment.as_bytes()).map_err(|_| BundleLiveError::RebuildRequired)?;
    segment
        .verify_predecessor(cursor)
        .map_err(|_| BundleLiveError::RebuildRequired)?;
    let index =
        decode_journal_index(raw_index.as_bytes()).map_err(|_| BundleLiveError::RebuildRequired)?;
    index
        .verify_segment(&segment)
        .map_err(|_| BundleLiveError::RebuildRequired)?;
    let index_frame =
        frame_digest(raw_index.as_bytes()).map_err(|_| BundleLiveError::RebuildRequired)?;
    let (head, head_digest) = decode_head(raw_head.as_bytes())?;
    let count = u32::try_from(segment.records().len()).map_err(|_| BundleLiveError::ChainLimit)?;
    let next_cursor = segment.cursor();
    if head.file_sequence != seq
        || head.record_sequence != next_cursor.sequence()
        || head.record_digest != next_cursor.record_digest()
        || head.segment_frame_digest != segment.local_head().segment_digest()
        || head.index_frame_digest != index_frame
        || head.previous_head_frame_digest != previous_head
        || head.record_count != count
    {
        return Err(BundleLiveError::RebuildRequired);
    }

    Ok(ReplayedBundle {
        cursor: next_cursor,
        head_digest,
        records: segment.records().len(),
    })
}
