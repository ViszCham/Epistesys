use super::Identity;
use std::fs::{DirBuilder, File, OpenOptions};
use std::io;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::Path;

pub(super) fn open_source(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
        .open(path)
}
pub(super) fn open_snapshot(path: &Path) -> io::Result<File> {
    open_source(path)
}
pub(super) fn open_directory(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
}
pub(super) fn create_directory(path: &Path) -> io::Result<()> {
    DirBuilder::new().mode(0o700).create(path)
}
pub(super) fn create_file(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
}
pub(super) fn identity(file: &File) -> io::Result<Identity> {
    let metadata = file.metadata()?;
    Ok(Identity(metadata.dev(), metadata.ino()))
}
pub(super) fn seal(file: &File) -> io::Result<()> {
    file.set_permissions(std::fs::Permissions::from_mode(0o400))
}
pub(super) fn remove_owned(path: &Path, expected: Identity, directory: bool) -> io::Result<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink()
        || Identity(metadata.dev(), metadata.ino()) != expected
        || metadata.is_dir() != directory
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "snapshot_cleanup_identity_mismatch",
        ));
    }
    // The directory is private to this process owner. A same-owner/privileged
    // rename racing this POSIX unlink is explicitly outside the sandbox claim.
    // No recursion and no user-supplied cleanup target are used.
    if directory {
        std::fs::remove_dir(path)
    } else {
        std::fs::remove_file(path)
    }
}
