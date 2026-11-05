use lc631_runtime::resource::{
    Cancellation, Deadline, Digest32, ResourceAmount, ResourceBroker, ResourceError, ResourceKind,
    ResourceLimits, ResourceRequest,
};
use std::time::Duration;

fn begin(owner: &ResourceBroker, id: u64) -> lc631_runtime::resource::RunResources {
    owner
        .begin_run(
            id,
            Digest32::from_bytes([1; 32]),
            ResourceLimits::uniform(8).unwrap(),
            Deadline::after(Duration::from_secs(60)).unwrap(),
            Cancellation::default(),
        )
        .unwrap()
}

#[test]
fn tlcx05_confirmed_run_retirement_does_not_accumulate_1024_history_records() {
    let owner = ResourceBroker::new(ResourceLimits::uniform(8).unwrap()).unwrap();
    for id in 1..=1_100 {
        let mut run = begin(&owner, id);
        run.close_confirmed().unwrap();
        assert_eq!(
            run.close_confirmed().unwrap_err(),
            ResourceError::AlreadyReleased
        );
    }
    match owner.begin_run(
        1,
        Digest32::from_bytes([1; 32]),
        ResourceLimits::uniform(8).unwrap(),
        Deadline::after(Duration::from_secs(60)).unwrap(),
        Cancellation::default(),
    ) {
        Err(error) => assert_eq!(error, ResourceError::RunIdReused),
        Ok(_) => panic!("retired run ID was reused"),
    }
}

#[test]
fn tlcx05_run_cannot_retire_with_active_or_unknown_cleanup_reservations() {
    let owner = ResourceBroker::new(ResourceLimits::uniform(8).unwrap()).unwrap();
    let mut run = begin(&owner, 1);
    let mut live = run
        .reserve(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(3).unwrap(),
        )])
        .unwrap();
    assert_eq!(
        run.close_confirmed().unwrap_err(),
        ResourceError::CleanupUnverified
    );
    live.lease().record_cleanup_unknown();
    assert_eq!(
        live.release_confirmed().unwrap_err(),
        ResourceError::CleanupUnverified
    );
    drop(live);
    assert_eq!(
        run.close_confirmed().unwrap_err(),
        ResourceError::CleanupUnverified
    );
    assert_eq!(owner.used(ResourceKind::Ram).unwrap(), 3);
}

#[test]
fn tlcx05_retired_run_handle_cannot_create_new_reservations() {
    let owner = ResourceBroker::new(ResourceLimits::uniform(8).unwrap()).unwrap();
    let mut run = begin(&owner, 1);
    let mut reservation = run
        .reserve(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(3).unwrap(),
        )])
        .unwrap();
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
    match run.reserve(&[ResourceRequest::new(
        ResourceKind::Ram,
        ResourceAmount::checked(1).unwrap(),
    )]) {
        Err(error) => assert_eq!(error, ResourceError::UnknownRun),
        Ok(_) => panic!("closed run issued a new reservation"),
    }
}
