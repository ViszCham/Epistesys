//! Read-only capsule; no Deref, writable handle extraction or mutation methods.
use super::{AnchoredFile, FileIdentity};
use std::io::{self, Read};
use std::path::PathBuf;

/// A read-only public surface, with no writable-handle extraction.
///
/// ```
/// use lc631_process_supervisor::filesystem::ReadOnlyAnchoredFile;
/// fn read(mut file: ReadOnlyAnchoredFile) { let _ = file.read_bounded(16); }
/// ```
/// ```compile_fail
/// use lc631_process_supervisor::filesystem::ReadOnlyAnchoredFile;
/// fn write(mut file: ReadOnlyAnchoredFile) { file.write_and_sync(b"x").unwrap(); }
/// ```
/// ```compile_fail
/// use lc631_process_supervisor::filesystem::ReadOnlyAnchoredFile;
/// fn append(mut file: ReadOnlyAnchoredFile) { file.append_and_sync(b"x").unwrap(); }
/// ```
/// ```compile_fail
/// use lc631_process_supervisor::filesystem::ReadOnlyAnchoredFile;
/// fn replace(mut file: ReadOnlyAnchoredFile) { file.compare_and_write(b"a", b"b").unwrap(); }
/// ```
pub struct ReadOnlyAnchoredFile {
    pub(super) inner: AnchoredFile,
}
impl ReadOnlyAnchoredFile {
    pub(super) fn from_inner(inner: AnchoredFile) -> io::Result<Self> {
        if inner.writable {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "read_only_capsule_requires_read_only_handle",
            ));
        }
        Ok(Self { inner })
    }
    pub fn path(&self) -> PathBuf {
        self.inner.path()
    }
    pub fn identity(&self) -> FileIdentity {
        self.inner.identity()
    }
    pub fn metadata(&self) -> io::Result<std::fs::Metadata> {
        self.inner.metadata()
    }
    pub fn validate_current(&self) -> io::Result<()> {
        self.inner.validate_current()
    }
    pub fn read_bounded(&mut self, limit: usize) -> io::Result<Vec<u8>> {
        self.validate_current()?;
        self.inner.read_bounded(limit)
    }
}
impl Read for ReadOnlyAnchoredFile {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.validate_current()?;
        self.inner.file.read(bytes)
    }
}
