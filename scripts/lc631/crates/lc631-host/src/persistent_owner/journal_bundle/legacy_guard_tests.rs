//! Deterministic guard-race tests. Roots are printed and retained.
use super::*;
use lc631_process_supervisor::test_fixtures::bounded_test_lease;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

fn root() -> PathBuf {
    let base = std::env::temp_dir().canonicalize().unwrap();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    for n in 0..32_u32 {
        let path = base.join(format!(
            "epistesys-tlcx07-guard-{}-{}",
            std::process::id(),
            now + u128::from(n)
        ));
        match fs::create_dir(&path) {
            Ok(()) => {
                eprintln!("retained test-owned root: {}", path.display());
                return path;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => panic!("owned root: {e}"),
        }
    }
    panic!("unique test root unavailable")
}
fn create(path: &Path, bytes: &[u8]) {
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap();
    f.write_all(bytes).unwrap();
}

#[test]
fn tlcx07_guard_marker_created_after_legacy_scan_prevents_reader() {
    let path = root();
    let old_lease = bounded_test_lease(41).unwrap();
    let new_lease = bounded_test_lease(42).unwrap();
    {
        let old_view = old_lease.view();
        let new_view = new_lease.view();
        let mut ran = false;
        let result = HostPersistentStoreReader::open_binary_checkpoint_root_after_scan(
            &path,
            PersistentStoreLimits::default(),
            &old_view,
            || {
                ran = true;
                let _writer = journal_bundle::HostJournalBundleSession::open(
                    &path,
                    0,
                    journal_bundle::JournalBundleLimits::checked(1000, 16 * 1024 * 1024).unwrap(),
                    &new_view,
                )
                .unwrap();
                create(&path.join("segment-00000000000000000001.bin"), b"partial");
                Ok(())
            },
        );
        assert!(ran, "test must actually reach the post-scan cut");
        assert!(matches!(result, Err(HostError::Io(message))
            if message.contains("bundle_marker_not_legacy")));
    }
    assert_eq!(
        fs::read(path.join(".journal-bundle-v1")).unwrap(),
        b"EJBDL001"
    );
    assert_eq!(
        fs::read(path.join("segment-00000000000000000001.bin")).unwrap(),
        b"partial"
    );
    assert!(!path.join("index-00000000000000000001.bin").exists());
}

#[test]
fn tlcx07_guard_existing_marker_or_marker_lookup_error_holds() {
    let marked = root();
    create(&marked.join(".journal-bundle-v1"), b"EJBDL001");
    let bad = root();
    fs::create_dir(bad.join(".journal-bundle-v1")).unwrap();
    let lease = bounded_test_lease(43).unwrap();
    {
        let view = lease.view();
        assert!(HostPersistentStoreReader::open_binary_checkpoint_root(
            &marked,
            PersistentStoreLimits::default(),
            &view
        )
        .is_err());
        assert!(HostPersistentStoreReader::open_binary_checkpoint_root(
            &bad,
            PersistentStoreLimits::default(),
            &view
        )
        .is_err());
    }
    assert_eq!(
        fs::read(marked.join(".journal-bundle-v1")).unwrap(),
        b"EJBDL001"
    );
    assert!(bad.join(".journal-bundle-v1").is_dir());
}
