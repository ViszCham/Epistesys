//! Canonical local-head encoding; hash continuity is not authentication.
use super::bundle_live::BundleLiveError;
use lc631_substrate::codec::{decode_envelope_borrowed, encode_frame, SectionInput, SectionKind};
use lc631_substrate::Digest32;

pub const JOURNAL_LOCAL_HEAD_SECTION_TAG: u16 = 0x0202;
pub const JOURNAL_LOCAL_HEAD_PAYLOAD_BYTES: usize = 152;
pub const JOURNAL_LOCAL_HEAD_FRAME_BYTES: usize = 248;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct LocalHead {
    pub(super) file_sequence: u64,
    pub(super) record_sequence: u64,
    pub(super) record_digest: Digest32,
    pub(super) segment_frame_digest: Digest32,
    pub(super) index_frame_digest: Digest32,
    pub(super) previous_head_frame_digest: Digest32,
    pub(super) record_count: u32,
}

pub(super) fn zero_digest() -> Digest32 {
    Digest32::from_bytes([0; 32])
}

fn put_digest(out: &mut [u8], offset: usize, digest: Digest32) {
    out[offset..offset + 32].copy_from_slice(digest.as_bytes());
}

fn array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N], BundleLiveError> {
    let end = offset
        .checked_add(N)
        .ok_or(BundleLiveError::RebuildRequired)?;
    let slice = bytes
        .get(offset..end)
        .ok_or(BundleLiveError::RebuildRequired)?;
    slice
        .try_into()
        .map_err(|_| BundleLiveError::RebuildRequired)
}

pub(super) fn encode_head(head: LocalHead) -> Result<Vec<u8>, BundleLiveError> {
    let mut payload = [0_u8; JOURNAL_LOCAL_HEAD_PAYLOAD_BYTES];
    payload[0..8].copy_from_slice(&head.file_sequence.to_le_bytes());
    payload[8..16].copy_from_slice(&head.record_sequence.to_le_bytes());
    put_digest(&mut payload, 16, head.record_digest);
    put_digest(&mut payload, 48, head.segment_frame_digest);
    put_digest(&mut payload, 80, head.index_frame_digest);
    put_digest(&mut payload, 112, head.previous_head_frame_digest);
    payload[144..148].copy_from_slice(&head.record_count.to_le_bytes());
    encode_frame(&[SectionInput {
        tag: JOURNAL_LOCAL_HEAD_SECTION_TAG,
        version: 1,
        flags: 0,
        count: 1,
        payload: &payload,
    }])
    .map_err(BundleLiveError::Codec)
}

pub(super) fn decode_head(bytes: &[u8]) -> Result<(LocalHead, Digest32), BundleLiveError> {
    if bytes.len() != JOURNAL_LOCAL_HEAD_FRAME_BYTES {
        return Err(BundleLiveError::RebuildRequired);
    }
    let frame = decode_envelope_borrowed(bytes).map_err(|_| BundleLiveError::RebuildRequired)?;
    if frame.minor() != 0 || frame.section_count() != 1 {
        return Err(BundleLiveError::RebuildRequired);
    }
    let section = frame
        .section(0)
        .map_err(|_| BundleLiveError::RebuildRequired)?
        .ok_or(BundleLiveError::RebuildRequired)?;
    if section.tag() != JOURNAL_LOCAL_HEAD_SECTION_TAG
        || section.version() != 1
        || section.flags() != 0
        || section.kind() != SectionKind::Opaque
        || section.count() != 1
        || section.payload().len() != JOURNAL_LOCAL_HEAD_PAYLOAD_BYTES
    {
        return Err(BundleLiveError::RebuildRequired);
    }
    let p = section.payload();
    if p[148..152] != [0; 4] {
        return Err(BundleLiveError::RebuildRequired);
    }
    let head = LocalHead {
        file_sequence: u64::from_le_bytes(array(p, 0)?),
        record_sequence: u64::from_le_bytes(array(p, 8)?),
        record_digest: Digest32::from_bytes(array(p, 16)?),
        segment_frame_digest: Digest32::from_bytes(array(p, 48)?),
        index_frame_digest: Digest32::from_bytes(array(p, 80)?),
        previous_head_frame_digest: Digest32::from_bytes(array(p, 112)?),
        record_count: u32::from_le_bytes(array(p, 144)?),
    };
    if head.file_sequence == 0 || head.record_sequence == 0 || head.record_count == 0 {
        return Err(BundleLiveError::RebuildRequired);
    }
    Ok((head, frame.digest()))
}

pub(super) fn frame_digest(bytes: &[u8]) -> Result<Digest32, BundleLiveError> {
    decode_envelope_borrowed(bytes)
        .map(|frame| frame.digest())
        .map_err(BundleLiveError::Codec)
}
