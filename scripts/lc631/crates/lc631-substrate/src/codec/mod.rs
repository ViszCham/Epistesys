//! Bounded canonical frames. Integrity checks never issue authority or evidence.
mod decode;
mod encode;
mod header;
mod reader;
mod sections;
mod strings;

pub use decode::{
    decode_envelope, decode_envelope_borrowed, BorrowedFrameView, FrameView, SourceProfile,
};
pub use encode::encode_frame;
pub use reader::read_frame;
pub use sections::{SectionInput, SectionKind, SectionView};
pub use strings::encode_string_map;

use std::fmt;

pub const FRAME_DOMAIN: &[u8] = b"Epistesys/TLCX/FrameDigest/sha256/v1\0";
pub const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_SECTIONS: usize = 64;
pub const MAX_MAP_ENTRIES: usize = 100_000;
pub const MAX_STRING_BYTES: usize = 256 * 1024;
pub const SOURCE_UTF8: u16 = 0x0001;
pub const STRING_MAP: u16 = 0x0002;
pub const CRITICAL: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodecError {
    Truncated,
    InvalidMagic,
    UnsupportedMajor,
    UnsupportedMinor,
    UnsupportedSectionVersion,
    UnknownFlags,
    InvalidHeaderLength,
    NonzeroReserved,
    TooManySections,
    FrameTooLarge,
    NumericOverflow,
    LengthMismatch,
    NonCanonicalTagOrder,
    NonContiguousSections,
    SectionOutOfBounds,
    DigestMismatch,
    UnknownCritical,
    InvalidUtf8,
    CountMismatch,
    StringTooLarge,
    TooManyMapEntries,
    NonCanonicalMap,
    UnexpectedSectionKind,
    MissingSource,
    SourceNotCritical,
    AllocationFailed,
    InvalidLimits,
    Io(std::io::ErrorKind),
}

impl fmt::Display for CodecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "TLCX frame rejected: {self:?}")
    }
}

impl std::error::Error for CodecError {}
