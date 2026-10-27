use lc631_process_supervisor::{run_bounded, AuthenticatedSourceSnapshot, RunRequest};
use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn fixture() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "epr-snapshot-input-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    root
}
fn digest(bytes: &[u8]) -> String {
    format!(
        "sha256:{}",
        Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}

#[test]
fn epr_17_snapshot_backend_reader_helper() {
    let Some(input) = std::env::var_os("EPR_SNAPSHOT_READER") else {
        return;
    };
    let original = std::env::var_os("EPR_SNAPSHOT_ORIGINAL").unwrap();
    assert_eq!(std::fs::read(input).unwrap(), b"original-A");
    assert_eq!(std::fs::read(original).unwrap(), b"swapped-B");
}

#[test]
fn epr_17_snapshot_prevents_original_a_b_a_from_rebinding_backend_bytes() {
    let root = fixture();
    let source = root.join("source.wav");
    std::fs::write(&source, b"original-A").unwrap();
    let snapshot =
        AuthenticatedSourceSnapshot::capture(&source, &digest(b"original-A"), 1024, 1024).unwrap();
    assert_eq!(snapshot.bytes(), 10);
    std::fs::write(&source, b"swapped-B").unwrap();
    let executable = std::env::current_exe().unwrap();
    let arguments = [
        "--exact",
        "epr_17_snapshot_backend_reader_helper",
        "--nocapture",
    ]
    .map(OsString::from);
    let mut environment = std::env::vars_os()
        .filter(|(key, _)| {
            matches!(
                key.to_str(),
                Some("PATH" | "SystemRoot" | "WINDIR" | "TEMP" | "TMP")
            )
        })
        .collect::<Vec<_>>();
    environment.push((
        "EPR_SNAPSHOT_READER".into(),
        snapshot.path().as_os_str().into(),
    ));
    environment.push(("EPR_SNAPSHOT_ORIGINAL".into(), source.as_os_str().into()));
    let output = run_bounded(RunRequest {
        executable: executable.as_os_str(),
        arguments: &arguments,
        current_dir: &root,
        environment: &environment,
        stdin: None,
        timeout: Duration::from_secs(5),
        stdout_limit: 16_384,
        stderr_limit: 16_384,
    })
    .unwrap();
    assert_eq!(
        output.exit_code,
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.io_cleanup_complete && output.process_tree_kill_verified);
    std::fs::write(&source, b"original-A").unwrap();
    let private_file = snapshot.path().to_path_buf();
    let private_directory = private_file.parent().unwrap().to_path_buf();
    snapshot.close().unwrap();
    assert!(!private_file.exists() && !private_directory.exists());
    assert_eq!(std::fs::read(&source).unwrap(), b"original-A");
}

#[test]
fn epr_17_read_only_source_and_digest_budget_boundaries_are_supported() {
    let root = fixture();
    let source = root.join("readonly.bin");
    std::fs::write(&source, b"read-only-source").unwrap();
    let mut permissions = std::fs::metadata(&source).unwrap().permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&source, permissions).unwrap();
    assert!(
        AuthenticatedSourceSnapshot::capture(&source, &digest(b"different"), 1024, 1024).is_err()
    );
    assert!(
        AuthenticatedSourceSnapshot::capture(&source, &digest(b"read-only-source"), 1024, 2)
            .is_err()
    );
    let snapshot =
        AuthenticatedSourceSnapshot::capture(&source, &digest(b"read-only-source"), 1024, 1024)
            .unwrap();
    assert_eq!(std::fs::read(snapshot.path()).unwrap(), b"read-only-source");
    #[cfg(windows)]
    assert!(std::fs::OpenOptions::new()
        .write(true)
        .open(snapshot.path())
        .is_err());
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        assert_eq!(
            std::fs::metadata(snapshot.path()).unwrap().mode() & 0o777,
            0o400
        );
        assert_eq!(
            std::fs::metadata(snapshot.path().parent().unwrap())
                .unwrap()
                .mode()
                & 0o777,
            0o700
        );
    }
    snapshot.close().unwrap();
    assert_eq!(std::fs::read(&source).unwrap(), b"read-only-source");
}

#[cfg(unix)]
#[test]
fn epr_17_cleanup_never_unlinks_a_replacement_identity() {
    let root = fixture();
    let source = root.join("input");
    std::fs::write(&source, b"original-A").unwrap();
    let snapshot =
        AuthenticatedSourceSnapshot::capture(&source, &digest(b"original-A"), 1024, 1024).unwrap();
    let path = snapshot.path().to_path_buf();
    std::fs::rename(&path, path.with_extension("moved-owned")).unwrap();
    std::fs::write(&path, b"foreign-replacement-do-not-delete").unwrap();
    assert!(snapshot.close().is_err());
    assert_eq!(
        std::fs::read(&path).unwrap(),
        b"foreign-replacement-do-not-delete"
    );
    assert!(source.exists());
}
