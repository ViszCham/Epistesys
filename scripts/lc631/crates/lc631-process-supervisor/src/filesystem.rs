//! Anchored, no-follow filesystem operations for declared local workspaces.
//!
//! Windows pins every ancestor against rename/deletion; Unix resolves each
//! component with openat(O_NOFOLLOW). Handles, not a prior canonical path check,
//! own the operation. This is not an OS sandbox against a privileged owner.
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};

#[cfg(unix)]
#[path = "filesystem/unix.rs"]
mod platform;
mod readonly;
pub use readonly::ReadOnlyAnchoredFile;
mod binary_readonly;
pub use binary_readonly::BinaryReadOnlyAnchoredFile;
#[cfg(windows)]
#[path = "filesystem/windows.rs"]
mod platform;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FileIdentity {
    volume: u64,
    index: u64,
}

pub struct DirectoryAnchor {
    path: PathBuf,
    directories: Vec<File>,
}

#[derive(Debug)]
enum DirectoryValidationStage {
    Reopen,
    AncestorCount,
    HeldDirectoryIdentity(usize),
    ReopenedDirectoryIdentity(usize),
    IdentityMismatch(usize),
}
impl DirectoryValidationStage {
    fn component_index(&self) -> Option<usize> {
        match self {
            Self::HeldDirectoryIdentity(index)
            | Self::ReopenedDirectoryIdentity(index)
            | Self::IdentityMismatch(index) => Some(*index),
            Self::Reopen | Self::AncestorCount => None,
        }
    }
}
#[derive(Debug)]
struct DirectoryValidationFailure {
    stage: DirectoryValidationStage,
    error: io::Error,
}

impl DirectoryAnchor {
    pub fn open_existing(path: &Path) -> io::Result<Self> {
        let absolute = std::path::absolute(path)?;
        let mut anchor = platform::root_anchor(&absolute)?;
        for component in absolute.components() {
            match component {
                Component::Prefix(_) | Component::RootDir => {}
                Component::Normal(name) => anchor.push_directory(name, false)?,
                _ => return Err(unsafe_path()),
            }
        }
        Ok(anchor)
    }

    /// Only the final owned root may be created; missing ancestors are denied.
    pub fn open_or_create(path: &Path) -> io::Result<Self> {
        match Self::open_existing(path) {
            Ok(anchor) => Ok(anchor),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let absolute = std::path::absolute(path)?;
                let parent = absolute.parent().ok_or_else(unsafe_path)?;
                let name = absolute.file_name().ok_or_else(unsafe_path)?;
                Self::open_existing(parent)?.directory(Path::new(name), true)
            }
            Err(error) => Err(error),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Reopen with the existing no-follow walk and compare every held ancestor.
    /// A mismatch is refusal, never an instruction to recreate or repair paths.
    pub fn validate_current(&self) -> io::Result<()> {
        self.validate_current_observed().map_err(|failure| {
            let _component = failure.stage.component_index();
            failure.error
        })
    }

    fn validate_current_observed(&self) -> Result<(), DirectoryValidationFailure> {
        let current =
            Self::open_existing(&self.path).map_err(|error| DirectoryValidationFailure {
                stage: DirectoryValidationStage::Reopen,
                error,
            })?;
        if self.directories.is_empty() || self.directories.len() != current.directories.len() {
            return Err(DirectoryValidationFailure {
                stage: DirectoryValidationStage::AncestorCount,
                error: unsafe_path(),
            });
        }
        for (index, (held, reopened)) in self
            .directories
            .iter()
            .zip(&current.directories)
            .enumerate()
        {
            let held =
                platform::directory_identity(held).map_err(|error| DirectoryValidationFailure {
                    stage: DirectoryValidationStage::HeldDirectoryIdentity(index),
                    error,
                })?;
            let reopened = platform::directory_identity(reopened).map_err(|error| {
                DirectoryValidationFailure {
                    stage: DirectoryValidationStage::ReopenedDirectoryIdentity(index),
                    error,
                }
            })?;
            if held != reopened {
                return Err(DirectoryValidationFailure {
                    stage: DirectoryValidationStage::IdentityMismatch(index),
                    error: unsafe_path(),
                });
            }
        }
        Ok(())
    }

    pub fn directory(&self, relative: &Path, create: bool) -> io::Result<Self> {
        let mut next = self.try_clone()?;
        for name in normal_components(relative)? {
            next.push_directory(&name, create)?;
        }
        Ok(next)
    }

    pub fn open_read_write(&self, relative: &Path) -> io::Result<AnchoredFile> {
        self.open_file(relative, false, false, true)
    }

    pub fn open_read_only(&self, relative: &Path) -> io::Result<ReadOnlyAnchoredFile> {
        ReadOnlyAnchoredFile::from_inner(self.open_file(relative, false, false, false)?)
    }

    /// Binary content observation permits hardlinks, unlike security files.
    /// One named regular leaf only; never a mutation or executable capability.
    pub fn open_binary_read_only(&self, relative: &Path) -> io::Result<BinaryReadOnlyAnchoredFile> {
        let names = normal_components(relative)?;
        let [name] = names.as_slice() else {
            return Err(unsafe_path());
        };
        BinaryReadOnlyAnchoredFile::from_parent(self.try_clone()?, name.clone())
    }

    pub fn open_or_create_file(&self, relative: &Path) -> io::Result<AnchoredFile> {
        self.open_file(relative, true, false, true)
    }

    pub fn create_private_file(&self, relative: &Path) -> io::Result<AnchoredFile> {
        self.open_file(relative, true, true, true)
    }

    fn open_file(
        &self,
        relative: &Path,
        create: bool,
        exclusive: bool,
        writable: bool,
    ) -> io::Result<AnchoredFile> {
        let mut names = normal_components(relative)?;
        let name = names.pop().ok_or_else(unsafe_path)?;
        let mut parent = self.try_clone()?;
        for component in names {
            parent.push_directory(&component, false)?;
        }
        if !writable && (create || exclusive) {
            return Err(unsafe_path());
        }
        let file = platform::open_file(&parent, &name, create, exclusive, writable)?;
        let identity = platform::identity(&file)?;
        let lock = if writable {
            file.try_lock()
        } else {
            file.try_lock_shared()
        };
        lock.map_err(|error| match error {
            std::fs::TryLockError::WouldBlock => {
                io::Error::new(io::ErrorKind::WouldBlock, "anchored_file_busy")
            }
            std::fs::TryLockError::Error(error) => error,
        })?;
        Ok(AnchoredFile {
            file,
            parent,
            name,
            identity,
            writable,
        })
    }

    fn push_directory(&mut self, name: &OsStr, create: bool) -> io::Result<()> {
        validate_name(name)?;
        let file = platform::open_directory(self, name, create)?;
        self.path.push(name);
        self.directories.push(file);
        Ok(())
    }

    fn try_clone(&self) -> io::Result<Self> {
        Ok(Self {
            path: self.path.clone(),
            directories: self
                .directories
                .iter()
                .map(File::try_clone)
                .collect::<io::Result<_>>()?,
        })
    }
}

pub struct AnchoredFile {
    file: File,
    parent: DirectoryAnchor,
    name: OsString,
    identity: FileIdentity,
    writable: bool,
}

impl AnchoredFile {
    fn require_writable(&self) -> io::Result<()> {
        if self.writable {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "anchored_file_read_only",
            ))
        }
    }
    pub fn path(&self) -> PathBuf {
        self.parent.path.join(&self.name)
    }

    pub fn metadata(&self) -> io::Result<std::fs::Metadata> {
        self.file.metadata()
    }

    pub fn identity(&self) -> FileIdentity {
        self.identity
    }

    /// Validate owned parent identities and the opened file without writing it.
    pub fn validate_current(&self) -> io::Result<()> {
        self.parent.validate_current()?;
        platform::validate_current(self)
    }

    pub fn read_bounded(&mut self, limit: usize) -> io::Result<Vec<u8>> {
        let sentinel = limit
            .checked_add(1)
            .filter(|bytes| *bytes <= isize::MAX as usize)
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or_else(budget_exceeded)?;
        let limit_u64 = u64::try_from(limit).map_err(|_| budget_exceeded())?;
        if self.file.metadata()?.len() > limit_u64 {
            return Err(budget_exceeded());
        }
        self.file.seek(SeekFrom::Start(0))?;
        let mut bytes = Vec::new();
        (&mut self.file).take(sentinel).read_to_end(&mut bytes)?;
        if bytes.len() > limit {
            return Err(budget_exceeded());
        }
        Ok(bytes)
    }

    pub fn write_and_sync(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.require_writable()?;
        platform::validate_current(self)?;
        self.file.seek(SeekFrom::Start(0))?;
        self.file.write_all(bytes)?;
        self.file.set_len(bytes.len() as u64)?;
        self.file.sync_all()
    }

    /// Compare the opened preimage immediately before the platform write. Unix
    /// replacement never writes through an inode which could gain an alias.
    /// Concurrent, noncooperating same-owner writers remain outside sandbox claims.
    pub fn compare_and_write(&mut self, expected: &[u8], replacement: &[u8]) -> io::Result<()> {
        self.require_writable()?;
        platform::validate_current(self)?;
        if self.read_bounded(expected.len())? != expected {
            return Err(unsafe_path());
        }
        platform::replace(self, replacement)
    }

    pub fn append_and_sync(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.require_writable()?;
        platform::validate_current(self)?;
        self.file.seek(SeekFrom::End(0))?;
        self.file.write_all(bytes)?;
        self.file.sync_all()
    }
}

fn normal_components(path: &Path) -> io::Result<Vec<OsString>> {
    path.components()
        .map(|component| match component {
            Component::Normal(name) => {
                validate_name(name)?;
                Ok(name.to_os_string())
            }
            _ => Err(unsafe_path()),
        })
        .collect()
}

fn validate_name(name: &OsStr) -> io::Result<()> {
    let text = name.to_str().ok_or_else(unsafe_path)?;
    let stem = text
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    if text.is_empty()
        || text.ends_with(['.', ' '])
        || text
            .chars()
            .any(|c| c.is_control() || matches!(c, ':' | '/' | '\\'))
        || matches!(
            stem.as_str(),
            "CON"
                | "PRN"
                | "AUX"
                | "NUL"
                | "CONIN$"
                | "CONOUT$"
                | "COM¹"
                | "COM²"
                | "COM³"
                | "LPT¹"
                | "LPT²"
                | "LPT³"
        )
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && matches!(stem.as_bytes()[3], b'1'..=b'9'))
    {
        Err(unsafe_path())
    } else {
        Ok(())
    }
}

fn unsafe_path() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "unanchored_or_aliased_path",
    )
}
fn budget_exceeded() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "file_byte_budget_exceeded")
}

#[cfg(test)]
mod admission_tests {
    use super::*;

    #[test]
    fn tlcx05_readonly_internal_mutators_refuse_before_any_write_or_replace() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("tlcx05-readonly-{}-{stamp}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("source.txt"), b"before").unwrap();
        let anchor = DirectoryAnchor::open_existing(&root).unwrap();
        let mut file = anchor.open_read_only(Path::new("source.txt")).unwrap();
        assert_eq!(file.read_bounded(32).unwrap(), b"before");
        for result in [
            file.inner.write_and_sync(b"after"),
            file.inner.append_and_sync(b"after"),
            file.inner.compare_and_write(b"before", b"after"),
        ] {
            assert_eq!(result.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
            assert_eq!(std::fs::read(root.join("source.txt")).unwrap(), b"before");
        }
    }

    #[test]
    fn tlcx05_directory_identity_diagnostic_identifies_the_failed_layer() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let anchor = DirectoryAnchor::open_existing(root).unwrap();
        let result = anchor.validate_current_observed();
        assert!(result.is_ok(), "directory validation stage: {result:?}");
    }

    #[test]
    fn epr_10_superscript_device_names_are_rejected_without_opening_devices() {
        for name in [
            "COM¹",
            "COM²",
            "COM³",
            "LPT¹",
            "LPT²",
            "LPT³",
            "com¹.rs",
            "lpt².txt",
        ] {
            assert!(
                validate_name(OsStr::new(name)).is_err(),
                "reserved device name admitted: {name}"
            );
        }
        assert!(validate_name(OsStr::new("component.rs")).is_ok());
    }
}
