//! New resource route rejects inadmissible requests before resolution/hash work.
//! This does not alter the separate bounded legacy API.
use lc631_analysis::{run_bounded_tool_with_resources, BoundedToolRequest};
use lc631_core::context::SourceRevision;
use lc631_runtime::resource::{
    Cancellation, Deadline, ResourceAmount, ResourceBroker, ResourceError, ResourceKind,
    ResourceLimits, ResourceRequest,
};
use std::time::Duration;

#[test]
fn tlcx05_resource_request_limits_reject_program_argv_stdin_output_and_time_before_resolution() {
    let limits = ResourceLimits::checked([4096, 0, 0, 0, 0, 0]).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let mut run = broker
        .begin_run(
            1,
            SourceRevision::of("request preflight").digest(),
            limits,
            Deadline::after(Duration::from_secs(10)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let mut reservation = run
        .reserve(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(4096).unwrap(),
        )])
        .unwrap();
    let view = reservation.lease();
    let oversized_program = "p".repeat(4097);
    let oversized_argument = "a".repeat(1024 * 1024 + 1);
    let oversized_stdin = vec![0_u8; 2 * 1024 * 1024 + 1];
    let excess_arguments = ["x"; 129];
    for (program, args, stdin, stdout, stderr, timeout) in [
        (oversized_program.as_str(), &[][..], None, 1, 1, 100),
        ("", &[][..], None, 1, 1, 100),
        (
            "never-resolve-this-budget-rejected-tool",
            &[oversized_argument.as_str()][..],
            None,
            1,
            1,
            100,
        ),
        (
            "never-resolve-this-budget-rejected-tool",
            &excess_arguments[..],
            None,
            1,
            1,
            100,
        ),
        (
            "never-resolve-this-budget-rejected-tool",
            &[][..],
            Some(oversized_stdin.as_slice()),
            1,
            1,
            100,
        ),
        (
            "never-resolve-this-budget-rejected-tool",
            &[][..],
            None,
            2 * 1024 * 1024 + 5,
            1,
            100,
        ),
        (
            "never-resolve-this-budget-rejected-tool",
            &[][..],
            None,
            1,
            2 * 1024 * 1024 + 5,
            100,
        ),
        (
            "never-resolve-this-budget-rejected-tool",
            &[][..],
            None,
            1,
            1,
            0,
        ),
        (
            "never-resolve-this-budget-rejected-tool",
            &[][..],
            None,
            1,
            1,
            120_001,
        ),
    ] {
        assert!(
            matches!(
                run_bounded_tool_with_resources(
                    BoundedToolRequest {
                        program,
                        args,
                        stdin,
                        timeout_ms: timeout,
                        stdout_limit: stdout,
                        stderr_limit: stderr,
                    },
                    &view
                ),
                Err(ResourceError::InvalidConfig)
            ),
            "inadmissible request reached a later state"
        );
    }
    assert_eq!(broker.used(ResourceKind::Child).unwrap(), 0);
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}
