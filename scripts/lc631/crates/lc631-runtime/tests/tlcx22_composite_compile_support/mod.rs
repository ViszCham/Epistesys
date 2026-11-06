//! 日本語: 現testのcompiled artifact選択だけ。source意味/権限/完全build証明ではない。
//! English: Select this test's compiled artifacts only, not semantics, authority or full-build proof.
// Shared fixture surface: individual integration targets use different subsets.
#![allow(dead_code)]
#![forbid(unsafe_code)]
pub mod diagnostics;
pub mod fingerprint;
mod linked;
mod snapshot;
mod sources;
use fingerprint::Kind;
pub use linked::LinkedArtifacts;

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    UnsupportedFormat,
    DuplicateKey,
    DuplicateDependency,
    Limit,
    AllocationFailed,
    MissingDependency(&'static str),
    MissingArtifact(Kind),
    AmbiguousArtifact(Kind),
    ContextMismatch,
    DependencyMismatch,
    ArtifactChanged,
    SourceChanged,
    InvalidCurrentExe,
    InvalidFile,
    Io(std::io::ErrorKind),
    AtStage {
        stage: &'static str,
        unit: String,
        cause: Box<Error>,
    },
}
pub fn check_digest(expected: [u8; 32], actual: [u8; 32]) -> Result<(), Error> {
    if expected != actual {
        return Err(Error::ArtifactChanged);
    }
    Ok(())
}
