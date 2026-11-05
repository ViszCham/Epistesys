use lc631_process_supervisor::AuthenticatedSourceSnapshot;
use lc631_runtime::resource::{
    Cancellation, Deadline, Digest32, ResourceAmount, ResourceBroker, ResourceError, ResourceKind,
    ResourceLimits, ResourceRequest,
};
use std::path::PathBuf;
use std::time::Duration;

fn input() -> (PathBuf, String) {
    // Read an existing small owned-repo fixture; never modify the source file.
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let bytes = std::fs::read_to_string(&path).unwrap();
    let digest = lc631_core::stable_sha256(&bytes);
    (path, digest)
}

fn setup(
    include_disk: bool,
    cancel: Cancellation,
    expected_sha256: &str,
) -> (ResourceBroker, lc631_runtime::resource::Reservation) {
    let owner = ResourceBroker::new(ResourceLimits::uniform(131_072).unwrap()).unwrap();
    let run = owner
        .begin_run(
            1,
            source_scope(expected_sha256),
            ResourceLimits::uniform(131_072).unwrap(),
            Deadline::after(Duration::from_secs(30)).unwrap(),
            cancel,
        )
        .unwrap();
    let mut requests = vec![
        ResourceRequest::new(ResourceKind::Ram, ResourceAmount::checked(65_536).unwrap()),
        ResourceRequest::new(ResourceKind::Cleanup, ResourceAmount::checked(1).unwrap()),
    ];
    if include_disk {
        requests.push(ResourceRequest::new(
            ResourceKind::Disk,
            ResourceAmount::checked(8192).unwrap(),
        ));
    }
    let reservation = run.reserve(&requests).unwrap();
    (owner, reservation)
}

fn source_scope(sha256: &str) -> Digest32 {
    let mut bytes = [0; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&sha256[7 + index * 2..9 + index * 2], 16).unwrap();
    }
    Digest32::from_bytes(bytes)
}

#[test]
fn tlcx05_snapshot_missing_common_disk_reservation_rejects_before_capture() {
    let (source, digest) = input();
    let (_owner, reservation) = setup(false, Cancellation::default(), &digest);
    let view = reservation.lease();
    match AuthenticatedSourceSnapshot::capture_with_resources(&source, &digest, 8192, 8192, &view) {
        Err(error) => assert_eq!(error, ResourceError::Unreserved(ResourceKind::Disk)),
        Ok(_) => panic!("snapshot bypassed common-root Disk admission"),
    };
}

#[test]
fn tlcx05_snapshot_retains_common_charge_until_actual_close_and_explicit_refund() {
    let (source, digest) = input();
    let original = std::fs::read(&source).unwrap();
    let (owner, mut reservation) = setup(true, Cancellation::default(), &digest);
    let snapshot_path;
    {
        let lease = reservation.lease();
        let snapshot = AuthenticatedSourceSnapshot::capture_with_resources(
            &source, &digest, 8192, 8192, &lease,
        )
        .unwrap();
        snapshot_path = snapshot.path().to_path_buf();
        assert_eq!(std::fs::read(snapshot.path()).unwrap(), original);
        assert_eq!(owner.used(ResourceKind::Disk).unwrap(), 8192);
        snapshot.close().unwrap();
        assert!(!snapshot_path.exists());
    }
    assert_eq!(owner.used(ResourceKind::Disk).unwrap(), 8192);
    reservation.release_confirmed().unwrap();
    assert_eq!(owner.used(ResourceKind::Disk).unwrap(), 0);
    assert_eq!(std::fs::read(&source).unwrap(), original);
}

#[test]
fn tlcx05_snapshot_cancel_does_not_create_or_refund_without_owner_confirmation() {
    let (source, digest) = input();
    let cancellation = Cancellation::default();
    let (owner, reservation) = setup(true, cancellation.clone(), &digest);
    cancellation.cancel();
    match AuthenticatedSourceSnapshot::capture_with_resources(
        &source,
        &digest,
        8192,
        8192,
        &reservation.lease(),
    ) {
        Err(error) => assert_eq!(error, ResourceError::Cancelled),
        Ok(_) => panic!("cancelled snapshot was captured"),
    }
    assert_eq!(owner.used(ResourceKind::Disk).unwrap(), 8192);
}

#[test]
fn tlcx05_snapshot_rejects_a_view_for_another_source_before_capture() {
    let (source, digest) = input();
    let wrong_source = format!("sha256:{}", "0".repeat(64));
    let (owner, reservation) = setup(true, Cancellation::default(), &wrong_source);
    let view = reservation.lease();
    match AuthenticatedSourceSnapshot::capture_with_resources(&source, &digest, 8192, 8192, &view) {
        Err(error) => assert_eq!(error, ResourceError::SourceMismatch),
        Ok(_) => panic!("a different source's reservation captured this file"),
    };
    assert_eq!(owner.used(ResourceKind::Disk).unwrap(), 8192);
}
