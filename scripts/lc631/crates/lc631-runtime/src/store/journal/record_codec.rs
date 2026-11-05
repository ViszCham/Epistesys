use super::types::{JournalError, JournalRecordKind};
use super::{
    MAX_JOURNAL_RECORD_BYTES, RECORD_DIGEST_RANGE, RECORD_DOMAIN, RECORD_HEADER_BYTES,
    RECORD_MAGIC, RECORD_VERSION,
};
use lc631_substrate::Digest32;
use sha2::{Digest, Sha256};
use std::convert::TryFrom;

#[derive(Clone, Copy, Debug)]
pub(super) struct DecodedRecordHeader {
    pub(super) sequence: u64,
    pub(super) kind: JournalRecordKind,
    pub(super) object_digest: Digest32,
    pub(super) previous_digest: Digest32,
    pub(super) record_digest: Digest32,
    pub(super) payload_len: usize,
}

pub(super) fn decode_record_header(header: &[u8]) -> Result<DecodedRecordHeader, JournalError> {
    if header.len() != RECORD_HEADER_BYTES {
        return Err(JournalError::TornRecord);
    }
    if header.get(..8) != Some(RECORD_MAGIC.as_slice()) {
        return Err(JournalError::InvalidRecordMagic);
    }
    if read_u16(header, 8)? != RECORD_VERSION {
        return Err(JournalError::UnsupportedRecordVersion);
    }
    let kind = JournalRecordKind::from_wire_tag(read_u16(header, 10)?)?;
    if read_u32(header, 12)? != 0 {
        return Err(JournalError::UnknownRecordFlags);
    }
    if read_u32(header, 28)? != 0 {
        return Err(JournalError::NonzeroReserved);
    }
    let sequence = read_u64(header, 16)?;
    if sequence == 0 {
        return Err(JournalError::ZeroSequence);
    }
    let payload_len =
        usize::try_from(read_u32(header, 24)?).map_err(|_| JournalError::RecordTooLarge)?;
    if payload_len > MAX_JOURNAL_RECORD_BYTES - RECORD_HEADER_BYTES {
        return Err(JournalError::RecordTooLarge);
    }
    let object_digest = read_digest(header, 32)?;
    if object_digest.as_bytes().iter().all(|byte| *byte == 0) {
        return Err(JournalError::InvalidObjectDigest);
    }
    Ok(DecodedRecordHeader {
        sequence,
        kind,
        object_digest,
        previous_digest: read_digest(header, 64)?,
        record_digest: read_digest(header, 96)?,
        payload_len,
    })
}

pub(super) fn encode_record(
    sequence: u64,
    kind: JournalRecordKind,
    object_digest: Digest32,
    previous_digest: Digest32,
    payload: &[u8],
) -> Result<Vec<u8>, JournalError> {
    if sequence == 0 {
        return Err(JournalError::ZeroSequence);
    }
    if object_digest.as_bytes().iter().all(|byte| *byte == 0) {
        return Err(JournalError::InvalidObjectDigest);
    }
    let payload_len = u32::try_from(payload.len()).map_err(|_| JournalError::RecordTooLarge)?;
    let total_len = RECORD_HEADER_BYTES
        .checked_add(payload.len())
        .ok_or(JournalError::RecordTooLarge)?;
    if total_len > MAX_JOURNAL_RECORD_BYTES {
        return Err(JournalError::RecordTooLarge);
    }
    let mut record = Vec::new();
    record
        .try_reserve_exact(total_len)
        .map_err(|_| JournalError::AllocationFailed)?;
    record.extend_from_slice(RECORD_MAGIC);
    record.extend_from_slice(&RECORD_VERSION.to_le_bytes());
    record.extend_from_slice(&kind.wire_tag().to_le_bytes());
    record.extend_from_slice(&0_u32.to_le_bytes());
    record.extend_from_slice(&sequence.to_le_bytes());
    record.extend_from_slice(&payload_len.to_le_bytes());
    record.extend_from_slice(&0_u32.to_le_bytes());
    record.extend_from_slice(object_digest.as_bytes());
    record.extend_from_slice(previous_digest.as_bytes());
    record.extend_from_slice(&[0; 32]);
    let digest = record_digest(&record, payload)?;
    record.extend_from_slice(payload);
    record[RECORD_DIGEST_RANGE].copy_from_slice(digest.as_bytes());
    Ok(record)
}

pub(super) fn record_digest(
    header_with_zero_digest: &[u8],
    payload: &[u8],
) -> Result<Digest32, JournalError> {
    if header_with_zero_digest.len() != RECORD_HEADER_BYTES {
        return Err(JournalError::TornRecord);
    }
    let mut hash = Sha256::new();
    hash.update(RECORD_DOMAIN);
    hash.update(&header_with_zero_digest[..RECORD_DIGEST_RANGE.start]);
    hash.update([0; 32]);
    hash.update(&header_with_zero_digest[RECORD_DIGEST_RANGE.end..]);
    hash.update(payload);
    Ok(Digest32::from_bytes(hash.finalize().into()))
}

pub(super) fn digest_from_record(record: &[u8]) -> Result<Digest32, JournalError> {
    if record.len() < RECORD_HEADER_BYTES {
        return Err(JournalError::TornRecord);
    }
    read_digest(record, RECORD_DIGEST_RANGE.start)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, JournalError> {
    let end = offset.checked_add(2).ok_or(JournalError::TornRecord)?;
    let slice = bytes.get(offset..end).ok_or(JournalError::TornRecord)?;
    Ok(u16::from_le_bytes([slice[0], slice[1]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, JournalError> {
    let end = offset.checked_add(4).ok_or(JournalError::TornRecord)?;
    let slice = bytes.get(offset..end).ok_or(JournalError::TornRecord)?;
    Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, JournalError> {
    let end = offset.checked_add(8).ok_or(JournalError::TornRecord)?;
    let slice = bytes.get(offset..end).ok_or(JournalError::TornRecord)?;
    Ok(u64::from_le_bytes([
        slice[0], slice[1], slice[2], slice[3], slice[4], slice[5], slice[6], slice[7],
    ]))
}

fn read_digest(bytes: &[u8], offset: usize) -> Result<Digest32, JournalError> {
    let end = offset.checked_add(32).ok_or(JournalError::TornRecord)?;
    let slice = bytes.get(offset..end).ok_or(JournalError::TornRecord)?;
    let mut digest = [0_u8; 32];
    digest.copy_from_slice(slice);
    Ok(Digest32::from_bytes(digest))
}
