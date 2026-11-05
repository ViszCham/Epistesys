//! Additive explicit cwd/env dispatch metadata. Checked shape is not a grant.
#![forbid(unsafe_code)]
use super::*;
use lc631_substrate::Digest32;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameDispatchPurpose {
    BaselineProbe,
    CurrentProbe,
    CompilerIdentity,
    CompilerSysroot,
    CompilerLink,
}
pub struct FrameDispatchBinding<'a> {
    pub source: Digest32,
    pub root_task: Digest32,
    pub frame: Digest32,
    pub purpose: FrameDispatchPurpose,
    pub grant_receipt_digest: &'a str,
}
pub struct PreparedToolFrame<'a> {
    pub(super) request: BoundedToolRequest<'a>,
    pub(super) cwd: &'a Path,
    pub(super) environment: &'a [(OsString, OsString)],
    pub(super) binding: FrameDispatchBinding<'a>,
}
impl<'a> PreparedToolFrame<'a> {
    pub fn checked(
        request: BoundedToolRequest<'a>,
        cwd: &'a Path,
        environment: &'a [(OsString, OsString)],
        binding: FrameDispatchBinding<'a>,
    ) -> Result<Self, ResourceError> {
        checked_request_binding(&request)?;
        let zero = Digest32::from_bytes([0; 32]);
        if !Path::new(request.program).is_absolute()
            || !cwd.is_absolute()
            || cwd.as_os_str().len() > 4096
            || cwd.to_str().is_none()
            || environment.len() > 32
            || binding.source == zero
            || binding.root_task == zero
            || binding.frame == zero
            || !canonical_digest(binding.grant_receipt_digest)
        {
            return Err(ResourceError::InvalidConfig);
        }
        let mut total = 0_usize;
        let mut previous: Option<&str> = None;
        for (key, value) in environment {
            let key = key.to_str().ok_or(ResourceError::InvalidConfig)?;
            let value = value.to_str().ok_or(ResourceError::InvalidConfig)?;
            if key.is_empty()
                || !key.is_ascii()
                || key.len() > 256
                || key.contains('=')
                || key.contains('\0')
                || value.len() > 8192
                || value.contains('\0')
                || previous.is_some_and(|p| p >= key)
            {
                return Err(ResourceError::InvalidConfig);
            }
            // Windows treats environment names without ASCII case distinctions.
            if cfg!(windows)
                && environment
                    .iter()
                    .filter(|(k, _)| k.to_str().is_some_and(|k| k.eq_ignore_ascii_case(key)))
                    .count()
                    != 1
            {
                return Err(ResourceError::InvalidConfig);
            }
            previous = Some(key);
            total = total
                .checked_add(key.len())
                .and_then(|n| n.checked_add(value.len()))
                .ok_or(ResourceError::ArithmeticOverflow)?;
            if total > 65536 {
                return Err(ResourceError::InvalidConfig);
            }
        }
        Ok(Self {
            request,
            cwd,
            environment,
            binding,
        })
    }
    pub fn binding(&self) -> &FrameDispatchBinding<'a> {
        &self.binding
    }
    pub fn invocation_digest(&self) -> Result<Digest32, ResourceError> {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(b"Epistesys/FrameBoundNativeDispatch/v1\0");
        h.update(checked_request_binding(&self.request)?.as_bytes());
        h.update(self.binding.source.as_bytes());
        h.update(self.binding.root_task.as_bytes());
        h.update(self.binding.frame.as_bytes());
        h.update([match self.binding.purpose {
            FrameDispatchPurpose::BaselineProbe => 0,
            FrameDispatchPurpose::CurrentProbe => 1,
            FrameDispatchPurpose::CompilerIdentity => 2,
            FrameDispatchPurpose::CompilerSysroot => 3,
            FrameDispatchPurpose::CompilerLink => 4,
        }]);
        h.update((self.environment.len() as u64).to_le_bytes());
        let mut field = |s: &str| {
            h.update((s.len() as u64).to_le_bytes());
            h.update(s.as_bytes());
        };
        field(self.binding.grant_receipt_digest);
        field(self.cwd.to_str().ok_or(ResourceError::InvalidConfig)?);
        for (key, value) in self.environment {
            field(key.to_str().ok_or(ResourceError::InvalidConfig)?);
            field(value.to_str().ok_or(ResourceError::InvalidConfig)?);
        }
        Ok(Digest32::from_bytes(h.finalize().into()))
    }
}
fn canonical_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}
pub fn run_prepared_tool_attempt<'res, E>(
    frame: &PreparedToolFrame<'_>,
    resources: &'res ResourceReservationView<'res>,
    admit: &mut dyn FnMut() -> Result<(), E>,
) -> ResourceToolAttempt<'res, E> {
    let r = &frame.request;
    let request = BoundedToolRequest {
        program: r.program,
        args: r.args,
        stdin: r.stdin,
        timeout_ms: r.timeout_ms,
        stdout_limit: r.stdout_limit,
        stderr_limit: r.stderr_limit,
    };
    let mut a = ResourceToolAttempt {
        process: None,
        run: None,
        effect: NativeProcessEffect::NotAttempted,
        resource_error: None,
        _preparation: None,
    };
    let result = if frame.binding.source != resources.source_revision() {
        Err(ResourceError::SourceMismatch)
    } else {
        produce(request, resources, admit, Some(frame), &mut a)
    };
    if let Err(error) = result {
        a.resource_error = Some(error);
    }
    a
}
