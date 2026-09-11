use std::collections::BTreeMap;
use std::env;
use std::ffi::OsString;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use lc631_core::stable_sha256;
use serde::Serialize;
use sha2::{Digest, Sha256};

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

pub fn run_bounded_tool(request: BoundedToolRequest<'_>) -> BoundedToolResult {
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
    let mut command = Command::new(executable);
    command
        .args(request.args)
        .current_dir(&provenance.cwd)
        .env_clear()
        .envs(&provenance.environment)
        .stdin(if request.stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            return unavailable(request.program, args_digest, error.to_string(), provenance);
        }
    };
    let stdin_writer = request.stdin.and_then(|input| {
        child.stdin.take().map(|mut stdin| {
            let input = input.to_vec();
            thread::spawn(move || stdin.write_all(&input))
        })
    });
    let stdout_reader = child.stdout.take().map(|stdout| {
        let limit = request.stdout_limit;
        thread::spawn(move || read_limited(stdout, limit))
    });
    let stderr_reader = child.stderr.take().map(|stderr| {
        let limit = request.stderr_limit;
        thread::spawn(move || read_limited(stderr, limit))
    });
    let start = Instant::now();
    let timeout = Duration::from_millis(request.timeout_ms.max(1));
    let mut timed_out = false;
    let mut kill = KillReceipt::not_needed();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if start.elapsed() >= timeout => {
                timed_out = true;
                kill = kill_tree(&mut child);
                let _ = child.kill();
                break child.wait().ok();
            }
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Err(error) => {
                let _ = child.kill();
                return unavailable(request.program, args_digest, error.to_string(), provenance);
            }
        }
    };
    let stdin_error = stdin_writer
        .and_then(|handle| handle.join().ok())
        .and_then(Result::err)
        .map(|error| error.to_string());
    let stdout = join_reader(stdout_reader);
    let stderr = join_reader(stderr_reader);
    let exit_code = status.and_then(|status| status.code());
    let state = if timed_out {
        ToolRunState::TimedOut
    } else if exit_code == Some(0) && stdin_error.is_none() {
        ToolRunState::Observed
    } else {
        ToolRunState::Failed
    };
    let diagnostic = stdin_error
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

fn join_reader(reader: Option<thread::JoinHandle<LimitedBytes>>) -> LimitedBytes {
    reader
        .and_then(|handle| handle.join().ok())
        .unwrap_or_default()
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

fn kill_tree(child: &mut Child) -> KillReceipt {
    #[cfg(windows)]
    {
        let verified = Command::new("taskkill")
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
        KillReceipt {
            attempted: true,
            verified,
            orphan_risk: if verified { "low" } else { "unknown" },
        }
    }
    #[cfg(not(windows))]
    {
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
