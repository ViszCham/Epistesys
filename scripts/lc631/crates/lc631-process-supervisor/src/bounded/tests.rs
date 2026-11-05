use super::*;

#[test]
fn root_exit_wait_observes_delayed_status() {
    let clock = std::cell::Cell::new(Instant::now());
    let mut polls = 0;
    let status = await_root_exit(
        None,
        clock.get() + Duration::from_millis(6),
        || {
            polls += 1;
            Ok(if polls < 3 { None } else { Some(-1) })
        },
        || clock.get(),
        |duration| clock.set(clock.get() + duration),
    )
    .unwrap();
    assert_eq!(status, Some(-1));
    assert_eq!(polls, 3);
}

#[test]
fn root_exit_wait_preserves_poll_error() {
    let error = await_root_exit(
        None,
        Instant::now(),
        || Err(io::Error::new(io::ErrorKind::PermissionDenied, "injected")),
        Instant::now,
        |_| panic!("must not sleep after a poll error"),
    )
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
}

#[test]
fn root_exit_wait_deadline_returns_unobserved() {
    let mut polls = 0;
    let deadline = Instant::now();
    let status = await_root_exit(
        None,
        deadline,
        || {
            polls += 1;
            Ok(None)
        },
        || deadline,
        |_| panic!("must not sleep past deadline"),
    )
    .unwrap();
    assert_eq!(status, None);
    assert_eq!(polls, 1);
}

#[test]
fn root_exit_wait_does_not_repoll_known_status() {
    let status = await_root_exit(
        Some(7),
        Instant::now(),
        || panic!("already observed status must not be polled again"),
        || panic!("already observed status must not inspect time"),
        |_| panic!("already observed status must not sleep"),
    )
    .unwrap();
    assert_eq!(status, Some(7));
}

fn fault_run(tree_fault: bool) -> RunOutcome {
    let executable = std::env::current_exe().unwrap();
    let arguments = [
        OsString::from("--exact"),
        OsString::from("bounded::tests::owned_observation_helper"),
        OsString::from("--nocapture"),
    ];
    let mut observers = Observers::default();
    if tree_fault {
        observers.tree_empty = |_| Err(io::Error::other("injected tree observation failure"));
    } else {
        observers.poll = |_| Err(io::Error::other("injected wait failure"));
    }
    run_observed(
        RunRequest {
            executable: executable.as_os_str(),
            arguments: &arguments,
            current_dir: &std::env::current_dir().unwrap(),
            environment: &std::env::vars_os().collect::<Vec<_>>(),
            stdin: Some(b"ordinary"),
            timeout: Duration::from_secs(1),
            stdout_limit: 4096,
            stderr_limit: 4096,
        },
        observers,
    )
    .unwrap()
}

#[test]
fn wait_error_still_terminates_and_reaps_the_owned_run() {
    let output = fault_run(false);
    assert_eq!(output.diagnostic, Some("supervisor_process_wait_failed"));
    assert!(output.process_tree_kill_attempted);
    assert!(output.process_tree_kill_verified && output.io_cleanup_complete);
}

#[test]
fn tree_observation_error_does_not_skip_cleanup_or_release_live_resources() {
    let output = fault_run(true);
    assert_eq!(
        output.diagnostic,
        Some("supervisor_tree_observation_failed")
    );
    assert!(output.process_tree_kill_attempted);
    assert!(output.process_tree_kill_verified && output.io_cleanup_complete);
}

#[test]
fn owned_observation_helper() {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).unwrap();
    if input == "ordinary" {
        println!("ordinary observation helper");
    }
}

#[test]
fn tlcx05_prepared_buffers_cover_declared_bounds_without_growth() {
    let arguments = [];
    let environment = [];
    let directory = std::env::current_dir().unwrap();
    let request = RunRequest {
        executable: OsStr::new("not-started-by-buffer-planning"),
        arguments: &arguments,
        current_dir: &directory,
        environment: &environment,
        stdin: Some(b"stdin-copy"),
        timeout: Duration::from_secs(2),
        stdout_limit: 20,
        stderr_limit: 13,
    };
    let mut buffers = PreparedIoBuffers::prepare(&request).unwrap();
    let capacities = (
        buffers.stdout.capacity(),
        buffers.stderr.capacity(),
        buffers.stdin.as_ref().unwrap().capacity(),
    );
    assert!(capacities.0 >= 20 && capacities.1 >= 13 && capacities.2 >= 10);
    assert_eq!(buffers.stdin.as_deref(), Some(b"stdin-copy".as_slice()));
    buffers.stdout.extend_from_slice(&[b'o'; 20]);
    buffers.stderr.extend_from_slice(&[b'e'; 13]);
    assert_eq!(
        capacities,
        (
            buffers.stdout.capacity(),
            buffers.stderr.capacity(),
            buffers.stdin.as_ref().unwrap().capacity()
        )
    );
}

#[test]
fn tlcx05_preallocation_failure_prevents_the_spawn_boundary() {
    let directory = std::env::current_dir().unwrap();
    let observers = Observers {
        prepare_io: |_| {
            Err(io::Error::new(
                io::ErrorKind::OutOfMemory,
                "injected pre-spawn allocation failure",
            ))
        },
        before_spawn: |_, _| panic!("spawn boundary reached after preallocation failed"),
        ..Observers::default()
    };
    let result = run_observed(
        RunRequest {
            executable: OsStr::new("must-not-be-started"),
            arguments: &[],
            current_dir: &directory,
            environment: &[],
            stdin: Some(b"input"),
            timeout: Duration::from_secs(2),
            stdout_limit: 20,
            stderr_limit: 13,
        },
        observers,
    );
    match result {
        Err(error) => assert_eq!(error.kind(), io::ErrorKind::OutOfMemory),
        Ok(_) => panic!("preallocation failure became an executed run"),
    }
}

#[test]
fn tlcx05_environment_entry_count_is_bounded_before_spawn() {
    let directory = std::env::current_dir().unwrap();
    let environment = vec![(OsString::from("KEY"), OsString::from("value")); 257];
    let request = RunRequest {
        executable: OsStr::new("never-spawn-invalid-environment"),
        arguments: &[],
        current_dir: &directory,
        environment: &environment,
        stdin: None,
        timeout: Duration::from_secs(1),
        stdout_limit: 8,
        stderr_limit: 8,
    };
    assert_eq!(
        validate(&request).unwrap_err().kind(),
        io::ErrorKind::InvalidInput
    );
}

#[test]
fn tlcx05_environment_total_bytes_are_bounded_before_spawn() {
    let directory = std::env::current_dir().unwrap();
    let environment = [(
        OsString::from("KEY"),
        OsString::from("v".repeat(1024 * 1024 + 1)),
    )];
    let request = RunRequest {
        executable: OsStr::new("never-spawn-invalid-environment"),
        arguments: &[],
        current_dir: &directory,
        environment: &environment,
        stdin: None,
        timeout: Duration::from_secs(1),
        stdout_limit: 8,
        stderr_limit: 8,
    };
    assert_eq!(
        validate(&request).unwrap_err().kind(),
        io::ErrorKind::InvalidInput
    );
}

fn thread_start_failure_run(lose_cleanup_observation: bool) -> RunOutcome {
    fn observed_cleanup(child: &mut ManagedChild, timeout: Duration) -> io::Result<bool> {
        child.terminate_tree(timeout)
    }
    fn lost_cleanup(child: &mut ManagedChild, timeout: Duration) -> io::Result<bool> {
        // Reap the actual test child; only the observation channel is faulted.
        child.terminate_tree(timeout)?;
        Err(io::Error::other("injected cleanup observation failure"))
    }
    let executable = std::env::current_exe().unwrap();
    let directory = std::env::current_dir().unwrap();
    let arguments = [
        OsString::from("--exact"),
        OsString::from("bounded::tests::owned_observation_helper"),
        OsString::from("--nocapture"),
    ];
    let observers = Observers {
        start_stdin: |_file, _bytes| Err(io::Error::other("injected OS thread start failure")),
        terminate_tree: if lose_cleanup_observation {
            lost_cleanup
        } else {
            observed_cleanup
        },
        ..Observers::default()
    };
    run_observed(
        RunRequest {
            executable: executable.as_os_str(),
            arguments: &arguments,
            current_dir: &directory,
            environment: &[],
            stdin: Some(b"ordinary"),
            timeout: Duration::from_secs(2),
            stdout_limit: 4096,
            stderr_limit: 4096,
        },
        observers,
    )
    .unwrap()
}

#[test]
fn tlcx05_stdin_thread_start_failure_runs_bounded_child_cleanup() {
    let output = thread_start_failure_run(false);
    assert_eq!(
        output.diagnostic,
        Some("supervisor_stdin_thread_start_failed")
    );
    assert!(output.process_tree_kill_attempted);
    assert!(output.process_tree_kill_verified && output.io_cleanup_complete);
}

#[test]
fn tlcx05_stdin_thread_start_failure_with_unobserved_cleanup_is_retained() {
    let output = thread_start_failure_run(true);
    assert_eq!(
        output.diagnostic,
        Some("supervisor_stdin_thread_start_failed_cleanup_unverified")
    );
    assert!(output.process_tree_kill_attempted);
    assert!(!output.process_tree_kill_verified);
}
