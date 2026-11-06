//! 日本語: source/capture/processの元の失敗種別を保持する。
//! English: Preserve source, capture and process failure categories.
use crate::declared_build_capture::SnapshotCaptureError;
use lc631_substrate::resource::ResourceError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BuildError {
    Unsupported(&'static str),
    FrameMismatch,
    OutputAlreadyExists,
    AuthorityUnavailable,
    Unauthorized,
    Expired,
    ClockRollback,
    RevocationRollback,
    CompilerRegistrationMismatch,
    ConsumerRejected,
    SourceDrift,
    ArtifactDrift,
    Resource(ResourceError),
    Capture(SnapshotCaptureError),
    Io(std::io::ErrorKind),
    InputLimit,
    ReadBudget,
    BuildAlreadyAttempted,
    BuildFailed,
    MissingArtifact,
    NotExecutable,
}
impl From<ResourceError> for BuildError {
    fn from(value: ResourceError) -> Self { Self::Resource(value) }
}
impl From<SnapshotCaptureError> for BuildError {
    fn from(value: SnapshotCaptureError) -> Self { Self::Capture(value) }
}
impl From<std::io::Error> for BuildError {
    fn from(value: std::io::Error) -> Self { Self::Io(value.kind()) }
}
