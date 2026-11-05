//! Actual bounded CLI processes; no fabricated native solver positives.
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

const MAXIMIZE: &str = "(set-logic QF_LIA)\n(declare-const x Int)\n(assert (<= 0 x))\n(assert (<= x 10))\n(maximize x)\n(check-sat)\n";
const NO_OBJECTIVE: &str =
    "(set-logic QF_LIA)\n(declare-const x Int)\n(assert (= x 0))\n(check-sat)\n";

fn cli(args: &[&str], input: &[u8]) -> (Option<i32>, String, String) {
    let arguments: Vec<OsString> = args.iter().map(|arg| OsString::from(*arg)).collect();
    let environment: Vec<_> = std::env::vars_os().collect();
    let cwd = std::env::current_dir().unwrap();
    let request = RunRequest {
        executable: OsStr::new(env!("CARGO_BIN_EXE_lc631")),
        arguments: &arguments,
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
fn tlcx34_objective_step_rejects_invalid_flags_before_formal_execution() {
    let base = [
        "epistesys-solve",
        "--input-format",
        "smtlib-qf-lia-v1",
        "--goal-policy",
        "declared-source-objective",
        "--prompt-stdin",
    ];
    let mut wrong = base.to_vec();
    wrong.extend(["--objective-step", "future-v2"]);
    let mut duplicate = base.to_vec();
    duplicate.extend([
        "--objective-step",
        "single-v1",
        "--objective-step",
        "single-v1",
    ]);
    let no_format = vec![
        "epistesys-solve",
        "--objective-step",
        "single-v1",
        "--prompt-stdin",
    ];
    let wrong_goal = vec![
        "epistesys-solve",
        "--input-format",
        "smtlib-qf-lia-v1",
        "--goal-policy",
        "satisfying-model",
        "--objective-step",
        "single-v1",
        "--prompt-stdin",
    ];
    for args in [wrong, duplicate, no_format, wrong_goal] {
        let (exit, text, error) = cli(&args, MAXIMIZE.as_bytes());
        assert!(exit.is_some_and(|n| n != 0));
        assert!(error.contains("canonical_arguments"), "{error}");
        assert!(
            !text.contains("assignments:") && !text.contains("Explicit single-v1 objective step:")
        );
    }
}

#[test]
fn tlcx34_objective_step_missing_registry_is_no_recommendation_without_fallback() {
    for option in [false, true] {
        let mut args = vec![
            "epistesys-solve",
            "--input-format",
            "smtlib-qf-lia-v1",
            "--goal-policy",
            "declared-source-objective",
            "--prompt-stdin",
        ];
        if option {
            args.extend(["--objective-step", "single-v1"]);
        }
        let (exit, text, error) = cli(&args, MAXIMIZE.as_bytes());
        assert_eq!(exit, Some(0), "{error}");
        assert!(
            text.contains("MissingBackend") && text.contains("NoRecommendation"),
            "{text}"
        );
        assert!(!text.contains("assignments:") && !text.contains("ImprovedModelRechecked"));
    }
}

#[test]
fn tlcx34_objective_step_cannot_invent_a_source_objective() {
    let (exit, text, error) = cli(
        &[
            "epistesys-solve",
            "--input-format",
            "smtlib-qf-lia-v1",
            "--goal-policy",
            "declared-source-objective",
            "--objective-step",
            "single-v1",
            "--prompt-stdin",
        ],
        NO_OBJECTIVE.as_bytes(),
    );
    assert!(exit.is_some_and(|n| n != 0));
    assert!(error.contains("GoalConflict"), "{error}");
    assert!(!text.contains("assignments:"));
}

fn registry_path(with_status: bool) -> std::path::PathBuf {
    let executable = std::env::var("EPISTESYS_TLCX10_NATIVE_Z3").expect("native path required");
    assert!(std::path::Path::new(&executable).is_absolute());
    let pin = if cfg!(windows) {
        "5f32b2d2b93291b56e3c10a43b088cae78acc84017ebff8fa741a7ecee6c6ca5"
    } else if cfg!(target_os = "linux") {
        "c45e787b5d1a3d94079d1e83b98e23bbc884ccaf542ea38c3814f31ac2f79197"
    } else {
        panic!("no native OS pin")
    };
    let entry = serde_json::json!({"id":"z3","profile":"smt_boolean_qf_lia",
        "protocol":"smt_lib_payload_wire_v2", "executable":executable,"version":"5.3.2",
        "binary_sha256":pin,"fixed_arguments":["-in","-smt2"],
        "caps":{"max_argv_count":16,"max_argv_bytes":4096,"max_stdin_bytes":16384,
        "max_stdout_bytes":65536,"max_stderr_bytes":16384,"max_timeout_ms":5000,
        "max_ram_bytes":67108864,"max_disk_bytes":0,"max_cleanup_bytes":65536,"max_child_count":1}});
    let mut entries = vec![entry.clone()];
    if with_status {
        let mut status = entry;
        status["protocol"] = "smt_lib_status_v1".into();
        entries.push(status);
    }
    let registry =
        serde_json::json!({"schema_version":"epistesys-solver-registry.v1","entries":entries});
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "epistesys-tlcx34-objective-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("registry.json");
    std::fs::write(&path, registry.to_string()).unwrap();
    path
}

fn solve(path: &std::path::Path, source: &str, step: bool) -> String {
    let mut args = vec![
        "epistesys-solve",
        "--input-format",
        "smtlib-qf-lia-v1",
        "--solver-registry",
        path.to_str().unwrap(),
        "--goal-policy",
        "declared-source-objective",
        "--prompt-stdin",
    ];
    if step {
        args.extend(["--objective-step", "single-v1"]);
    }
    let (exit, text, error) = cli(&args, source.as_bytes());
    assert_eq!(exit, Some(0), "{error}");
    assert!(
        text.contains("NoRecommendation") && text.contains("NoScalar/MissingEvaluator"),
        "{text}"
    );
    text
}

fn selected(text: &str) -> i64 {
    text.split("selected=Some(")
        .nth(1)
        .unwrap()
        .split(')')
        .next()
        .unwrap()
        .parse()
        .unwrap()
}
fn ratio(n: i64, d: i64) -> String {
    let (mut a, mut b) = (n, d);
    while b != 0 {
        (a, b) = (b, a % b);
    }
    if d / a == 1 {
        (n / a).to_string()
    } else {
        format!("{}/{}", n / a, d / a)
    }
}

#[test]
#[ignore = "explicit actual pinned Z3; requires EPISTESYS_TLCX10_NATIVE_Z3"]
fn tlcx34_objective_step_real_maximize_and_explicit_cuts() {
    let both = registry_path(true);
    let text = solve(&both, MAXIMIZE, true);
    let value = selected(&text);
    assert!(value > 0 && value <= 10, "{text}");
    assert!(
        text.contains("ImprovedModelRechecked")
            && text.contains("initial=Some(0)")
            && text.contains("attempts=3"),
        "{text}"
    );
    assert!(text.contains("parsed Maximize x bounds [0..=10]"), "{text}");
    assert!(
        text.contains(&format!("x={value};"))
            && text.contains(&format!("gap upper bound=Some({})", 10 - value)),
        "{text}"
    );
    let quality = ratio(value, 10);
    assert!(
        text.contains(&format!(
            "Declared finite goal quality=[{quality},{quality}]"
        )),
        "{text}"
    );
    assert!(text.contains(
        "overall search remains BudgetLimited/Deferred, never an optimality certificate"
    ));
    assert!(text.contains("Raw probe stdout/stderr are unowned"));
    let cut = solve(&both, MAXIMIZE, false);
    assert!(
        cut.contains("is retained and deferred; no optimization step or optimum is claimed"),
        "{cut}"
    );
    assert!(
        !cut.contains("Explicit single-v1 objective step:")
            && !cut.contains("Declared finite goal quality=")
    );
    let wire_only = registry_path(false);
    let unavailable = solve(&wire_only, MAXIMIZE, true);
    assert!(
        unavailable.contains("UnavailableStatusCapability") && unavailable.contains("attempts=1"),
        "{unavailable}"
    );
    assert!(!unavailable.contains("ImprovedModelRechecked"));
}

#[test]
#[ignore = "explicit actual pinned Z3; requires EPISTESYS_TLCX10_NATIVE_Z3"]
fn tlcx34_objective_step_real_minimize_and_unsat_zero_width_are_not_optimality_proof() {
    let both = registry_path(true);
    let minimize = "(set-logic QF_LIA)\n(declare-const x Int)\n(assert (<= (- 10) x))\n(assert (<= x 0))\n(minimize x)\n(check-sat)\n";
    let text = solve(&both, minimize, true);
    let initial: i64 = text
        .split("initial=Some(")
        .nth(1)
        .unwrap()
        .split(')')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    let value = selected(&text);
    assert!(
        (-10..=0).contains(&initial) && (-10..=0).contains(&value),
        "{text}"
    );
    assert!(
        text.contains("parsed Minimize x bounds [-10..=0]"),
        "{text}"
    );
    if text.contains("Explicit single-v1 objective step: ReportedNoStrictImprovement;") {
        assert_eq!((initial, value), (-10, -10), "{text}");
        assert!(text.contains("attempts=2"), "{text}");
        assert!(text.contains("Strict-probe UNSAT is only SolverReported, not checked proof"));
    } else {
        assert!(
            text.contains("Explicit single-v1 objective step: ImprovedModelRechecked;"),
            "{text}"
        );
        assert!(value < initial && text.contains("attempts=3"), "{text}");
    }
    let quality = ratio(-value, 10);
    assert!(
        text.contains(&format!(
            "Declared finite goal quality=[{quality},{quality}]"
        )) && text.contains(&format!("gap upper bound=Some({})", value + 10)),
        "{text}"
    );
    let zero = MAXIMIZE.replace("(<= x 10)", "(<= x 0)");
    let text = solve(&both, &zero, true);
    assert!(
        text.contains("ReportedNoStrictImprovement") && text.contains("attempts=2"),
        "{text}"
    );
    assert!(
        text.contains("parsed Maximize x bounds [0..=0]") && text.contains("selected=Some(0)"),
        "{text}"
    );
    assert!(
        text.contains("UndefinedZeroWidth; NoScalar/NeedsEvidence"),
        "{text}"
    );
    assert!(text.contains("Strict-probe UNSAT is only SolverReported, not checked proof"));
    assert!(!text.contains("Declared finite goal quality="));
}
