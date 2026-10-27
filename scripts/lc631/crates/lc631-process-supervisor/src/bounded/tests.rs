use super::*;

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
