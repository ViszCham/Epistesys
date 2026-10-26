use std::collections::BTreeMap;
use std::env;
use std::ffi::OsString;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
#[cfg(test)]
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
    /// Verified only within the supervisor's owned Job/process-group scope,
    /// never an OS-wide or adversarial sandbox guarantee.
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

pub fn run_bounded_tool(request: BoundedToolRequest<'_>) -> BoundedToolResult {
    if !request_within_budget(&request) {
        return rejected_request(
            request.program,
            ToolRunState::Failed,
            "tool_request_budget_exceeded",
        );
    }
    let started = Instant::now();
    let timeout = Duration::from_millis(request.timeout_ms);
    let args_digest = stable_sha256(&request.args.join("\u{1f}"));
    let provenance = execution_provenance(request.program, started + timeout);
    let Some(executable) = provenance.resolved_path.as_ref() else {
        return unavailable(
            request.program,
            args_digest,
            "executable_not_resolved".into(),
            provenance,
        );
    };
    let Some(remaining) = timeout.checked_sub(started.elapsed()) else {
        return rejected_request(
            request.program,
            ToolRunState::TimedOut,
            "tool_resolution_deadline_exceeded",
        );
    };
    let arguments = request.args.iter().map(OsString::from).collect::<Vec<_>>();
    let environment = provenance
        .environment
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<Vec<_>>();
    let output = match lc631_process_supervisor::run_bounded(lc631_process_supervisor::RunRequest {
        executable: executable.as_os_str(),
        arguments: &arguments,
        current_dir: &provenance.cwd,
        environment: &environment,
        stdin: request.stdin,
        timeout: remaining,
        stdout_limit: request.stdout_limit,
        stderr_limit: request.stderr_limit,
    }) {
        Ok(output) => output,
        Err(error) => {
            return unavailable(request.program, args_digest, error.to_string(), provenance);
        }
    };
    let state = if output.timed_out {
        ToolRunState::TimedOut
    } else if output.exit_code == Some(0)
        && output.io_cleanup_complete
        && output.process_tree_kill_verified
        && !output.stdout_truncated
        && !output.stderr_truncated
        && output.diagnostic.is_none()
    {
        ToolRunState::Observed
    } else {
        ToolRunState::Failed
    };
    let diagnostic = output
        .diagnostic
        .map(str::to_string)
        .or_else(|| (state != ToolRunState::Observed).then(|| bounded_text(&output.stderr, 512)));
    BoundedToolResult {
        state,
        program: request.program.to_string(),
        args_digest,
        exit_code: output.exit_code,
        timed_out: output.timed_out,
        stdout_digest: digest_bytes(&output.stdout),
        stderr_digest: digest_bytes(&output.stderr),
        stdout_truncated: output.stdout_truncated,
        stderr_truncated: output.stderr_truncated,
        process_tree_kill_attempted: output.process_tree_kill_attempted,
        process_tree_kill_verified: output.process_tree_kill_verified,
        orphan_risk: if output.process_tree_kill_verified && output.io_cleanup_complete {
            match output.containment {
                "windows_job" => "none_observed_within_windows_job",
                "unix_process_group" => "none_observed_within_unix_process_group",
                _ => "unknown",
            }
        } else {
            "unknown"
        }
        .into(),
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
        stdout: output.stdout,
        stderr: output.stderr,
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

fn execution_provenance(program: &str, deadline: Instant) -> ExecutionProvenance {
    let resolved_path = resolve_executable(program);
    let executable_sha256 = resolved_path
        .as_deref()
        .and_then(|path| digest_file(path, deadline).ok());
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

fn digest_file(path: &Path, deadline: Instant) -> Result<String, std::io::Error> {
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        if Instant::now() >= deadline {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "executable_digest_deadline_exceeded",
            ));
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::process::{Command, Stdio};

    #[test]
    fn rustc_version_is_bounded_and_observed_when_available() {
        // Cold Windows CI also fingerprints a large compiler executable. This
        // positive observation has an explicit budget; it is not a latency gate.
        let output = run_bounded_tool(BoundedToolRequest {
            program: "rustc",
            args: &["--version"],
            stdin: None,
            timeout_ms: 15_000,
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
    fn expired_executable_fingerprint_budget_returns_visible_timeout() {
        let started = Instant::now();
        let result = digest_file(
            &std::env::current_exe().unwrap(),
            started - Duration::from_millis(1),
        );
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(1));
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

    #[test]
    fn pipe_drain_is_inside_the_tool_deadline_even_if_a_descendant_holds_a_pipe() {
        #[cfg(windows)]
        {
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
        #[cfg(unix)]
        {
            let started = Instant::now();
            let output = run_bounded_tool(BoundedToolRequest {
                program: "/bin/sh",
                args: &["-c", "sleep 4 &"],
                stdin: None,
                timeout_ms: 500,
                stdout_limit: 4_096,
                stderr_limit: 4_096,
            });
            assert!(started.elapsed() < Duration::from_secs(2));
            assert!(output.timed_out);
            assert!(output.process_tree_kill_attempted);
            assert!(output.process_tree_kill_verified);
        }
    }

    #[test]
    fn lone_root_is_terminated_and_reaped_through_the_production_adapter() {
        #[cfg(windows)]
        let executable = std::path::PathBuf::from(
            std::env::var_os("SystemRoot").expect("Windows requires SystemRoot"),
        )
        .join("System32/WindowsPowerShell/v1.0/powershell.exe");
        #[cfg(windows)]
        let program = executable.to_str().unwrap();
        #[cfg(unix)]
        let program = "/bin/sleep";
        #[cfg(windows)]
        let args: &[&str] = &[
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "[System.Threading.Thread]::Sleep(60000)",
        ];
        #[cfg(unix)]
        let args: &[&str] = &["4"];
        let output = run_bounded_tool(BoundedToolRequest {
            program,
            args,
            // The small system executable keeps this 500 ms test about an
            // actually started root, not a cold fingerprint of this test binary.
            stdin: None,
            timeout_ms: 500,
            stdout_limit: 4_096,
            stderr_limit: 4_096,
        });
        assert_eq!(output.state, ToolRunState::TimedOut);
        assert!(output.process_tree_kill_attempted);
        assert!(output.process_tree_kill_verified);
        assert!(output.orphan_risk.starts_with("none_observed_within_"));
        assert!(!output.network_isolation_os_enforced);
    }

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
