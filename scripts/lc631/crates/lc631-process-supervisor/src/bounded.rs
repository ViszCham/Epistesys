use crate::{spawn_with_state, ManagedChild, SpawnRequest};
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use std::sync::{Condvar, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

mod buffers;
use buffers::PreparedIoBuffers;
mod root_exit;
use root_exit::await_root_exit;
pub(crate) mod launch;
pub use launch::required_ram_bytes;

mod read;
pub use read::{read_bounded_bytes, BoundedBytes};
mod stdin;
use stdin::StdinPump;
mod resources;
pub use resources::{
    run_bounded_with_resources, run_bounded_with_resources_observed, ResourceRunError,
    ResourceRunOutcome,
};

const MAX_ACTIVE_RUNS: usize = 2;
const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;
const MAX_ARGUMENT_BYTES: usize = 1024 * 1024;

pub struct RunRequest<'a> {
    pub executable: &'a OsStr,
    pub arguments: &'a [OsString],
    pub current_dir: &'a Path,
    pub environment: &'a [(OsString, OsString)],
    pub stdin: Option<&'a [u8]>,
    pub timeout: Duration,
    pub stdout_limit: usize,
    pub stderr_limit: usize,
}

#[derive(Debug)]
pub struct RunOutcome {
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub process_tree_kill_attempted: bool,
    pub process_tree_kill_verified: bool,
    pub io_cleanup_complete: bool,
    pub containment: &'static str,
    pub memory_limit_bytes: Option<usize>,
    pub memory_limit_kind: &'static str,
    pub diagnostic: Option<&'static str>,
}

struct Admission {
    active: Mutex<usize>,
    available: Condvar,
}

struct Permit;

static ADMISSION: OnceLock<Admission> = OnceLock::new();
static RETIRED: Mutex<Vec<RetiredRun>> = Mutex::new(Vec::new());

fn admission() -> &'static Admission {
    ADMISSION.get_or_init(|| Admission {
        active: Mutex::new(0),
        available: Condvar::new(),
    })
}

impl Drop for Permit {
    fn drop(&mut self) {
        let mut count = admission()
            .active
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *count -= 1;
        admission().available.notify_one();
    }
}

struct RetiredRun {
    child: ManagedChild,
    stdin: StdinPump,
    _permit: Option<Permit>,
}

fn reap_retired() {
    let mut retired = RETIRED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut completed = Vec::new();
    let mut index = 0;
    while index < retired.len() {
        let job = &mut retired[index];
        job.stdin.cancel();
        if job.stdin.finished()
            && job
                .child
                .wait_for_tree_empty(Duration::ZERO)
                .unwrap_or(false)
        {
            completed.push(retired.swap_remove(index));
        } else {
            index += 1;
        }
    }
    drop(retired);
    // Drop permits outside RETIRED to preserve mutex acquisition order.
    drop(completed);
}

fn acquire(deadline: Instant) -> Option<Permit> {
    loop {
        reap_retired();
        if Instant::now() >= deadline {
            return None;
        }
        let mut count = admission()
            .active
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if *count < MAX_ACTIVE_RUNS {
            *count += 1;
            return Some(Permit);
        }
        let remaining = deadline.checked_duration_since(Instant::now())?;
        let _ = admission()
            .available
            .wait_timeout(count, remaining.min(Duration::from_millis(5)));
    }
}

/// Runs only a caller-authorized executable. This observes bounded process I/O;
/// it supplies neither network/filesystem isolation nor semantic validation.
pub fn run_bounded(request: RunRequest<'_>) -> io::Result<RunOutcome> {
    run_observed(request, Observers::default())
}

struct Observers {
    poll: fn(&mut ManagedChild) -> io::Result<Option<i32>>,
    tree_empty: fn(&mut ManagedChild) -> io::Result<bool>,
    prepare_io: fn(&RunRequest<'_>) -> io::Result<PreparedIoBuffers>,
    before_spawn: fn(&RunRequest<'_>, &PreparedIoBuffers),
    start_stdin: fn(File, Option<Vec<u8>>) -> io::Result<StdinPump>,
    terminate_tree: fn(&mut ManagedChild, Duration) -> io::Result<bool>,
}

impl Default for Observers {
    fn default() -> Self {
        Self {
            poll: |child| child.try_wait(),
            tree_empty: |child| child.wait_for_tree_empty(Duration::ZERO),
            prepare_io: PreparedIoBuffers::prepare,
            before_spawn: |_, _| {},
            start_stdin: StdinPump::new,
            terminate_tree: |child, timeout| child.terminate_tree(timeout),
        }
    }
}

fn run_observed(request: RunRequest<'_>, observers: Observers) -> io::Result<RunOutcome> {
    run_controlled(request, observers, None).map_err(RunFailure::into_io)
}

enum RunFailure {
    NotStarted(io::Error),
    StartedUnknown(io::Error),
}
impl RunFailure {
    fn into_io(self) -> io::Error {
        match self {
            Self::NotStarted(error) | Self::StartedUnknown(error) => error,
        }
    }
}

fn run_controlled(
    request: RunRequest<'_>,
    observers: Observers,
    resources: Option<&lc631_substrate::resource::ResourceReservationView<'_>>,
) -> Result<RunOutcome, RunFailure> {
    validate(&request).map_err(RunFailure::NotStarted)?;
    let started = Instant::now();
    let deadline = started + request.timeout;
    let cleanup = Duration::from_millis(250).min(request.timeout / 3);
    let work_deadline = deadline - cleanup;
    // Common-owner callers already hold an admitted Child claim. Legacy calls
    // alone use the historical static admission capsule.
    let permit = if resources.is_some() {
        None
    } else {
        acquire(work_deadline)
    };
    if resources.is_none() && permit.is_none() {
        return Ok(RunOutcome {
            exit_code: None,
            timed_out: true,
            stdout: Vec::new(),
            stderr: Vec::new(),
            stdout_truncated: false,
            stderr_truncated: false,
            process_tree_kill_attempted: false,
            process_tree_kill_verified: false,
            io_cleanup_complete: true,
            containment: containment(),
            memory_limit_bytes: None,
            memory_limit_kind: "process_not_started",
            diagnostic: Some("supervisor_admission_deadline_exceeded"),
        });
    }
    let buffers = (observers.prepare_io)(&request).map_err(RunFailure::NotStarted)?;
    buffers.validate(&request).map_err(RunFailure::NotStarted)?;
    (observers.before_spawn)(&request, &buffers);
    let launched = spawn_with_state(SpawnRequest {
        executable: request.executable,
        arguments: request.arguments,
        current_dir: request.current_dir,
        environment: request.environment,
    })
    .map_err(|failure| match failure {
        crate::spawn_failure::SpawnFailure::BeforeChild(error) => RunFailure::NotStarted(error),
        crate::spawn_failure::SpawnFailure::MayHaveStarted(error) => {
            RunFailure::StartedUnknown(error)
        }
    })?;
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
        // Retain ownership and its admission slot. Never silently detach a
        // blocked worker or let repeated failures create unbounded resources.
        RETIRED
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(RetiredRun {
                child,
                stdin,
                _permit: permit,
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

#[cfg(test)]
mod tests;

fn validate(request: &RunRequest<'_>) -> io::Result<()> {
    validate_io(request)?;
    launch::LaunchPlan::checked(request)
        .map(|_| ())
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "supervisor_launch_budget_exceeded",
            )
        })
}

fn validate_io(request: &RunRequest<'_>) -> io::Result<()> {
    if request.timeout.is_zero()
        || request.timeout > Duration::from_secs(1_800)
        || request.stdout_limit > MAX_OUTPUT_BYTES
        || request.stderr_limit > MAX_OUTPUT_BYTES
        || request
            .stdin
            .is_some_and(|input| input.len() > MAX_INPUT_BYTES)
        || request.arguments.len() > 128
        || request
            .arguments
            .iter()
            .try_fold(0_usize, |size, value| size.checked_add(value.len()))
            .is_none_or(|size| size > MAX_ARGUMENT_BYTES)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "supervisor request budget exceeded",
        ));
    }
    Ok(())
}

fn containment() -> &'static str {
    if cfg!(windows) {
        "windows_job"
    } else {
        "unix_process_group"
    }
}

struct Reader {
    file: File,
    bytes: Vec<u8>,
    limit: usize,
    truncated: bool,
    eof: bool,
}

impl Reader {
    fn new(file: File, bytes: Vec<u8>, limit: usize) -> Self {
        Self {
            file,
            bytes,
            limit,
            truncated: false,
            eof: false,
        }
    }

    fn drain(&mut self) -> io::Result<()> {
        let mut buffer = [0_u8; 8_192];
        // Bound each pump turn as well as captured bytes so a continuously
        // writable child cannot starve deadline checks or the other pipe.
        for _ in 0..8 {
            if self.eof {
                break;
            }
            match read_available(&mut self.file, &mut buffer) {
                Ok(0) => {
                    self.eof = true;
                    break;
                }
                Ok(read) => {
                    let keep = read.min(self.limit.saturating_sub(self.bytes.len()));
                    self.bytes.extend_from_slice(&buffer[..keep]);
                    self.truncated |= keep < read;
                    if self.truncated {
                        break;
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }
}

#[cfg(unix)]
fn read_available(file: &mut File, buffer: &mut [u8]) -> io::Result<usize> {
    file.read(buffer)
}

#[cfg(windows)]
fn read_available(file: &mut File, buffer: &mut [u8]) -> io::Result<usize> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::{ERROR_BROKEN_PIPE, ERROR_PIPE_NOT_CONNECTED};
    use windows_sys::Win32::System::Pipes::PeekNamedPipe;
    let mut available = 0_u32;
    // SAFETY: file owns a readable pipe endpoint created by our spawn capsule.
    // No data is copied; available points to one writable u32.
    if unsafe {
        PeekNamedPipe(
            file.as_raw_handle().cast(),
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            &mut available,
            std::ptr::null_mut(),
        )
    } == 0
    {
        let error = io::Error::last_os_error();
        if matches!(
            error.raw_os_error().map(|value| value as u32),
            Some(ERROR_BROKEN_PIPE | ERROR_PIPE_NOT_CONNECTED)
        ) {
            return Ok(0);
        }
        return Err(error);
    }
    if available == 0 {
        return Err(io::ErrorKind::WouldBlock.into());
    }
    let length = buffer.len().min(available as usize);
    // Only this reader owns the endpoint, so the observed bytes cannot be
    // consumed by another reader before this synchronous read.
    file.read(&mut buffer[..length])
}
