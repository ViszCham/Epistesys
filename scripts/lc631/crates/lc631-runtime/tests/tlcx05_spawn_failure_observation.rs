use lc631_process_supervisor::{
    required_ram_bytes, run_bounded_with_resources_observed, ResourceRunError, RunRequest,
};
use lc631_runtime::resource::{
    Cancellation, Deadline, Digest32, ResourceAmount, ResourceBroker, ResourceKind, ResourceLimits,
    ResourceRequest,
};
use std::io;
use std::path::PathBuf;
use std::time::Duration;

#[test]
fn tlcx05_detailed_pre_child_marker_reports_not_started_without_error_kind_inference() {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let missing = directory.join("tlcx05-test-executable-that-does-not-exist");
    assert!(!missing.exists());
    let request = RunRequest {
        executable: missing.as_os_str(),
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
        match run_bounded_with_resources_observed(request, &view) {
            Err(ResourceRunError::NotStarted(kind)) => assert_eq!(kind, io::ErrorKind::NotFound),
            Err(other) => panic!("wrong spawn-stage observation: {other:?}"),
            Ok(_) => panic!("missing executable became an executed observation"),
        };
    }
    reservation.release_confirmed().unwrap();
    assert_eq!(owner.used(ResourceKind::Child).unwrap(), 0);
}
