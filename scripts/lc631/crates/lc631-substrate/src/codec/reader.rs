use super::header::{Header, HEADER_BYTES};
use super::{decode_envelope, CodecError, MAX_FRAME_BYTES};
use std::io::{Error, ErrorKind, Read};

fn read_error(error: Error) -> CodecError {
    if error.kind() == ErrorKind::UnexpectedEof {
        CodecError::Truncated
    } else {
        CodecError::Io(error.kind())
    }
}

/// Read one bounded frame. Do not probe EOF or consume a following frame.
/// The caller owns deadlines/cancellation and aggregate memory reservations.
pub fn read_frame(reader: &mut impl Read, max_frame_bytes: usize) -> Result<Vec<u8>, CodecError> {
    if !(HEADER_BYTES..=MAX_FRAME_BYTES).contains(&max_frame_bytes) {
        return Err(CodecError::InvalidLimits);
    }
    let mut bytes = [0; HEADER_BYTES];
    reader.read_exact(&mut bytes).map_err(read_error)?;
    let header = Header::parse(&bytes)?;
    let total = header.frame_len()?;
    if total > max_frame_bytes {
        return Err(CodecError::FrameTooLarge);
    }
    let mut frame = Vec::new();
    frame
        .try_reserve_exact(total)
        .map_err(|_| CodecError::AllocationFailed)?;
    frame.extend_from_slice(&bytes);
    frame.resize(total, 0);
    reader
        .read_exact(&mut frame[HEADER_BYTES..])
        .map_err(read_error)?;
    decode_envelope(&frame)?;
    Ok(frame)
}
