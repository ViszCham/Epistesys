use lc631_host::persistent_owner::{
    HostPersistentStoreOwner, PersistentStoreKind, PersistentStoreLimits,
};
use lc631_runtime::resource::{
    Cancellation, Deadline, Digest32, Reservation, ResourceAmount, ResourceBroker, ResourceKind,
    ResourceLimits, ResourceRequest, RunResources,
};
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn fixture() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "epistesys-tlcx05-store-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir(&root).unwrap();
    root
}

fn reservation() -> (ResourceBroker, RunResources, Reservation) {
    let broker = ResourceBroker::new(ResourceLimits::uniform(64 * 1024 * 1024).unwrap()).unwrap();
    let run = broker
        .begin_run(
            1,
            Digest32::from_bytes([6; 32]),
            ResourceLimits::uniform(64 * 1024 * 1024).unwrap(),
            Deadline::after(Duration::from_secs(30)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let request =
        |kind, amount| ResourceRequest::new(kind, ResourceAmount::checked(amount).unwrap());
    let reservation = run
        .reserve(&[
            request(ResourceKind::Ram, 17 * 1024 * 1024),
            request(ResourceKind::Output, 8 * 1024 * 1024),
            request(ResourceKind::Disk, 1_048_576),
        ])
        .unwrap();
    (broker, run, reservation)
}

#[test]
fn tlcx05_persistent_owner_rejects_unknown_root_entries_and_outside_targets() {
    let root = fixture();
    std::fs::write(root.join("unknown.bin"), b"unknown").unwrap();
    let (_broker, _run, reservation) = reservation();
    let view = reservation.lease();
    assert!(HostPersistentStoreOwner::begin(
        &root,
        &root.join("receipt-root-v1.key"),
        PersistentStoreKind::ReceiptKey,
        65,
        PersistentStoreLimits::default(),
        &view
    )
    .is_err());
    assert!(!root.join(".resource-owner-v1.lock").exists());
    let clean = fixture();
    assert!(HostPersistentStoreOwner::begin(
        &clean,
        &root.join("receipt-root-v1.key"),
        PersistentStoreKind::ReceiptKey,
        65,
        PersistentStoreLimits::default(),
        &view
    )
    .is_err());
    assert!(!clean.join(".resource-owner-v1.lock").exists());
}

#[test]
fn tlcx05_persistent_owner_serializes_same_root_and_reconstructs_durable_usage() {
    let root = fixture();
    let target = root.join("receipt-root-v1.key");
    let (_broker, _run, mut reservation) = reservation();
    {
        let view = reservation.lease();
        let mut owner = HostPersistentStoreOwner::begin(
            &root,
            &target,
            PersistentStoreKind::ReceiptKey,
            65,
            PersistentStoreLimits::default(),
            &view,
        )
        .unwrap();
        assert!(HostPersistentStoreOwner::begin(
            &root,
            &target,
            PersistentStoreKind::ReceiptKey,
            65,
            PersistentStoreLimits::default(),
            &view
        )
        .is_err());
        let bytes = format!("{}\n", "a".repeat(64));
        owner.before_write(bytes.len() as u64).unwrap();
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
            .unwrap();
        file.write_all(bytes.as_bytes()).unwrap();
        file.sync_all().unwrap();
        drop(file);
        owner
            .confirm_handoff(&lc631_core::stable_sha256(&bytes))
            .unwrap();
        assert_eq!(owner.usage().total_bytes, 65);
    }
    reservation.release_confirmed().unwrap();
    assert!(
        target.exists(),
        "runtime refund follows persistent-owner handoff, not deletion"
    );
    let (_broker, _run, next) = self::reservation();
    let view = next.lease();
    let owner = HostPersistentStoreOwner::begin(
        &root,
        &target,
        PersistentStoreKind::ReceiptKey,
        65,
        PersistentStoreLimits::default(),
        &view,
    )
    .unwrap();
    assert_eq!(owner.usage().total_bytes, 65);
}

#[test]
fn tlcx05_persistent_owner_rejects_corruption_and_root_caps() {
    let root = fixture();
    let target = root.join("receipt-root-v1.key");
    std::fs::write(&target, b"not a valid key").unwrap();
    let (_broker, _run, reservation) = reservation();
    let view = reservation.lease();
    assert!(HostPersistentStoreOwner::begin(
        &root,
        &target,
        PersistentStoreKind::ReceiptKey,
        65,
        PersistentStoreLimits::default(),
        &view
    )
    .is_err());
    let root = fixture();
    let target = root.join("receipt-root-v1.key");
    std::fs::write(&target, format!("{}\n", "a".repeat(64))).unwrap();
    let limits = PersistentStoreLimits {
        max_files: 1,
        max_total_bytes: 32,
        max_depth: 4,
    };
    assert!(HostPersistentStoreOwner::begin(
        &root,
        &target,
        PersistentStoreKind::ReceiptKey,
        65,
        limits,
        &view
    )
    .is_err());
}

#[test]
fn tlcx05_uncertain_persistent_handoff_never_refunds_disk() {
    let root = fixture();
    let target = root.join("receipt-root-v1.key");
    let (broker, _run, mut reservation) = reservation();
    {
        let view = reservation.lease();
        let mut owner = HostPersistentStoreOwner::begin(
            &root,
            &target,
            PersistentStoreKind::ReceiptKey,
            65,
            PersistentStoreLimits::default(),
            &view,
        )
        .unwrap();
        owner.before_write(65).unwrap();
        std::fs::write(&target, b"partial").unwrap();
        assert!(owner
            .confirm_handoff(&lc631_core::stable_sha256(&format!("{}\n", "a".repeat(64))))
            .is_err());
    }
    assert!(reservation.release_confirmed().is_err());
    assert_eq!(broker.used(ResourceKind::Disk).unwrap(), 1_048_576);
}

#[cfg(unix)]
#[test]
fn tlcx05_persistent_owner_does_not_follow_a_symlink() {
    let root = fixture();
    let outside = fixture().join("outside-key");
    std::fs::write(&outside, "a".repeat(64)).unwrap();
    let target = root.join("receipt-root-v1.key");
    std::os::unix::fs::symlink(&outside, &target).unwrap();
    let (_broker, _run, reservation) = reservation();
    let view = reservation.lease();
    assert!(HostPersistentStoreOwner::begin(
        &root,
        &target,
        PersistentStoreKind::ReceiptKey,
        65,
        PersistentStoreLimits::default(),
        &view
    )
    .is_err());
    assert_eq!(std::fs::read(&outside).unwrap(), vec![b'a'; 64]);
}
