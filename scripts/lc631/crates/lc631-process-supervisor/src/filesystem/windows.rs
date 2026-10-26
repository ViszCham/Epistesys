use super::{unsafe_path, AnchoredFile, DirectoryAnchor, FileIdentity};
use std::ffi::OsStr;
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
    CreateFileW, GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION, CREATE_NEW,
    FILE_ATTRIBUTE_NORMAL, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
    FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_ALWAYS,
};

pub(super) fn root_anchor(path: &Path) -> io::Result<DirectoryAnchor> {
    let mut components = path.components();
    let Some(Component::Prefix(prefix)) = components.next() else {
        return Err(unsafe_path());
    };
    if !matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)) {
        return Err(unsafe_path());
    }
    let root = path
        .ancestors()
        .last()
        .filter(|root| root.is_absolute())
        .ok_or_else(unsafe_path)?
        .to_path_buf();
    let file = open_dir(&root)?;
    Ok(DirectoryAnchor {
        path: root,
        directories: vec![file],
    })
}

fn open_dir(path: &Path) -> io::Result<File> {
    // Denying FILE_SHARE_DELETE pins this directory against rename/replacement
    // while the handle remains owned. OPEN_REPARSE_POINT inspects, not follows.
    let file = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_dir() || metadata.file_attributes() & 0x400 != 0 {
        return Err(unsafe_path());
    }
    Ok(file)
}

pub(super) fn open_directory(
    parent: &DirectoryAnchor,
    name: &OsStr,
    create: bool,
) -> io::Result<File> {
    let path = parent.path.join(name);
    if create {
        match std::fs::create_dir(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    open_dir(&path)
}

struct PrivateDescriptor(PSECURITY_DESCRIPTOR);

impl PrivateDescriptor {
    fn new() -> io::Result<Self> {
        // Protected DACL: only the owner-rights SID, not inherited Users/Everyone.
        let sddl = "D:P(A;;FA;;;OW)\0".encode_utf16().collect::<Vec<_>>();
        let mut descriptor = std::ptr::null_mut();
        // SAFETY: sddl is terminated; output points to writable storage. Windows
        // allocates the descriptor, which this unique owner releases with LocalFree.
        if unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                1,
                &mut descriptor,
                std::ptr::null_mut(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(descriptor))
    }
}

impl Drop for PrivateDescriptor {
    fn drop(&mut self) {
        // SAFETY: this pointer is uniquely owned output from the conversion API.
        unsafe {
            LocalFree(self.0);
        }
    }
}

pub(super) fn open_file(
    parent: &DirectoryAnchor,
    name: &OsStr,
    create: bool,
    exclusive: bool,
) -> io::Result<File> {
    let path = parent.path.join(name);
    let file = if create {
        let descriptor = PrivateDescriptor::new()?;
        let security = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor.0,
            bInheritHandle: 0,
        };
        let wide = path
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        // SAFETY: all pointers remain live for the call; exclusive sharing and
        // no-follow flags bind the newly owned disk-file handle to this operation.
        let handle = unsafe {
            CreateFileW(
                wide.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                0,
                &security,
                if exclusive { CREATE_NEW } else { OPEN_ALWAYS },
                FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
                std::ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: successful CreateFileW returns one newly owned handle.
        unsafe { File::from_raw_handle(handle) }
    } else {
        OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(0)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)?
    };
    identity(&file)?;
    Ok(file)
}

pub(super) fn identity(file: &File) -> io::Result<FileIdentity> {
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.file_attributes() & 0x400 != 0 {
        return Err(unsafe_path());
    }
    let mut info = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
    // SAFETY: file owns a live handle; info points to writable correctly sized storage.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), info.as_mut_ptr()) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the successful API call fully initialized the structure.
    let info = unsafe { info.assume_init() };
    if info.nNumberOfLinks != 1 {
        return Err(unsafe_path());
    }
    Ok(FileIdentity {
        volume: u64::from(info.dwVolumeSerialNumber),
        index: (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
    })
}

pub(super) fn validate_current(file: &AnchoredFile) -> io::Result<()> {
    // Ancestors deny rename/delete and the final file is exclusively opened.
    if identity(&file.file)? != file.identity {
        return Err(unsafe_path());
    }
    Ok(())
}

pub(super) fn replace(file: &mut AnchoredFile, bytes: &[u8]) -> io::Result<()> {
    file.write_and_sync(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::Security::Authorization::{
        ConvertSecurityDescriptorToStringSecurityDescriptorW, GetSecurityInfo, SE_FILE_OBJECT,
    };
    use windows_sys::Win32::Security::DACL_SECURITY_INFORMATION;

    #[test]
    fn epr_10_backup_dacl_is_protected_and_owner_only_at_creation() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("epr-private-acl-{unique}"));
        std::fs::create_dir(&root).unwrap();
        let anchor = DirectoryAnchor::open_existing(&root).unwrap();
        let backup = anchor
            .create_private_file(Path::new("fixture.backup"))
            .unwrap();
        let mut descriptor = std::ptr::null_mut();
        // SAFETY: owned live handle and valid output storage; Windows allocates
        // the returned descriptor, then the RAII wrapper owns its LocalFree.
        let status = unsafe {
            GetSecurityInfo(
                backup.file.as_raw_handle(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut descriptor,
            )
        };
        assert_eq!(status, 0);
        let descriptor = PrivateDescriptor(descriptor);
        let mut text = std::ptr::null_mut();
        let mut length = 0;
        // SAFETY: descriptor is live and validated by GetSecurityInfo; writable
        // outputs receive a Windows-owned terminated string and its size.
        assert_ne!(
            unsafe {
                ConvertSecurityDescriptorToStringSecurityDescriptorW(
                    descriptor.0,
                    1,
                    DACL_SECURITY_INFORMATION,
                    &mut text,
                    &mut length,
                )
            },
            0
        );
        assert!(length > 0 && length <= 4096);
        let text_owner = PrivateDescriptor(text.cast());
        // SAFETY: conversion allocated exactly this many UTF-16 code units.
        let encoded = unsafe { std::slice::from_raw_parts(text, length as usize) };
        let sddl = String::from_utf16(encoded).unwrap();
        assert!(sddl.starts_with("D:P"), "unprotected DACL: {sddl}");
        assert_eq!(
            sddl.matches('(').count(),
            1,
            "unexpected additional trustees: {sddl}"
        );
        assert!(
            sddl.contains(";;;OW)"),
            "backup not owner-restricted: {sddl}"
        );
        drop(text_owner);
    }
}
