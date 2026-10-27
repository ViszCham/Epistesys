use lc631_process_supervisor::filesystem::DirectoryAnchor;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn fixture() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("epr-filesystem-{}-{unique}", std::process::id()));
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/lib.rs"), b"before").unwrap();
    root
}

#[test]
fn epr_10_hardlink_alias_is_rejected_before_any_write() {
    let root = fixture();
    let alias = root.with_extension("outside-alias");
    fs::hard_link(root.join("src/lib.rs"), &alias).unwrap();
    let anchor = DirectoryAnchor::open_existing(&root).unwrap();
    assert!(anchor.open_read_write(Path::new("src/lib.rs")).is_err());
    assert_eq!(fs::read(alias).unwrap(), b"before");
}

#[test]
fn epr_10_relative_escape_and_windows_stream_names_are_rejected() {
    let root = fixture();
    let anchor = DirectoryAnchor::open_existing(&root).unwrap();
    for path in [
        "../outside.rs",
        "src/lib.rs:stream",
        "src/NUL",
        "src/trailing.",
    ] {
        assert!(anchor.open_read_write(Path::new(path)).is_err());
    }
    assert!(anchor.directory(Path::new("../outside"), true).is_err());
}

#[test]
fn epr_10_preimage_compare_rejects_changed_bytes_without_mutation() {
    let root = fixture();
    let anchor = DirectoryAnchor::open_existing(&root).unwrap();
    let mut file = anchor.open_read_write(Path::new("src/lib.rs")).unwrap();
    assert!(file.compare_and_write(b"different", b"after").is_err());
    assert_eq!(file.read_bounded(64).unwrap(), b"before");
}

#[test]
fn epr_10_impossible_read_budget_is_rejected_without_panic_or_wrapping() {
    let root = fixture();
    let anchor = DirectoryAnchor::open_existing(&root).unwrap();
    let mut file = anchor.open_read_write(Path::new("src/lib.rs")).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        file.read_bounded(usize::MAX)
    }));
    assert!(
        matches!(result, Ok(Err(_))),
        "impossible cap panicked or wrapped into success"
    );
    assert_eq!(file.read_bounded(64).unwrap(), b"before");
}

#[cfg(target_os = "linux")]
#[test]
fn epr_10_inherited_default_acl_is_removed_before_permission_restore() {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::PermissionsExt;
    let root = fixture();
    fs::set_permissions(root.join("src/lib.rs"), fs::Permissions::from_mode(0o640)).unwrap();
    let mut acl = 2_u32.to_le_bytes().to_vec();
    for (tag, permissions, id) in [
        (1_u16, 7_u16, u32::MAX),
        (2, 4, 65534),
        (4, 0, u32::MAX),
        (16, 4, u32::MAX),
        (32, 0, u32::MAX),
    ] {
        acl.extend_from_slice(&tag.to_le_bytes());
        acl.extend_from_slice(&permissions.to_le_bytes());
        acl.extend_from_slice(&id.to_le_bytes());
    }
    let parent = CString::new(root.join("src").as_os_str().as_bytes()).unwrap();
    let attribute = CString::new("system.posix_acl_default").unwrap();
    // SAFETY: fixture-owned path, terminated attribute, valid ACL byte buffer.
    assert_eq!(
        unsafe {
            libc::setxattr(
                parent.as_ptr(),
                attribute.as_ptr(),
                acl.as_ptr().cast(),
                acl.len(),
                0,
            )
        },
        0
    );
    let anchor = DirectoryAnchor::open_existing(&root).unwrap();
    let mut file = anchor.open_read_write(Path::new("src/lib.rs")).unwrap();
    file.compare_and_write(b"before", b"after").unwrap();
    let target = CString::new(root.join("src/lib.rs").as_os_str().as_bytes()).unwrap();
    let access_acl = CString::new("system.posix_acl_access").unwrap();
    // SAFETY: live fixture path and terminated attribute; null requests only size.
    let inherited = unsafe {
        libc::getxattr(
            target.as_ptr(),
            access_acl.as_ptr(),
            std::ptr::null_mut(),
            0,
        )
    };
    assert_eq!(
        inherited, -1,
        "replacement retained an inherited named-user ACL"
    );
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ENODATA)
    );
    assert_eq!(
        fs::metadata(root.join("src/lib.rs"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o640
    );
    let mut backup = anchor
        .create_private_file(Path::new("src/private.backup"))
        .unwrap();
    backup.write_and_sync(b"preimage").unwrap();
    let backup_path = CString::new(root.join("src/private.backup").as_os_str().as_bytes()).unwrap();
    // SAFETY: same bounded fixture-only size observation as above.
    assert_eq!(
        unsafe {
            libc::getxattr(
                backup_path.as_ptr(),
                access_acl.as_ptr(),
                std::ptr::null_mut(),
                0,
            )
        },
        -1
    );
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ENODATA)
    );
}

#[test]
fn epr_10_real_write_retains_private_preimage_and_never_overwrites_backups() {
    let root = fixture();
    let anchor = DirectoryAnchor::open_existing(&root).unwrap();
    let backups = anchor
        .directory(Path::new(".dgcl-repair-backups"), true)
        .unwrap();
    let mut file = anchor.open_read_write(Path::new("src/lib.rs")).unwrap();
    let before = file.read_bounded(64).unwrap();
    let mut backup = backups
        .create_private_file(Path::new("one.backup"))
        .unwrap();
    backup.write_and_sync(&before).unwrap();
    assert!(backups
        .create_private_file(Path::new("one.backup"))
        .is_err());
    file.compare_and_write(&before, b"after").unwrap();
    assert_eq!(file.read_bounded(64).unwrap(), b"after");
    assert_eq!(backup.read_bounded(64).unwrap(), b"before");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            backup.metadata().unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

#[cfg(windows)]
#[test]
fn epr_10_windows_drive_anchor_is_an_absolute_root_not_drive_relative() {
    let absolute = std::path::absolute(std::env::temp_dir()).unwrap();
    let root = absolute.ancestors().last().unwrap();
    let anchor = DirectoryAnchor::open_existing(root).unwrap();
    assert!(anchor.path().is_absolute());
    assert_eq!(anchor.path(), root);
}

#[cfg(windows)]
#[test]
fn epr_10_windows_ancestor_handles_prevent_path_swap_while_file_is_owned() {
    let root = fixture();
    let anchor = DirectoryAnchor::open_existing(&root).unwrap();
    let mut file = anchor.open_read_write(Path::new("src/lib.rs")).unwrap();
    assert!(fs::rename(root.join("src"), root.join("moved")).is_err());
    file.compare_and_write(b"before", b"after").unwrap();
    assert_eq!(file.read_bounded(64).unwrap(), b"after");
}

#[cfg(unix)]
#[test]
fn epr_11_competing_ledger_open_reports_busy_without_releasing_the_owner() {
    let root = fixture();
    let anchor = DirectoryAnchor::open_existing(&root).unwrap();
    let mut first = anchor
        .open_or_create_file(Path::new("ledger.jsonl"))
        .unwrap();
    match anchor.open_or_create_file(Path::new("ledger.jsonl")) {
        Ok(_) => panic!("competing writer acquired owned ledger"),
        Err(error) => assert_eq!(error.kind(), std::io::ErrorKind::WouldBlock),
    }
    first.append_and_sync(b"owned\n").unwrap();
    assert_eq!(first.read_bounded(64).unwrap(), b"owned\n");
}

#[cfg(unix)]
#[test]
fn epr_10_changed_directory_namespace_is_rejected_before_commit() {
    let root = fixture();
    let anchor = DirectoryAnchor::open_existing(&root).unwrap();
    let mut file = anchor.open_read_write(Path::new("src/lib.rs")).unwrap();
    fs::rename(root.join("src"), root.join("moved")).unwrap();
    fs::create_dir(root.join("src")).unwrap();
    fs::write(root.join("src/lib.rs"), b"replacement").unwrap();
    assert!(file.compare_and_write(b"before", b"after").is_err());
    assert_eq!(fs::read(root.join("moved/lib.rs")).unwrap(), b"before");
    assert_eq!(fs::read(root.join("src/lib.rs")).unwrap(), b"replacement");
}

#[cfg(unix)]
#[test]
fn epr_10_symlink_component_never_redirects_an_anchored_write() {
    use std::os::unix::fs::symlink;
    let root = fixture();
    let outside = fixture();
    symlink(outside.join("src"), root.join("redirect")).unwrap();
    let anchor = DirectoryAnchor::open_existing(&root).unwrap();
    assert!(anchor
        .open_read_write(Path::new("redirect/lib.rs"))
        .is_err());
    assert_eq!(fs::read(outside.join("src/lib.rs")).unwrap(), b"before");
}
