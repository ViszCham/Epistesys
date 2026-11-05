//! Additive producer-owned capture path. Late errors retain the actual run.
#![forbid(unsafe_code)]
use super::*;
pub use lc631_process_supervisor::NativeProcessEffect;
use lc631_process_supervisor::{run_bounded_with_resources_attempt, ResourceRunAttempt};
mod frame;
mod metadata;
pub use frame::{
    run_prepared_tool_attempt, FrameDispatchBinding, FrameDispatchPurpose, PreparedToolFrame,
};

pub struct ResourceToolAttempt<'res, E> {
    process: Option<ResourceToolResult<'res>>,
    run: Option<ResourceRunAttempt<'res, E>>,
    effect: NativeProcessEffect,
    resource_error: Option<ResourceError>,
    _preparation: Option<lc631_substrate::resource::ResourceClaim<'res>>,
}
impl<'res, E> ResourceToolAttempt<'res, E> {
    pub fn process(&self) -> Option<&ResourceToolResult<'res>> {
        self.process.as_ref()
    }
    pub fn effect(&self) -> NativeProcessEffect {
        self.effect
    }
    pub fn resource_error(&self) -> Option<&ResourceError> {
        self.resource_error
            .as_ref()
            .or_else(|| self.run.as_ref().and_then(|r| r.resource_error()))
    }
    pub fn admission_error(&self) -> Option<&E> {
        self.run.as_ref().and_then(|r| r.admission_error())
    }
    pub fn retained_run(&self) -> Option<&ResourceRunAttempt<'res, E>> {
        self.run.as_ref()
    }
}
pub fn run_bounded_tool_attempt<'res, E>(
    request: BoundedToolRequest<'_>,
    resources: &'res ResourceReservationView<'res>,
    admit: &mut dyn FnMut() -> Result<(), E>,
) -> ResourceToolAttempt<'res, E> {
    let mut a = ResourceToolAttempt {
        process: None,
        run: None,
        effect: NativeProcessEffect::NotAttempted,
        resource_error: None,
        _preparation: None,
    };
    let result = produce(request, resources, admit, None, &mut a);
    if let Err(error) = result {
        a.resource_error = Some(error);
    }
    a
}
fn produce<'res, E>(
    request: BoundedToolRequest<'_>,
    resources: &'res ResourceReservationView<'res>,
    admit: &mut dyn FnMut() -> Result<(), E>,
    frame: Option<&PreparedToolFrame<'_>>,
    a: &mut ResourceToolAttempt<'res, E>,
) -> Result<(), ResourceError> {
    use lc631_substrate::resource::{ResourceAmount, ResourceKind, ResourceRequest};
    resources.check()?;
    let source = resources.source_revision();
    let invocation = match frame {
        Some(frame) => frame.invocation_digest()?,
        None => checked_request_binding(&request)?,
    };
    // Fixed bounded preparation capacity is charged before provenance/argv IO.
    // This is reservation accounting, not measured RSS or an OS memory sandbox.
    a._preparation = Some(resources.claim_resources(&[ResourceRequest::new(
        ResourceKind::Ram,
        ResourceAmount::checked(4 * 1024 * 1024)?,
    )])?);
    let metadata_claim = resources.claim_resources(&[ResourceRequest::new(
        ResourceKind::Ram,
        ResourceAmount::checked(32768)?,
    )])?;
    let mut reports = Vec::new();
    reports
        .try_reserve_exact(1)
        .map_err(|_| ResourceError::AllocationFailed)?;
    if reports.capacity() != 1 {
        return Err(ResourceError::AllocationFailed);
    }
    let started = Instant::now();
    let timeout = Duration::from_millis(request.timeout_ms).min(resources.remaining()?);
    let deadline = started
        .checked_add(timeout)
        .ok_or(ResourceError::ArithmeticOverflow)?;
    let executable = Path::new(request.program);
    if !executable.is_absolute() {
        return Err(ResourceError::InvalidConfig);
    }
    resources.amount(ResourceKind::Child)?;
    let executable_sha256 = digest_file_read(executable, deadline, Some(resources))
        .map_err(|e| ResourceError::Io(e.kind()))?;
    let cwd = match frame {
        Some(frame) => frame.cwd.canonicalize(),
        None => env::current_dir().and_then(|p| p.canonicalize()),
    }
    .map_err(|e| ResourceError::Io(e.kind()))?;
    if frame.is_some_and(|frame| frame.cwd != cwd) {
        return Err(ResourceError::InvalidConfig);
    }
    if cwd.as_os_str().len() > 4096 {
        return Err(ResourceError::InvalidConfig);
    }
    // Explicit native-attempt profile: absolute executable, no inherited env.
    // Avoid cloning an unbounded host environment before a provider is sampled.
    let inherited = frame.map_or(&[][..], |frame| frame.environment);
    let environment = inherited
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect::<BTreeMap<_, _>>();
    let environment_basis = environment
        .iter()
        .map(|(k, v)| format!("{}={}", k.to_string_lossy(), v.to_string_lossy()))
        .collect::<Vec<_>>()
        .join("\u{1f}");
    let provenance = ExecutionProvenance {
        resolved_path: Some(executable.to_path_buf()),
        executable_sha256: Some(executable_sha256),
        cwd_digest: stable_sha256(&cwd.to_string_lossy()),
        cwd,
        environment,
        environment_digest: stable_sha256(&environment_basis),
    };
    resources.check()?;
    let executable = provenance
        .resolved_path
        .as_ref()
        .ok_or(ResourceError::Io(std::io::ErrorKind::NotFound))?;
    if provenance.executable_sha256.is_none()
        || executable.as_os_str().len() > 4096
        || provenance.cwd.as_os_str().len() > 4096
    {
        return Err(ResourceError::InvalidConfig);
    }
    let arguments = request.args.iter().map(OsString::from).collect::<Vec<_>>();
    let environment = provenance
        .environment
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect::<Vec<_>>();
    let prepared_metadata = metadata::PreparedMetadata::prepare(&request, &provenance)?;
    let remaining = timeout
        .checked_sub(started.elapsed())
        .filter(|d| !d.is_zero())
        .ok_or(ResourceError::DeadlineExceeded)?;
    let run = run_bounded_with_resources_attempt(
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
        admit,
    );
    a.effect = run.effect();
    let capture = match run.into_completed() {
        Ok(capture) => capture,
        Err(run) => {
            a.run = Some(run);
            return Ok(());
        }
    };
    // All fallible report-container allocation happened before native creation.
    // The real capture remains local until ownership moves into ResourceToolResult.
    reports.push(prepared_metadata.finish(capture.outcome()));
    a.process = Some(ResourceToolResult {
        source,
        invocation,
        reported: reports,
        _reported_claim: metadata_claim,
        capture: Some(capture),
    });
    Ok(())
}
