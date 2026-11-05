use lc631_runtime::resource::{
    Cancellation, Deadline, Digest32, ResourceAmount, ResourceBroker, ResourceError, ResourceKind,
    ResourceLimits, ResourceRequest,
};
use std::time::Duration;

fn request(kind: ResourceKind, bytes: u64) -> ResourceRequest {
    ResourceRequest::new(kind, ResourceAmount::checked(bytes).unwrap())
}

#[test]
fn tlcx05_payload_sources_share_one_run_cumulative_quota_and_identity() {
    let owner = ResourceBroker::new(ResourceLimits::uniform(20).unwrap()).unwrap();
    let run = owner
        .begin_run(
            1,
            Digest32::from_bytes([1; 32]),
            ResourceLimits::uniform(6).unwrap(),
            Deadline::after(Duration::from_secs(30)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let first = run
        .reserve_for_source(
            Digest32::from_bytes([2; 32]),
            &[request(ResourceKind::Ram, 4)],
        )
        .unwrap();
    match run.reserve_for_source(
        Digest32::from_bytes([3; 32]),
        &[request(ResourceKind::Ram, 3)],
    ) {
        Err(error) => assert_eq!(error, ResourceError::Exhausted(ResourceKind::Ram)),
        Ok(_) => panic!("new payload source silently created a fresh run quota"),
    }
    assert_eq!(first.lease().run_id(), 1);
    assert_eq!(
        first.lease().source_revision(),
        Digest32::from_bytes([2; 32])
    );
    assert_eq!(owner.used(ResourceKind::Ram).unwrap(), 4);
}

#[test]
fn tlcx05_payload_scope_does_not_overwrite_the_parent_or_default_source() {
    let owner = ResourceBroker::new(ResourceLimits::uniform(20).unwrap()).unwrap();
    let parent = Digest32::from_bytes([1; 32]);
    let payload = Digest32::from_bytes([2; 32]);
    let run = owner
        .begin_run(
            1,
            parent,
            ResourceLimits::uniform(6).unwrap(),
            Deadline::after(Duration::from_secs(30)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let derived = run
        .reserve_for_source(payload, &[request(ResourceKind::Ram, 2)])
        .unwrap();
    owner
        .validate_lease(&derived.lease(), 1, payload, ResourceKind::Ram)
        .unwrap();
    assert_eq!(
        owner
            .validate_lease(&derived.lease(), 1, parent, ResourceKind::Ram)
            .unwrap_err(),
        ResourceError::SourceMismatch
    );
    let original = run.reserve(&[request(ResourceKind::Ram, 1)]).unwrap();
    assert_eq!(original.lease().run_id(), derived.lease().run_id());
    assert_eq!(original.lease().source_revision(), parent);
    owner
        .validate_lease(&original.lease(), 1, parent, ResourceKind::Ram)
        .unwrap();
}

#[test]
fn tlcx05_payload_source_reservation_inherits_the_same_cancellation() {
    let owner = ResourceBroker::new(ResourceLimits::uniform(20).unwrap()).unwrap();
    let cancellation = Cancellation::default();
    let run = owner
        .begin_run(
            1,
            Digest32::from_bytes([1; 32]),
            ResourceLimits::uniform(6).unwrap(),
            Deadline::after(Duration::from_secs(30)).unwrap(),
            cancellation.clone(),
        )
        .unwrap();
    let derived = run
        .reserve_for_source(
            Digest32::from_bytes([2; 32]),
            &[request(ResourceKind::Ram, 2)],
        )
        .unwrap();
    cancellation.cancel();
    assert_eq!(
        derived.lease().check().unwrap_err(),
        ResourceError::Cancelled
    );
    match run.reserve_for_source(
        Digest32::from_bytes([3; 32]),
        &[request(ResourceKind::Ram, 1)],
    ) {
        Err(error) => assert_eq!(error, ResourceError::Cancelled),
        Ok(_) => panic!("payload-specific source detached from task cancellation"),
    }
    assert_eq!(owner.used(ResourceKind::Ram).unwrap(), 2);
}
