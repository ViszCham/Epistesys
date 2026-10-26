use lc631_process_supervisor::{run_bounded, RunRequest};
use std::ffi::OsString;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn run_helper(role: &str, timeout: Duration) -> lc631_process_supervisor::RunOutcome {
    let executable = std::env::current_exe().unwrap();
    let arguments = [
        OsString::from("--exact"),
        OsString::from("owned_runtime_helper"),
        OsString::from("--nocapture"),
    ];
    run_bounded(RunRequest {
        executable: executable.as_os_str(),
        arguments: &arguments,
        current_dir: &std::env::current_dir().unwrap(),
        environment: &std::env::vars_os().collect::<Vec<_>>(),
        stdin: Some(role.as_bytes()),
        timeout,
        stdout_limit: 4_096,
        stderr_limit: 4_096,
    })
    .unwrap()
}

#[test]
fn lone_root_timeout_is_terminated_reaped_and_bounded() {
    let start = Instant::now();
    let output = run_helper("sleep", Duration::from_millis(350));
    assert!(start.elapsed() < Duration::from_secs(2));
    assert!(output.timed_out);
    assert!(output.process_tree_kill_attempted);
    assert!(output.process_tree_kill_verified);
    assert!(output.io_cleanup_complete);
}

#[test]
fn descendant_inherited_pipe_does_not_extend_deadline_or_leave_io_workers() {
    let start = Instant::now();
    let output = run_helper("descendant-pipe", Duration::from_millis(350));
    assert!(start.elapsed() < Duration::from_secs(2));
    assert!(output.timed_out);
    assert!(output.process_tree_kill_verified);
    assert!(output.io_cleanup_complete);
}

#[test]
fn output_flood_is_capped_and_terminated() {
    let output = run_helper("flood", Duration::from_secs(2));
    assert!(output.stdout_truncated);
    assert!(output.stdout.len() <= 4_096);
    assert!(output.process_tree_kill_attempted);
    assert!(output.io_cleanup_complete);
}

#[test]
fn repeated_blocked_stdin_is_cancelled_without_losing_admission_slots() {
    let executable = std::env::current_exe().unwrap();
    let arguments = [
        OsString::from("--exact"),
        OsString::from("owned_runtime_helper"),
        OsString::from("--nocapture"),
    ];
    let mut environment = std::env::vars_os().collect::<Vec<_>>();
    environment.push((
        OsString::from("EPISTESYS_RUNTIME_TEST_IGNORE_STDIN"),
        OsString::from("1"),
    ));
    let input = vec![b'x'; 2 * 1024 * 1024];
    for _ in 0..3 {
        let start = Instant::now();
        let output = run_bounded(RunRequest {
            executable: executable.as_os_str(),
            arguments: &arguments,
            current_dir: &std::env::current_dir().unwrap(),
            environment: &environment,
            stdin: Some(&input),
            timeout: Duration::from_millis(350),
            stdout_limit: 4_096,
            stderr_limit: 4_096,
        })
        .unwrap();
        assert!(start.elapsed() < Duration::from_secs(2));
        assert!(output.timed_out);
        assert!(output.process_tree_kill_verified);
        assert!(output.io_cleanup_complete);
    }
}

#[cfg(unix)]
#[test]
fn term_ignoring_root_is_killed_and_reaped() {
    let output = run_helper("ignore-term", Duration::from_millis(350));
    assert!(output.timed_out);
    assert!(output.process_tree_kill_verified);
    assert!(output.io_cleanup_complete);
}

#[test]
fn ordinary_exit_preserves_output_without_claiming_isolation() {
    let output = run_helper("ordinary", Duration::from_secs(2));
    assert_eq!(output.exit_code, Some(0));
    assert!(!output.timed_out);
    assert!(!output.stdout_truncated);
    assert!(String::from_utf8_lossy(&output.stdout).contains("ordinary-runtime-output"));
    assert!(output.io_cleanup_complete);
    assert!(matches!(
        output.containment,
        "windows_job" | "unix_process_group"
    ));
    if cfg!(windows) {
        assert_eq!(output.memory_limit_kind, "job_wide_committed_memory");
        assert_eq!(output.memory_limit_bytes, Some(2 * 1024 * 1024 * 1024));
    } else {
        assert_eq!(output.memory_limit_bytes, None);
    }
}

#[test]
#[allow(clippy::zombie_processes)] // This owned helper deliberately leaves its child to its supervisor.
fn owned_runtime_helper() {
    if std::env::var_os("EPISTESYS_RUNTIME_TEST_IGNORE_STDIN").is_some() {
        std::thread::sleep(Duration::from_secs(10));
        return;
    }
    let mut role = String::new();
    std::io::stdin().read_to_string(&mut role).unwrap();
    match role.as_str() {
        "sleep" => std::thread::sleep(Duration::from_secs(10)),
        #[cfg(unix)]
        "ignore-term" => {
            // SAFETY: the test changes SIGTERM disposition only in the owned
            // helper process; the supervisor must escalate to kill this group.
            unsafe { libc::signal(libc::SIGTERM, libc::SIG_IGN) };
            std::thread::sleep(Duration::from_secs(10));
        }
        "descendant-pipe" => {
            let mut child = Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "owned_runtime_helper", "--nocapture"])
                .stdin(Stdio::piped())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .spawn()
                .unwrap();
            child.stdin.take().unwrap().write_all(b"sleep").unwrap();
        }
        "flood" => {
            let block = [b'x'; 8_192];
            for _ in 0..10_000 {
                if std::io::stdout().write_all(&block).is_err() {
                    break;
                }
            }
        }
        "ordinary" => println!("ordinary-runtime-output"),
        _ => {}
    }
}
