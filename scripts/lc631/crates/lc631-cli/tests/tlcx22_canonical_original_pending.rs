//! Actual bounded normal CLI process: missing original inputs must remain visible.
use lc631_process_supervisor::{
    required_ram_bytes, run_bounded_with_resources_observed, RunRequest,
};
use lc631_runtime::resource::{
    Cancellation, Deadline, Digest32, ResourceAmount, ResourceBroker, ResourceKind, ResourceLimits,
    ResourceRequest,
};
use std::{
    ffi::{OsStr, OsString},
    time::Duration,
};

fn cli(source: &str) -> String {
    let args: Vec<OsString> = [
        "epistesys-solve",
        "--input-format",
        "smtlib-qf-lia-v1",
        "--prompt-stdin",
    ]
    .into_iter()
    .map(OsString::from)
    .collect();
    let environment: Vec<_> = std::env::vars_os().collect();
    let cwd = std::env::current_dir().unwrap();
    let request = RunRequest {
        executable: OsStr::new(env!("CARGO_BIN_EXE_lc631")),
        arguments: &args,
        current_dir: &cwd,
        environment: &environment,
        stdin: Some(source.as_bytes()),
        timeout: Duration::from_secs(20),
        stdout_limit: 65536,
        stderr_limit: 16384,
    };
    let ram = required_ram_bytes(&request).unwrap();
    let limits = ResourceLimits::uniform(ram.max(1024 * 1024)).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let mut run = broker
        .begin_run(
            22,
            Digest32::from_bytes([22; 32]),
            limits,
            Deadline::after(Duration::from_secs(25)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let amount = |n| ResourceAmount::checked(n).unwrap();
    let mut reservation = run
        .reserve(&[
            ResourceRequest::new(ResourceKind::Ram, amount(ram)),
            ResourceRequest::new(ResourceKind::Output, amount(81920)),
            ResourceRequest::new(ResourceKind::Child, amount(1)),
            ResourceRequest::new(ResourceKind::Cleanup, amount(1)),
        ])
        .unwrap();
    let text = {
        let view = reservation.lease();
        let observed = run_bounded_with_resources_observed(request, &view).unwrap();
        let output = observed.outcome();
        assert!(!observed.cancelled() && !output.timed_out);
        assert!(!output.stdout_truncated && !output.stderr_truncated);
        assert!(observed.cleanup_complete());
        assert_eq!(
            output.exit_code,
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout.clone()).unwrap()
    };
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
    text
}

#[test]
fn tlcx22_normal_cli_reports_original_missing_without_default_score() {
    for source in [
        "(set-logic QF_LIA)\n(declare-const x Int)\n(assert (= x 3))\n(check-sat)\n",
        "An unsupported natural-language question",
    ] {
        let text = cli(source);
        assert!(text.contains("original F1/F2: NoScalar; profile: Missing; total TL owner: Missing; recommendation: None"), "{text}");
        assert!(text.contains("NoRecommendation"), "{text}");
        assert!(!text.contains("original_score=0"));
    }
}
