use std::collections::BTreeMap;
use std::env;
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
#[cfg(not(windows))]
use std::process::{Child, Command, Stdio};
use std::sync::{Condvar, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use lc631_core::stable_sha256;
use serde::Serialize;
use sha2::{Digest, Sha256};

const MAX_TOOL_ARGUMENTS: usize = 128;
const MAX_TOOL_ARGUMENT_BYTES: usize = 1024 * 1024;
const MAX_TOOL_STDIN_BYTES: usize = 2 * 1024 * 1024;
const MAX_TOOL_OUTPUT_BYTES: usize = 2 * 1024 * 1024 + 4;
const MAX_TOOL_TIMEOUT_MS: u64 = 120_000;
const MAX_ACTIVE_TOOL_JOBS: usize = 2;

struct ProcessAdmission {
    active: Mutex<usize>,
    available: Condvar,
}

struct ProcessPermit {
    admission: &'static ProcessAdmission,
}

static PROCESS_ADMISSION: OnceLock<ProcessAdmission> = OnceLock::new();

fn process_admission() -> &'static ProcessAdmission {
    PROCESS_ADMISSION.get_or_init(|| ProcessAdmission {
        active: Mutex::new(0),
        available: Condvar::new(),
    })
}

fn acquire_process_permit(deadline: Instant) -> Option<ProcessPermit> {
    let admission = process_admission();
    let mut active = admission
        .active
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    loop {
        if *active < MAX_ACTIVE_TOOL_JOBS {
            *active += 1;
            return Some(ProcessPermit { admission });
        }
        let remaining = deadline.checked_duration_since(Instant::now())?;
        let (next, timeout) = admission
            .available
            .wait_timeout(active, remaining)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        active = next;
        if timeout.timed_out() && *active >= MAX_ACTIVE_TOOL_JOBS {
            return None;
        }
    }
}

impl Drop for ProcessPermit {
    fn drop(&mut self) {
        let mut active = self
            .admission
            .active
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *active = active.saturating_sub(1);
        self.admission.available.notify_one();
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolRunState {
    Observed,
    Unavailable,
    TimedOut,
    Failed,
}

pub struct BoundedToolRequest<'a> {
    pub program: &'a str,
    pub args: &'a [&'a str],
    pub stdin: Option<&'a [u8]>,
    pub timeout_ms: u64,
    pub stdout_limit: usize,
    pub stderr_limit: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BoundedToolResult {
    pub state: ToolRunState,
    pub program: String,
    pub args_digest: String,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub stdout_digest: String,
    pub stderr_digest: String,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub process_tree_kill_attempted: bool,
    pub process_tree_kill_verified: bool,
    pub orphan_risk: String,
    pub diagnostic: Option<String>,
    pub resolved_executable: Option<String>,
    pub executable_sha256: Option<String>,
    pub cwd_digest: String,
    pub environment_digest: String,
    pub environment_allowlist_count: usize,
    pub network_policy: String,
    pub network_isolation_os_enforced: bool,
    pub hermetic_execution_observed: bool,
    #[serde(skip)]
    pub stdout: Vec<u8>,
    #[serde(skip)]
    pub stderr: Vec<u8>,
}

struct ProcessHandle {
    #[cfg(windows)]
    inner: lc631_process_supervisor::ManagedChild,
    #[cfg(not(windows))]
    inner: Child,
}

struct SpawnedTool {
    child: ProcessHandle,
    stdin: Option<Box<dyn Write + Send>>,
    stdout: Box<dyn Read + Send>,
    stderr: Box<dyn Read + Send>,
}

impl ProcessHandle {
    fn try_wait(&mut self) -> std::io::Result<Option<i32>> {
        #[cfg(windows)]
        {
            self.inner.try_wait()
        }
        #[cfg(not(windows))]
        {
            self.inner
                .try_wait()
                .map(|status| status.map(|status| status.code().unwrap_or(-1)))
        }
    }

    fn terminate_tree(&mut self) -> KillReceipt {
        #[cfg(windows)]
        {
            KillReceipt {
                attempted: true,
                verified: self
                    .inner
                    .terminate_tree(Duration::from_millis(500))
                    .unwrap_or(false),
                orphan_risk: if self
                    .inner
                    .active_process_count()
                    .is_ok_and(|count| count == 0)
                {
                    "none_observed"
                } else {
                    "unknown"
                },
            }
        }
        #[cfg(not(windows))]
        {
            kill_tree(&mut self.inner)
        }
    }

    fn kill_root(&mut self) {
        #[cfg(windows)]
        {
            let _ = self.inner.terminate_tree(Duration::from_millis(500));
        }
        #[cfg(not(windows))]
        {
            let _ = self.inner.kill();
        }
    }

    fn wait_for_tree_empty_until(&self, deadline: Instant) -> std::io::Result<Option<bool>> {
        #[cfg(windows)]
        {
            Ok(Some(self.inner.wait_for_tree_empty(
                deadline.saturating_duration_since(Instant::now()),
            )?))
        }
        #[cfg(not(windows))]
        {
            Ok(None)
        }
    }
}

#[cfg(windows)]
fn spawn_tool(
    executable: &OsStr,
    args: &[&str],
    cwd: &Path,
    environment: &BTreeMap<OsString, OsString>,
    input: Option<&[u8]>,
) -> std::io::Result<SpawnedTool> {
    let args = args.iter().map(OsString::from).collect::<Vec<_>>();
    let environment = environment
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<Vec<_>>();
    let launched = lc631_process_supervisor::spawn(lc631_process_supervisor::SpawnRequest {
        executable,
        arguments: &args,
        current_dir: cwd,
        environment: &environment,
    })?;
    let stdin = input.map(|_| Box::new(launched.stdin) as Box<dyn Write + Send>);
    Ok(SpawnedTool {
        child: ProcessHandle {
            inner: launched.child,
        },
        stdin,
        stdout: Box::new(launched.stdout),
        stderr: Box::new(launched.stderr),
    })
}

#[cfg(not(windows))]
fn spawn_tool(
    executable: &OsStr,
    args: &[&str],
    cwd: &Path,
    environment: &BTreeMap<OsString, OsString>,
    input: Option<&[u8]>,
) -> std::io::Result<SpawnedTool> {
    let mut command = Command::new(executable);
    command
        .args(args)
        .current_dir(cwd)
        .env_clear()
        .envs(environment)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn()?;
    let stdin = input.and_then(|_| {
        child
            .stdin
            .take()
            .map(|writer| Box::new(writer) as Box<dyn Write + Send>)
    });
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| std::io::Error::other("child stdout unavailable"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| std::io::Error::other("child stderr unavailable"))?;
    Ok(SpawnedTool {
        child: ProcessHandle { inner: child },
        stdin,
        stdout: Box::new(stdout),
        stderr: Box::new(stderr),
    })
}

pub fn run_bounded_tool(request: BoundedToolRequest<'_>) -> BoundedToolResult {
    if !request_within_budget(&request) {
        return rejected_request(
            request.program,
            ToolRunState::Failed,
            "tool_request_budget_exceeded",
        );
    }
    let start = Instant::now();
    let timeout = Duration::from_millis(request.timeout_ms);
    let deadline = start + timeout;
    let args_digest = stable_sha256(&request.args.join("\u{1f}"));
    let provenance = execution_provenance(request.program);
    let Some(executable) = provenance.resolved_path.as_ref() else {
        return unavailable(
            request.program,
            args_digest,
            "executable_not_resolved".into(),
            provenance,
        );
    };
    let Some(_process_permit) = acquire_process_permit(deadline) else {
        return rejected_request(
            request.program,
            ToolRunState::TimedOut,
            "tool_supervisor_queue_deadline_exceeded",
        );
    };
    let cleanup_grace = Duration::from_millis(100).min(timeout / 4);
    let work_deadline = deadline.checked_sub(cleanup_grace).unwrap_or(start);
    let spawned = match spawn_tool(
        executable.as_os_str(),
        request.args,
        &provenance.cwd,
        &provenance.environment,
        request.stdin,
    ) {
        Ok(spawned) => spawned,
        Err(error) => {
            return unavailable(request.program, args_digest, error.to_string(), provenance);
        }
    };
    let stdin_writer = request.stdin.and_then(|input| {
        spawned.stdin.map(|mut stdin| {
            let input = input.to_vec();
            thread::spawn(move || stdin.write_all(&input))
        })
    });
    let mut stdout_reader = Some(thread::spawn(move || {
        let limit = request.stdout_limit;
        read_limited(spawned.stdout, limit)
    }));
    let mut stderr_reader = Some(thread::spawn(move || {
        let limit = request.stderr_limit;
        read_limited(spawned.stderr, limit)
    }));
    let stdin_requested = request.stdin.is_some();
    let mut stdin_writer = stdin_writer;
    let mut child = spawned.child;
    let mut timed_out = false;
    let mut kill = KillReceipt::not_needed();
    let mut status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() >= work_deadline => break None,
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Err(error) => {
                child.kill_root();
                return unavailable(request.program, args_digest, error.to_string(), provenance);
            }
        }
    };
    if status.is_none() {
        timed_out = true;
        kill = child.terminate_tree();
        status = wait_for_child_until(&mut child, deadline);
    }

    let mut stdin_result = if stdin_requested {
        join_until(&mut stdin_writer, work_deadline)
    } else {
        None
    };
    let mut stdout_result = join_until(&mut stdout_reader, work_deadline);
    let mut stderr_result = join_until(&mut stderr_reader, work_deadline);
    let io_stalled = (stdin_requested && stdin_result.is_none())
        || stdout_result.is_none()
        || stderr_result.is_none();
    if io_stalled {
        timed_out = true;
        merge_kill(&mut kill, child.terminate_tree());
        if status.is_none() {
            status = wait_for_child_until(&mut child, deadline);
        }
        if stdin_requested && stdin_result.is_none() {
            stdin_result = join_until(&mut stdin_writer, deadline);
        }
        if stdout_result.is_none() {
            stdout_result = join_until(&mut stdout_reader, deadline);
        }
        if stderr_result.is_none() {
            stderr_result = join_until(&mut stderr_reader, deadline);
        }
    }

    let mut descendant_outlived_root = false;
    let mut tree_state_unknown = false;
    if !timed_out && status.is_some() {
        match child.wait_for_tree_empty_until(work_deadline) {
            Ok(Some(true)) | Ok(None) => {}
            Ok(Some(false)) => {
                descendant_outlived_root = true;
                timed_out = true;
                merge_kill(&mut kill, child.terminate_tree());
            }
            Err(_) => {
                tree_state_unknown = true;
                merge_kill(&mut kill, child.terminate_tree());
            }
        }
    }
    if stdin_requested && stdin_result.is_none() {
        drop(stdin_writer.take());
    }
    let stdin_error = stdin_result
        .and_then(Result::err)
        .map(|error| error.to_string());
    let stdout = stdout_result.unwrap_or_else(|| LimitedBytes {
        bytes: Vec::new(),
        truncated: true,
    });
    let stderr = stderr_result.unwrap_or_else(|| LimitedBytes {
        bytes: Vec::new(),
        truncated: true,
    });
    let exit_code = status;
    let state = if timed_out {
        ToolRunState::TimedOut
    } else if descendant_outlived_root || tree_state_unknown {
        ToolRunState::Failed
    } else if exit_code == Some(0) && stdin_error.is_none() {
        ToolRunState::Observed
    } else {
        ToolRunState::Failed
    };
    let diagnostic = stdin_error
        .or_else(|| descendant_outlived_root.then(|| "descendant_outlived_root_process".into()))
        .or_else(|| tree_state_unknown.then(|| "process_tree_state_unavailable".into()))
        .or_else(|| {
            (timed_out && kill.attempted && !kill.verified)
                .then(|| "process_tree_termination_unverified".into())
        })
        .or_else(|| (state != ToolRunState::Observed).then(|| bounded_text(&stderr.bytes, 512)));
    BoundedToolResult {
        state,
        program: request.program.to_string(),
        args_digest,
        exit_code,
        timed_out,
        stdout_digest: digest_bytes(&stdout.bytes),
        stderr_digest: digest_bytes(&stderr.bytes),
        stdout_truncated: stdout.truncated,
        stderr_truncated: stderr.truncated,
        process_tree_kill_attempted: kill.attempted,
        process_tree_kill_verified: kill.verified,
        orphan_risk: kill.orphan_risk.to_string(),
        diagnostic,
        resolved_executable: provenance
            .resolved_path
            .as_ref()
            .map(|path| path.display().to_string()),
        executable_sha256: provenance.executable_sha256,
        cwd_digest: provenance.cwd_digest,
        environment_digest: provenance.environment_digest,
        environment_allowlist_count: provenance.environment.len(),
        network_policy: "environment_offline_hints_without_os_enforcement".into(),
        network_isolation_os_enforced: false,
        hermetic_execution_observed: false,
        stdout: stdout.bytes,
        stderr: stderr.bytes,
    }
}

fn request_within_budget(request: &BoundedToolRequest<'_>) -> bool {
    request.args.len() <= MAX_TOOL_ARGUMENTS
        && request.timeout_ms > 0
        && request.timeout_ms <= MAX_TOOL_TIMEOUT_MS
        && request.stdout_limit <= MAX_TOOL_OUTPUT_BYTES
        && request.stderr_limit <= MAX_TOOL_OUTPUT_BYTES
        && request
            .stdin
            .is_none_or(|input| input.len() <= MAX_TOOL_STDIN_BYTES)
        && request
            .args
            .iter()
            .try_fold(0_usize, |total, argument| total.checked_add(argument.len()))
            .is_some_and(|total| total <= MAX_TOOL_ARGUMENT_BYTES)
}

fn rejected_request(program: &str, state: ToolRunState, reason: &'static str) -> BoundedToolResult {
    BoundedToolResult {
        state,
        program: program.to_string(),
        args_digest: stable_sha256("request_rejected_before_resolution"),
        exit_code: None,
        timed_out: state == ToolRunState::TimedOut,
        stdout_digest: stable_sha256(""),
        stderr_digest: stable_sha256(""),
        stdout_truncated: false,
        stderr_truncated: false,
        process_tree_kill_attempted: false,
        process_tree_kill_verified: false,
        orphan_risk: "none_observed_no_process_started".into(),
        diagnostic: Some(reason.into()),
        resolved_executable: None,
        executable_sha256: None,
        cwd_digest: stable_sha256(""),
        environment_digest: stable_sha256(""),
        environment_allowlist_count: 0,
        network_policy: "not_applicable_process_not_started".into(),
        network_isolation_os_enforced: false,
        hermetic_execution_observed: false,
        stdout: Vec::new(),
        stderr: Vec::new(),
    }
}

pub(crate) fn not_started_request(
    program: &str,
    args: &[String],
    reason: &'static str,
) -> BoundedToolResult {
    let mut result = rejected_request(program, ToolRunState::Unavailable, reason);
    result.args_digest = stable_sha256(&args.join("\u{1f}"));
    result
}

#[derive(Default)]
struct LimitedBytes {
    bytes: Vec<u8>,
    truncated: bool,
}

fn read_limited(mut reader: impl Read, limit: usize) -> LimitedBytes {
    let mut output = LimitedBytes::default();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = match reader.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(read) => read,
        };
        let remaining = limit.saturating_sub(output.bytes.len());
        let keep = remaining.min(read);
        output.bytes.extend_from_slice(&buffer[..keep]);
        output.truncated |= keep < read || remaining == 0;
    }
    output
}

fn join_until<T>(handle: &mut Option<thread::JoinHandle<T>>, deadline: Instant) -> Option<T> {
    loop {
        let worker = handle.as_ref()?;
        if worker.is_finished() {
            return handle.take()?.join().ok();
        }
        if Instant::now() >= deadline {
            return None;
        }
        thread::sleep(Duration::from_millis(5));
    }
}

fn wait_for_child_until(child: &mut ProcessHandle, deadline: Instant) -> Option<i32> {
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
            Ok(None) | Err(_) => return None,
        }
    }
}

fn merge_kill(previous: &mut KillReceipt, latest: KillReceipt) {
    if latest.attempted {
        previous.attempted = true;
        previous.verified = latest.verified;
        previous.orphan_risk = latest.orphan_risk;
    }
}

fn digest_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(71);
    encoded.push_str("sha256:");
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}

fn bounded_text(bytes: &[u8], limit: usize) -> String {
    String::from_utf8_lossy(bytes)
        .chars()
        .take(limit)
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect()
}

struct ExecutionProvenance {
    resolved_path: Option<PathBuf>,
    executable_sha256: Option<String>,
    cwd: PathBuf,
    cwd_digest: String,
    environment: BTreeMap<OsString, OsString>,
    environment_digest: String,
}

fn execution_provenance(program: &str) -> ExecutionProvenance {
    let resolved_path = resolve_executable(program);
    let executable_sha256 = resolved_path
        .as_deref()
        .and_then(|path| digest_file(path).ok());
    let cwd = env::current_dir()
        .ok()
        .and_then(|path| path.canonicalize().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    let environment = sanitized_environment();
    let environment_basis = environment
        .iter()
        .map(|(key, value)| format!("{}={}", key.to_string_lossy(), value.to_string_lossy()))
        .collect::<Vec<_>>()
        .join("\u{1f}");
    ExecutionProvenance {
        resolved_path,
        executable_sha256,
        cwd_digest: stable_sha256(&cwd.to_string_lossy()),
        cwd,
        environment_digest: stable_sha256(&environment_basis),
        environment,
    }
}

fn resolve_executable(program: &str) -> Option<PathBuf> {
    let candidate = Path::new(program);
    if candidate.is_file() {
        return absolute_without_resolving_file_link(candidate);
    }
    let path = env::var_os("PATH")?;
    let extensions: &[&str] = if cfg!(windows) {
        &["", ".exe", ".cmd", ".bat", ".com"]
    } else {
        &[""]
    };
    for directory in env::split_paths(&path) {
        for extension in extensions {
            let candidate = directory.join(format!("{program}{extension}"));
            if candidate.is_file() {
                return absolute_without_resolving_file_link(&candidate);
            }
        }
    }
    None
}

fn absolute_without_resolving_file_link(path: &Path) -> Option<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir().ok()?.join(path)
    };
    let file_name = absolute.file_name()?.to_os_string();
    let parent = absolute.parent()?.canonicalize().ok()?;
    Some(parent.join(file_name))
}

fn sanitized_environment() -> BTreeMap<OsString, OsString> {
    let mut output = BTreeMap::new();
    for key in [
        "CARGO_HOME",
        "HOME",
        "LANG",
        "LOCALAPPDATA",
        "PATH",
        "RUSTUP_HOME",
        "SystemDrive",
        "SystemRoot",
        "TEMP",
        "TMP",
        "USERPROFILE",
        "WINDIR",
    ] {
        if let Some(value) = env::var_os(key) {
            output.insert(OsString::from(key), value);
        }
    }
    for (key, value) in [
        ("CARGO_NET_OFFLINE", "true"),
        ("HF_HUB_OFFLINE", "1"),
        ("PIP_NO_INDEX", "1"),
        ("TRANSFORMERS_OFFLINE", "1"),
    ] {
        output.insert(OsString::from(key), OsString::from(value));
    }
    output
}

fn digest_file(path: &Path) -> Result<String, std::io::Error> {
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    let bytes = digest.finalize();
    let mut encoded = String::with_capacity(71);
    encoded.push_str("sha256:");
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(encoded, "{byte:02x}");
    }
    Ok(encoded)
}

fn unavailable(
    program: &str,
    args_digest: String,
    diagnostic: String,
    provenance: ExecutionProvenance,
) -> BoundedToolResult {
    BoundedToolResult {
        state: ToolRunState::Unavailable,
        program: program.to_string(),
        args_digest,
        exit_code: None,
        timed_out: false,
        stdout_digest: stable_sha256(""),
        stderr_digest: stable_sha256(""),
        stdout_truncated: false,
        stderr_truncated: false,
        process_tree_kill_attempted: false,
        process_tree_kill_verified: false,
        orphan_risk: "unknown".to_string(),
        diagnostic: Some(diagnostic),
        resolved_executable: provenance
            .resolved_path
            .as_ref()
            .map(|path| path.display().to_string()),
        executable_sha256: provenance.executable_sha256,
        cwd_digest: provenance.cwd_digest,
        environment_digest: provenance.environment_digest,
        environment_allowlist_count: provenance.environment.len(),
        network_policy: "environment_offline_hints_without_os_enforcement".into(),
        network_isolation_os_enforced: false,
        hermetic_execution_observed: false,
        stdout: Vec::new(),
        stderr: Vec::new(),
    }
}

struct KillReceipt {
    attempted: bool,
    verified: bool,
    orphan_risk: &'static str,
}

impl KillReceipt {
    fn not_needed() -> Self {
        Self {
            attempted: false,
            verified: true,
            orphan_risk: "none_observed",
        }
    }
}

#[cfg(not(windows))]
fn kill_tree(child: &mut Child) -> KillReceipt {
    let verified = Command::new("pkill")
        .args(["-TERM", "-P", &child.id().to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    KillReceipt {
        attempted: true,
        verified,
        orphan_risk: if verified { "medium" } else { "unknown" },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    use std::process::{Command, Stdio};

    #[test]
    fn rustc_version_is_bounded_and_observed_when_available() {
        let output = run_bounded_tool(BoundedToolRequest {
            program: "rustc",
            args: &["--version"],
            stdin: None,
            timeout_ms: 2_000,
            stdout_limit: 4096,
            stderr_limit: 4096,
        });
        assert_eq!(output.state, ToolRunState::Observed);
        assert!(!output.stdout.is_empty());
    }

    #[test]
    fn missing_tool_is_visible_and_never_observed() {
        let output = run_bounded_tool(BoundedToolRequest {
            program: "lc631-definitely-missing-tool",
            args: &[],
            stdin: None,
            timeout_ms: 100,
            stdout_limit: 32,
            stderr_limit: 32,
        });
        assert_eq!(output.state, ToolRunState::Unavailable);
        assert!(output.diagnostic.is_some());
    }

    #[test]
    fn oversized_tool_requests_are_rejected_before_executable_lookup() {
        let input = vec![0_u8; 2 * 1024 * 1024 + 1];
        let output = run_bounded_tool(BoundedToolRequest {
            program: "lc631-tool-must-not-be-resolved-for-oversized-input",
            args: &[],
            stdin: Some(&input),
            timeout_ms: 100,
            stdout_limit: 1,
            stderr_limit: 1,
        });
        assert_eq!(output.state, ToolRunState::Failed);
        assert_eq!(
            output.diagnostic.as_deref(),
            Some("tool_request_budget_exceeded")
        );
        assert!(output.resolved_executable.is_none());

        let output = run_bounded_tool(BoundedToolRequest {
            program: "lc631-tool-must-not-be-resolved-for-oversized-output-budget",
            args: &[],
            stdin: None,
            timeout_ms: 100,
            stdout_limit: 2 * 1024 * 1024 + 5,
            stderr_limit: 1,
        });
        assert_eq!(output.state, ToolRunState::Failed);
        assert_eq!(
            output.diagnostic.as_deref(),
            Some("tool_request_budget_exceeded")
        );
        assert!(output.resolved_executable.is_none());
    }

    #[cfg(windows)]
    #[test]
    fn pipe_drain_is_inside_the_tool_deadline_even_if_a_descendant_holds_a_pipe() {
        let executable = std::env::current_exe().unwrap();
        let executable = executable.to_str().unwrap();
        let started = Instant::now();
        let output = run_bounded_tool(BoundedToolRequest {
            program: executable,
            args: &[
                "--exact",
                "process::tests::deadline_helper_child",
                "--nocapture",
            ],
            stdin: Some(b"spawn-descendant"),
            timeout_ms: 500,
            stdout_limit: 4_096,
            stderr_limit: 4_096,
        });
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(output.timed_out);
        assert!(output.process_tree_kill_attempted);
        assert!(output.process_tree_kill_verified);
    }

    #[cfg(windows)]
    #[allow(clippy::zombie_processes)] // The helper must exit while its child intentionally retains a pipe.
    #[test]
    fn deadline_helper_child() {
        let mut role = String::new();
        let _ = std::io::stdin().read_to_string(&mut role);
        match role.trim() {
            "spawn-descendant" => {
                let executable = std::env::current_exe().unwrap();
                let mut child = Command::new(executable)
                    .args([
                        "--exact",
                        "process::tests::deadline_helper_child",
                        "--nocapture",
                    ])
                    .stdin(Stdio::piped())
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::inherit())
                    .spawn()
                    .unwrap();
                child.stdin.take().unwrap().write_all(b"sleep").unwrap();
            }
            "sleep" => thread::sleep(Duration::from_secs(4)),
            _ => {}
        }
    }

    #[cfg(unix)]
    #[test]
    fn executable_resolution_preserves_proxy_symlink_name() {
        use std::os::unix::fs::symlink;
        use std::time::{SystemTime, UNIX_EPOCH};

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("lc631-proxy-name-{nonce}"));
        std::fs::create_dir_all(&root).unwrap();
        let proxy = root.join("rustc-proxy");
        symlink(std::env::current_exe().unwrap(), &proxy).unwrap();
        let resolved = resolve_executable(proxy.to_str().unwrap()).unwrap();
        assert_eq!(resolved.file_name().unwrap(), "rustc-proxy");
        std::fs::remove_file(proxy).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}
