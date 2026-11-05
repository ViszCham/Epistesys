//! Bounded codec for a rebuildable local journal-segment index.
//!
//! Persistent bytes use the shared canonical EPITLCX1 frame. Its one optional
//! opaque extension carries this module's bounded EJIDX001 payload. The index
//! is only a content-checked locator cache, never an external anchor,
//! authenticity proof, semantic truth, completion record, or rollback
//! protection. Decode failures are typed errors; callers must rebuild from a
//! validated segment or hold, never promote the index itself.
#![forbid(unsafe_code)]

use super::types::JournalSegmentView;
use super::{
    MAX_JOURNAL_RECORD_BYTES, MAX_JOURNAL_SEGMENT_BYTES, MAX_RECORDS_PER_SEGMENT,
    RECORD_HEADER_BYTES,
};
use lc631_substrate::codec::{
    decode_envelope, encode_frame, CodecError, SectionInput, SectionKind,
};
use lc631_substrate::Digest32;
use sha2::{Digest, Sha256};
use std::convert::TryFrom;

const INDEX_MAGIC: &[u8; 8] = b"EJIDX001";
const INDEX_VERSION: u16 = 1;
const INDEX_DOMAIN: &[u8] = b"Epistesys/TLCX/JournalIndex/sha256/v1\0";
/// Optional opaque EPITLCX1 section carrying the journal-index v1 payload.
pub const JOURNAL_INDEX_SECTION_TAG: u16 = 0x0201;
const INDEX_FRAME_OVERHEAD: usize = 64 + 32;
const INDEX_FLAGS: u16 = 0;
const HEADER_LEN_OFFSET: usize = 12;
const TOTAL_LEN_OFFSET: usize = 16;
const ENTRY_COUNT_OFFSET: usize = 24;
const ENTRIES_LEN_OFFSET: usize = 28;
const RECORDS_LEN_OFFSET: usize = 32;
const RESERVED_OFFSET: usize = 36;
const SEGMENT_DIGEST_OFFSET: usize = 40;
const INDEX_DIGEST_OFFSET: usize = 72;

/// Fixed encoded header size; fields are serialized explicitly in LE order.
pub const JOURNAL_INDEX_HEADER_BYTES: usize = 104;
/// Fixed encoded entry size: sequence:u64, offset:u32, length:u32, digest:32.
pub const JOURNAL_INDEX_ENTRY_BYTES: usize = 48;
/// A segment cannot contain more records than the journal decoder admits.
pub const MAX_JOURNAL_INDEX_ENTRIES: usize = MAX_RECORDS_PER_SEGMENT;
/// Maximum inner EJIDX001 payload size, checked before any entry allocation.
pub const MAX_JOURNAL_INDEX_BYTES: usize =
    JOURNAL_INDEX_HEADER_BYTES + JOURNAL_INDEX_ENTRY_BYTES * MAX_JOURNAL_INDEX_ENTRIES;
/// Maximum complete EPITLCX1 index frame, including its header and directory.
pub const MAX_JOURNAL_INDEX_FRAME_BYTES: usize = MAX_JOURNAL_INDEX_BYTES + INDEX_FRAME_OVERHEAD;

/// Typed failures for malformed, stale, or unallocatable local indexes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JournalIndexError {
    Frame(CodecError),
    InputTooLarge,
    TruncatedHeader,
    InvalidMagic,
    UnsupportedVersion,
    InvalidFrameShape,
    WrongSectionTag,
    UnsupportedSectionVersion,
    InvalidSectionFlags,
    InvalidSectionKind,
    SectionCountMismatch,
    UnknownFlags,
    InvalidHeaderLength,
    InvalidTotalLength,
    TooManyEntries,
    EmptyIndex,
    EntryCountMismatch,
    NonzeroReserved,
    RecordBytesOutOfBounds,
    IntegrityMismatch,
    ZeroSequence,
    NonMonotonicSequence,
    SequenceGap,
    NonContiguousOffset,
    RecordTooSmall,
    RecordTooLarge,
    RecordBytesLengthMismatch,
    InvalidSegmentMetadata,
    StaleSegment,
    AllocationFailed,
}

impl From<CodecError> for JournalIndexError {
    fn from(error: CodecError) -> Self {
        Self::Frame(error)
    }
}

/// A record locator relative to the concatenated journal-record section.
///
/// This is not a file offset and cannot be dereferenced without first
/// validating the target segment and matching it with [`JournalIndex::verify_segment`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JournalIndexEntry {
    sequence: u64,
    offset: u32,
    record_len: u32,
    record_digest: Digest32,
}

impl JournalIndexEntry {
    pub const fn sequence(self) -> u64 {
        self.sequence
    }

    pub const fn offset(self) -> u32 {
        self.offset
    }

    pub const fn record_len(self) -> u32 {
        self.record_len
    }

    pub const fn record_digest(self) -> Digest32 {
        self.record_digest
    }
}

/// An owned, validated index for one bounded journal segment.
///
/// The private fields ensure entries originate from a validated segment or
/// from [`decode_journal_index`]'s complete header, digest, order, and bounds
/// checks. This type confers no retention, resume, completion, or anchor power.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JournalIndex {
    segment_digest: Digest32,
    records_len: u32,
    entries: Vec<JournalIndexEntry>,
}

impl JournalIndex {
    /// Build a local index from a segment already admitted by `decode_segment`.
    pub fn from_segment(segment: &JournalSegmentView<'_>) -> Result<Self, JournalIndexError> {
        let records = segment.records();
        if records.is_empty() {
            return Err(JournalIndexError::EmptyIndex);
        }
        if records.len() > MAX_JOURNAL_INDEX_ENTRIES {
            return Err(JournalIndexError::TooManyEntries);
        }

        let mut entries = Vec::new();
        entries
            .try_reserve_exact(records.len())
            .map_err(|_| JournalIndexError::AllocationFailed)?;
        let mut next_offset = 0_usize;

        for record in records.iter().copied() {
            let offset = record
                .payload_offset()
                .checked_sub(RECORD_HEADER_BYTES)
                .ok_or(JournalIndexError::InvalidSegmentMetadata)?;
            if offset != next_offset || record.sequence() == 0 {
                return Err(JournalIndexError::InvalidSegmentMetadata);
            }
            let record_len = RECORD_HEADER_BYTES
                .checked_add(record.payload().len())
                .ok_or(JournalIndexError::InvalidSegmentMetadata)?;
            if record_len < RECORD_HEADER_BYTES {
                return Err(JournalIndexError::RecordTooSmall);
            }
            if record_len > MAX_JOURNAL_RECORD_BYTES {
                return Err(JournalIndexError::RecordTooLarge);
            }
            let end = offset
                .checked_add(record_len)
                .ok_or(JournalIndexError::RecordBytesOutOfBounds)?;
            let offset =
                u32::try_from(offset).map_err(|_| JournalIndexError::RecordBytesOutOfBounds)?;
            let record_len =
                u32::try_from(record_len).map_err(|_| JournalIndexError::RecordBytesOutOfBounds)?;
            entries.push(JournalIndexEntry {
                sequence: record.sequence(),
                offset,
                record_len,
                record_digest: record.digest(),
            });
            next_offset = end;
        }

        let records_len =
            u32::try_from(next_offset).map_err(|_| JournalIndexError::RecordBytesOutOfBounds)?;
        validate_entries(&entries, records_len as usize)?;
        Ok(Self {
            segment_digest: segment.frame_digest(),
            records_len,
            entries,
        })
    }

    /// Encode one canonical EPITLCX1 frame containing this module's fixed-width
    /// LE index payload. Both frame and inner digests detect byte changes, not
    /// rollback or authenticity.
    pub fn encode(&self) -> Result<Vec<u8>, JournalIndexError> {
        let payload = self.encode_payload()?;
        let count =
            u32::try_from(self.entries.len()).map_err(|_| JournalIndexError::TooManyEntries)?;
        let frame = encode_frame(&[SectionInput {
            tag: JOURNAL_INDEX_SECTION_TAG,
            version: INDEX_VERSION,
            flags: 0,
            count,
            payload: &payload,
        }])?;
        if frame.len() > MAX_JOURNAL_INDEX_FRAME_BYTES {
            return Err(JournalIndexError::InputTooLarge);
        }
        Ok(frame)
    }

    fn encode_payload(&self) -> Result<Vec<u8>, JournalIndexError> {
        validate_entries(&self.entries, self.records_len as usize)?;
        let entry_count =
            u32::try_from(self.entries.len()).map_err(|_| JournalIndexError::TooManyEntries)?;
        let entries_len = self
            .entries
            .len()
            .checked_mul(JOURNAL_INDEX_ENTRY_BYTES)
            .ok_or(JournalIndexError::InputTooLarge)?;
        let total_len = JOURNAL_INDEX_HEADER_BYTES
            .checked_add(entries_len)
            .ok_or(JournalIndexError::InputTooLarge)?;
        if total_len > MAX_JOURNAL_INDEX_BYTES {
            return Err(JournalIndexError::InputTooLarge);
        }
        let total_len_u64 =
            u64::try_from(total_len).map_err(|_| JournalIndexError::InputTooLarge)?;
        let entries_len_u32 =
            u32::try_from(entries_len).map_err(|_| JournalIndexError::InputTooLarge)?;

        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(total_len)
            .map_err(|_| JournalIndexError::AllocationFailed)?;
        bytes.extend_from_slice(INDEX_MAGIC);
        bytes.extend_from_slice(&INDEX_VERSION.to_le_bytes());
        bytes.extend_from_slice(&INDEX_FLAGS.to_le_bytes());
        bytes.extend_from_slice(
            &u32::try_from(JOURNAL_INDEX_HEADER_BYTES)
                .map_err(|_| JournalIndexError::InvalidHeaderLength)?
                .to_le_bytes(),
        );
        bytes.extend_from_slice(&total_len_u64.to_le_bytes());
        bytes.extend_from_slice(&entry_count.to_le_bytes());
        bytes.extend_from_slice(&entries_len_u32.to_le_bytes());
        bytes.extend_from_slice(&self.records_len.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(self.segment_digest.as_bytes());
        bytes.extend_from_slice(&[0_u8; 32]);

        for entry in &self.entries {
            bytes.extend_from_slice(&entry.sequence.to_le_bytes());
            bytes.extend_from_slice(&entry.offset.to_le_bytes());
            bytes.extend_from_slice(&entry.record_len.to_le_bytes());
            bytes.extend_from_slice(entry.record_digest.as_bytes());
        }
        if bytes.len() != total_len {
            return Err(JournalIndexError::InvalidTotalLength);
        }
        let digest = index_digest(&bytes);
        bytes[INDEX_DIGEST_OFFSET..JOURNAL_INDEX_HEADER_BYTES].copy_from_slice(digest.as_bytes());
        Ok(bytes)
    }

    /// Compare this local index with a freshly decoded segment view.
    ///
    /// A mismatch is a stale-index error and must be handled as Hold/rebuild.
    /// A match is local consistency only and is never an external anchor.
    pub fn verify_segment(
        &self,
        segment: &JournalSegmentView<'_>,
    ) -> Result<(), JournalIndexError> {
        let rebuilt = Self::from_segment(segment)?;
        if *self == rebuilt {
            Ok(())
        } else {
            Err(JournalIndexError::StaleSegment)
        }
    }

    pub const fn segment_digest(&self) -> Digest32 {
        self.segment_digest
    }

    pub const fn records_len(&self) -> u32 {
        self.records_len
    }

    pub fn entries(&self) -> &[JournalIndexEntry] {
        &self.entries
    }

    pub const fn claim_boundary(&self) -> &'static str {
        "validated local segment index only; not an external anchor, rollback protection, authenticity, truth, completion, or Authority"
    }
}

/// Decode one exact canonical EPITLCX1 frame with the required optional
/// journal-index section. The shared frame decoder rejects noncanonical or
/// trailing bytes; inner entry vectors are allocated only after their full
/// header, count, digest, order, and bounds checks.
pub fn decode_journal_index(bytes: &[u8]) -> Result<JournalIndex, JournalIndexError> {
    if bytes.len() > MAX_JOURNAL_INDEX_FRAME_BYTES {
        return Err(JournalIndexError::InputTooLarge);
    }
    let frame = decode_envelope(bytes)?;
    if frame.minor() != 0 {
        return Err(JournalIndexError::Frame(CodecError::UnsupportedMinor));
    }
    if frame.sections().len() != 1 {
        return Err(JournalIndexError::InvalidFrameShape);
    }
    let section = &frame.sections()[0];
    if section.tag() != JOURNAL_INDEX_SECTION_TAG {
        return Err(JournalIndexError::WrongSectionTag);
    }
    if section.version() != INDEX_VERSION {
        return Err(JournalIndexError::UnsupportedSectionVersion);
    }
    if section.flags() != 0 {
        return Err(JournalIndexError::InvalidSectionFlags);
    }
    if section.kind() != SectionKind::Opaque {
        return Err(JournalIndexError::InvalidSectionKind);
    }
    decode_index_payload(section.payload(), section.count())
}

fn decode_index_payload(
    bytes: &[u8],
    section_count: u32,
) -> Result<JournalIndex, JournalIndexError> {
    if bytes.len() > MAX_JOURNAL_INDEX_BYTES {
        return Err(JournalIndexError::InputTooLarge);
    }
    if bytes.len() < JOURNAL_INDEX_HEADER_BYTES {
        return Err(JournalIndexError::TruncatedHeader);
    }
    if bytes.get(..8) != Some(INDEX_MAGIC.as_slice()) {
        return Err(JournalIndexError::InvalidMagic);
    }
    if read_u16(bytes, 8)? != INDEX_VERSION {
        return Err(JournalIndexError::UnsupportedVersion);
    }
    if read_u16(bytes, 10)? != INDEX_FLAGS {
        return Err(JournalIndexError::UnknownFlags);
    }
    if usize::try_from(read_u32(bytes, HEADER_LEN_OFFSET)?)
        .map_err(|_| JournalIndexError::InvalidHeaderLength)?
        != JOURNAL_INDEX_HEADER_BYTES
    {
        return Err(JournalIndexError::InvalidHeaderLength);
    }

    let declared_total = usize::try_from(read_u64(bytes, TOTAL_LEN_OFFSET)?)
        .map_err(|_| JournalIndexError::InvalidTotalLength)?;
    if declared_total > MAX_JOURNAL_INDEX_BYTES {
        return Err(JournalIndexError::InputTooLarge);
    }
    if declared_total != bytes.len() {
        return Err(JournalIndexError::InvalidTotalLength);
    }

    let entry_count = usize::try_from(read_u32(bytes, ENTRY_COUNT_OFFSET)?)
        .map_err(|_| JournalIndexError::TooManyEntries)?;
    if entry_count == 0 {
        return Err(JournalIndexError::EmptyIndex);
    }
    if entry_count > MAX_JOURNAL_INDEX_ENTRIES {
        return Err(JournalIndexError::TooManyEntries);
    }
    if u32::try_from(entry_count).map_err(|_| JournalIndexError::TooManyEntries)? != section_count {
        return Err(JournalIndexError::SectionCountMismatch);
    }
    let entries_len = usize::try_from(read_u32(bytes, ENTRIES_LEN_OFFSET)?)
        .map_err(|_| JournalIndexError::EntryCountMismatch)?;
    let expected_entries_len = entry_count
        .checked_mul(JOURNAL_INDEX_ENTRY_BYTES)
        .ok_or(JournalIndexError::EntryCountMismatch)?;
    if entries_len != expected_entries_len {
        return Err(JournalIndexError::EntryCountMismatch);
    }
    let expected_total = JOURNAL_INDEX_HEADER_BYTES
        .checked_add(entries_len)
        .ok_or(JournalIndexError::InvalidTotalLength)?;
    if expected_total != declared_total {
        return Err(JournalIndexError::InvalidTotalLength);
    }
    let records_len = usize::try_from(read_u32(bytes, RECORDS_LEN_OFFSET)?)
        .map_err(|_| JournalIndexError::RecordBytesOutOfBounds)?;
    let minimum_records_len = entry_count
        .checked_mul(RECORD_HEADER_BYTES)
        .ok_or(JournalIndexError::RecordBytesOutOfBounds)?;
    if records_len < minimum_records_len || records_len > MAX_JOURNAL_SEGMENT_BYTES {
        return Err(JournalIndexError::RecordBytesOutOfBounds);
    }
    if read_u32(bytes, RESERVED_OFFSET)? != 0 {
        return Err(JournalIndexError::NonzeroReserved);
    }

    let stored_digest = read_digest(bytes, INDEX_DIGEST_OFFSET)?;
    if stored_digest != index_digest(bytes) {
        return Err(JournalIndexError::IntegrityMismatch);
    }
    validate_encoded_entries(bytes, entry_count, records_len)?;

    let mut entries = Vec::new();
    entries
        .try_reserve_exact(entry_count)
        .map_err(|_| JournalIndexError::AllocationFailed)?;
    for index in 0..entry_count {
        let offset = entry_wire_offset(index)?;
        entries.push(JournalIndexEntry {
            sequence: read_u64(bytes, offset)?,
            offset: read_u32(bytes, offset + 8)?,
            record_len: read_u32(bytes, offset + 12)?,
            record_digest: read_digest(bytes, offset + 16)?,
        });
    }
    Ok(JournalIndex {
        segment_digest: read_digest(bytes, SEGMENT_DIGEST_OFFSET)?,
        records_len: u32::try_from(records_len)
            .map_err(|_| JournalIndexError::RecordBytesOutOfBounds)?,
        entries,
    })
}

fn validate_encoded_entries(
    bytes: &[u8],
    entry_count: usize,
    records_len: usize,
) -> Result<(), JournalIndexError> {
    let fields = (0..entry_count).map(|index| -> Result<_, JournalIndexError> {
        let offset = entry_wire_offset(index)?;
        Ok((
            read_u64(bytes, offset)?,
            read_u32(bytes, offset + 8)?,
            read_u32(bytes, offset + 12)?,
        ))
    });
    validate_entry_fields(fields, entry_count, records_len)
}

fn validate_entries(
    entries: &[JournalIndexEntry],
    records_len: usize,
) -> Result<(), JournalIndexError> {
    let fields = entries
        .iter()
        .map(|entry| Ok::<_, JournalIndexError>((entry.sequence, entry.offset, entry.record_len)));
    validate_entry_fields(fields, entries.len(), records_len)
}

fn validate_entry_fields<Fields>(
    fields: Fields,
    entry_count: usize,
    records_len: usize,
) -> Result<(), JournalIndexError>
where
    Fields: IntoIterator<Item = Result<(u64, u32, u32), JournalIndexError>>,
{
    if entry_count == 0 {
        return Err(JournalIndexError::EmptyIndex);
    }
    if entry_count > MAX_JOURNAL_INDEX_ENTRIES {
        return Err(JournalIndexError::TooManyEntries);
    }
    let minimum_records_len = entry_count
        .checked_mul(RECORD_HEADER_BYTES)
        .ok_or(JournalIndexError::RecordBytesOutOfBounds)?;
    if records_len < minimum_records_len || records_len > MAX_JOURNAL_SEGMENT_BYTES {
        return Err(JournalIndexError::RecordBytesOutOfBounds);
    }

    let mut expected_offset = 0_usize;
    let mut previous_sequence: Option<u64> = None;

    for fields in fields {
        let (sequence, entry_offset, encoded_record_len) = fields?;
        if sequence == 0 {
            return Err(JournalIndexError::ZeroSequence);
        }
        if let Some(previous) = previous_sequence {
            let expected_sequence = previous
                .checked_add(1)
                .ok_or(JournalIndexError::SequenceGap)?;
            if sequence < expected_sequence {
                return Err(JournalIndexError::NonMonotonicSequence);
            }
            if sequence > expected_sequence {
                return Err(JournalIndexError::SequenceGap);
            }
        }
        let entry_offset =
            usize::try_from(entry_offset).map_err(|_| JournalIndexError::RecordBytesOutOfBounds)?;
        if entry_offset != expected_offset {
            return Err(JournalIndexError::NonContiguousOffset);
        }
        let record_len = usize::try_from(encoded_record_len)
            .map_err(|_| JournalIndexError::RecordBytesOutOfBounds)?;
        if record_len < RECORD_HEADER_BYTES {
            return Err(JournalIndexError::RecordTooSmall);
        }
        if record_len > MAX_JOURNAL_RECORD_BYTES {
            return Err(JournalIndexError::RecordTooLarge);
        }
        expected_offset = entry_offset
            .checked_add(record_len)
            .ok_or(JournalIndexError::RecordBytesOutOfBounds)?;
        if expected_offset > records_len {
            return Err(JournalIndexError::RecordBytesOutOfBounds);
        }
        previous_sequence = Some(sequence);
    }

    if expected_offset != records_len {
        return Err(JournalIndexError::RecordBytesLengthMismatch);
    }
    Ok(())
}

fn entry_wire_offset(index: usize) -> Result<usize, JournalIndexError> {
    index
        .checked_mul(JOURNAL_INDEX_ENTRY_BYTES)
        .and_then(|offset| offset.checked_add(JOURNAL_INDEX_HEADER_BYTES))
        .ok_or(JournalIndexError::InvalidTotalLength)
}

fn index_digest(bytes: &[u8]) -> Digest32 {
    let mut hash = Sha256::new();
    hash.update(INDEX_DOMAIN);
    hash.update(&bytes[..INDEX_DIGEST_OFFSET]);
    hash.update([0_u8; 32]);
    hash.update(&bytes[JOURNAL_INDEX_HEADER_BYTES..]);
    Digest32::from_bytes(hash.finalize().into())
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, JournalIndexError> {
    let end = offset
        .checked_add(2)
        .ok_or(JournalIndexError::InvalidTotalLength)?;
    let value = bytes
        .get(offset..end)
        .ok_or(JournalIndexError::TruncatedHeader)?;
    Ok(u16::from_le_bytes([value[0], value[1]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, JournalIndexError> {
    let end = offset
        .checked_add(4)
        .ok_or(JournalIndexError::InvalidTotalLength)?;
    let value = bytes
        .get(offset..end)
        .ok_or(JournalIndexError::TruncatedHeader)?;
    Ok(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, JournalIndexError> {
    let end = offset
        .checked_add(8)
        .ok_or(JournalIndexError::InvalidTotalLength)?;
    let value = bytes
        .get(offset..end)
        .ok_or(JournalIndexError::TruncatedHeader)?;
    Ok(u64::from_le_bytes([
        value[0], value[1], value[2], value[3], value[4], value[5], value[6], value[7],
    ]))
}

fn read_digest(bytes: &[u8], offset: usize) -> Result<Digest32, JournalIndexError> {
    let end = offset
        .checked_add(32)
        .ok_or(JournalIndexError::InvalidTotalLength)?;
    let value = bytes
        .get(offset..end)
        .ok_or(JournalIndexError::InvalidTotalLength)?;
    let mut digest = [0_u8; 32];
    digest.copy_from_slice(value);
    Ok(Digest32::from_bytes(digest))
}
