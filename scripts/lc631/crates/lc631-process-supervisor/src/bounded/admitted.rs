//! Dedicated admitted controller: partial owners return to the same funded attempt.
#![forbid(unsafe_code)]
use super::*;
use crate::native_attempt::NativeSpawnFailure;
pub(super) enum ControlledFailure<E> {
    BeforeChild(io::Error),
    Native(NativeSpawnFailure<E>),
}
pub(super) struct PartialIoOwner {
    pub(super) child: ManagedChild,
    pub(super) stdin: StdinPump,
    pub(super) stdout: File,
    pub(super) stderr: File,
}
impl PartialIoOwner {
    pub(super) fn cleanup(&mut self, timeout: Duration) -> io::Result<bool> {
        self.stdin.cancel();
        let deadline = Instant::now() + timeout;
        let tree = self.child.terminate_tree(timeout)?;
        let mut scratch = [0; 8192];
        // Drain only owned handles, under the separately bounded cleanup horizon.
        while Instant::now() < deadline && !self.stdin.finished() {
            let _ = read_available(&mut self.stdout, &mut scratch);
            let _ = read_available(&mut self.stderr, &mut scratch);
            thread::sleep(Duration::from_millis(2));
        }
        Ok(tree && self.stdin.finished())
    }
}
impl Drop for PartialIoOwner {
    fn drop(&mut self) {
        let _ = self.cleanup(Duration::from_millis(250));
    }
}

pub(super) fn run_admitted<E>(
    request: RunRequest<'_>,
    observers: Observers,
    view: &lc631_substrate::resource::ResourceReservationView<'_>,
    admit: &mut dyn FnMut() -> Result<(), E>,
    retained: &mut Option<PartialIoOwner>,
) -> Result<RunOutcome, ControlledFailure<E>> {
    validate(&request).map_err(ControlledFailure::BeforeChild)?;
    let started = Instant::now();
    let deadline = started + request.timeout;
    let cleanup = Duration::from_millis(250).min(request.timeout / 3);
    let work_deadline = deadline - cleanup;
    let resources = Some(view);
    let buffers = (observers.prepare_io)(&request).map_err(ControlledFailure::BeforeChild)?;
    buffers
        .validate(&request)
        .map_err(ControlledFailure::BeforeChild)?;
    (observers.before_spawn)(&request, &buffers);
    // Native platform preparation is still nested here. Only spawn_owned
    // invokes the fallible callback immediately before actual OS creation.
    let launched = crate::spawn_owned(
        SpawnRequest {
            executable: request.executable,
            arguments: request.arguments,
            current_dir: request.current_dir,
            environment: request.environment,
        },
        view,
        work_deadline,
        admit,
    )
    .map_err(ControlledFailure::Native)?;
    let mut child = launched.child;
    let memory_limit_bytes = child.memory_limit_bytes().unwrap_or(None);
    let mut stdout = Reader::new(launched.stdout, buffers.stdout, request.stdout_limit);
    let mut stderr = Reader::new(launched.stderr, buffers.stderr, request.stderr_limit);
    let (mut stdin, stdin_start_failed) =
        match (observers.start_stdin)(launched.stdin, buffers.stdin) {
            Ok(stdin) => (stdin, false),
            Err(_error) => (StdinPump::closed_after_start_failure(), true),
        };
    let mut exit_code = None;
    let mut timed_out = false;
    let mut diagnostic = if stdin_start_failed {
        Some("supervisor_stdin_thread_start_failed")
    } else {
        None
    };
    let mut tree_empty = false;
    loop {
        if stdin_start_failed {
            break;
        }
        if let Some(resources) = resources {
            if let Err(error) = resources.check() {
                timed_out = error == lc631_substrate::resource::ResourceError::DeadlineExceeded;
                diagnostic = Some(if timed_out {
                    "supervisor_deadline_exceeded"
                } else if error == lc631_substrate::resource::ResourceError::Cancelled {
                    "supervisor_cancelled"
                } else {
                    "supervisor_resource_scope_failed"
                });
                break;
            }
        }
        if stdout.drain().is_err() || stderr.drain().is_err() {
            diagnostic = Some("supervisor_pipe_read_failed");
            break;
        }
        if stdout.truncated || stderr.truncated {
            diagnostic = Some("supervisor_output_budget_exceeded");
            break;
        }
        if stdin.advance().is_err() {
            diagnostic = Some("supervisor_stdin_write_failed");
            break;
        }
        if exit_code.is_none() {
            match (observers.poll)(&mut child) {
                Ok(status) => exit_code = status,
                Err(_) => {
                    diagnostic = Some("supervisor_process_wait_failed");
                    break;
                }
            }
        }
        if exit_code.is_some() && stdout.eof && stderr.eof && stdin.finished() {
            tree_empty = match (observers.tree_empty)(&mut child) {
                Ok(empty) => empty,
                Err(_) => {
                    diagnostic = Some("supervisor_tree_observation_failed");
                    break;
                }
            };
            if tree_empty {
                break;
            }
        }
        if Instant::now() >= work_deadline {
            timed_out = true;
            diagnostic = Some("supervisor_deadline_exceeded");
            break;
        }
        thread::sleep(Duration::from_millis(2));
    }
    // Cleanup has its own already-funded bounded allowance after work stops.
    let deadline = deadline.min(Instant::now() + cleanup);
    let killed = !tree_empty;
    if killed {
        stdin.cancel();
        tree_empty = (observers.terminate_tree)(
            &mut child,
            deadline.saturating_duration_since(Instant::now()),
        )
        .unwrap_or(false);
    }
    while Instant::now() < deadline {
        let _ = stdout.drain();
        let _ = stderr.drain();
        stdin.cancel_if_finished_run();
        if stdout.eof && stderr.eof && stdin.finished() {
            break;
        }
        thread::sleep(Duration::from_millis(2));
    }
    match await_root_exit(
        exit_code,
        deadline,
        || (observers.poll)(&mut child),
        Instant::now,
        thread::sleep,
    ) {
        Ok(status) => exit_code = status,
        Err(_) => {
            exit_code = None;
            diagnostic = Some("supervisor_process_wait_failed");
        }
    }
    let cleanup_complete = stdin.finished() && stdout.eof && stderr.eof;
    if stdin.error && diagnostic.is_none() {
        diagnostic = Some("supervisor_stdin_write_failed");
    }
    if !cleanup_complete || !tree_empty {
        if let Some(resources) = resources {
            resources.record_cleanup_unknown();
        }
        diagnostic = Some(if stdin_start_failed {
            "supervisor_stdin_thread_start_failed_cleanup_unverified"
        } else {
            "supervisor_cleanup_unverified"
        });
        // Return actual ownership with explicit CleanupUnverified. Drop is
        // best effort, not a cleanup receipt; the lease remains quarantined.
        *retained = Some(PartialIoOwner {
            child,
            stdin,
            stdout: stdout.file,
            stderr: stderr.file,
        });
    }
    Ok(RunOutcome {
        exit_code,
        timed_out,
        stdout: stdout.bytes,
        stderr: stderr.bytes,
        stdout_truncated: stdout.truncated,
        stderr_truncated: stderr.truncated,
        process_tree_kill_attempted: killed,
        process_tree_kill_verified: tree_empty,
        io_cleanup_complete: cleanup_complete,
        containment: containment(),
        memory_limit_bytes,
        memory_limit_kind: if cfg!(windows) && memory_limit_bytes.is_some() {
            "job_wide_committed_memory"
        } else {
            "unobserved_by_supervisor"
        },
        diagnostic,
    })
}
