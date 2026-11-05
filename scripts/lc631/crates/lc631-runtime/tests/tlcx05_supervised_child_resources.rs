use lc631_process_supervisor::{run_bounded_with_resources, RunRequest};
use lc631_runtime::resource::{
    Cancellation, Deadline, Digest32, ResourceAmount, ResourceBroker, ResourceError, ResourceKind,
    ResourceLimits, ResourceRequest,
};
use std::ffi::OsString;
use std::time::Duration;

fn reservation(
    cancellation: Cancellation,
) -> (ResourceBroker, lc631_runtime::resource::Reservation) {
    let owner = ResourceBroker::new(ResourceLimits::uniform(131_072).unwrap()).unwrap();
    let run = owner
        .begin_run(
            1,
            Digest32::from_bytes([9; 32]),
            ResourceLimits::uniform(131_072).unwrap(),
            Deadline::after(Duration::from_secs(30)).unwrap(),
            cancellation,
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

fn arguments() -> [OsString; 3] {
    [
        "--exact".into(),
        "tlcx05_owned_child_helper".into(),
        "--nocapture".into(),
    ]
}

#[test]
fn tlcx05_child_rejects_cancel_before_spawn_and_keeps_broker_charge() {
    let cancellation = Cancellation::default();
    let (owner, reservation) = reservation(cancellation.clone());
    cancellation.cancel();
    let executable = std::env::current_exe().unwrap();
    let directory = std::env::current_dir().unwrap();
    let arguments = arguments();
    match run_bounded_with_resources(
        RunRequest {
            executable: executable.as_os_str(),
            arguments: &arguments,
            current_dir: &directory,
            environment: &[],
            stdin: None,
            timeout: Duration::from_secs(2),
            stdout_limit: 1024,
            stderr_limit: 1024,
        },
        &reservation.lease(),
    ) {
        Err(error) => assert_eq!(error, ResourceError::Cancelled),
        Ok(_) => panic!("cancelled request spawned a child"),
    }
    assert_eq!(owner.used(ResourceKind::Child).unwrap(), 1);
}

#[test]
fn tlcx05_actual_child_observes_cleanup_before_explicit_reservation_refund() {
    let (owner, mut reservation) = reservation(Cancellation::default());
    let executable = std::env::current_exe().unwrap();
    let directory = std::env::current_dir().unwrap();
    let arguments = arguments();
    let environment = [(
        OsString::from("TLCX05_CHILD_MODE"),
        OsString::from("normal"),
    )];
    {
        let view = reservation.lease();
        let result = run_bounded_with_resources(
            RunRequest {
                executable: executable.as_os_str(),
                arguments: &arguments,
                current_dir: &directory,
                environment: &environment,
                stdin: None,
                timeout: Duration::from_secs(2),
                stdout_limit: 1024,
                stderr_limit: 1024,
            },
            &view,
        )
        .unwrap();
        assert_eq!(result.outcome().exit_code, Some(0));
        assert!(result.cleanup_complete());
        assert!(!result.cancelled());
        assert_eq!(owner.used(ResourceKind::Child).unwrap(), 1);
    }
    assert_eq!(owner.used(ResourceKind::Child).unwrap(), 1);
    reservation.release_confirmed().unwrap();
    assert_eq!(owner.used(ResourceKind::Child).unwrap(), 0);
}

#[test]
fn tlcx05_child_deadline_does_not_implicitly_refund_root_reservation() {
    let (owner, reservation) = reservation(Cancellation::default());
    let executable = std::env::current_exe().unwrap();
    let directory = std::env::current_dir().unwrap();
    let arguments = arguments();
    let environment = [(OsString::from("TLCX05_CHILD_MODE"), OsString::from("sleep"))];
    {
        let view = reservation.lease();
        let result = run_bounded_with_resources(
            RunRequest {
                executable: executable.as_os_str(),
                arguments: &arguments,
                current_dir: &directory,
                environment: &environment,
                stdin: None,
                timeout: Duration::from_millis(150),
                stdout_limit: 1024,
                stderr_limit: 1024,
            },
            &view,
        )
        .unwrap();
        assert!(result.outcome().timed_out);
    }
    assert_eq!(owner.used(ResourceKind::Child).unwrap(), 1);
    assert_eq!(owner.used(ResourceKind::Cleanup).unwrap(), 1);
    drop(reservation);
    assert_eq!(owner.used(ResourceKind::Child).unwrap(), 1);
}

#[test]
fn tlcx05_child_mid_run_cancel_is_observed_without_automatic_refund() {
    let cancellation = Cancellation::default();
    let (owner, reservation) = reservation(cancellation.clone());
    let cancel = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        cancellation.cancel();
    });
    let executable = std::env::current_exe().unwrap();
    let directory = std::env::current_dir().unwrap();
    let arguments = arguments();
    let environment = [(OsString::from("TLCX05_CHILD_MODE"), OsString::from("sleep"))];
    {
        let view = reservation.lease();
        let result = run_bounded_with_resources(
            RunRequest {
                executable: executable.as_os_str(),
                arguments: &arguments,
                current_dir: &directory,
                environment: &environment,
                stdin: None,
                timeout: Duration::from_secs(3),
                stdout_limit: 1024,
                stderr_limit: 1024,
            },
            &view,
        )
        .unwrap();
        assert!(result.cancelled());
    }
    cancel.join().unwrap();
    assert_eq!(owner.used(ResourceKind::Child).unwrap(), 1);
    assert_eq!(owner.used(ResourceKind::Cleanup).unwrap(), 1);
}

#[test]
fn tlcx05_owned_child_helper() {
    match std::env::var("TLCX05_CHILD_MODE").as_deref() {
        Ok("normal") => println!("owned child observed"),
        Ok("sleep") => std::thread::sleep(Duration::from_secs(5)),
        _ => (),
    }
}
