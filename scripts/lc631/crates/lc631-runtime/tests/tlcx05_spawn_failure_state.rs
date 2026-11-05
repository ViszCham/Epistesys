use lc631_process_supervisor::{required_ram_bytes, run_bounded_with_resources, RunRequest};
use lc631_runtime::resource::{
    Cancellation, Deadline, Digest32, ResourceAmount, ResourceBroker, ResourceError, ResourceKind,
    ResourceLimits, ResourceRequest,
};
use std::ffi::OsStr;
use std::io;
use std::time::Duration;

#[test]
fn tlcx05_missing_executable_is_visible_not_started_not_unknown_child_cleanup() {
    let directory = std::env::current_dir().unwrap();
    let request = RunRequest {
        executable: OsStr::new("tlcx05-missing-executable-never-created"),
        arguments: &[],
        current_dir: &directory,
        environment: &[],
        stdin: None,
        timeout: Duration::from_secs(1),
        stdout_limit: 8,
        stderr_limit: 8,
    };
    let owner = ResourceBroker::new(ResourceLimits::uniform(262_144).unwrap()).unwrap();
    let run = owner
        .begin_run(
            1,
            Digest32::from_bytes([1; 32]),
            ResourceLimits::uniform(262_144).unwrap(),
            Deadline::after(Duration::from_secs(30)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let mut reservation = run
        .reserve(&[
            ResourceRequest::new(
                ResourceKind::Ram,
                ResourceAmount::checked(required_ram_bytes(&request).unwrap()).unwrap(),
            ),
            ResourceRequest::new(ResourceKind::Output, ResourceAmount::checked(16).unwrap()),
            ResourceRequest::new(ResourceKind::Child, ResourceAmount::checked(1).unwrap()),
            ResourceRequest::new(ResourceKind::Cleanup, ResourceAmount::checked(1).unwrap()),
        ])
        .unwrap();
    {
        let view = reservation.lease();
        match run_bounded_with_resources(request, &view) {
            Err(error) => assert_eq!(error, ResourceError::Io(io::ErrorKind::NotFound)),
            Ok(_) => panic!("missing executable was reported as an executed child"),
        };
    }
    // Failure never auto-refunds; because no child existed, the owner can
    // explicitly confirm unused-reservation reclamation without a false Hold.
    assert_eq!(owner.used(ResourceKind::Child).unwrap(), 1);
    reservation.release_confirmed().unwrap();
    assert_eq!(owner.used(ResourceKind::Child).unwrap(), 0);
}
