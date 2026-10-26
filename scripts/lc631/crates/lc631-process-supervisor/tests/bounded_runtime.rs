use lc631_process_supervisor::{run_bounded, RunRequest};
use std::ffi::OsString;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

// Cleanup fixtures test an admitted process, not competition between unrelated
// test threads. Admission saturation has its own explicit negative below.
static RUNTIME_FIXTURE: Mutex<()> = Mutex::new(());

fn runtime_fixture() -> MutexGuard<'static, ()> {
    RUNTIME_FIXTURE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

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
    let _fixture = runtime_fixture();
    let start = Instant::now();
    let output = run_helper("sleep", Duration::from_millis(350));
    assert!(start.elapsed() < Duration::from_secs(2));
    assert!(output.timed_out);
    assert!(output.process_tree_kill_attempted, "{output:?}");
    assert!(output.process_tree_kill_verified);
    assert!(output.io_cleanup_complete);
}

#[test]
fn descendant_inherited_pipe_does_not_extend_deadline_or_leave_io_workers() {
    let _fixture = runtime_fixture();
    let start = Instant::now();
    // A positive two-process cleanup observation needs OS scheduling margin.
    // This explicit test budget is not a production default or a guarantee
    // that every shorter request can verify cleanup instead of retiring it.
    let output = run_helper("descendant-pipe", Duration::from_millis(1_500));
    assert!(start.elapsed() < Duration::from_secs(3));
    assert!(output.timed_out);
    assert!(output.process_tree_kill_verified, "{output:?}");
    assert!(output.io_cleanup_complete, "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("owned descendant inherited pipes"),
        "{output:?}"
    );
}

#[test]
fn output_flood_is_capped_and_terminated() {
    let _fixture = runtime_fixture();
    let output = run_helper("flood", Duration::from_secs(2));
    assert!(output.stdout_truncated);
    assert!(output.stdout.len() <= 4_096);
    assert!(output.process_tree_kill_attempted);
    assert!(output.io_cleanup_complete);
}

#[test]
fn repeated_blocked_stdin_is_cancelled_without_losing_admission_slots() {
    let _fixture = runtime_fixture();
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
        assert!(output.process_tree_kill_verified, "{output:?}");
        assert!(output.io_cleanup_complete);
    }
}

#[cfg(unix)]
#[test]
fn term_ignoring_root_is_killed_and_reaped() {
    let _fixture = runtime_fixture();
    let output = run_helper("ignore-term", Duration::from_millis(350));
    assert!(output.timed_out);
    assert!(output.process_tree_kill_verified);
    assert!(output.io_cleanup_complete);
}

#[test]
fn ordinary_exit_preserves_output_without_claiming_isolation() {
    let _fixture = runtime_fixture();
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
fn saturated_admission_is_not_a_spawn_and_capacity_recovers_after_release() {
    let _fixture = runtime_fixture();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "epistesys-owned-admission-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir(&root).unwrap();
    let markers = [root.join("first"), root.join("second")];
    let workers = markers
        .iter()
        .map(|marker| {
            let role = format!("hold-admission:{}", marker.display());
            std::thread::spawn(move || run_helper(&role, Duration::from_secs(4)))
        })
        .collect::<Vec<_>>();
    let readiness_deadline = Instant::now() + Duration::from_secs(2);
    while !markers.iter().all(|marker| marker.is_file()) && Instant::now() < readiness_deadline {
        std::thread::sleep(Duration::from_millis(2));
    }
    let both_started = markers.iter().all(|marker| marker.is_file());
    let queued = both_started.then(|| run_helper("ordinary", Duration::from_millis(60)));

    // Release and join every owned worker before any assertion can unwind.
    // Their runner deadlines still bound cleanup if a fixture write fails.
    let releases = markers
        .iter()
        .map(|marker| std::fs::write(marker.with_extension("release"), b"release"))
        .collect::<Vec<_>>();
    let completed = workers
        .into_iter()
        .map(|worker| worker.join())
        .collect::<Vec<_>>();
    assert!(releases.iter().all(Result::is_ok));
    assert!(
        both_started,
        "both owned helpers must actually occupy a slot"
    );
    for result in completed {
        let output = result.unwrap();
        assert_eq!(output.exit_code, Some(0), "{output:?}");
        assert!(output.process_tree_kill_verified, "{output:?}");
        assert!(output.io_cleanup_complete, "{output:?}");
    }
    let queued = queued.unwrap();
    assert!(queued.timed_out, "{queued:?}");
    assert_eq!(queued.exit_code, None, "{queued:?}");
    assert_eq!(queued.memory_limit_kind, "process_not_started");
    assert_eq!(
        queued.diagnostic,
        Some("supervisor_admission_deadline_exceeded")
    );
    assert!(!queued.process_tree_kill_attempted, "{queued:?}");
    assert!(!queued.process_tree_kill_verified, "{queued:?}");
    assert!(queued.io_cleanup_complete, "{queued:?}");
    let recovered = run_helper("ordinary", Duration::from_secs(2));
    assert_eq!(recovered.exit_code, Some(0), "{recovered:?}");
    assert!(!recovered.timed_out, "{recovered:?}");
    assert!(recovered.process_tree_kill_verified, "{recovered:?}");
    assert!(recovered.io_cleanup_complete, "{recovered:?}");
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
            println!("owned descendant inherited pipes");
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
        role if role.starts_with("hold-admission:") => {
            let marker = std::path::Path::new(role.strip_prefix("hold-admission:").unwrap());
            let release = marker.with_extension("release");
            std::fs::write(marker, b"owned helper started").unwrap();
            let deadline = Instant::now() + Duration::from_secs(8);
            while !release.is_file() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(2));
            }
            assert!(release.is_file(), "owned admission fixture release missing");
        }
        _ => {}
    }
}
