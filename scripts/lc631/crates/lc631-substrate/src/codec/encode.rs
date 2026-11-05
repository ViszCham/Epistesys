use super::header::{frame_digest, Header};
use super::sections::{classify, Entry};
use super::{CodecError, SectionInput, MAX_FRAME_BYTES, MAX_SECTIONS};
use crate::Digest32;

pub fn encode_frame(sections: &[SectionInput<'_>]) -> Result<Vec<u8>, CodecError> {
    if sections.len() > MAX_SECTIONS {
        return Err(CodecError::TooManySections);
    }
    let mut payload_len = 0_usize;
    let mut previous = None;
    for section in sections {
        if previous.is_some_and(|tag| tag >= section.tag) {
            return Err(CodecError::NonCanonicalTagOrder);
        }
        payload_len = payload_len
            .checked_add(section.payload.len())
            .ok_or(CodecError::NumericOverflow)?;
        if payload_len > MAX_FRAME_BYTES {
            return Err(CodecError::FrameTooLarge);
        }
        classify(
            section.tag,
            section.version,
            section.flags,
            section.count,
            section.payload,
        )?;
        previous = Some(section.tag);
    }
    let header = Header {
        minor: 0,
        payload_len: u64::try_from(payload_len).map_err(|_| CodecError::NumericOverflow)?,
        section_count: u32::try_from(sections.len()).map_err(|_| CodecError::NumericOverflow)?,
        digest: Digest32::from_bytes([0; 32]),
    };
    let total = header.frame_len()?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(total)
        .map_err(|_| CodecError::AllocationFailed)?;
    output.extend_from_slice(&header.encode());
    let mut offset =
        u64::try_from(header.payload_start()?).map_err(|_| CodecError::NumericOverflow)?;
    for section in sections {
        let length =
            u64::try_from(section.payload.len()).map_err(|_| CodecError::NumericOverflow)?;
        Entry {
            tag: section.tag,
            version: section.version,
            flags: section.flags,
            offset,
            length,
            count: section.count,
        }
        .append(&mut output);
        offset = offset
            .checked_add(length)
            .ok_or(CodecError::NumericOverflow)?;
    }
    for section in sections {
        output.extend_from_slice(section.payload);
    }
    let digest = frame_digest(&output)?;
    output[32..64].copy_from_slice(digest.as_bytes());
    Ok(output)
}
