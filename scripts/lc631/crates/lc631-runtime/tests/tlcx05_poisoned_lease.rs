use lc631_process_supervisor::{read_bounded_bytes, run_bounded_with_resources, RunRequest};
use lc631_runtime::resource::{
    Cancellation, Deadline, Digest32, ResourceAmount, ResourceBroker, ResourceError, ResourceKind,
    ResourceLimits, ResourceRequest,
};
use std::ffi::OsStr;
use std::io::Cursor;
use std::time::Duration;

fn reservation() -> (ResourceBroker, lc631_runtime::resource::Reservation) {
    let owner = ResourceBroker::new(ResourceLimits::uniform(131_072).unwrap()).unwrap();
    let run = owner
        .begin_run(
            1,
            Digest32::from_bytes([1; 32]),
            ResourceLimits::uniform(131_072).unwrap(),
            Deadline::after(Duration::from_secs(30)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let reservation = run
        .reserve(&[
            ResourceRequest::new(ResourceKind::Ram, ResourceAmount::checked(65_536).unwrap()),
            ResourceRequest::new(ResourceKind::Output, ResourceAmount::checked(2048).unwrap()),
            ResourceRequest::new(ResourceKind::Child, ResourceAmount::checked(1).unwrap()),
            ResourceRequest::new(ResourceKind::Cleanup, ResourceAmount::checked(1).unwrap()),
        ])
        .unwrap();
    (owner, reservation)
}

#[test]
fn tlcx05_unknown_cleanup_rejects_new_view_and_retains_root_charge() {
    let (owner, mut reservation) = reservation();
    reservation.lease().record_cleanup_unknown();
    assert_eq!(
        reservation.lease().check().unwrap_err(),
        ResourceError::CleanupUnverified
    );
    assert_eq!(
        reservation.release_confirmed().unwrap_err(),
        ResourceError::CleanupUnverified
    );
    assert_eq!(owner.used(ResourceKind::Child).unwrap(), 1);
}

#[test]
fn tlcx05_unknown_cleanup_blocks_new_buffer_before_read() {
    let (_owner, reservation) = reservation();
    reservation.lease().record_cleanup_unknown();
    let mut input = Cursor::new(b"input");
    match read_bounded_bytes(&mut input, &reservation.lease(), ResourceKind::Output) {
        Err(error) => assert_eq!(error, ResourceError::CleanupUnverified),
        Ok(_) => panic!("poisoned reservation allocated another buffer"),
    }
    assert_eq!(input.position(), 0);
}

#[test]
fn tlcx05_unknown_cleanup_blocks_another_child_before_spawn() {
    let (owner, reservation) = reservation();
    reservation.lease().record_cleanup_unknown();
    let directory = std::env::current_dir().unwrap();
    match run_bounded_with_resources(
        RunRequest {
            executable: OsStr::new("tlcx05-never-spawn-this-missing-executable"),
            arguments: &[],
            current_dir: &directory,
            environment: &[],
            stdin: None,
            timeout: Duration::from_secs(1),
            stdout_limit: 1024,
            stderr_limit: 1024,
        },
        &reservation.lease(),
    ) {
        Err(error) => assert_eq!(error, ResourceError::CleanupUnverified),
        Ok(_) => panic!("poisoned reservation started another child"),
    }
    assert_eq!(owner.used(ResourceKind::Child).unwrap(), 1);
    assert_eq!(owner.used(ResourceKind::Cleanup).unwrap(), 1);
}
