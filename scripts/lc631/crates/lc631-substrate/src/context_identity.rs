//! Shared representation identities with no authority or execution dependency.
use crate::Digest32;
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SourceRevision(Digest32);

impl SourceRevision {
    /// Preserve the existing unprefixed SHA-256 source identity exactly.
    pub fn of(source: &str) -> Self {
        Self(Digest32::from_bytes(
            Sha256::digest(source.as_bytes()).into(),
        ))
    }

    pub const fn digest(self) -> Digest32 {
        self.0
    }
}

/// An interpretation identity, not a truth assertion or permission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContextRevision {
    source: SourceRevision,
    interpretation: Digest32,
}

impl ContextRevision {
    pub const fn original(source: SourceRevision) -> Self {
        Self {
            source,
            interpretation: source.digest(),
        }
    }

    pub const fn reconstructed(self, interpretation: [u8; 32]) -> Self {
        Self {
            interpretation: Digest32::from_bytes(interpretation),
            ..self
        }
    }

    pub const fn source(self) -> SourceRevision {
        self.source
    }

    pub const fn interpretation_digest(self) -> Digest32 {
        self.interpretation
    }
}
