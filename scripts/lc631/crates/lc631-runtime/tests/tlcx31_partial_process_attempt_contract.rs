//! Actual lower output-budget failure before a completed ResourceToolResult.
#![forbid(unsafe_code)]
use lc631_analysis::{run_bounded_tool_attempt, BoundedToolRequest, NativeProcessEffect};
use lc631_runtime::resource::*;
use std::{
    io::Write,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
fn q(kind: ResourceKind, n: u64) -> ResourceRequest {
    ResourceRequest::new(kind, ResourceAmount::checked(n).unwrap())
}
#[test]
fn tlcx31_lower_output_failure_retains_real_capture_and_live_ram_claim() {
    let marker = std::env::temp_dir().join(format!(
        "t31partial-{}-{}.partial-marker",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let limits = ResourceLimits::checked([16 * 1024 * 1024, 0, 8192, 0, 1, 1]).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let mut run = broker
        .begin_run(
            1,
            Digest32::from_bytes([31; 32]),
            limits,
            Deadline::after(Duration::from_secs(10)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let mut reservation = run
        .reserve(&[
            q(ResourceKind::Ram, 8 * 1024 * 1024),
            q(ResourceKind::Output, 8192),
            q(ResourceKind::Child, 1),
            q(ResourceKind::Cleanup, 1),
        ])
        .unwrap();
    {
        let view = reservation.lease();
        let exe = std::env::current_exe().unwrap();
        let args = [
            "--exact",
            "tlcx31_partial_child_fixture",
            "--skip",
            marker.to_str().unwrap(),
            "--nocapture",
        ];
        let mut calls = 0;
        let mut gate = || {
            calls += 1;
            Ok::<_, ()>(())
        };
        let attempt = run_bounded_tool_attempt(
            BoundedToolRequest {
                program: exe.to_str().unwrap(),
                args: &args,
                stdin: None,
                timeout_ms: 3000,
                stdout_limit: 4096,
                stderr_limit: 1024,
            },
            &view,
            &mut gate,
        );
        assert_eq!(calls, 1);
        assert_eq!(std::fs::read(&marker).unwrap(), b"started");
        assert_eq!(attempt.effect(), NativeProcessEffect::Completed);
        assert_eq!(
            attempt.resource_error(),
            Some(&ResourceError::Exhausted(ResourceKind::Output))
        );
        assert!(
            attempt.process().is_none(),
            "no completed metadata owner was manufactured after lower failure"
        );
        let raw = attempt.retained_run().unwrap().outcome().unwrap();
        assert!(raw.stdout_truncated);
        assert_eq!(raw.stdout.len(), 4096);
        assert!(raw.stdout.windows(32).any(|w| w.iter().all(|b| *b == b'P')));
        assert!(view
            .claim_resources(&[q(ResourceKind::Ram, 8 * 1024 * 1024)])
            .is_err());
        drop(attempt);
        let full = view
            .claim_resources(&[q(ResourceKind::Ram, 8 * 1024 * 1024)])
            .unwrap();
        drop(full);
    }
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
    assert_eq!(broker.used(ResourceKind::Ram).unwrap(), 0);
    eprintln!("retained lower-process marker {}", marker.display());
}
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
