use super::{CodecError, FRAME_DOMAIN, MAX_FRAME_BYTES, MAX_SECTIONS};
use crate::Digest32;
use sha2::{Digest, Sha256};

pub(super) const HEADER_BYTES: usize = 64;
pub(super) const ENTRY_BYTES: usize = 32;

#[derive(Clone, Copy)]
pub(super) struct Header {
    pub minor: u16,
    pub payload_len: u64,
    pub section_count: u32,
    pub digest: Digest32,
}

impl Header {
    pub fn parse(bytes: &[u8]) -> Result<Self, CodecError> {
        if bytes.len() < HEADER_BYTES {
            return Err(CodecError::Truncated);
        }
        if bytes.get(..8) != Some(b"EPITLCX1".as_slice()) {
            return Err(CodecError::InvalidMagic);
        }
        if u16::from_le_bytes(array(bytes, 8)?) != 1 {
            return Err(CodecError::UnsupportedMajor);
        }
        if u32::from_le_bytes(array(bytes, 12)?) != 0 {
            return Err(CodecError::UnknownFlags);
        }
        if u32::from_le_bytes(array(bytes, 28)?) != HEADER_BYTES as u32 {
            return Err(CodecError::InvalidHeaderLength);
        }
        let header = Self {
            minor: u16::from_le_bytes(array(bytes, 10)?),
            payload_len: u64::from_le_bytes(array(bytes, 16)?),
            section_count: u32::from_le_bytes(array(bytes, 24)?),
            digest: Digest32::from_bytes(array(bytes, 32)?),
        };
        header.frame_len()?;
        Ok(header)
    }

    pub fn payload_start(self) -> Result<usize, CodecError> {
        let count = usize::try_from(self.section_count).map_err(|_| CodecError::NumericOverflow)?;
        if count > MAX_SECTIONS {
            return Err(CodecError::TooManySections);
        }
        count
            .checked_mul(ENTRY_BYTES)
            .and_then(|directory| HEADER_BYTES.checked_add(directory))
            .ok_or(CodecError::NumericOverflow)
    }

    pub fn frame_len(self) -> Result<usize, CodecError> {
        let start =
            u64::try_from(self.payload_start()?).map_err(|_| CodecError::NumericOverflow)?;
        let total = start
            .checked_add(self.payload_len)
            .ok_or(CodecError::NumericOverflow)?;
        if total > MAX_FRAME_BYTES as u64 {
            return Err(CodecError::FrameTooLarge);
        }
        usize::try_from(total).map_err(|_| CodecError::NumericOverflow)
    }

    pub fn encode(self) -> [u8; HEADER_BYTES] {
        let mut bytes = [0; HEADER_BYTES];
        bytes[..8].copy_from_slice(b"EPITLCX1");
        bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
        bytes[10..12].copy_from_slice(&self.minor.to_le_bytes());
        bytes[16..24].copy_from_slice(&self.payload_len.to_le_bytes());
        bytes[24..28].copy_from_slice(&self.section_count.to_le_bytes());
        bytes[28..32].copy_from_slice(&(HEADER_BYTES as u32).to_le_bytes());
        bytes[32..64].copy_from_slice(self.digest.as_bytes());
        bytes
    }
}

pub(super) fn array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N], CodecError> {
    let end = offset.checked_add(N).ok_or(CodecError::NumericOverflow)?;
    let input = bytes.get(offset..end).ok_or(CodecError::Truncated)?;
    let mut output = [0; N];
    output.copy_from_slice(input);
    Ok(output)
}

pub(super) fn frame_digest(bytes: &[u8]) -> Result<Digest32, CodecError> {
    if bytes.len() < HEADER_BYTES {
        return Err(CodecError::Truncated);
    }
    let mut digest = Sha256::new();
    digest.update(FRAME_DOMAIN);
    digest.update(&bytes[..32]);
    digest.update([0; 32]);
    digest.update(&bytes[64..]);
    Ok(Digest32::from_bytes(digest.finalize().into()))
}
