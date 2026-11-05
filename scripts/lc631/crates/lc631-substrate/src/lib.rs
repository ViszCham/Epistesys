//! Small representation contracts shared by reasoning and storage boundaries.
#![forbid(unsafe_code)]

pub mod arena;
pub mod codec;
pub mod context_identity;
pub mod ids;
pub mod measurement_numeric;
pub mod reported;
pub mod resource;
pub mod semantic_term;
pub mod source;

/// Digest bytes identify content; possession is not evidence authenticity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Digest32([u8; 32]);

impl Digest32 {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}
