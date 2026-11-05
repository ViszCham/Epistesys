use lc631_cli::tlcx_commands::{parse, run_io, CommandError, MAX_SOURCE_BYTES};
use lc631_core::context::{DecisionStatus, MissingStage, TaskKind};
use std::io::{self, Read};

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).into()).collect()
}

struct NoRead;
impl Read for NoRead {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        panic!("unexpected stdin read")
    }
}

#[test]
fn tlcx34_formal_services_options_require_explicit_format_and_never_use_legacy_runtime() {
    let valid = args(&[
        "epistesys-solve",
        "--prompt",
        "(set-logic QF_LIA) (check-sat)",
        "--input-format",
        "smtlib-qf-lia-v1",
        "--solver-registry",
        "local-registry.json",
        "--goal-policy",
        "satisfying-model",
    ]);
    assert!(parse(&valid).is_ok());
    let mut output = Vec::new();
    assert!(run_io(&valid, &mut NoRead, &mut output).is_err());
    assert!(output.is_empty());
    for options in [
        vec!["--solver-registry", "registry.json"],
        vec!["--goal-policy", "satisfying-model"],
        vec!["--input-format", "guess-from-text"],
        vec![
            "--input-format",
            "smtlib-qf-lia-v1",
            "--goal-policy",
            "best-effort",
        ],
        vec![
            "--input-format",
            "smtlib-qf-lia-v1",
            "--input-format",
            "smtlib-qf-lia-v1",
        ],
        vec![
            "--input-format",
            "smtlib-qf-lia-v1",
            "--goal-policy",
            "no-goal",
            "--goal-policy",
            "satisfying-model",
        ],
    ] {
        let mut values = vec!["epistesys-solve", "--prompt", "source"];
        values.extend(options);
        assert!(parse(&args(&values)).is_err(), "{values:?}");
    }
    for name in ["epistesys-inspect", "epistesys-resume"] {
        assert!(parse(&args(&[
            name,
            "--artifact",
            "local:1",
            "--input-format",
            "smtlib-qf-lia-v1"
        ]))
        .is_err());
    }
}

#[test]
fn tlcx34_formal_profile_does_not_silently_ignore_non_reasoning_task_kinds() {
    for task in ["coding", "media", "explanation"] {
        let values = args(&[
            "epistesys-solve",
            "--prompt",
            "(set-logic QF_LIA) (check-sat)",
            "--input-format",
            "smtlib-qf-lia-v1",
            "--task",
            task,
        ]);
        assert!(
            parse(&values).is_err(),
            "unsupported formal task was accepted: {task}"
        );
        let legacy = args(&["epistesys-solve", "--prompt", "source", "--task", task]);
        assert!(
            parse(&legacy).is_ok(),
            "legacy task profile must remain available: {task}"
        );
    }
    for task in ["mathematics", "reasoning"] {
        let values = args(&[
            "epistesys-solve",
            "--prompt",
            "(set-logic QF_LIA) (check-sat)",
            "--input-format",
            "smtlib-qf-lia-v1",
            "--task",
            task,
        ]);
        assert!(parse(&values).is_ok());
    }
}

#[test]
fn tlcx34_objective_step_is_explicit_and_requires_a_declared_source_goal() {
    let prefix = [
        "epistesys-solve",
        "--prompt",
        "source",
        "--input-format",
        "smtlib-qf-lia-v1",
    ];
    let mut allowed = prefix.to_vec();
    allowed.extend([
        "--goal-policy",
        "declared-source-objective",
        "--objective-step",
        "single-v1",
    ]);
    assert!(parse(&args(&allowed)).is_ok());
    for extra in [
        vec!["--objective-step", "single-v1"],
        vec![
            "--goal-policy",
            "satisfying-model",
            "--objective-step",
            "single-v1",
        ],
        vec![
            "--goal-policy",
            "declared-source-objective",
            "--objective-step",
            "unbounded",
        ],
        vec![
            "--goal-policy",
            "declared-source-objective",
            "--objective-step",
            "single-v1",
            "--objective-step",
            "single-v1",
        ],
    ] {
        let mut values = prefix.to_vec();
        values.extend(extra);
        assert!(
            parse(&args(&values)).is_err(),
            "invalid step policy: {values:?}"
        );
    }
    assert!(parse(&args(&[
        "epistesys-solve",
        "--prompt",
        "source",
        "--objective-step",
        "single-v1"
    ]))
    .is_err());
}

#[test]
fn tlcx34_rejects_conflicting_duplicate_missing_and_unknown_arguments() {
    for values in [
        vec!["epistesys-solve", "--prompt", "a", "--prompt-stdin"],
        vec!["epistesys-solve", "--prompt", "a", "--prompt", "b"],
        vec!["epistesys-solve", "--task", "coding"],
        vec!["epistesys-solve", "--prompt"],
        vec!["epistesys-solve", "--prompt", "a", "--execute"],
        vec!["epistesys-solve", "--prompt", "a", "--task", "invalid"],
        vec!["lc631-doctor", "--prompt", "a"],
    ] {
        assert!(parse(&args(&values)).is_err(), "{values:?}");
    }
}

#[test]
fn tlcx34_literal_prompt_calls_real_runtime_and_preserves_not_connected() {
    let arguments = args(&[
        "epistesys-solve",
        "--prompt",
        "あ exact\n",
        "--task",
        "coding",
    ]);
    let mut output = Vec::new();
    let observation = run_io(&arguments, &mut NoRead, &mut output).unwrap();
    let report = observation.runtime_report().unwrap();
    let audit = observation.connection_audit().unwrap();
    assert_eq!(audit.entry().report(), report);
    assert_eq!(audit.registry().contracts().len(), 20);
    assert_eq!(
        audit.gap(),
        lc631_runtime::connection_audit::AuditGap::RequiredEdgeExecutionsUnobserved
    );
    assert_eq!(
        report.status,
        DecisionStatus::NotConnected(MissingStage::ContextLowering)
    );
    assert_eq!(report.task, TaskKind::Coding);
    assert_eq!(report.source_bytes, "あ exact\n".len());
    let rendered: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(rendered["status"], "not_connected");
    assert_eq!(rendered["missing_stage"], "context_lowering");
    assert_eq!(rendered["implementation_complete"], false);
    assert_eq!(rendered["authority_created"], false);
    assert!(!String::from_utf8(output).unwrap().contains("あ exact"));
}

#[test]
fn tlcx34_stdin_is_exact_bounded_and_invalid_utf8_never_calls_runtime() {
    let arguments = args(&["epistesys-solve", "--prompt-stdin"]);
    let mut output = Vec::new();
    let observed = run_io(&arguments, &mut &b"first\nlast\n"[..], &mut output).unwrap();
    assert_eq!(observed.runtime_report().unwrap().source_bytes, 11);
    for bytes in [vec![b'a'; MAX_SOURCE_BYTES + 1], vec![0xff], Vec::new()] {
        output.clear();
        assert!(run_io(&arguments, &mut &bytes[..], &mut output).is_err());
        assert!(output.is_empty());
    }
    output.clear();
    let boundary = vec![b'a'; MAX_SOURCE_BYTES];
    assert!(run_io(&arguments, &mut &boundary[..], &mut output).is_ok());
    assert!(output.len() < 4096);
}

#[test]
fn tlcx34_task_policies_share_one_runtime_entry_and_source_binding() {
    for task in ["mathematics", "reasoning", "explanation", "coding", "media"] {
        let arguments = args(&["epistesys-solve", "--prompt", "same source", "--task", task]);
        let observation = run_io(&arguments, &mut NoRead, &mut Vec::new()).unwrap();
        assert_eq!(
            observation.runtime_report().unwrap().context.source(),
            lc631_core::context::SourceRevision::of("same source")
        );
    }
}

#[test]
fn tlcx34_inspect_resume_require_real_store_and_never_restore_completion() {
    for name in ["epistesys-inspect", "epistesys-resume"] {
        let arguments = args(&[name, "--artifact", "local:decision-1"]);
        let mut output = Vec::new();
        let observation = run_io(&arguments, &mut NoRead, &mut output).unwrap();
        assert!(observation.runtime_report().is_none());
        assert!(observation.connection_audit().is_none());
        let rendered: serde_json::Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(rendered["status"], "not_connected");
        assert_eq!(rendered["missing_stage"], "artifact_store_capability");
        assert_eq!(rendered["implementation_complete"], false);
    }
}

#[test]
fn tlcx34_io_failure_propagates_without_second_runtime_or_fallback() {
    struct Broken;
    impl Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("cut"))
        }
    }
    let arguments = args(&["epistesys-solve", "--prompt-stdin"]);
    let mut output = Vec::new();
    assert_eq!(
        run_io(&arguments, &mut Broken, &mut output).unwrap_err(),
        CommandError::ReadFailed
    );
    assert!(output.is_empty());
}

#[test]
fn tlcx34_partial_output_retains_runtime_attempt_and_unknown_delivery() {
    struct Partial {
        bytes: Vec<u8>,
    }
    impl std::io::Write for Partial {
        fn write(&mut self, input: &[u8]) -> io::Result<usize> {
            if self.bytes.is_empty() {
                self.bytes.extend_from_slice(&input[..5]);
                Ok(5)
            } else {
                Err(io::Error::other("broken sink"))
            }
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let arguments = args(&["epistesys-solve", "--prompt", "source"]);
    let mut sink = Partial { bytes: Vec::new() };
    assert_eq!(
        run_io(&arguments, &mut NoRead, &mut sink).unwrap_err(),
        CommandError::WriteFailedAfterRuntime
    );
    assert_eq!(sink.bytes.len(), 5);
}
