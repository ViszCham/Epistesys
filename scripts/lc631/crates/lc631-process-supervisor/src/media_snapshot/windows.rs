use super::Identity;
use std::fs::{File, OpenOptions};
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::path::{Component, Path, Prefix};
use windows_sys::Win32::Foundation::{
    LocalFree, GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW;
use windows_sys::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
use windows_sys::Win32::Storage::FileSystem::{
    CreateDirectoryW, CreateFileW, FileDispositionInfo, GetFileInformationByHandle,
    SetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION, CREATE_NEW, FILE_DISPOSITION_INFO,
    FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ, FILE_SHARE_WRITE,
};

struct Descriptor(PSECURITY_DESCRIPTOR);
impl Descriptor {
    fn new() -> io::Result<Self> {
        let sddl = "D:P(A;;FA;;;OW)\0".encode_utf16().collect::<Vec<_>>();
        let mut value = std::ptr::null_mut();
        // SAFETY: terminated SDDL and writable output; Descriptor owns LocalFree.
        if unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                1,
                &mut value,
                std::ptr::null_mut(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(value))
    }
    fn attributes(&self) -> SECURITY_ATTRIBUTES {
        SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: self.0,
            bInheritHandle: 0,
        }
    }
}
impl Drop for Descriptor {
    fn drop(&mut self) {
        unsafe {
            LocalFree(self.0);
        }
    }
}
fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

pub(super) fn open_source(path: &Path) -> io::Result<File> {
    let absolute = std::path::absolute(path)?;
    if !matches!(absolute.components().next(), Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)))
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "media_snapshot_requires_local_disk_source",
        ));
    }
    let file = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(absolute)?;
    if file.metadata()?.file_attributes() & 0x400 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "media_snapshot_reparse_source",
        ));
    }
    Ok(file)
}
pub(super) fn open_snapshot(path: &Path) -> io::Result<File> {
    open_source(path)
}
pub(super) fn open_directory(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
}
pub(super) fn create_directory(path: &Path) -> io::Result<()> {
    let descriptor = Descriptor::new()?;
    let security = descriptor.attributes();
    let name = wide(path);
    // SAFETY: live terminated name and protected owner-only descriptor.
    if unsafe { CreateDirectoryW(name.as_ptr(), &security) } == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
pub(super) fn create_file(path: &Path) -> io::Result<File> {
    let descriptor = Descriptor::new()?;
    let security = descriptor.attributes();
    let name = wide(path);
    // SAFETY: successful CREATE_NEW returns one owned handle, with no inheritance.
    let handle = unsafe {
        CreateFileW(
            name.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            0,
            &security,
            CREATE_NEW,
            FILE_FLAG_OPEN_REPARSE_POINT,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { File::from_raw_handle(handle) })
}
pub(super) fn identity(file: &File) -> io::Result<Identity> {
    let mut information = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
    // SAFETY: borrowed live handle and writable, correctly sized output.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), information.as_mut_ptr()) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let information = unsafe { information.assume_init() };
    if information.dwFileAttributes & 0x400 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "snapshot_reparse_identity",
        ));
    }
    Ok(Identity(
        u64::from(information.dwVolumeSerialNumber),
        (u64::from(information.nFileIndexHigh) << 32) | u64::from(information.nFileIndexLow),
    ))
}
pub(super) fn seal(_file: &File) -> io::Result<()> {
    Ok(())
}
pub(super) fn remove_owned(path: &Path, expected: Identity, directory: bool) -> io::Result<()> {
    const DELETE_ACCESS: u32 = 0x0001_0000;
    let file = OpenOptions::new()
        .read(true)
        .access_mode(GENERIC_READ | DELETE_ACCESS)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(
            FILE_FLAG_OPEN_REPARSE_POINT
                | if directory {
                    FILE_FLAG_BACKUP_SEMANTICS
                } else {
                    0
                },
        )
        .open(path)?;
    if identity(&file)? != expected || file.metadata()?.is_dir() != directory {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "snapshot_cleanup_identity_mismatch",
        ));
    }
    let disposition = FILE_DISPOSITION_INFO { DeleteFile: true };
    // SAFETY: deletion addresses this identity-checked owned handle, not a
    // later path lookup. A nonempty directory fails; cleanup never recurses.
    if unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle(),
            FileDispositionInfo,
            (&disposition as *const FILE_DISPOSITION_INFO).cast(),
            std::mem::size_of::<FILE_DISPOSITION_INFO>() as u32,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    drop(file);
    Ok(())
}
