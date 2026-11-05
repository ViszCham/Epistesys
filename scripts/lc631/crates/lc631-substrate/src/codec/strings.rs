use super::{CodecError, MAX_FRAME_BYTES, MAX_MAP_ENTRIES, MAX_STRING_BYTES};

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    fn string(&mut self) -> Result<&'a str, CodecError> {
        let length_end = self
            .offset
            .checked_add(4)
            .ok_or(CodecError::NumericOverflow)?;
        let prefix = self
            .bytes
            .get(self.offset..length_end)
            .ok_or(CodecError::Truncated)?;
        let length = u32::from_le_bytes([prefix[0], prefix[1], prefix[2], prefix[3]]);
        let length = usize::try_from(length).map_err(|_| CodecError::NumericOverflow)?;
        if length > MAX_STRING_BYTES {
            return Err(CodecError::StringTooLarge);
        }
        let end = length_end
            .checked_add(length)
            .ok_or(CodecError::NumericOverflow)?;
        let bytes = self
            .bytes
            .get(length_end..end)
            .ok_or(CodecError::Truncated)?;
        let text = std::str::from_utf8(bytes).map_err(|_| CodecError::InvalidUtf8)?;
        self.offset = end;
        Ok(text)
    }
}

pub(super) fn validate_map(bytes: &[u8], count: u32) -> Result<(), CodecError> {
    let count = usize::try_from(count).map_err(|_| CodecError::NumericOverflow)?;
    if count > MAX_MAP_ENTRIES {
        return Err(CodecError::TooManyMapEntries);
    }
    if count.checked_mul(8).ok_or(CodecError::NumericOverflow)? > bytes.len() {
        return Err(CodecError::CountMismatch);
    }
    let mut cursor = Cursor { bytes, offset: 0 };
    let mut previous: Option<&str> = None;
    for _ in 0..count {
        let key = cursor.string()?;
        if previous.is_some_and(|old| old.as_bytes() >= key.as_bytes()) {
            return Err(CodecError::NonCanonicalMap);
        }
        cursor.string()?;
        previous = Some(key);
    }
    if cursor.offset != bytes.len() {
        return Err(CodecError::CountMismatch);
    }
    Ok(())
}

pub(super) fn decode_map(bytes: &[u8], count: u32) -> Result<Vec<(&str, &str)>, CodecError> {
    validate_map(bytes, count)?;
    let count = usize::try_from(count).map_err(|_| CodecError::NumericOverflow)?;
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(count)
        .map_err(|_| CodecError::AllocationFailed)?;
    let mut cursor = Cursor { bytes, offset: 0 };
    for _ in 0..count {
        entries.push((cursor.string()?, cursor.string()?));
    }
    Ok(entries)
}

pub fn encode_string_map(entries: &[(&str, &str)]) -> Result<Vec<u8>, CodecError> {
    if entries.len() > MAX_MAP_ENTRIES {
        return Err(CodecError::TooManyMapEntries);
    }
    let mut length = 0_usize;
    for (key, value) in entries {
        if key.len() > MAX_STRING_BYTES || value.len() > MAX_STRING_BYTES {
            return Err(CodecError::StringTooLarge);
        }
        length = length
            .checked_add(8)
            .and_then(|sum| sum.checked_add(key.len()))
            .and_then(|sum| sum.checked_add(value.len()))
            .ok_or(CodecError::NumericOverflow)?;
        if length > MAX_FRAME_BYTES {
            return Err(CodecError::FrameTooLarge);
        }
    }
    let mut sorted = Vec::new();
    sorted
        .try_reserve_exact(entries.len())
        .map_err(|_| CodecError::AllocationFailed)?;
    sorted.extend_from_slice(entries);
    sorted.sort_unstable_by(|left, right| left.0.as_bytes().cmp(right.0.as_bytes()));
    if sorted.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(CodecError::NonCanonicalMap);
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| CodecError::AllocationFailed)?;
    for (key, value) in sorted {
        for text in [key, value] {
            let length = u32::try_from(text.len()).map_err(|_| CodecError::NumericOverflow)?;
            output.extend_from_slice(&length.to_le_bytes());
            output.extend_from_slice(text.as_bytes());
        }
    }
    Ok(output)
}
