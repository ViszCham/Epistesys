use super::header::array;
use super::{strings, CodecError, CRITICAL, MAX_STRING_BYTES, SOURCE_UTF8, STRING_MAP};

/// Untrusted encoder input. Construction alone validates nothing.
#[derive(Clone, Copy)]
pub struct SectionInput<'a> {
    pub tag: u16,
    pub version: u16,
    pub flags: u32,
    pub count: u32,
    pub payload: &'a [u8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SectionKind {
    SourceUtf8,
    StringMap,
    Opaque,
}

#[derive(Clone, Copy)]
pub(super) struct Entry {
    pub tag: u16,
    pub version: u16,
    pub flags: u32,
    pub offset: u64,
    pub length: u64,
    pub count: u32,
}

impl Entry {
    pub fn parse(bytes: &[u8]) -> Result<Self, CodecError> {
        let entry = Self {
            tag: u16::from_le_bytes(array(bytes, 0)?),
            version: u16::from_le_bytes(array(bytes, 2)?),
            flags: u32::from_le_bytes(array(bytes, 4)?),
            offset: u64::from_le_bytes(array(bytes, 8)?),
            length: u64::from_le_bytes(array(bytes, 16)?),
            count: u32::from_le_bytes(array(bytes, 24)?),
        };
        if entry.flags & !CRITICAL != 0 {
            return Err(CodecError::UnknownFlags);
        }
        if u32::from_le_bytes(array(bytes, 28)?) != 0 {
            return Err(CodecError::NonzeroReserved);
        }
        Ok(entry)
    }

    pub fn append(self, output: &mut Vec<u8>) {
        output.extend_from_slice(&self.tag.to_le_bytes());
        output.extend_from_slice(&self.version.to_le_bytes());
        output.extend_from_slice(&self.flags.to_le_bytes());
        output.extend_from_slice(&self.offset.to_le_bytes());
        output.extend_from_slice(&self.length.to_le_bytes());
        output.extend_from_slice(&self.count.to_le_bytes());
        output.extend_from_slice(&0_u32.to_le_bytes());
    }
}

/// A structurally and integrity-checked view, not an authenticated observation.
pub struct SectionView<'a> {
    pub(super) entry: Entry,
    pub(super) payload: &'a [u8],
    pub(super) kind: SectionKind,
}

impl<'a> SectionView<'a> {
    pub const fn tag(&self) -> u16 {
        self.entry.tag
    }

    pub const fn version(&self) -> u16 {
        self.entry.version
    }

    pub const fn flags(&self) -> u32 {
        self.entry.flags
    }

    pub const fn count(&self) -> u32 {
        self.entry.count
    }

    pub const fn kind(&self) -> SectionKind {
        self.kind
    }

    pub const fn payload(&self) -> &'a [u8] {
        self.payload
    }

    pub fn string_map(&self) -> Result<Vec<(&'a str, &'a str)>, CodecError> {
        if self.kind != SectionKind::StringMap {
            return Err(CodecError::UnexpectedSectionKind);
        }
        strings::decode_map(self.payload, self.entry.count)
    }
}

pub(super) fn classify(
    tag: u16,
    version: u16,
    flags: u32,
    count: u32,
    payload: &[u8],
) -> Result<SectionKind, CodecError> {
    if flags & !CRITICAL != 0 {
        return Err(CodecError::UnknownFlags);
    }
    let known_tag = matches!(tag, SOURCE_UTF8 | STRING_MAP);
    if version != 1 || !known_tag {
        if flags & CRITICAL != 0 {
            return Err(if known_tag {
                CodecError::UnsupportedSectionVersion
            } else {
                CodecError::UnknownCritical
            });
        }
        return Ok(SectionKind::Opaque);
    }
    match tag {
        SOURCE_UTF8 => {
            if count != 1 {
                return Err(CodecError::CountMismatch);
            }
            if payload.len() > MAX_STRING_BYTES {
                return Err(CodecError::StringTooLarge);
            }
            std::str::from_utf8(payload).map_err(|_| CodecError::InvalidUtf8)?;
            Ok(SectionKind::SourceUtf8)
        }
        STRING_MAP => {
            strings::validate_map(payload, count)?;
            Ok(SectionKind::StringMap)
        }
        _ => Err(CodecError::UnexpectedSectionKind),
    }
}
