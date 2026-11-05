//! Plain CLI subprocess tests; embedding run_io remains an explicit legacy diagnostic.
use lc631_process_supervisor::{
    required_ram_bytes, run_bounded_with_resources_observed, RunRequest,
};
use lc631_runtime::resource::{
    Cancellation, Deadline, Digest32, ResourceAmount, ResourceBroker, ResourceKind, ResourceLimits,
    ResourceRequest,
};
use serde_json::Value;
use std::ffi::{OsStr, OsString};
use std::time::Duration;

struct Output {
    exit_code: Option<i32>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

fn cli(arguments: &[&str], input: &[u8]) -> Output {
    const STDOUT: usize = 64 * 1024;
    const STDERR: usize = 16 * 1024;
    let arguments: Vec<OsString> = arguments
        .iter()
        .map(|argument| OsString::from(*argument))
        .collect();
    // Caller-owned test transport only. Environment values are never logged.
    let environment: Vec<_> = std::env::vars_os().collect();
    let current_dir = std::env::current_dir().unwrap();
    let request = RunRequest {
        executable: OsStr::new(env!("CARGO_BIN_EXE_lc631")),
        arguments: &arguments,
        current_dir: &current_dir,
        environment: &environment,
        stdin: Some(input),
        timeout: Duration::from_secs(20),
        stdout_limit: STDOUT,
        stderr_limit: STDERR,
    };
    let ram = required_ram_bytes(&request).unwrap();
    let output = (STDOUT + STDERR) as u64;
    let limits = ResourceLimits::uniform(ram.max(output).max(1024 * 1024)).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let mut run = broker
        .begin_run(
            1,
            Digest32::from_bytes([34; 32]),
            limits,
            Deadline::after(Duration::from_secs(25)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let amount = |n| ResourceAmount::checked(n).unwrap();
    let mut reservation = run
        .reserve(&[
            ResourceRequest::new(ResourceKind::Ram, amount(ram)),
            ResourceRequest::new(ResourceKind::Output, amount(output)),
            ResourceRequest::new(ResourceKind::Child, amount(1)),
            ResourceRequest::new(ResourceKind::Cleanup, amount(1)),
        ])
        .unwrap();
    let captured = {
        let view = reservation.lease();
        let observed = run_bounded_with_resources_observed(request, &view).unwrap();
        let outcome = observed.outcome();
        assert!(!observed.cancelled() && !outcome.timed_out);
        assert!(!outcome.stdout_truncated && !outcome.stderr_truncated);
        assert!(
            observed.cleanup_complete(),
            "supervisor did not observe complete process/pipe cleanup"
        );
        // Bounded copies are caller-owned test artifacts, outside child claims.
        Output {
            exit_code: outcome.exit_code,
            stdout: outcome.stdout.clone(),
            stderr: outcome.stderr.clone(),
        }
    };
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
    captured
}
fn summary(output: &Output) -> Value {
    assert!(
        output.exit_code == Some(0),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.len() <= 64 * 1024);
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["schema_version"], "epistesys-canonical-summary.v1");
    for key in [
        "implementation_complete",
        "authority_created",
        "host_delivery_observed",
    ] {
        assert_eq!(value[key], false);
    }
    assert!(value.get("pipeline").is_none());
    assert!(value.get("payload").is_none());
    assert!(value.get("recommendation").is_none());
    assert_eq!(value.as_object().unwrap().len(), 9);
    value
}
fn errors(output: &Output) -> String {
    assert!(output.exit_code.is_some_and(|code| code != 0));
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!text.contains("epistesys-canonical-summary.v1"));
    assert!(!text.contains("\"pipeline\""));
    text
}

#[test]
fn tlcx34_production_cli_uses_actual_preparation_clarify_without_legacy_placeholder() {
    let output = cli(
        &[
            "epistesys-solve",
            "--prompt",
            "Please verify the result if ready.",
            "--task",
            "coding",
        ],
        &[],
    );
    let value = summary(&output);
    assert_eq!(value["status"], "clarify");
    assert!(value["missing_stage"].is_null());
    assert_eq!(value["task"], "coding");
    assert!(!String::from_utf8_lossy(&output.stdout).contains("context_lowering"));
}

#[test]
fn tlcx34_production_cli_stdin_preserves_exact_utf8_bytes_and_digest() {
    let source = "日本語🙂 exact\r\n最後\n";
    let output = cli(
        &["epistesys-solve", "--prompt-stdin", "--task", "explanation"],
        source.as_bytes(),
    );
    let value = summary(&output);
    assert_eq!(value["source_bytes"], source.len());
    assert_eq!(
        value["source_digest"],
        // Independent .NET SHA-256 over the exact 28 UTF-8 bytes, including CRLF.
        "6233a0d8ef9458d24726dad16dc543a61a68b427b3275721a48dc580131f1e61"
    );
    assert_eq!(value["task"], "explanation");
    assert!(!String::from_utf8_lossy(&output.stdout).contains("日本語"));
}

#[test]
fn tlcx34_production_cli_explicit_preparation_source_limit_never_falls_back() {
    let source = "a".repeat(16 * 1024 + 1);
    let literal = cli(&["epistesys-solve", "--prompt", &source], &[]);
    assert!(errors(&literal).contains("SourceLimit"));
    let stdin = cli(&["epistesys-solve", "--prompt-stdin"], source.as_bytes());
    assert!(errors(&stdin).contains("SourceLimit"));
}

#[test]
fn tlcx34_production_cli_invalid_utf8_stdin_is_an_error_without_fallback() {
    let output = cli(&["epistesys-solve", "--prompt-stdin"], &[0xff, 0xfe]);
    assert!(errors(&output).contains("InvalidUtf8"));
}

#[test]
fn tlcx34_production_cli_accepts_exact_preparation_boundary_without_dump() {
    let source = "a".repeat(16 * 1024);
    let output = cli(&["epistesys-solve", "--prompt-stdin"], source.as_bytes());
    let value = summary(&output);
    assert_eq!(value["source_bytes"], 16 * 1024);
    assert_eq!(value["status"], "clarify");
    assert!(value["missing_stage"].is_null());
}

#[test]
fn tlcx34_production_cli_inspect_and_resume_keep_missing_store_capability() {
    for command in ["epistesys-inspect", "epistesys-resume"] {
        let output = cli(&[command, "--artifact", "local:decision-1"], &[]);
        let value = summary(&output);
        assert_eq!(value["status"], "not_connected");
        assert_eq!(value["missing_stage"], "artifact_store_capability");
        assert!(value["source_bytes"].is_null());
        assert!(value["source_digest"].is_null());
    }
}
