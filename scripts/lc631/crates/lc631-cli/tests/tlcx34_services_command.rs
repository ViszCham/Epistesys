//! Actual bounded CLI processes. Native positives require explicitly selected Z3.
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
const FORMAL: &str =
    "(set-logic QF_LIA)\n(declare-const x Int)\n(assert (= x (- 3)))\n(check-sat)\n";
fn cli(args: &[&str], input: &[u8]) -> (Option<i32>, String, String) {
    let args: Vec<OsString> = args.iter().map(|a| OsString::from(*a)).collect();
    let environment: Vec<_> = std::env::vars_os().collect();
    let cwd = std::env::current_dir().unwrap();
    let request = RunRequest {
        executable: OsStr::new(env!("CARGO_BIN_EXE_lc631")),
        arguments: &args,
        current_dir: &cwd,
        environment: &environment,
        stdin: Some(input),
        timeout: Duration::from_secs(20),
        stdout_limit: 65536,
        stderr_limit: 16384,
    };
    let ram = required_ram_bytes(&request).unwrap();
    let limits = ResourceLimits::uniform(ram.max(1024 * 1024)).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let mut run = broker
        .begin_run(
            34,
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
            ResourceRequest::new(ResourceKind::Output, amount(81920)),
            ResourceRequest::new(ResourceKind::Child, amount(1)),
            ResourceRequest::new(ResourceKind::Cleanup, amount(1)),
        ])
        .unwrap();
    let result = {
        let view = reservation.lease();
        let observed = run_bounded_with_resources_observed(request, &view).unwrap();
        let output = observed.outcome();
        assert!(!observed.cancelled() && !output.timed_out);
        assert!(!output.stdout_truncated && !output.stderr_truncated);
        assert!(observed.cleanup_complete(), "process cleanup unobserved");
        (
            output.exit_code,
            String::from_utf8(output.stdout.clone()).unwrap(),
            String::from_utf8(output.stderr.clone()).unwrap(),
        )
    };
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
    result
}
#[test]
fn tlcx34_services_command_missing_registry_is_actual_unavailable_without_legacy_view() {
    let (exit, text, error) = cli(
        &[
            "epistesys-solve",
            "--input-format",
            "smtlib-qf-lia-v1",
            "--prompt-stdin",
        ],
        FORMAL.as_bytes(),
    );
    assert_eq!(exit, Some(0), "{error}");
    assert!(
        text.contains("NoRecommendation") && text.contains("MissingBackend"),
        "{text}"
    );
    assert!(!text.contains("epistesys-canonical-summary.v1") && !text.contains("\"pipeline\""));
}
#[test]
fn tlcx34_services_command_unsupported_formal_is_clarify_and_never_a_model() {
    let (exit, text, error) = cli(
        &[
            "epistesys-solve",
            "--input-format",
            "smtlib-qf-lia-v1",
            "--prompt-stdin",
        ],
        b"Please solve this natural-language question.",
    );
    assert_eq!(exit, Some(0), "{error}");
    assert!(text.contains("Clarify"), "{text}");
    assert!(text.contains("NoRecommendation") && !text.contains("assignments:"));
}
#[test]
fn tlcx34_services_command_rejects_relative_registry_and_unmatched_declared_goal() {
    let (exit, _, error) = cli(
        &[
            "epistesys-solve",
            "--input-format",
            "smtlib-qf-lia-v1",
            "--solver-registry",
            "relative.json",
            "--prompt",
            FORMAL,
        ],
        b"",
    );
    assert!(exit.is_some_and(|c| c != 0));
    assert!(error.contains("registry"), "{error}");
    let (exit, _, error) = cli(
        &[
            "epistesys-solve",
            "--input-format",
            "smtlib-qf-lia-v1",
            "--goal-policy",
            "declared-source-objective",
            "--prompt",
            FORMAL,
        ],
        b"",
    );
    assert!(exit.is_some_and(|c| c != 0));
    assert!(error.contains("GoalConflict"), "{error}");
}
#[test]
fn tlcx34_services_command_utf8_and_source_cap_fail_without_fallback() {
    for source in [vec![0xff, 0xfe], vec![b'a'; 16385]] {
        let (exit, text, _) = cli(
            &[
                "epistesys-solve",
                "--input-format",
                "smtlib-qf-lia-v1",
                "--prompt-stdin",
            ],
            &source,
        );
        assert!(exit.is_some_and(|c| c != 0));
        assert!(!text.contains("epistesys-canonical-summary.v1"));
    }
}

#[test]
#[ignore = "explicit native Z3 test; requires EPISTESYS_TLCX10_NATIVE_Z3 and the registered OS binary"]
fn tlcx34_services_command_real_z3_retains_conditional_model_and_no_recommendation() {
    let executable = std::env::var("EPISTESYS_TLCX10_NATIVE_Z3").expect("native path required");
    assert!(std::path::Path::new(&executable).is_absolute());
    let pin = if cfg!(windows) {
        "5f32b2d2b93291b56e3c10a43b088cae78acc84017ebff8fa741a7ecee6c6ca5"
    } else if cfg!(target_os = "linux") {
        "c45e787b5d1a3d94079d1e83b98e23bbc884ccaf542ea38c3814f31ac2f79197"
    } else {
        panic!("no native OS pin")
    };
    let registry = serde_json::json!({"schema_version":"epistesys-solver-registry.v1","entries":[{
        "id":"z3","profile":"smt_boolean_qf_lia","protocol":"smt_lib_payload_wire_v2",
        "executable":executable,"version":"5.3.2","binary_sha256":pin,"fixed_arguments":["-in","-smt2"],
        "caps":{"max_argv_count":16,"max_argv_bytes":4096,"max_stdin_bytes":16384,"max_stdout_bytes":65536,
        "max_stderr_bytes":16384,"max_timeout_ms":5000,"max_ram_bytes":67108864,"max_disk_bytes":0,
        "max_cleanup_bytes":65536,"max_child_count":1}}]});
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "epistesys-tlcx34-native-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("registry.json");
    std::fs::write(&path, registry.to_string()).unwrap();
    let (exit, text, error) = cli(
        &[
            "epistesys-solve",
            "--input-format",
            "smtlib-qf-lia-v1",
            "--solver-registry",
            path.to_str().unwrap(),
            "--goal-policy",
            "satisfying-model",
            "--prompt-stdin",
        ],
        FORMAL.as_bytes(),
    );
    assert_eq!(exit, Some(0), "{error}");
    assert!(text.contains("x=-3"), "{text}");
    assert!(
        text.contains("Translation evaluators/probe plans are missing"),
        "{text}"
    );
    assert!(text.contains("NoRecommendation"), "{text}");
    let (exit, no_goal, error) = cli(
        &[
            "epistesys-solve",
            "--input-format",
            "smtlib-qf-lia-v1",
            "--solver-registry",
            path.to_str().unwrap(),
            "--goal-policy",
            "no-goal",
            "--prompt-stdin",
        ],
        FORMAL.as_bytes(),
    );
    assert_eq!(exit, Some(0), "{error}");
    assert!(no_goal.contains("x=-3"), "{no_goal}");
    assert!(
        no_goal.contains("selection: NoRecommendation; goal: Unknown/NeedsEvidence"),
        "{no_goal}"
    );
}
