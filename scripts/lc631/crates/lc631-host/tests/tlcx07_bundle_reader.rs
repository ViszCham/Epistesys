//! Storage-only byte observations under synthetic per-lease quotas, not a broker.
use lc631_host::{BundlePart, HostJournalBundleReader, JournalBundleLimits};
use lc631_process_supervisor::test_fixtures::bounded_test_lease;
use lc631_process_supervisor::{ResourceAmount, ResourceKind, ResourceRequest};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn root() -> PathBuf {
    let base = std::env::temp_dir().canonicalize().unwrap();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    for attempt in 0..32 {
        let path = base.join(format!(
            "epistesys-tlcx07-reader-{}-{now}-{attempt}",
            std::process::id()
        ));
        match fs::create_dir(&path) {
            Ok(()) => {
                eprintln!("retained owned reader fixture: {}", path.display());
                return path;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => panic!("owned reader fixture: {error}"),
        }
    }
    panic!("unique owned reader fixture unavailable")
}
fn limits() -> JournalBundleLimits {
    JournalBundleLimits::checked(10, 1024 * 1024).unwrap()
}
fn part(root: &Path, name: &str) -> PathBuf {
    root.join(format!("{name}-00000000000000000001.bin"))
}
fn opaque_bundle(root: &Path) {
    fs::write(root.join(".journal-bundle-v1"), b"EJBDL001").unwrap();
    // Deliberately not semantic frames: the host must not claim codec validity.
    for (name, bytes) in [
        ("segment", b"seg"),
        ("index", b"idx"),
        ("local-head", b"hdr"),
    ] {
        fs::write(part(root, name), bytes).unwrap();
    }
}

#[test]
fn tlcx07_bundle_reader_never_initializes_missing_or_unmarked_roots() {
    let root = root();
    let missing = root.join("missing");
    let lease = bounded_test_lease(1).unwrap();
    {
        let view = lease.view();
        assert!(HostJournalBundleReader::open_existing(&missing, limits(), &view).is_err());
        assert!(!missing.exists());
        assert!(HostJournalBundleReader::open_existing(&root, limits(), &view).is_err());
        assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
    }
    lease.close_confirmed().unwrap();
}

#[test]
fn tlcx07_bundle_reader_reads_without_disk_admission_or_creating_lock() {
    let root = root();
    opaque_bundle(&root);
    let lease = bounded_test_lease(2).unwrap();
    {
        let view = lease.view();
        let disk_pressure = view
            .claim_resources(&[ResourceRequest::new(
                ResourceKind::Disk,
                ResourceAmount::checked(16 * 1024 * 1024).unwrap(),
            )])
            .unwrap();
        let mut reader = HostJournalBundleReader::open_existing(&root, limits(), &view).unwrap();
        let inventory = reader.inventory().unwrap();
        assert_eq!(inventory.items().len(), 1);
        assert_eq!(
            reader.read_part(1, BundlePart::Segment).unwrap().as_bytes(),
            b"seg"
        );
        reader.validate_snapshot().unwrap();
        assert_eq!(reader.next_sequence().unwrap(), 2);
        assert_eq!(fs::read_dir(&root).unwrap().count(), 4);
        assert!(!root.join(".resource-owner-v1.lock").exists());
        drop(disk_pressure);
    }
    lease.close_confirmed().unwrap();
}

#[test]
fn tlcx07_bundle_reader_holds_partial_data_without_repair() {
    let root = root();
    fs::write(root.join(".journal-bundle-v1"), b"EJBDL001").unwrap();
    fs::write(part(&root, "segment"), b"retained").unwrap();
    let lease = bounded_test_lease(3).unwrap();
    {
        let view = lease.view();
        assert!(HostJournalBundleReader::open_existing(&root, limits(), &view).is_err());
    }
    assert_eq!(fs::read(part(&root, "segment")).unwrap(), b"retained");
    assert_eq!(fs::read_dir(&root).unwrap().count(), 2);
    lease.close_confirmed().unwrap();
}

#[test]
fn tlcx07_bundle_reader_mutation_poison_and_retained_part_claims() {
    let root = root();
    opaque_bundle(&root);
    let lease = bounded_test_lease(4).unwrap();
    {
        let view = lease.view();
        let mut reader = HostJournalBundleReader::open_existing(&root, limits(), &view).unwrap();
        let bytes = reader.read_part(1, BundlePart::Segment).unwrap();
        fs::write(part(&root, "index"), b"mutated").unwrap();
        assert!(reader.read_part(1, BundlePart::Index).is_err());
        assert!(reader.inventory().is_err());
        drop(reader);
        let amount = ResourceAmount::checked(16 * 1024 * 1024).unwrap();
        assert!(view
            .claim_resources(&[ResourceRequest::new(ResourceKind::Ram, amount)])
            .is_err());
        assert!(view
            .claim_resources(&[ResourceRequest::new(ResourceKind::Output, amount)])
            .is_err());
        drop(bytes);
        let available = view
            .claim_resources(&[
                ResourceRequest::new(ResourceKind::Ram, amount),
                ResourceRequest::new(ResourceKind::Output, amount),
            ])
            .unwrap();
        drop(available);
    }
    assert_eq!(fs::read(part(&root, "index")).unwrap(), b"mutated");
    lease.close_confirmed().unwrap();
}

#[test]
fn tlcx07_bundle_reader_final_snapshot_rejects_new_entries_without_removing_them() {
    let root = root();
    opaque_bundle(&root);
    let lease = bounded_test_lease(5).unwrap();
    {
        let view = lease.view();
        let mut reader = HostJournalBundleReader::open_existing(&root, limits(), &view).unwrap();
        fs::write(root.join("unexpected.bin"), b"retained unknown entry").unwrap();
        assert_eq!(
            reader.read_part(1, BundlePart::Segment).unwrap().as_bytes(),
            b"seg"
        );
        assert!(reader.validate_snapshot().is_err());
        assert!(reader.next_sequence().is_err());
    }
    assert_eq!(
        fs::read(root.join("unexpected.bin")).unwrap(),
        b"retained unknown entry"
    );
    lease.close_confirmed().unwrap();
}
