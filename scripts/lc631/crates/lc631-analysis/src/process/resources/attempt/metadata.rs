//! Allocate every report buffer before native effects; finalize from real bytes.
#![forbid(unsafe_code)]
use super::*;
pub(super) struct PreparedMetadata {
    report: BoundedToolResult,
    diagnostic: String,
}
fn buffer(cap: usize) -> Result<String, ResourceError> {
    let mut s = String::new();
    s.try_reserve_exact(cap)
        .map_err(|_| ResourceError::AllocationFailed)?;
    if s.capacity()
        > cap
            .checked_mul(2)
            .ok_or(ResourceError::ArithmeticOverflow)?
    {
        return Err(ResourceError::AllocationFailed);
    }
    Ok(s)
}
fn text(value: &str) -> Result<String, ResourceError> {
    let mut s = buffer(value.len())?;
    s.push_str(value);
    Ok(s)
}
impl PreparedMetadata {
    pub(super) fn prepare(
        r: &BoundedToolRequest<'_>,
        p: &ExecutionProvenance,
    ) -> Result<Self, ResourceError> {
        let executable = p
            .resolved_path
            .as_ref()
            .and_then(|p| p.to_str())
            .ok_or(ResourceError::InvalidConfig)?;
        Ok(Self {
            report: BoundedToolResult {
                state: ToolRunState::Failed,
                program: text(r.program)?,
                args_digest: stable_sha256(&r.args.join("\u{1f}")),
                exit_code: None,
                timed_out: false,
                stdout_digest: buffer(71)?,
                stderr_digest: buffer(71)?,
                stdout_truncated: false,
                stderr_truncated: false,
                process_tree_kill_attempted: false,
                process_tree_kill_verified: false,
                orphan_risk: buffer(64)?,
                diagnostic: None,
                resolved_executable: Some(text(executable)?),
                executable_sha256: p.executable_sha256.as_deref().map(text).transpose()?,
                cwd_digest: text(&p.cwd_digest)?,
                environment_digest: text(&p.environment_digest)?,
                environment_allowlist_count: p.environment.len(),
                network_policy: text("explicit_environment_without_os_network_enforcement")?,
                network_isolation_os_enforced: false,
                hermetic_execution_observed: false,
                stdout: Vec::new(),
                stderr: Vec::new(),
            },
            diagnostic: buffer(2048)?,
        })
    }
    pub(super) fn finish(mut self, o: &lc631_process_supervisor::RunOutcome) -> BoundedToolResult {
        let r = &mut self.report;
        r.state = if o.timed_out {
            ToolRunState::TimedOut
        } else if o.exit_code == Some(0)
            && o.io_cleanup_complete
            && o.process_tree_kill_verified
            && !o.stdout_truncated
            && !o.stderr_truncated
            && o.diagnostic.is_none()
        {
            ToolRunState::Observed
        } else {
            ToolRunState::Failed
        };
        r.exit_code = o.exit_code;
        r.timed_out = o.timed_out;
        hash_into(&mut r.stdout_digest, &o.stdout);
        hash_into(&mut r.stderr_digest, &o.stderr);
        r.stdout_truncated = o.stdout_truncated;
        r.stderr_truncated = o.stderr_truncated;
        r.process_tree_kill_attempted = o.process_tree_kill_attempted;
        r.process_tree_kill_verified = o.process_tree_kill_verified;
        r.orphan_risk
            .push_str(if o.process_tree_kill_verified && o.io_cleanup_complete {
                match o.containment {
                    "windows_job" => "none_observed_within_windows_job",
                    "unix_process_group" => "none_observed_within_unix_process_group",
                    _ => "unknown",
                }
            } else {
                "unknown"
            });
        if let Some(d) = o.diagnostic {
            for c in d.chars().take(512) {
                self.diagnostic.push(c);
            }
        } else if r.state != ToolRunState::Observed {
            lossy_prefix(&mut self.diagnostic, &o.stderr[..o.stderr.len().min(512)]);
        }
        if !self.diagnostic.is_empty() {
            r.diagnostic = Some(self.diagnostic);
        }
        self.report
    }
}
fn hash_into(out: &mut String, bytes: &[u8]) {
    use sha2::{Digest, Sha256};
    const HEX: &[u8; 16] = b"0123456789abcdef";
    out.push_str("sha256:");
    for b in Sha256::digest(bytes) {
        out.push(HEX[usize::from(b >> 4)] as char);
        out.push(HEX[usize::from(b & 15)] as char);
    }
}
fn lossy_prefix(out: &mut String, mut bytes: &[u8]) {
    while !bytes.is_empty() {
        match std::str::from_utf8(bytes) {
            Ok(text) => {
                out.push_str(text);
                break;
            }
            Err(error) => {
                let good = error.valid_up_to();
                if let Ok(prefix) = std::str::from_utf8(&bytes[..good]) {
                    out.push_str(prefix);
                }
                out.push('\u{fffd}');
                let bad = error.error_len().unwrap_or(bytes.len() - good);
                bytes = &bytes[good + bad..];
            }
        }
    }
}
