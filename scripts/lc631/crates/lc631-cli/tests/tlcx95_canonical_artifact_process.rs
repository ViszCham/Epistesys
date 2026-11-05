//! Bounded production CLI processes; native positives require pinned real Z3.
#![forbid(unsafe_code)]
use lc631_process_supervisor::{
    required_ram_bytes, run_bounded_with_resources_observed, RunRequest,
};
use lc631_runtime::{
    canonical_solve::replay::{ReplayGoal, ReplayProfile},
    resource::{
        Cancellation, Deadline, Digest32, ResourceAmount, ResourceBroker, ResourceKind,
        ResourceLimits, ResourceRequest,
    },
};
use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
const SOURCE: &str =
    "(set-logic QF_LIA)\n(declare-const x Int)\n(assert (= x (- 3)))\n(check-sat)\n";

fn cli(arguments: &[&str], input: &[u8]) -> (Option<i32>, String, String) {
    let args: Vec<OsString> = arguments
        .iter()
        .map(|value| OsString::from(*value))
        .collect();
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
            95,
            Digest32::from_bytes([95; 32]),
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
        assert!(observed.cleanup_complete());
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
fn root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "epistesys-tlcx95-cli-artifact-{label}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}
fn key() -> String {
    ReplayProfile::checked(ReplayGoal::NoGoal, false, false)
        .unwrap()
        .key(SOURCE)
        .to_string()
}

#[test]
fn tlcx95_cli_prompt_value_named_store_root_does_not_relax_legacy_sixteen() {
    use lc631_cli::tlcx_commands::{parse, CommandError};
    let mut arguments = vec![
        "epistesys-solve".to_owned(),
        "--prompt".to_owned(),
        "--store-root".to_owned(),
    ];
    for _ in 0..7 {
        arguments.extend(["--task".to_owned(), "reasoning".to_owned()]);
    }
    assert_eq!(arguments.len(), 17);
    assert_eq!(
        parse(&arguments).err(),
        Some(CommandError::TooManyArguments)
    );
}

#[test]
fn tlcx95_cli_actual_paired_archive_profile_has_its_separate_fixed_budget() {
    use lc631_cli::tlcx_commands::parse;
    let source = "(set-logic QF_LIA)\n(declare-const x Int)\n(assert (<= 0 x))\n(assert (<= x 10))\n(minimize x)\n(check-sat)\n";
    let key = ReplayProfile::checked(ReplayGoal::DeclaredSourceObjective, false, true)
        .unwrap()
        .key(source)
        .to_string();
    let root = root("parse-only-not-created");
    let registry = root.join("not-opened-registry.json");
    let arguments: Vec<String> = [
        "epistesys-solve",
        "--input-format",
        "smtlib-qf-lia-v1",
        "--solver-registry",
        registry.to_str().unwrap(),
        "--goal-policy",
        "declared-source-objective",
        "--objective-step",
        "single-v1",
        "--task",
        "mathematics",
        "--prompt",
        source,
        "--artifact",
        &key,
        "--store-root",
        root.to_str().unwrap(),
    ]
    .iter()
    .map(|value| (*value).to_owned())
    .collect();
    assert_eq!(arguments.len(), 17);
    assert!(parse(&arguments).is_ok());
    assert!(!root.exists());
    let mut excessive = arguments;
    excessive.extend((0..8).map(|_| "--prompt-stdin".to_owned()));
    assert_eq!(excessive.len(), 25);
    assert!(matches!(
        parse(&excessive),
        Err(lc631_cli::tlcx_commands::CommandError::TooManyArguments)
    ));
}

#[test]
fn tlcx95_cli_old_ephemeral_inspect_resume_stays_explicitly_unconnected() {
    for command in ["epistesys-inspect", "epistesys-resume"] {
        let (exit, output, error) = cli(&[command, "--artifact", "local:decision-1"], b"");
        assert_eq!(exit, Some(0), "{error}");
        let value: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(value["missing_stage"], "artifact_store_capability");
        assert_eq!(value["implementation_complete"], false);
    }
}

#[test]
fn tlcx95_cli_new_artifact_profile_rejects_missing_store_without_creation() {
    let root = root("missing");
    let key = key();
    let (exit, text, error) = cli(
        &[
            "epistesys-inspect",
            "--artifact",
            &key,
            "--store-root",
            root.to_str().unwrap(),
        ],
        b"",
    );
    assert!(exit.is_some_and(|code| code != 0));
    assert!(text.is_empty());
    assert!(error.contains("canonical_artifact"), "{error}");
    assert!(!root.exists());
}

fn registry(parent: &Path) -> PathBuf {
    let executable = std::env::var("EPISTESYS_TLCX10_NATIVE_Z3").expect("native path required");
    assert!(Path::new(&executable).is_absolute());
    let pin = if cfg!(windows) {
        "5f32b2d2b93291b56e3c10a43b088cae78acc84017ebff8fa741a7ecee6c6ca5"
    } else if cfg!(target_os = "linux") {
        "c45e787b5d1a3d94079d1e83b98e23bbc884ccaf542ea38c3814f31ac2f79197"
    } else {
        panic!("native OS profile unavailable")
    };
    let value = serde_json::json!({"schema_version":"epistesys-solver-registry.v1","entries":[{
        "id":"z3","profile":"smt_boolean_qf_lia","protocol":"smt_lib_payload_wire_v2",
        "executable":executable,"version":"5.3.2","binary_sha256":pin,"fixed_arguments":["-in","-smt2"],
        "caps":{"max_argv_count":16,"max_argv_bytes":4096,"max_stdin_bytes":16384,"max_stdout_bytes":65536,
        "max_stderr_bytes":16384,"max_timeout_ms":5000,"max_ram_bytes":67108864,"max_disk_bytes":0,
        "max_cleanup_bytes":65536,"max_child_count":1}}]});
    let file = parent.join("registry.json");
    std::fs::write(&file, value.to_string()).unwrap();
    file
}

#[test]
#[ignore = "actual Z3 production solve/save -> process exit -> inspect -> fresh resume and cuts"]
fn tlcx95_cli_actual_native_artifact_is_historical_until_fresh_reexecution() {
    let parent = root("native");
    std::fs::create_dir(&parent).unwrap();
    let root = parent.join("store");
    let config = registry(&parent);
    let key = key();
    let root_text = root.to_str().unwrap();
    let config_text = config.to_str().unwrap();
    let (exit, original, error) = cli(
        &[
            "epistesys-solve",
            "--input-format",
            "smtlib-qf-lia-v1",
            "--solver-registry",
            config_text,
            "--prompt-stdin",
            "--artifact",
            &key,
            "--store-root",
            root_text,
        ],
        SOURCE.as_bytes(),
    );
    assert_eq!(exit, Some(0), "{error}");
    assert!(original.contains("x=-3"), "{original}");
    let (exit, inspection, error) = cli(
        &[
            "epistesys-inspect",
            "--artifact",
            &key,
            "--store-root",
            root_text,
        ],
        b"",
    );
    assert_eq!(exit, Some(0), "{error}");
    let historical: serde_json::Value = serde_json::from_str(&inspection).unwrap();
    assert_eq!(
        historical["schema_version"],
        "epistesys-canonical-artifact-inspection.v1"
    );
    assert_eq!(historical["state"], "historical_unanchored");
    assert_eq!(historical["current_model"], false);
    assert_eq!(historical["authority_created"], false);
    assert_eq!(historical["completion_restored"], false);
    assert_eq!(historical["host_delivery_observed"], false);
    assert_eq!(historical["historical_text"], original);
    let (exit, resumed, error) = cli(
        &[
            "epistesys-resume",
            "--artifact",
            &key,
            "--store-root",
            root_text,
            "--solver-registry",
            config_text,
        ],
        b"",
    );
    assert_eq!(exit, Some(0), "{error}");
    assert!(resumed.contains("x=-3"), "{resumed}");
    let (exit, no_registry, _) = cli(
        &[
            "epistesys-resume",
            "--artifact",
            &key,
            "--store-root",
            root_text,
        ],
        b"",
    );
    assert!(exit.is_some_and(|code| code != 0));
    assert!(!no_registry.contains("x=-3"));
    let swapped = ReplayProfile::checked(ReplayGoal::NoGoal, true, false)
        .unwrap()
        .key(SOURCE)
        .to_string();
    let (exit, text, _) = cli(
        &[
            "epistesys-inspect",
            "--artifact",
            &swapped,
            "--store-root",
            root_text,
        ],
        b"",
    );
    assert!(exit.is_some_and(|code| code != 0));
    assert!(text.is_empty());
    let file = root.join("segment-00000000000000000001.bin");
    let mut corrupted = std::fs::read(&file).unwrap();
    let last = corrupted.len() - 1;
    corrupted[last] ^= 1;
    std::fs::write(&file, corrupted).unwrap();
    let (exit, text, _) = cli(
        &[
            "epistesys-resume",
            "--artifact",
            &key,
            "--store-root",
            root_text,
            "--solver-registry",
            config_text,
        ],
        b"",
    );
    assert!(exit.is_some_and(|code| code != 0));
    assert!(!text.contains("x=-3"));
    eprintln!(
        "owned production artifact fixture retained: {}",
        parent.display()
    );
}
