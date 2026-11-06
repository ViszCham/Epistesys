//! Runtime-owned process resources, separate from the bounded legacy wrapper.
use super::*;
use lc631_process_supervisor::{ResourceError, ResourceReservationView, ResourceRunOutcome};

mod attempt;
pub use attempt::{run_bounded_tool_attempt, NativeProcessEffect, ResourceToolAttempt};

/// Captured buffers are never cloned into the legacy DTO. Their digest/flags
/// are reported unchanged; raw access stays borrowed from the live claim.
pub struct ResourceToolResult<'a> {
    source: lc631_substrate::Digest32,
    invocation: lc631_substrate::Digest32,
    // Exactly one metadata DTO, fallibly allocated to keep ToolCapture small.
    reported: Vec<BoundedToolResult>,
    _reported_claim: lc631_substrate::resource::ResourceClaim<'a>,
    capture: Option<ResourceRunOutcome<'a>>,
}

pub(crate) enum ToolCapture<'a> {
    Legacy(BoundedToolResult),
    Reserved(ResourceToolResult<'a>),
}
impl ToolCapture<'_> {
    pub(crate) fn reported(&self) -> &BoundedToolResult {
        match self {
            Self::Legacy(result) => result,
            Self::Reserved(result) => result.reported(),
        }
    }
    pub(crate) fn stdout(&self) -> &[u8] {
        match self {
            Self::Legacy(result) => &result.stdout,
            Self::Reserved(result) => result.stdout(),
        }
    }
    pub(crate) fn into_report(self) -> BoundedToolResult {
        match self {
            Self::Legacy(result) => result,
            Self::Reserved(result) => result.into_report(),
        }
    }
}

impl<'a> ToolCapture<'a> {
    pub(crate) fn into_parts(self) -> (BoundedToolResult, Option<ResourceRunOutcome<'a>>) {
        match self {
            Self::Legacy(report) => (report, None),
            Self::Reserved(result) => result.into_parts(),
        }
    }
}

pub(crate) fn capture_tool<'a>(
    request: BoundedToolRequest<'_>,
    resources: Option<&'a ResourceReservationView<'a>>,
) -> Result<ToolCapture<'a>, ResourceError> {
    match resources {
        Some(view) => run_bounded_tool_with_resources(request, view).map(ToolCapture::Reserved),
        None => Ok(ToolCapture::Legacy(run_bounded_tool(request))),
    }
}
impl<'a> ResourceToolResult<'a> {
    pub fn source_revision(&self) -> lc631_substrate::Digest32 {
        self.source
    }
    pub fn invocation_digest(&self) -> lc631_substrate::Digest32 {
        self.invocation
    }
    pub fn reported(&self) -> &BoundedToolResult {
        &self.reported[0]
    }
    pub fn stdout(&self) -> &[u8] {
        self.capture
            .as_ref()
            .map_or(&[], |capture| capture.outcome().stdout.as_slice())
    }
    pub fn stderr(&self) -> &[u8] {
        self.capture
            .as_ref()
            .map_or(&[], |capture| capture.outcome().stderr.as_slice())
    }
    /// The old JSON skips raw buffers, so this metadata-only view preserves
    /// the wire without letting unaccounted Vec copies escape the reservation.
    pub fn into_report(mut self) -> BoundedToolResult {
        self.reported
            .pop()
            .expect("private constructor stores exactly one DTO")
    }

    fn into_parts(mut self) -> (BoundedToolResult, Option<ResourceRunOutcome<'a>>) {
        let report = self
            .reported
            .pop()
            .expect("private constructor stores exactly one DTO");
        (report, self.capture.take())
    }
}

pub fn run_bounded_tool_with_resources<'a>(
    request: BoundedToolRequest<'_>,
    resources: &'a ResourceReservationView<'a>,
) -> Result<ResourceToolResult<'a>, ResourceError> {
    resources.check()?;
    let source = resources.source_revision();
    let invocation = checked_request_binding(&request)?;
    let started = Instant::now();
    let timeout = Duration::from_millis(request.timeout_ms).min(resources.remaining()?);
    let deadline = started
        .checked_add(timeout)
        .ok_or(ResourceError::ArithmeticOverflow)?;
    let resolved_path = resolve_executable(request.program);
    // Preserve a no-process optional-tool diagnostic without Child resources.
    // A real target needs a declared reservation BEFORE costly fingerprinting.
    // Resolve once: a second lookup could turn a missing target into an
    // unreserved hash/run after a filesystem race. Spawn still claims capacity.
    if resolved_path.is_some() {
        resources.amount(lc631_substrate::resource::ResourceKind::Child)?;
    }
    let provenance = execution_provenance_read(resolved_path, deadline, Some(resources));
    resources.check()?;
    let remaining = timeout
        .checked_sub(started.elapsed())
        .filter(|d| !d.is_zero())
        .ok_or(ResourceError::DeadlineExceeded)?;
    let Some(executable) = provenance.resolved_path.as_ref() else {
        if known_missing(request.program, resources, deadline)? {
            return resource_result(
                source,
                invocation,
                unavailable(
                    request.program,
                    stable_sha256(&request.args.join("\u{1f}")),
                    "executable_not_resolved_process_not_started".into(),
                    provenance,
                ),
                None,
                resources,
            );
        }
        return Err(ResourceError::Io(std::io::ErrorKind::InvalidData));
    };
    if provenance.executable_sha256.is_none() {
        return Err(ResourceError::Io(std::io::ErrorKind::InvalidData));
    }
    let arguments = request.args.iter().map(OsString::from).collect::<Vec<_>>();
    let environment = provenance
        .environment
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect::<Vec<_>>();
    let capture = match lc631_process_supervisor::run_bounded_with_resources_observed(
        lc631_process_supervisor::RunRequest {
            executable: executable.as_os_str(),
            arguments: &arguments,
            current_dir: &provenance.cwd,
            environment: &environment,
            stdin: request.stdin,
            timeout: remaining,
            stdout_limit: request.stdout_limit,
            stderr_limit: request.stderr_limit,
        },
        resources,
    ) {
        Ok(capture) => capture,
        Err(lc631_process_supervisor::ResourceRunError::NotStarted(
            std::io::ErrorKind::NotFound,
        )) => {
            return resource_result(
                source,
                invocation,
                unavailable(
                    request.program,
                    stable_sha256(&request.args.join("\u{1f}")),
                    "native_not_started_not_found".into(),
                    provenance,
                ),
                None,
                resources,
            );
        }
        Err(lc631_process_supervisor::ResourceRunError::NotStarted(kind)) => {
            return Err(ResourceError::Io(kind));
        }
        Err(lc631_process_supervisor::ResourceRunError::Resource(error)) => return Err(error),
    };
    if !capture.cleanup_complete() {
        resources.record_cleanup_unknown();
        return Err(ResourceError::CleanupUnverified);
    }
    if capture.cancelled() {
        return Err(ResourceError::Cancelled);
    }
    if capture.outcome().timed_out {
        return Err(ResourceError::DeadlineExceeded);
    }
    if capture.outcome().stdout_truncated || capture.outcome().stderr_truncated {
        return Err(ResourceError::Exhausted(
            lc631_process_supervisor::ResourceKind::Output,
        ));
    }
    if capture.outcome().diagnostic.is_some() {
        return Err(ResourceError::Io(std::io::ErrorKind::Other));
    }
    let reported = metadata(&request, &provenance, capture.outcome());
    resource_result(source, invocation, reported, Some(capture), resources)
}

fn resource_result<'a>(
    source: lc631_substrate::Digest32,
    invocation: lc631_substrate::Digest32,
    report: BoundedToolResult,
    capture: Option<ResourceRunOutcome<'a>>,
    view: &'a ResourceReservationView<'a>,
) -> Result<ResourceToolResult<'a>, ResourceError> {
    use lc631_substrate::resource::{ResourceAmount, ResourceRequest};
    let amount = u64::try_from(std::mem::size_of::<BoundedToolResult>())
        .map_err(|_| ResourceError::ArithmeticOverflow)?;
    let claim = view.claim_resources(&[ResourceRequest::new(
        lc631_process_supervisor::ResourceKind::Ram,
        ResourceAmount::checked(amount)?,
    )])?;
    let mut reported = Vec::new();
    reported
        .try_reserve_exact(1)
        .map_err(|_| ResourceError::AllocationFailed)?;
    if reported.capacity() != 1 {
        return Err(ResourceError::AllocationFailed);
    }
    reported.push(report);
    Ok(ResourceToolResult {
        source,
        invocation,
        reported,
        _reported_claim: claim,
        capture,
    })
}

/// A missing optional tool is a pre-child fact, not an arbitrary Io downgrade.
fn known_missing(
    program: &str,
    view: &ResourceReservationView<'_>,
    deadline: Instant,
) -> Result<bool, ResourceError> {
    let inspect = |path: &Path| -> Result<bool, ResourceError> {
        view.check()?;
        if Instant::now() >= deadline {
            return Err(ResourceError::DeadlineExceeded);
        }
        match std::fs::metadata(path) {
            Ok(meta) if meta.is_file() => Ok(false),
            Ok(_) => Err(ResourceError::Io(std::io::ErrorKind::InvalidInput)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(true),
            Err(error) => Err(ResourceError::Io(error.kind())),
        }
    };
    let direct = Path::new(program);
    if !inspect(direct)? {
        return Ok(false);
    }
    if direct.is_absolute() || direct.components().count() > 1 {
        return Ok(true);
    }
    let Some(path) = std::env::var_os("PATH") else {
        return Ok(true);
    };
    if path.len() > MAX_TOOL_ARGUMENT_BYTES {
        return Err(ResourceError::InvalidConfig);
    }
    for (count, directory) in std::env::split_paths(&path).enumerate() {
        if count >= 1024 {
            return Err(ResourceError::InvalidConfig);
        }
        for suffix in if cfg!(windows) {
            &["", ".exe", ".cmd", ".bat", ".com"][..]
        } else {
            &[""][..]
        } {
            if !inspect(&directory.join(format!("{program}{suffix}")))? {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

pub(super) fn checked_request_binding(
    request: &BoundedToolRequest<'_>,
) -> Result<lc631_substrate::Digest32, ResourceError> {
    // Reject using lengths only before hashing any caller-supplied bytes or
    // resolving an executable. The separate legacy limits are unchanged.
    if request.program.is_empty() || request.program.len() > 4096 || !request_within_budget(request)
    {
        return Err(ResourceError::InvalidConfig);
    }
    Ok(request_binding(request))
}

fn request_binding(request: &BoundedToolRequest<'_>) -> lc631_substrate::Digest32 {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    hash.update(b"epistesys.tool-request-binding.v1\0");
    hash.update((request.program.len() as u64).to_le_bytes());
    hash.update(request.program.as_bytes());
    hash.update((request.args.len() as u64).to_le_bytes());
    for argument in request.args {
        hash.update((argument.len() as u64).to_le_bytes());
        hash.update(argument.as_bytes());
    }
    match request.stdin {
        Some(bytes) => {
            hash.update([1]);
            hash.update((bytes.len() as u64).to_le_bytes());
            hash.update(bytes);
        }
        None => hash.update([0]),
    }
    hash.update(request.timeout_ms.to_le_bytes());
    hash.update((request.stdout_limit as u64).to_le_bytes());
    hash.update((request.stderr_limit as u64).to_le_bytes());
    lc631_substrate::Digest32::from_bytes(hash.finalize().into())
}

fn metadata(
    request: &BoundedToolRequest<'_>,
    provenance: &ExecutionProvenance,
    output: &lc631_process_supervisor::RunOutcome,
) -> BoundedToolResult {
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
    BoundedToolResult {
        state,
        program: request.program.into(),
        args_digest: stable_sha256(&request.args.join("\u{1f}")),
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
        diagnostic: output.diagnostic.map(str::to_string).or_else(|| {
            (state != ToolRunState::Observed).then(|| bounded_text(&output.stderr, 512))
        }),
        resolved_executable: provenance
            .resolved_path
            .as_ref()
            .map(|p| p.display().to_string()),
        executable_sha256: provenance.executable_sha256.clone(),
        cwd_digest: provenance.cwd_digest.clone(),
        environment_digest: provenance.environment_digest.clone(),
        environment_allowlist_count: provenance.environment.len(),
        network_policy: "environment_offline_hints_without_os_enforcement".into(),
        network_isolation_os_enforced: false,
        hermetic_execution_observed: false,
        stdout: Vec::new(),
        stderr: Vec::new(),
    }
}
