use super::header::{frame_digest, Header, ENTRY_BYTES, HEADER_BYTES};
use super::sections::{classify, Entry};
use super::{
    CodecError, SectionKind, SectionView, CRITICAL, MAX_FRAME_BYTES, MAX_SECTIONS, SOURCE_UTF8,
};
use crate::ids::SourceId;
use crate::Digest32;
use sha2::{Digest, Sha256};

pub struct FrameView<'a> {
    bytes: &'a [u8],
    header: Header,
    sections: Vec<SectionView<'a>>,
}

/// A validated envelope whose section metadata is read from the original
/// immutable bytes on demand. Validation and checked access allocate no
/// section collection; materializing a string map remains a separate operation.
/// At most 64 kinds are retained inline; this is not a wire or ABI layout.
pub struct BorrowedFrameView<'a> {
    bytes: &'a [u8],
    header: Header,
    kinds: [SectionKind; MAX_SECTIONS],
}

impl<'a> BorrowedFrameView<'a> {
    pub const fn minor(&self) -> u16 {
        self.header.minor
    }

    pub const fn digest(&self) -> Digest32 {
        self.header.digest
    }

    pub const fn original_bytes(&self) -> &'a [u8] {
        self.bytes
    }

    pub fn section_count(&self) -> usize {
        // Header admission already bounds the count to MAX_SECTIONS.
        self.header.section_count as usize
    }

    /// Returns a checked view into the validated frame, or None out of range.
    pub fn section(&self, index: usize) -> Result<Option<SectionView<'a>>, CodecError> {
        if index >= self.section_count() {
            return Ok(None);
        }
        let (entry, payload) = section_parts(self.bytes, index)?;
        // Only the private constructor can create this view. It classified all
        // sections against these immutable bytes before storing their kinds.
        let kind = *self.kinds.get(index).ok_or(CodecError::TooManySections)?;
        Ok(Some(SectionView {
            entry,
            payload,
            kind,
        }))
    }
}

impl<'a> FrameView<'a> {
    pub const fn minor(&self) -> u16 {
        self.header.minor
    }

    pub const fn digest(&self) -> Digest32 {
        self.header.digest
    }

    pub const fn original_bytes(&self) -> &'a [u8] {
        self.bytes
    }

    pub fn sections(&self) -> &[SectionView<'a>] {
        &self.sections
    }

    /// Typed format admission only: source content is not authority or truth.
    pub fn source_profile(&self) -> Result<SourceProfile<'a>, CodecError> {
        if self.header.minor != 0 {
            return Err(CodecError::UnsupportedMinor);
        }
        let source = self
            .sections
            .iter()
            .find(|section| section.tag() == SOURCE_UTF8)
            .ok_or(CodecError::MissingSource)?;
        if source.version() != 1 || source.kind() != SectionKind::SourceUtf8 {
            return Err(CodecError::UnsupportedSectionVersion);
        }
        if source.flags() & CRITICAL == 0 {
            return Err(CodecError::SourceNotCritical);
        }
        let text = std::str::from_utf8(source.payload()).map_err(|_| CodecError::InvalidUtf8)?;
        let digest = Digest32::from_bytes(Sha256::digest(text.as_bytes()).into());
        Ok(SourceProfile {
            text,
            id: SourceId::from_digest(digest),
        })
    }
}

pub struct SourceProfile<'a> {
    text: &'a str,
    id: SourceId,
}

impl<'a> SourceProfile<'a> {
    pub const fn source(&self) -> &'a str {
        self.text
    }

    pub const fn source_id(&self) -> SourceId {
        self.id
    }
}

fn entry_at(bytes: &[u8], index: usize) -> Result<Entry, CodecError> {
    let start = index
        .checked_mul(ENTRY_BYTES)
        .and_then(|offset| HEADER_BYTES.checked_add(offset))
        .ok_or(CodecError::NumericOverflow)?;
    let end = start
        .checked_add(ENTRY_BYTES)
        .ok_or(CodecError::NumericOverflow)?;
    Entry::parse(bytes.get(start..end).ok_or(CodecError::Truncated)?)
}

fn validate_directory(bytes: &[u8], header: Header) -> Result<(), CodecError> {
    let mut end =
        u64::try_from(header.payload_start()?).map_err(|_| CodecError::NumericOverflow)?;
    let mut previous = None;
    for index in 0..header.section_count as usize {
        let entry = entry_at(bytes, index)?;
        if previous.is_some_and(|tag| tag >= entry.tag) {
            return Err(CodecError::NonCanonicalTagOrder);
        }
        if entry.offset != end {
            return Err(CodecError::NonContiguousSections);
        }
        end = entry
            .offset
            .checked_add(entry.length)
            .ok_or(CodecError::NumericOverflow)?;
        if end > bytes.len() as u64 {
            return Err(CodecError::SectionOutOfBounds);
        }
        previous = Some(entry.tag);
    }
    if end != bytes.len() as u64 {
        return Err(CodecError::LengthMismatch);
    }
    Ok(())
}

fn section_parts(bytes: &[u8], index: usize) -> Result<(Entry, &[u8]), CodecError> {
    let entry = entry_at(bytes, index)?;
    let start = usize::try_from(entry.offset).map_err(|_| CodecError::NumericOverflow)?;
    let length = usize::try_from(entry.length).map_err(|_| CodecError::NumericOverflow)?;
    let end = start
        .checked_add(length)
        .ok_or(CodecError::NumericOverflow)?;
    let payload = bytes
        .get(start..end)
        .ok_or(CodecError::SectionOutOfBounds)?;
    Ok((entry, payload))
}

/// Validate the complete envelope and every section without allocating a
/// section-view vector. Integrity is not authentication or authority.
pub fn decode_envelope_borrowed(bytes: &[u8]) -> Result<BorrowedFrameView<'_>, CodecError> {
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(CodecError::FrameTooLarge);
    }
    let header = Header::parse(bytes)?;
    if header.frame_len()? != bytes.len() {
        return Err(CodecError::LengthMismatch);
    }
    validate_directory(bytes, header)?;
    if frame_digest(bytes)? != header.digest {
        return Err(CodecError::DigestMismatch);
    }
    let count = usize::try_from(header.section_count).map_err(|_| CodecError::NumericOverflow)?;
    let mut kinds = [SectionKind::Opaque; MAX_SECTIONS];
    for (index, kind) in kinds.iter_mut().enumerate().take(count) {
        let (entry, payload) = section_parts(bytes, index)?;
        *kind = classify(entry.tag, entry.version, entry.flags, entry.count, payload)?;
    }
    Ok(BorrowedFrameView {
        bytes,
        header,
        kinds,
    })
}

pub fn decode_envelope(bytes: &[u8]) -> Result<FrameView<'_>, CodecError> {
    let borrowed = decode_envelope_borrowed(bytes)?;
    let count = borrowed.section_count();
    let mut sections = Vec::new();
    sections
        .try_reserve_exact(count)
        .map_err(|_| CodecError::AllocationFailed)?;
    for index in 0..count {
        sections.push(borrowed.section(index)?.ok_or(CodecError::Truncated)?);
    }
    Ok(FrameView {
        bytes,
        header: borrowed.header,
        sections,
    })
}
