use super::{unsafe_path, AnchoredFile, DirectoryAnchor, FileIdentity};
use std::ffi::{CString, OsStr};
use std::fs::File;
use std::io;
#[cfg(target_os = "linux")]
use std::io::Write;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
#[cfg(target_os = "linux")]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
#[cfg(target_os = "linux")]
use std::sync::atomic::{AtomicU64, Ordering};

pub(super) fn root_anchor(_path: &Path) -> io::Result<DirectoryAnchor> {
    let root = CString::new("/").expect("static root");
    // SAFETY: root is terminated; successful open returns one owned descriptor.
    let fd = unsafe {
        libc::open(
            root.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: fd is a newly owned successful descriptor, never duplicated here.
    let file = unsafe { File::from_raw_fd(fd) };
    Ok(DirectoryAnchor {
        path: PathBuf::from("/"),
        directories: vec![file],
    })
}

fn name_bytes(name: &OsStr) -> io::Result<CString> {
    CString::new(name.as_bytes()).map_err(|_| unsafe_path())
}

fn directory_fd(parent: &DirectoryAnchor) -> io::Result<i32> {
    parent
        .directories
        .last()
        .map(AsRawFd::as_raw_fd)
        .ok_or_else(unsafe_path)
}

pub(super) fn open_directory(
    parent: &DirectoryAnchor,
    name: &OsStr,
    create: bool,
) -> io::Result<File> {
    let name = name_bytes(name)?;
    let parent_fd = directory_fd(parent)?;
    if create {
        // SAFETY: the parent descriptor is owned/live and name is one terminated component.
        if unsafe { libc::mkdirat(parent_fd, name.as_ptr(), 0o700) } != 0 {
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::AlreadyExists {
                return Err(error);
            }
        }
    }
    // SAFETY: the owned directory anchors resolution; NOFOLLOW rejects symlinks.
    let fd = unsafe {
        libc::openat(
            parent_fd,
            name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: a successful openat returns a newly owned descriptor.
    Ok(unsafe { File::from_raw_fd(fd) })
}

pub(super) fn open_file(
    parent: &DirectoryAnchor,
    name: &OsStr,
    create: bool,
    exclusive: bool,
) -> io::Result<File> {
    let name = name_bytes(name)?;
    let flags = libc::O_RDWR | libc::O_CLOEXEC | libc::O_NOFOLLOW;
    let parent_fd = directory_fd(parent)?;
    // SAFETY: parent owns the descriptor and name is one component; 0600 applies atomically on creation.
    let mut newly_created = false;
    // SAFETY: the anchored descriptor and single-component name remain live;
    // O_EXCL distinguishes newly owned files from existing ledger/source files.
    let mut fd = unsafe {
        libc::openat(
            parent_fd,
            name.as_ptr(),
            flags
                | if create {
                    libc::O_CREAT | libc::O_EXCL
                } else {
                    0
                },
            0o600,
        )
    };
    if fd >= 0 {
        newly_created = create;
    }
    if fd < 0
        && create
        && !exclusive
        && io::Error::last_os_error().kind() == io::ErrorKind::AlreadyExists
    {
        // SAFETY: existing files are opened without changing their ACL/mode.
        fd = unsafe { libc::openat(parent_fd, name.as_ptr(), flags) };
    }
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: a successful openat returns a newly owned descriptor.
    let file = unsafe { File::from_raw_fd(fd) };
    if newly_created {
        strip_inherited_access_acl(&file)?;
    }
    identity(&file)?;
    Ok(file)
}

#[cfg(target_os = "linux")]
fn strip_inherited_access_acl(file: &File) -> io::Result<()> {
    let attribute = CString::new("system.posix_acl_access").expect("static ACL name");
    // SAFETY: file owns a live descriptor and attribute is terminated. This is
    // called only for newly created, exclusively owned private files, never an
    // existing user's file. Removing the inherited ACL precedes chmod so a mask
    // change cannot reactivate named users/groups from a parent default ACL.
    if unsafe { libc::fremovexattr(file.as_raw_fd(), attribute.as_ptr()) } != 0 {
        let error = io::Error::last_os_error();
        if !matches!(error.raw_os_error(), Some(libc::ENODATA | libc::ENOTSUP)) {
            return Err(error);
        }
    }
    file.set_permissions(std::fs::Permissions::from_mode(0o600))
}

#[cfg(not(target_os = "linux"))]
fn strip_inherited_access_acl(_file: &File) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "private_file_acl_profile_unavailable",
    ))
}

pub(super) fn identity(file: &File) -> io::Result<FileIdentity> {
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.nlink() != 1 {
        return Err(unsafe_path());
    }
    Ok(FileIdentity {
        volume: metadata.dev(),
        index: metadata.ino(),
    })
}

pub(super) fn validate_current(file: &AnchoredFile) -> io::Result<()> {
    // Keep the user-visible namespace tied to each pinned ancestor as well as
    // the final inode. A renamed/replaced parent is a conflict, not permission
    // to keep editing the old inode under a different target reference.
    let paths = file.parent.path.ancestors().collect::<Vec<_>>();
    if paths.len() != file.parent.directories.len() {
        return Err(unsafe_path());
    }
    for (path, held) in paths.into_iter().rev().zip(&file.parent.directories) {
        let named = std::fs::symlink_metadata(path)?;
        let pinned = held.metadata()?;
        if !named.is_dir()
            || named.file_type().is_symlink()
            || named.dev() != pinned.dev()
            || named.ino() != pinned.ino()
        {
            return Err(unsafe_path());
        }
    }
    if identity(&file.file)? != file.identity {
        return Err(unsafe_path());
    }
    let name = name_bytes(&file.name)?;
    let mut current = std::mem::MaybeUninit::<libc::stat>::uninit();
    // SAFETY: live anchored parent, terminated name and writable stat storage.
    if unsafe {
        libc::fstatat(
            directory_fd(&file.parent)?,
            name.as_ptr(),
            current.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: successful fstatat initialized the structure.
    let current = unsafe { current.assume_init() };
    if current.st_dev as u128 != u128::from(file.identity.volume)
        || current.st_ino as u128 != u128::from(file.identity.index)
        || current.st_nlink != 1
        || current.st_mode & libc::S_IFMT != libc::S_IFREG
    {
        return Err(unsafe_path());
    }
    Ok(())
}

#[cfg(target_os = "linux")]
pub(super) fn replace(file: &mut AnchoredFile, bytes: &[u8]) -> io::Result<()> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let metadata = file.file.metadata()?;
    // Replacement must not silently change ownership or broaden ACL semantics.
    // Other-owner/extended-ACL sources require a distinct supported profile.
    // SAFETY: geteuid is a no-argument process identity observation.
    if metadata.uid() != unsafe { libc::geteuid() } {
        return Err(unsafe_path());
    }
    let acl_name = CString::new("system.posix_acl_access").expect("static ACL name");
    // SAFETY: the descriptor is live; a null/zero-size output requests only length.
    let acl_size = unsafe {
        libc::fgetxattr(
            file.file.as_raw_fd(),
            acl_name.as_ptr(),
            std::ptr::null_mut(),
            0,
        )
    };
    if acl_size > 0 {
        return Err(unsafe_path());
    }
    if acl_size < 0 {
        let error = io::Error::last_os_error();
        if !matches!(error.raw_os_error(), Some(libc::ENODATA | libc::ENOTSUP)) {
            return Err(error);
        }
    }
    let name = format!(
        ".epr-replace-{}-{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let mut staged = file.parent.create_private_file(Path::new(&name))?;
    staged.file.write_all(bytes)?;
    // SAFETY: staged owns a live descriptor; original numeric ownership was
    // observed from the locked target, and failure aborts before replacement.
    if unsafe { libc::fchown(staged.file.as_raw_fd(), metadata.uid(), metadata.gid()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    staged
        .file
        .set_permissions(std::fs::Permissions::from_mode(metadata.mode() & 0o777))?;
    staged.file.sync_all()?;
    validate_current(file)?;
    let old_name = name_bytes(&file.name)?;
    let staged_name = name_bytes(OsStr::new(&name))?;
    let fd = directory_fd(&file.parent)?;
    // SAFETY: both names are one component in the same owned directory. Rename
    // replaces only this directory entry, never writes through an outside alias.
    if unsafe { libc::renameat(fd, staged_name.as_ptr(), fd, old_name.as_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    file.parent
        .directories
        .last()
        .ok_or_else(unsafe_path)?
        .sync_all()?;
    file.file = staged.file.try_clone()?;
    file.identity = identity(&file.file)?;
    validate_current(file)
}

#[cfg(not(target_os = "linux"))]
pub(super) fn replace(_file: &mut AnchoredFile, _bytes: &[u8]) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "atomic_repair_metadata_profile_unavailable",
    ))
}
