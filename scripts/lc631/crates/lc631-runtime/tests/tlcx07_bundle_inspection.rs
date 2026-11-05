//! Read-only runtime consumption of real bundle frames, not task completion.
use lc631_host::{HostJournalBundleSession, JournalBundleLimits};
use lc631_runtime::journal::{
    inspect_bundle_root, BundleInspectionError, JournalBundleLiveSession, JournalChainHeadState,
    JournalChainLimits, JournalCursor, JournalRecordKind, JournalSegmentBuilder,
};
use lc631_runtime::resource::{
    Cancellation, Deadline, Reservation, ResourceAmount, ResourceBroker, ResourceKind,
    ResourceLimits, ResourceRequest, RunResources,
};
use lc631_substrate::Digest32;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn root() -> PathBuf {
    let base = std::env::temp_dir().canonicalize().unwrap();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    for attempt in 0..32 {
        let path = base.join(format!(
            "epistesys-tlcx07-inspection-{}-{now}-{attempt}",
            std::process::id()
        ));
        match fs::create_dir(&path) {
            Ok(()) => {
                eprintln!("retained owned inspection root: {}", path.display());
                return path;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => panic!("owned inspection root: {error}"),
        }
    }
    panic!("unique owned inspection root unavailable")
}
fn limits() -> JournalBundleLimits {
    JournalBundleLimits::checked(1000, 16 * 1024 * 1024).unwrap()
}
fn resources(id: u64, disk: bool) -> (ResourceBroker, RunResources, Reservation) {
    bounded_resources(id, disk, 32 * 1024 * 1024, 32 * 1024 * 1024)
}

fn bounded_resources(
    id: u64,
    disk: bool,
    ram: u64,
    output: u64,
) -> (ResourceBroker, RunResources, Reservation) {
    let cap = ResourceLimits::uniform(64 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(cap).unwrap();
    let run = broker
        .begin_run(
            id,
            Digest32::from_bytes([9; 32]),
            cap,
            Deadline::after(Duration::from_secs(60)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let mut requests = vec![
        ResourceRequest::new(ResourceKind::Ram, ResourceAmount::checked(ram).unwrap()),
        ResourceRequest::new(
            ResourceKind::Output,
            ResourceAmount::checked(output).unwrap(),
        ),
    ];
    if disk {
        requests.push(ResourceRequest::new(
            ResourceKind::Disk,
            ResourceAmount::checked(16 * 1024 * 1024).unwrap(),
        ));
    }
    let reservation = run.reserve(&requests).unwrap();
    (broker, run, reservation)
}
fn populated_root() -> (PathBuf, Vec<u8>) {
    let root = root();
    let mut builder = JournalSegmentBuilder::genesis();
    builder
        .append_at(
            1,
            JournalRecordKind::Evidence,
            Digest32::from_bytes([1; 32]),
            b"one",
        )
        .unwrap();
    let bytes = builder.finish().unwrap();
    let (_broker, _run, mut reservation) = resources(1, true);
    {
        let view = reservation.lease();
        let mut writer = JournalBundleLiveSession::open(
            &root,
            1024 * 1024,
            limits(),
            &view,
            JournalChainLimits::default(),
        )
        .unwrap();
        writer.append(JournalCursor::genesis(), &bytes).unwrap();
    }
    reservation.release_confirmed().unwrap();
    (root, bytes)
}

#[test]
fn tlcx07_readonly_runtime_consumes_segment_index_head_without_disk_lease() {
    let (root, segment) = populated_root();
    let before = fs::read_dir(&root).unwrap().count();
    let (_broker, _run, mut reservation) = resources(2, false);
    {
        let view = reservation.lease();
        let observed =
            inspect_bundle_root(&root, limits(), &view, JournalChainLimits::default()).unwrap();
        assert_eq!(observed.segment_count(), 1);
        assert_eq!(observed.record_count(), 1);
        assert_eq!(observed.segment_input_bytes(), segment.len());
        assert_eq!(observed.cursor().sequence(), 1);
        assert_eq!(observed.head_state(), JournalChainHeadState::Unanchored);
        assert_eq!(
            fs::read(root.join("segment-00000000000000000001.bin")).unwrap(),
            segment
        );
    }
    assert_eq!(fs::read_dir(&root).unwrap().count(), before);
    reservation.release_confirmed().unwrap();
}

#[test]
fn tlcx07_readonly_runtime_rejects_opaque_storage_even_when_host_accepts_it() {
    let root = root();
    fs::write(root.join(".journal-bundle-v1"), b"EJBDL001").unwrap();
    for name in ["segment", "index", "local-head"] {
        fs::write(
            root.join(format!("{name}-00000000000000000001.bin")),
            b"opaque",
        )
        .unwrap();
    }
    let (_broker, _run, mut reservation) = resources(3, false);
    {
        let view = reservation.lease();
        assert!(
            inspect_bundle_root(&root, limits(), &view, JournalChainLimits::default()).is_err()
        );
    }
    assert_eq!(fs::read_dir(&root).unwrap().count(), 4);
    reservation.release_confirmed().unwrap();
}

#[test]
fn tlcx07_readonly_runtime_does_not_promote_empty_or_initialize_missing_roots() {
    let root = root();
    let (_broker, _run, mut writer_reservation) = resources(4, true);
    {
        let view = writer_reservation.lease();
        let _writer = HostJournalBundleSession::open(&root, 0, limits(), &view).unwrap();
    }
    writer_reservation.release_confirmed().unwrap();
    let (_broker, _run, mut reservation) = resources(5, false);
    {
        let view = reservation.lease();
        assert!(matches!(
            inspect_bundle_root(&root, limits(), &view, JournalChainLimits::default()),
            Err(BundleInspectionError::EmptyInventory)
        ));
        let missing = root.join("missing");
        assert!(
            inspect_bundle_root(&missing, limits(), &view, JournalChainLimits::default()).is_err()
        );
        assert!(!missing.exists());
    }
    reservation.release_confirmed().unwrap();
}

#[test]
fn tlcx07_readonly_runtime_corrupt_index_holds_without_rebuilding_or_deleting() {
    let (root, segment) = populated_root();
    let path = root.join("index-00000000000000000001.bin");
    let mut index = fs::read(&path).unwrap();
    index[0] ^= 1;
    fs::write(&path, &index).unwrap();
    let (_broker, _run, mut reservation) = resources(6, false);
    {
        let view = reservation.lease();
        assert!(
            inspect_bundle_root(&root, limits(), &view, JournalChainLimits::default()).is_err()
        );
    }
    assert_eq!(fs::read(&path).unwrap(), index);
    assert_eq!(
        fs::read(root.join("segment-00000000000000000001.bin")).unwrap(),
        segment
    );
    reservation.release_confirmed().unwrap();
}

#[test]
fn tlcx07_readonly_runtime_work_admission_holds_with_no_disk_or_part_changes() {
    use lc631_runtime::journal::BundleLiveError;
    use lc631_runtime::resource::ResourceError;
    let (root, segment) = populated_root();
    let before = fs::read_dir(&root).unwrap().count();
    for (id, ram, output, denied) in [
        (10, 10 * 1024 * 1024, 32 * 1024 * 1024, ResourceKind::Ram),
        (11, 32 * 1024 * 1024, 10 * 1024 * 1024, ResourceKind::Output),
    ] {
        let (_broker, _run, mut reservation) = bounded_resources(id, false, ram, output);
        {
            let view = reservation.lease();
            assert!(matches!(
                inspect_bundle_root(&root, limits(), &view, JournalChainLimits::default()),
                Err(BundleInspectionError::Validation(BundleLiveError::Resource(
                    ResourceError::Exhausted(kind)))) if kind == denied
            ));
        }
        reservation.release_confirmed().unwrap();
    }
    assert_eq!(fs::read_dir(&root).unwrap().count(), before);
    assert_eq!(
        fs::read(root.join("segment-00000000000000000001.bin")).unwrap(),
        segment
    );
}

#[test]
fn tlcx07_readonly_runtime_applies_lower_chain_limits_before_decode_work() {
    use lc631_runtime::journal::BundleLiveError;
    let (root, segment) = populated_root();
    let (_broker, _run, mut reservation) =
        bounded_resources(12, false, 10 * 1024 * 1024, 10 * 1024 * 1024);
    {
        let view = reservation.lease();
        let lower = JournalChainLimits::checked(1, segment.len() - 1).unwrap();
        assert!(matches!(
            inspect_bundle_root(&root, limits(), &view, lower),
            Err(BundleInspectionError::Validation(
                BundleLiveError::ChainLimit
            ))
        ));
    }
    reservation.release_confirmed().unwrap();
}
