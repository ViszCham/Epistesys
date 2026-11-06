//! Opt-in Windows launch diagnosis. No model, media worker, or product mutation.
//! The per-lease issuer below is a synthetic test owner, not an aggregate broker.
#![cfg(windows)]

use lc631_process_supervisor::{
    required_ram_bytes, run_bounded, run_bounded_with_resources_observed, Digest32, ResourceAmount,
    ResourceKind, ResourceRequest, ResourceReservationView, RunOutcome, RunRequest,
};
use lc631_substrate::resource::{
    Cancellation, Deadline, ReportedResourceLease, ResourceLeaseIssuer,
};
use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, Read};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::AsRawHandle;
use std::os::windows::process::CommandExt;
use std::path::{Component, Path, PathBuf, Prefix};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use windows_sys::Win32::Foundation::{ERROR_BROKEN_PIPE, ERROR_PIPE_NOT_CONNECTED};
use windows_sys::Win32::System::Pipes::PeekNamedPipe;
use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;

const PYTHON_VAR: &str = "EPISTESYS_TEST_MEDIA_PYTHON";
const FIXTURE_PARENT_VAR: &str = "EPISTESYS_TEST_PROCESS_DIAG_ROOT";
const EXPECTED_PYTHON_SHA256: &str =
    "4942b86a6597e5aee0128daa00050ed79bc21f6e709a78eb19cbfeb0c2f39ac9";
const STREAM_CAP: usize = 4_096;
#[derive(Clone, Copy, Debug)]
struct DiagnosticProfile {
    label: &'static str,
    request_timeout: Duration,
    nominal_managed_work: Duration,
    lease_timeout: Duration,
}
const BASELINE_10S: DiagnosticProfile = DiagnosticProfile {
    label: "baseline_10s_direct_work10000ms_managed_work9750ms",
    request_timeout: Duration::from_secs(10),
    nominal_managed_work: Duration::from_millis(9_750),
    lease_timeout: Duration::from_secs(15),
};
const SUPPLEMENTAL_30S: DiagnosticProfile = DiagnosticProfile {
    label: "supplemental_30s_direct_work30000ms_managed_work29750ms",
    request_timeout: Duration::from_secs(30),
    nominal_managed_work: Duration::from_millis(29_750),
    lease_timeout: Duration::from_secs(45),
};
const DIRECT_CLEANUP: Duration = Duration::from_millis(500);
const INTERPRETER_RAM_HEADROOM: u64 = 256 * 1024 * 1024;

struct LaunchTuple {
    executable: PathBuf,
    arguments: Vec<OsString>,
    environment: Vec<(OsString, OsString)>,
    current_dir: PathBuf,
}
impl LaunchTuple {
    fn request(&self, profile: DiagnosticProfile) -> RunRequest<'_> {
        RunRequest {
            executable: self.executable.as_os_str(),
            arguments: &self.arguments,
            current_dir: &self.current_dir,
            environment: &self.environment,
            stdin: None,
            timeout: profile.request_timeout,
            stdout_limit: STREAM_CAP,
            stderr_limit: STREAM_CAP,
        }
    }
}

fn lowercase_sha256_hex(bytes: impl IntoIterator<Item = u8>) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(64);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    assert_eq!(encoded.len(), 64, "expected a 32-byte SHA-256 digest");
    encoded
}

fn sha256_file(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    let mut scratch = [0_u8; 8_192];
    let mut total = 0_u64;
    loop {
        let count = file.read(&mut scratch)?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count as u64)
            .ok_or(io::ErrorKind::InvalidData)?;
        if total > 64 * 1024 * 1024 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "diagnostic executable hash cap exceeded",
            ));
        }
        digest.update(&scratch[..count]);
    }
    Ok(lowercase_sha256_hex(digest.finalize().iter().copied()))
}

// Record the exact immutable requested environment without printing any values.
fn environment_digest(environment: &[(OsString, OsString)]) -> String {
    let mut digest = Sha256::new();
    for (key, value) in environment {
        for part in [key, value] {
            let units = part.encode_wide().collect::<Vec<_>>();
            digest.update((units.len() as u64).to_le_bytes());
            for unit in units {
                digest.update(unit.to_le_bytes());
            }
        }
    }
    lowercase_sha256_hex(digest.finalize().iter().copied())
}

fn owned_d_fixture() -> PathBuf {
    let parent = std::env::var_os(FIXTURE_PARENT_VAR)
        .map(PathBuf::from)
        .expect("BETA7_PROCESS_SETUP: explicit EPISTESYS_TEST_PROCESS_DIAG_ROOT is required");
    assert!(
        parent.is_absolute(),
        "BETA7_PROCESS_SETUP: fixture parent must be absolute"
    );
    let parent =
        fs::canonicalize(parent).expect("BETA7_PROCESS_SETUP: fixture parent must already exist");
    let drive = match parent.components().next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            Prefix::Disk(drive) | Prefix::VerbatimDisk(drive) => drive,
            _ => panic!("BETA7_PROCESS_SETUP: fixture parent must be a D-drive disk directory"),
        },
        _ => panic!("BETA7_PROCESS_SETUP: fixture parent must be a D-drive disk directory"),
    };
    assert!(
        drive == b'D' || drive == b'd',
        "BETA7_PROCESS_SETUP: this diagnostic requires the central D-drive fixture parent"
    );
    assert!(
        parent.is_dir(),
        "BETA7_PROCESS_SETUP: fixture parent is not a directory"
    );
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let fixture = parent.join(format!(
        "beta7-python-parity-{}-{stamp}",
        std::process::id()
    ));
    assert!(fixture.is_absolute() && fixture.starts_with(&parent));
    fs::create_dir(&fixture).expect("BETA7_PROCESS_SETUP: fresh fixture creation failed");
    // Retain this exact empty diagnostic cwd. No recursive cleanup or prior-root deletion.
    fixture
}

// Same nonblocking polling pattern as bounded.rs:483-518, applied only to this
// control's std::process-owned readable pipe. No reader thread or join is used.
fn read_available<R: Read + AsRawHandle>(reader: &mut R, buffer: &mut [u8]) -> io::Result<usize> {
    let mut available = 0_u32;
    // SAFETY: the caller retains this readable pipe; only this reader consumes it.
    // No data is copied by PeekNamedPipe, and available is one writable u32.
    if unsafe {
        PeekNamedPipe(
            reader.as_raw_handle().cast(),
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
    reader.read(&mut buffer[..length])
}

struct CappedPipe<R> {
    reader: R,
    bytes: Vec<u8>,
    truncated: bool,
    eof: bool,
}
impl<R: Read + AsRawHandle> CappedPipe<R> {
    fn new(reader: R) -> Self {
        Self {
            reader,
            bytes: Vec::with_capacity(STREAM_CAP),
            truncated: false,
            eof: false,
        }
    }

    fn drain(&mut self) -> io::Result<()> {
        let mut scratch = [0_u8; 1_024];
        for _ in 0..8 {
            if self.eof || self.truncated {
                break;
            }
            match read_available(&mut self.reader, &mut scratch) {
                Ok(0) => self.eof = true,
                Ok(count) => {
                    let keep = count.min(STREAM_CAP.saturating_sub(self.bytes.len()));
                    self.bytes.extend_from_slice(&scratch[..keep]);
                    self.truncated = keep < count;
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }
}

struct OwnedDirectChild {
    child: Child,
    exit_observed: bool,
    exit_code: Option<i32>,
}
impl OwnedDirectChild {
    fn poll(&mut self) -> io::Result<()> {
        if !self.exit_observed {
            if let Some(status) = self.child.try_wait()? {
                self.exit_observed = true;
                self.exit_code = status.code();
            }
        }
        Ok(())
    }
}
impl Drop for OwnedDirectChild {
    fn drop(&mut self) {
        if !self.exit_observed {
            // Only this test's newly spawned root is eligible. Never wait or
            // claim cleanup from this best-effort fallback.
            let _ = self.child.kill();
        }
    }
}

#[derive(Debug)]
struct DirectObservation {
    exit_code: Option<i32>,
    timed_out: bool,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    stdout_truncated: bool,
    stderr_truncated: bool,
    root_exit_observed: bool,
    io_cleanup_complete: bool,
    kill_attempted: bool,
    kill_error: Option<io::ErrorKind>,
    diagnostic: Option<io::ErrorKind>,
    elapsed_millis: u128,
}
impl DirectObservation {
    fn positive(&self) -> bool {
        self.exit_code == Some(0)
            && !self.timed_out
            && self.root_exit_observed
            && self.io_cleanup_complete
            && !self.kill_attempted
            && self.kill_error.is_none()
            && self.diagnostic.is_none()
            && !self.stdout_truncated
            && !self.stderr_truncated
            && !self.stdout.is_empty()
            && self.stderr.is_empty()
    }
}

fn direct_control(
    tuple: &LaunchTuple,
    profile: DiagnosticProfile,
) -> io::Result<DirectObservation> {
    let started = Instant::now();
    let deadline = started + profile.request_timeout;
    let child = Command::new(&tuple.executable)
        .args(&tuple.arguments)
        .env_clear()
        .envs(tuple.environment.iter().map(|(key, value)| (key, value)))
        .current_dir(&tuple.current_dir)
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut owned = OwnedDirectChild {
        child,
        exit_observed: false,
        exit_code: None,
    };
    // EOF is delivered through a closed anonymous pipe, matching managed stdin=None.
    drop(owned.child.stdin.take());
    let stdout = owned
        .child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("direct stdout missing"))?;
    let stderr = owned
        .child
        .stderr
        .take()
        .ok_or_else(|| io::Error::other("direct stderr missing"))?;
    let mut stdout = CappedPipe::new(stdout);
    let mut stderr = CappedPipe::new(stderr);
    let mut timed_out = false;
    let mut diagnostic = None;
    loop {
        if let Err(error) = stdout
            .drain()
            .and_then(|_| stderr.drain())
            .and_then(|_| owned.poll())
        {
            diagnostic = Some(error.kind());
            break;
        }
        if owned.exit_observed && stdout.eof && stderr.eof {
            break;
        }
        if stdout.truncated || stderr.truncated {
            break;
        }
        if Instant::now() >= deadline {
            timed_out = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    let kill_attempted = !owned.exit_observed;
    let kill_error = if kill_attempted {
        owned.child.kill().err().map(|error| error.kind())
    } else {
        None
    };
    let cleanup_deadline = Instant::now() + DIRECT_CLEANUP;
    while Instant::now() < cleanup_deadline {
        if let Err(error) = stdout
            .drain()
            .and_then(|_| stderr.drain())
            .and_then(|_| owned.poll())
        {
            diagnostic.get_or_insert(error.kind());
            break;
        }
        if owned.exit_observed && stdout.eof && stderr.eof {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    Ok(DirectObservation {
        exit_code: owned.exit_code,
        timed_out,
        stdout: stdout.bytes,
        stderr: stderr.bytes,
        stdout_truncated: stdout.truncated,
        stderr_truncated: stderr.truncated,
        root_exit_observed: owned.exit_observed,
        io_cleanup_complete: stdout.eof && stderr.eof,
        kill_attempted,
        kill_error,
        diagnostic,
        elapsed_millis: started.elapsed().as_millis(),
    })
}

fn complete_managed(outcome: &RunOutcome, direct_stdout: &[u8]) -> bool {
    outcome.exit_code == Some(0)
        && !outcome.timed_out
        && outcome.stdout == direct_stdout
        && outcome.stderr.is_empty()
        && !outcome.stdout_truncated
        && !outcome.stderr_truncated
        && !outcome.process_tree_kill_attempted
        && outcome.process_tree_kill_verified
        && outcome.io_cleanup_complete
        && outcome.containment == "windows_job"
        && outcome.memory_limit_bytes.is_some()
        && outcome.memory_limit_kind == "job_wide_committed_memory"
        && outcome.diagnostic.is_none()
}

fn report_view(stage: &str, view: &ResourceReservationView<'_>) {
    let amounts = ResourceKind::ALL.map(|kind| (kind, view.amount(kind)));
    eprintln!(
        "BETA7_LEASE {stage}: reserved_amounts={amounts:?} remaining_deadline={:?} check={:?}",
        view.remaining(),
        view.check(),
    );
    // amount() reports the admitted amount, not unused live quota. The public
    // view exposes no free-quota counter; do not relabel these as such.
}

#[test]
#[ignore = "opt-in central Windows diagnosis; requires explicit pinned Python and D-drive fixture parent"]
fn beta7_windows_python_direct_and_managed_exact_tuple() {
    run_profile(BASELINE_10S);
}

#[test]
#[ignore = "supplemental 30s Windows diagnosis; distinct from the preserved 10s baseline"]
fn beta7_windows_python_direct_and_managed_exact_tuple_supplemental_30s() {
    run_profile(SUPPLEMENTAL_30S);
}

fn run_profile(profile: DiagnosticProfile) {
    let profile_started = Instant::now();
    let lease_timeout = profile.lease_timeout;
    eprintln!(
        "BETA7_PROFILE label={} requested_timeout={:?} lease_timeout={:?} \
         direct_work={:?} nominal_managed_work={:?} direct_cleanup_allowance={:?} \
         managed_work_basis=request_timeout_minus_existing_250ms_cleanup_reserve",
        profile.label,
        profile.request_timeout,
        profile.lease_timeout,
        profile.request_timeout,
        profile.nominal_managed_work,
        DIRECT_CLEANUP,
    );
    let executable = std::env::var_os(PYTHON_VAR)
        .map(PathBuf::from)
        .expect("BETA7_PROCESS_SETUP: EPISTESYS_TEST_MEDIA_PYTHON is required; no fallback");
    assert!(
        executable.is_absolute(),
        "BETA7_PROCESS_SETUP: Python must be an absolute path"
    );
    assert!(
        executable.is_file(),
        "BETA7_PROCESS_SETUP: Python executable is not a file"
    );
    let executable_digest =
        sha256_file(&executable).expect("BETA7_PROCESS_SETUP: executable hashing failed");
    assert_eq!(
        executable_digest, EXPECTED_PYTHON_SHA256,
        "BETA7_PROCESS_SETUP: Python pin differs"
    );
    let tuple = LaunchTuple {
        executable,
        arguments: vec![
            OsString::from("-c"),
            OsString::from("import sys; print(sys.executable)"),
        ],
        environment: std::env::vars_os().collect(),
        current_dir: owned_d_fixture(),
    };
    let required_ram = required_ram_bytes(&tuple.request(profile))
        .expect("BETA7_PROCESS_SETUP: exact launch tuple exceeds the existing launch plan");
    let requested_ram = required_ram
        .checked_add(INTERPRETER_RAM_HEADROOM)
        .expect("BETA7_PROCESS_SETUP: diagnostic RAM overflow");
    eprintln!(
        "BETA7_TUPLE executable={:?} executable_sha256={} args={:?} cwd={:?} \
         stdin=None timeout={:?} stdout_cap={} stderr_cap={} environment_count={} \
         environment_sha256={} direct_creation_flags=CREATE_NO_WINDOW",
        tuple.executable,
        executable_digest,
        tuple.arguments,
        tuple.current_dir,
        profile.request_timeout,
        STREAM_CAP,
        STREAM_CAP,
        tuple.environment.len(),
        environment_digest(&tuple.environment),
    );
    let direct_started = Instant::now();
    let direct = direct_control(&tuple, profile).expect(
        "BETA7_PROCESS_SETUP: bounded std::process direct control failed to launch/observe",
    );
    let direct_wall = direct_started.elapsed();
    eprintln!(
        "BETA7_DIRECT_WALL profile={} elapsed={direct_wall:?}",
        profile.label
    );
    eprintln!("BETA7_DIRECT_OBSERVATION {direct:#?}");
    assert!(
        direct.positive(),
        "BETA7_PROCESS_SETUP: direct positive not established; not a managed behavioral RED"
    );

    let legacy_started = Instant::now();
    let legacy = run_bounded(tuple.request(profile));
    let legacy_ok = match &legacy {
        Ok(outcome) => {
            eprintln!("BETA7_LEGACY_RUN_OUTCOME {outcome:#?}");
            complete_managed(outcome, &direct.stdout)
        }
        Err(error) => {
            eprintln!(
                "BETA7_LEGACY_OUTCOME unavailable error_kind={:?} error={error:?}",
                error.kind(),
            );
            false
        }
    };
    let legacy_wall = legacy_started.elapsed();
    eprintln!(
        "BETA7_LEGACY_WALL profile={} elapsed={legacy_wall:?}",
        profile.label
    );

    let amount = |value| ResourceAmount::checked(value).expect("BETA7_PROCESS_SETUP: zero amount");
    let requests = [
        ResourceRequest::new(ResourceKind::Ram, amount(requested_ram)),
        ResourceRequest::new(ResourceKind::Output, amount((2 * STREAM_CAP) as u64)),
        ResourceRequest::new(ResourceKind::Child, amount(1)),
        ResourceRequest::new(ResourceKind::Cleanup, amount(1)),
    ];
    let cancellation = Cancellation::default();
    let report = ReportedResourceLease::checked(
        7,
        Digest32::from_bytes([0xb7; 32]),
        &requests,
        Deadline::after(profile.lease_timeout)
            .expect("BETA7_PROCESS_SETUP: invalid lease deadline"),
        cancellation.clone(),
    )
    .expect("BETA7_PROCESS_SETUP: invalid per-lease report");
    let applied_amounts = *report.amounts();
    let issuer = ResourceLeaseIssuer::new().expect("BETA7_PROCESS_SETUP: issuer unavailable");
    let lease = issuer
        .issue_admitted(7, report)
        .expect("BETA7_PROCESS_SETUP: test-only per-lease admission failed");
    eprintln!(
        "BETA7_RESOURCE_PLAN basis=test_only_per_lease_no_aggregate_broker \
         required_parent_ram={} interpreter_headroom={} requested={requests:?} \
         applied_amounts={applied_amounts:?} lease_timeout={lease_timeout:?} \
         os_child_memory_limit=reported_separately_in_RunOutcome",
        required_ram, INTERPRETER_RAM_HEADROOM,
    );
    let (owned_ok, owned_wall) = {
        let view = lease.view();
        report_view("before_managed", &view);
        let owned_started = Instant::now();
        let owned = run_bounded_with_resources_observed(tuple.request(profile), &view);
        let ok = match &owned {
            Ok(run) => {
                eprintln!("BETA7_RESOURCE_RUN_OUTCOME {:#?}", run.outcome());
                eprintln!(
                    "BETA7_RESOURCE_STATE cleanup_complete={} cancelled={} token_cancelled={}",
                    run.cleanup_complete(),
                    run.cancelled(),
                    cancellation.is_cancelled(),
                );
                run.cleanup_complete()
                    && !run.cancelled()
                    && complete_managed(run.outcome(), &direct.stdout)
            }
            Err(error) => {
                eprintln!("BETA7_RESOURCE_OUTCOME unavailable typed_error={error:?}");
                false
            }
        };
        let owned_wall = owned_started.elapsed();
        eprintln!(
            "BETA7_RESOURCE_WALL profile={} elapsed={owned_wall:?}",
            profile.label
        );
        report_view("outcome_live", &view);
        drop(owned); // Release the real per-lease capture claim before settlement.
        report_view("after_capture_drop", &view);
        (ok, owned_wall)
    };
    let settlement = lease.close_confirmed();
    eprintln!(
        "BETA7_RESOURCE_SETTLEMENT close_confirmed={settlement:?} token_cancelled={} \
         aggregate_reservation_refund=not_applicable",
        cancellation.is_cancelled(),
    );
    eprintln!(
        "BETA7_PROFILE_TOTAL label={} direct_wall={direct_wall:?} \
         legacy_wall={legacy_wall:?} resource_wall={owned_wall:?} total_wall={:?}",
        profile.label,
        profile_started.elapsed(),
    );
    assert!(
        legacy_ok && owned_ok && settlement.is_ok(),
        "BETA7_MANAGED_PARITY: direct positive passed; inspect full legacy/resource outcomes and settlement"
    );
}
