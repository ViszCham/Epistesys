//! One immutable UTF-8 allocation, with checked byte spans into that allocation.
use crate::ids::SourceId;
use crate::Digest32;
use sha2::{Digest, Sha256};
use std::fmt;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceError {
    InvalidLimit,
    LimitExceeded,
    OffsetTooLarge,
    ReversedSpan,
    OutOfBounds,
    InvalidUtf8Boundary,
    SourceMismatch,
}

impl fmt::Display for SourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidLimit => "source byte limit is invalid",
            Self::LimitExceeded => "source exceeds its byte limit",
            Self::OffsetTooLarge => "source offset exceeds the span representation",
            Self::ReversedSpan => "source span is reversed",
            Self::OutOfBounds => "source span exceeds source bounds",
            Self::InvalidUtf8Boundary => "source span is not on UTF-8 boundaries",
            Self::SourceMismatch => "source span belongs to different source bytes",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for SourceError {}

/// Cloning shares the original bytes; it never clones the source String.
/// The caller owns bounded acquisition before creating the Arc.
#[derive(Clone)]
pub struct SourceStore {
    text: Arc<str>,
    id: SourceId,
}

impl SourceStore {
    pub fn new(text: Arc<str>, max_bytes: usize) -> Result<Self, SourceError> {
        if max_bytes == 0 || max_bytes > u32::MAX as usize {
            return Err(SourceError::InvalidLimit);
        }
        if text.len() > max_bytes {
            return Err(SourceError::LimitExceeded);
        }
        let digest = Digest32::from_bytes(Sha256::digest(text.as_bytes()).into());
        Ok(Self {
            text,
            id: SourceId::from_digest(digest),
        })
    }

    pub const fn id(&self) -> SourceId {
        self.id
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn slice(&self, span: SourceSpan) -> Result<&str, SourceError> {
        if span.source != self.id {
            return Err(SourceError::SourceMismatch);
        }
        SourceSpan::checked(self, span.start(), span.end())?;
        self.text
            .get(span.start()..span.end())
            .ok_or(SourceError::InvalidUtf8Boundary)
    }
}

impl fmt::Debug for SourceStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SourceStore")
            .field("id", &self.id)
            .field("byte_len", &self.text.len())
            .finish()
    }
}

/// Half-open byte offsets. Empty ranges are valid only at UTF-8 boundaries.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SourceSpan {
    source: SourceId,
    start: u32,
    end: u32,
}

impl SourceSpan {
    pub fn checked(source: &SourceStore, start: usize, end: usize) -> Result<Self, SourceError> {
        if start > end {
            return Err(SourceError::ReversedSpan);
        }
        if end > source.text.len() {
            return Err(SourceError::OutOfBounds);
        }
        if !source.text.is_char_boundary(start) || !source.text.is_char_boundary(end) {
            return Err(SourceError::InvalidUtf8Boundary);
        }
        let start = u32::try_from(start).map_err(|_| SourceError::OffsetTooLarge)?;
        let end = u32::try_from(end).map_err(|_| SourceError::OffsetTooLarge)?;
        Ok(Self {
            source: source.id,
            start,
            end,
        })
    }

    pub const fn source_id(self) -> SourceId {
        self.source
    }

    pub const fn start(self) -> usize {
        self.start as usize
    }

    pub const fn end(self) -> usize {
        self.end as usize
    }
}
