use super::{Failure, Purpose};
use lc631_process_supervisor::{required_ram_bytes, RunRequest};
use lc631_substrate::resource::{
    ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
    ResourceReservationView,
};
use sha2::{Digest, Sha256};
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;
use std::time::Duration;

pub(super) const MAX_CAPTURE: usize = 1024 * 1024;
const MAX_INPUT: usize = 64 * 1024;
const DIAGNOSTIC_SCRATCH: usize = 8 * 1024 * 1024;

pub(super) struct PreparedRequest<'v> {
    executable: OsString,
    arguments: Vec<OsString>,
    directory: PathBuf,
    environment: Vec<(OsString, OsString)>,
    stdin: Option<Vec<u8>>,
    timeout: Duration,
    stdout_limit: usize,
    stderr_limit: usize,
    // Includes ordered raw OS strings, presence of stdin, caps and purpose.
    // This is request identity only, not executable authentication.
    _identity: [u8; 32],
    // Owned descriptors and parser scratch are separate from supervisor RAM.
    // Drop descriptors before releasing this claim.
    claim: ResourceClaim<'v>,
}

impl<'v> PreparedRequest<'v> {
    pub(super) fn new(
        request: RunRequest<'_>,
        purpose: Purpose,
        view: &'v ResourceReservationView<'v>,
    ) -> Result<Self, Failure> {
        if request.stdout_limit == 0
            || request.stderr_limit == 0
            || request.stdout_limit > MAX_CAPTURE
            || request.stderr_limit > MAX_CAPTURE
            || request.timeout.is_zero()
            || request.timeout > Duration::from_secs(120)
        {
            return Err(Failure::InvalidCaptureLimits);
        }
        if request.stdin.map_or(0, <[u8]>::len) > MAX_INPUT {
            return Err(Failure::InputLimit);
        }
        view.check().map_err(Failure::Resource)?;
        // Validate all kernel launch limits before walking/copying descriptors.
        // The supervisor itself claims this RAM; do not charge it a second time.
        required_ram_bytes(&request).map_err(Failure::Resource)?;
        let extra_ram = descriptor_ram(&request).map_err(Failure::Resource)?;
        let claim = view
            .claim_resources(&[ResourceRequest::new(
                ResourceKind::Ram,
                ResourceAmount::checked(extra_ram).map_err(Failure::Resource)?,
            )])
            .map_err(Failure::Resource)?;

        let executable = copy_os(request.executable)?;
        let directory = PathBuf::from(copy_os(request.current_dir.as_os_str())?);
        let mut arguments = Vec::new();
        arguments
            .try_reserve_exact(request.arguments.len())
            .map_err(|_| Failure::Resource(ResourceError::AllocationFailed))?;
        for argument in request.arguments {
            arguments.push(copy_os(argument)?);
        }
        let mut environment = Vec::new();
        environment
            .try_reserve_exact(request.environment.len())
            .map_err(|_| Failure::Resource(ResourceError::AllocationFailed))?;
        for (key, value) in request.environment {
            environment.push((copy_os(key)?, copy_os(value)?));
        }
        let stdin = match request.stdin {
            Some(bytes) => {
                let mut owned = Vec::new();
                owned
                    .try_reserve_exact(bytes.len())
                    .map_err(|_| Failure::Resource(ResourceError::AllocationFailed))?;
                owned.extend_from_slice(bytes);
                Some(owned)
            }
            None => None,
        };
        let identity = request_identity(&request, purpose);
        claim.check().map_err(Failure::Resource)?;
        Ok(Self {
            executable,
            arguments,
            directory,
            environment,
            stdin,
            timeout: request.timeout,
            stdout_limit: request.stdout_limit,
            stderr_limit: request.stderr_limit,
            _identity: identity,
            claim,
        })
    }

    pub(super) fn as_request(&self) -> RunRequest<'_> {
        RunRequest {
            executable: &self.executable,
            arguments: &self.arguments,
            current_dir: &self.directory,
            environment: &self.environment,
            stdin: self.stdin.as_deref(),
            timeout: self.timeout,
            stdout_limit: self.stdout_limit,
            stderr_limit: self.stderr_limit,
        }
    }

    pub(super) fn check_current(&self, view: &ResourceReservationView<'_>) -> Result<(), Failure> {
        if !self.claim.same_reservation(view) {
            return Err(Failure::ForeignReservation);
        }
        self.claim.check().map_err(Failure::Resource)
    }
}

fn copy_os(value: &OsStr) -> Result<OsString, Failure> {
    let mut copy = OsString::new();
    copy.try_reserve_exact(value.len())
        .map_err(|_| Failure::Resource(ResourceError::AllocationFailed))?;
    copy.push(value);
    Ok(copy)
}

fn add(left: usize, right: usize) -> Result<usize, ResourceError> {
    left.checked_add(right)
        .ok_or(ResourceError::ArithmeticOverflow)
}

fn descriptor_ram(request: &RunRequest<'_>) -> Result<u64, ResourceError> {
    let mut raw = add(
        request.executable.len(),
        request.current_dir.as_os_str().len(),
    )?;
    raw = add(raw, request.stdin.map_or(0, <[u8]>::len))?;
    for argument in request.arguments {
        raw = add(raw, argument.len())?;
    }
    for (key, value) in request.environment {
        raw = add(raw, add(key.len(), value.len())?)?;
    }
    let entries = add(add(request.arguments.len(), request.environment.len())?, 4)?;
    // Conservative logical backing, not an allocator/RSS measurement. Native
    // encoding and pipe buffers are funded independently by the supervisor.
    let descriptors = entries
        .checked_mul(2 * std::mem::size_of::<OsString>())
        .ok_or(ResourceError::ArithmeticOverflow)?;
    let backing = add(
        add(
            raw.checked_mul(2)
                .ok_or(ResourceError::ArithmeticOverflow)?,
            descriptors,
        )?,
        add(DIAGNOSTIC_SCRATCH, 4096)?,
    )?;
    u64::try_from(backing).map_err(|_| ResourceError::ArithmeticOverflow)
}

fn request_identity(request: &RunRequest<'_>, purpose: Purpose) -> [u8; 32] {
    fn bytes(hasher: &mut Sha256, value: &[u8]) {
        hasher.update((value.len() as u64).to_le_bytes());
        hasher.update(value);
    }
    let mut hash = Sha256::new();
    hash.update(b"epistesys.cf.request.v1\0");
    hash.update([match purpose {
        Purpose::PositiveControl => 0,
        Purpose::NegativeCompile => 1,
    }]);
    bytes(&mut hash, request.executable.as_encoded_bytes());
    bytes(
        &mut hash,
        request.current_dir.as_os_str().as_encoded_bytes(),
    );
    hash.update((request.arguments.len() as u64).to_le_bytes());
    for argument in request.arguments {
        bytes(&mut hash, argument.as_encoded_bytes());
    }
    hash.update((request.environment.len() as u64).to_le_bytes());
    for (key, value) in request.environment {
        bytes(&mut hash, key.as_encoded_bytes());
        bytes(&mut hash, value.as_encoded_bytes());
    }
    hash.update([u8::from(request.stdin.is_some())]);
    if let Some(stdin) = request.stdin {
        bytes(&mut hash, stdin);
    }
    hash.update(request.timeout.as_secs().to_le_bytes());
    hash.update(request.timeout.subsec_nanos().to_le_bytes());
    hash.update((request.stdout_limit as u64).to_le_bytes());
    hash.update((request.stderr_limit as u64).to_le_bytes());
    hash.finalize().into()
}
