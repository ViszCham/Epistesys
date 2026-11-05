//! Actual child plus a single cleanup fault; not a host/provider positive.
use super::*;
use lc631_substrate::{
    resource::{Cancellation, Deadline, ReportedResourceLease, ResourceLeaseIssuer},
    Digest32,
};
use std::{
    ffi::OsString,
    io::Write,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[test]
fn tlcx31_partial_child_fixture() {
    let args = std::env::args().collect::<Vec<_>>();
    if let Some(marker) = args
        .windows(2)
        .find(|a| a[0] == "--skip" && a[1].ends_with(".partial-marker"))
    {
        std::fs::write(&marker[1], b"started").unwrap();
        let mut out = std::io::stdout().lock();
        out.write_all(&[b'P'; 16384]).unwrap();
        out.flush().unwrap();
        std::thread::sleep(Duration::from_secs(2));
    }
}
#[test]
fn tlcx31_after_spawn_cleanup_fault_keeps_actual_partial_owner() {
    let marker = std::env::temp_dir().join(format!(
        "tlcx31-{}-{}.partial-marker",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let issuer = ResourceLeaseIssuer::new().unwrap();
    let report = ReportedResourceLease::checked(
        1,
        Digest32::from_bytes([31; 32]),
        &[
            resource(ResourceKind::Ram, 8 * 1024 * 1024).unwrap(),
            resource(ResourceKind::Output, 8192).unwrap(),
            resource(ResourceKind::Child, 1).unwrap(),
            resource(ResourceKind::Cleanup, 1).unwrap(),
        ],
        Deadline::after(Duration::from_secs(10)).unwrap(),
        Cancellation::default(),
    )
    .unwrap();
    // This is a real issuer-owned lease fixture. It is not a runtime-broker
    // aggregate-quota claim or a restored process observation.
    let lease = issuer.issue_admitted(1, report).unwrap();
    let view = lease.view();
    let exe = std::env::current_exe().unwrap();
    let cwd = std::env::current_dir().unwrap();
    let args = [
        OsString::from("--exact"),
        OsString::from("bounded::resources::attempt::tests::tlcx31_partial_child_fixture"),
        OsString::from("--skip"),
        marker.as_os_str().to_owned(),
        OsString::from("--nocapture"),
    ];
    let mut gate_calls = 0;
    let mut gate = || {
        gate_calls += 1;
        Ok::<_, ()>(())
    };
    let observers = Observers {
        terminate_tree: |_, _| Ok(false),
        ..Observers::default()
    };
    let mut attempt = run_observed_attempt(
        RunRequest {
            executable: exe.as_os_str(),
            arguments: &args,
            current_dir: &cwd,
            environment: &[],
            stdin: None,
            timeout: Duration::from_secs(3),
            stdout_limit: 4096,
            stderr_limit: 1024,
        },
        &view,
        &mut gate,
        observers,
    );
    assert_eq!(gate_calls, 1);
    assert_eq!(std::fs::read(&marker).unwrap(), b"started");
    assert_eq!(attempt.effect(), NativeProcessEffect::Completed);
    assert!(
        attempt.retains_partial(),
        "the actual child/stdin/stdout/stderr owner must survive the lower cleanup error"
    );
    assert_eq!(
        attempt.resource_error(),
        Some(&ResourceError::CleanupUnverified)
    );
    assert!(attempt.outcome().unwrap().stdout_truncated);
    assert_eq!(attempt.outcome().unwrap().stdout.len(), 4096);
    assert!(attempt
        .outcome()
        .unwrap()
        .stdout
        .windows(32)
        .any(|w| w.iter().all(|b| *b == b'P')));
    // Real cleanup is retried against the retained actual owner, never a report.
    attempt
        .try_close_partial(Duration::from_millis(250))
        .unwrap();
    assert!(!attempt.retains_partial());
    assert_eq!(
        attempt.resource_error(),
        Some(&ResourceError::CleanupUnverified),
        "cleanup cannot erase historical uncertainty"
    );
    drop(attempt);
    assert_eq!(
        lease.close_confirmed(),
        Err(ResourceError::CleanupUnverified)
    );
    eprintln!("retained actual partial marker {}", marker.display());
}
