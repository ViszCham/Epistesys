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
fn tlcx34_evaluation_profile_rejects_missing_format_invalid_tag_and_duplicate_option() {
    let cases: Vec<Vec<&str>> = vec![
        vec![
            "epistesys-solve",
            "--evaluation-profile",
            "finite-boolean-v1",
            "--prompt",
            FORMAL,
        ],
        vec![
            "epistesys-solve",
            "--input-format",
            "smtlib-qf-lia-v1",
            "--evaluation-profile",
            "future-v9",
            "--prompt",
            FORMAL,
        ],
        vec![
            "epistesys-solve",
            "--input-format",
            "smtlib-qf-lia-v1",
            "--evaluation-profile",
            "finite-boolean-v1",
            "--evaluation-profile",
            "finite-boolean-v1",
            "--prompt",
            FORMAL,
        ],
    ];
    for arguments in cases {
        let (exit, text, error) = cli(&arguments, b"");
        assert!(exit.is_some_and(|code| code != 0));
        assert!(error.contains("canonical_arguments"), "{error}");
        assert!(!text.contains("assignments:"));
    }
}

#[test]
fn tlcx34_evaluation_profile_missing_registry_preserves_no_recommendation() {
    let source = "(set-logic QF_UF)\n(declare-const p Bool)\n(assert p)\n(check-sat)\n";
    let (exit, text, error) = cli(
        &[
            "epistesys-solve",
            "--input-format",
            "smtlib-qf-lia-v1",
            "--evaluation-profile",
            "finite-boolean-v1",
            "--prompt-stdin",
        ],
        source.as_bytes(),
    );
    assert_eq!(exit, Some(0), "{error}");
    assert!(
        text.contains("NoRecommendation") && text.contains("MissingBackend"),
        "{text}"
    );
    assert!(!text.contains("assignments:"));
}

#[test]
fn tlcx34_evaluation_profile_parse_residuals_are_clarify_without_measurement() {
    for source in [
        "Please solve this natural-language question.",
        "(set-logic QF_UF)\n(declare-const p Bool)\n(future-command p)\n(check-sat)\n",
    ] {
        let (exit, text, error) = cli(
            &[
                "epistesys-solve",
                "--input-format",
                "smtlib-qf-lia-v1",
                "--evaluation-profile",
                "finite-boolean-v1",
                "--prompt-stdin",
            ],
            source.as_bytes(),
        );
        assert_eq!(exit, Some(0), "{error}");
        assert!(
            text.contains("Clarify") && text.contains("NoRecommendation"),
            "{text}"
        );
        assert!(!text.contains("TheoryToWitnessModelSpace-v1: Measured"));
        assert!(!text.contains("assignments:"));
    }
}

fn registry_path() -> std::path::PathBuf {
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
    path
}

#[test]
#[ignore = "explicit native Z3 profile test; requires registered EPISTESYS_TLCX10_NATIVE_Z3"]
fn tlcx34_evaluation_profile_real_boolean_measurement_keeps_total_tl_and_no_goal_separate() {
    let path = registry_path();
    let source = "(set-logic QF_UF)\n(declare-const p Bool)\n(assert p)\n(check-sat)\n";
    for policy in ["satisfying-model", "no-goal"] {
        let (exit, text, error) = cli(
            &[
                "epistesys-solve",
                "--input-format",
                "smtlib-qf-lia-v1",
                "--solver-registry",
                path.to_str().unwrap(),
                "--evaluation-profile",
                "finite-boolean-v1",
                "--goal-policy",
                policy,
                "--prompt-stdin",
            ],
            source.as_bytes(),
        );
        assert_eq!(exit, Some(0), "{error}");
        assert!(text.contains("p=true"), "{text}");
        assert!(
            text.contains("TheoryToWitnessModelSpace-v1: Measured"),
            "{text}"
        );
        assert!(
            text.contains("Soundness loss=[0,0]; completeness loss=[0,0]"),
            "{text}"
        );
        assert!(
            text.contains("total TL is NoScalar/MissingEvaluator")
                && text.contains("NoRecommendation"),
            "{text}"
        );
        if policy == "no-goal" {
            assert!(
                text.contains("selection: NoRecommendation; goal: Unknown/NeedsEvidence"),
                "{text}"
            );
        }
    }
}
#[test]
#[ignore = "explicit native Z3 profile cut/unsupported tests; requires registered EPISTESYS_TLCX10_NATIVE_Z3"]
fn tlcx34_evaluation_profile_real_integer_is_unsupported_and_profile_cut_is_not_zero() {
    let path = registry_path();
    let (exit, text, error) = cli(
        &[
            "epistesys-solve",
            "--input-format",
            "smtlib-qf-lia-v1",
            "--solver-registry",
            path.to_str().unwrap(),
            "--evaluation-profile",
            "finite-boolean-v1",
            "--prompt-stdin",
        ],
        FORMAL.as_bytes(),
    );
    assert_eq!(exit, Some(0), "{error}");
    assert!(text.contains("UnsupportedInteger"), "{text}");
    assert!(!text.contains("Soundness loss=["));
    assert!(
        text.contains("total TL is NoScalar/MissingEvaluator") && text.contains("NoRecommendation"),
        "{text}"
    );
    let source = "(set-logic QF_UF)\n(declare-const p Bool)\n(assert p)\n(check-sat)\n";
    let (exit, cut, error) = cli(
        &[
            "epistesys-solve",
            "--input-format",
            "smtlib-qf-lia-v1",
            "--solver-registry",
            path.to_str().unwrap(),
            "--prompt-stdin",
        ],
        source.as_bytes(),
    );
    assert_eq!(exit, Some(0), "{error}");
    assert!(
        cut.contains("p=true") && cut.contains("NoRecommendation"),
        "{cut}"
    );
    assert!(
        !cut.contains("TheoryToWitnessModelSpace-v1: Measured")
            && !cut.contains("Soundness loss=[")
    );
    assert!(cut.contains("NoScalar/MissingEvaluator"), "{cut}");
}
