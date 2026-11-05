//! Checked launch-memory engineering plan, not allocator/RSS/OS containment.
use super::{buffers::IO_SCRATCH_BYTES, validate_io, RunRequest};
use lc631_substrate::resource::ResourceError;
use std::ffi::OsStr;

const MAX_ENTRIES: usize = 256;
const MAX_ENV_BYTES: usize = 1024 * 1024;
const MAX_PATH_BYTES: usize = 128 * 1024;
#[cfg(windows)]
const MAX_VARIABLE_UNITS: usize = 32_767;
#[cfg(windows)]
pub(crate) const MAX_ENV_BLOCK_UNITS: usize = 1024 * 1024;

pub(super) struct LaunchPlan {
    pub(super) backing: usize,
}

impl LaunchPlan {
    pub(super) fn checked(request: &RunRequest<'_>) -> Result<Self, ResourceError> {
        if request.environment.len() > MAX_ENTRIES
            || request.executable.len() > MAX_PATH_BYTES
            || request.current_dir.as_os_str().len() > MAX_PATH_BYTES
            || invalid_text(request.executable)
            || invalid_text(request.current_dir.as_os_str())
            || request.arguments.iter().any(|value| invalid_text(value))
        {
            return Err(ResourceError::InvalidConfig);
        }
        let mut environment_bytes = 0;
        for (key, value) in request.environment {
            if key.is_empty()
                || invalid_text(key)
                || invalid_text(value)
                || key.as_encoded_bytes().contains(&b'=')
            {
                return Err(ResourceError::InvalidConfig);
            }
            environment_bytes = add(environment_bytes, add(key.len(), value.len())?)?;
        }
        if environment_bytes > MAX_ENV_BYTES {
            return Err(ResourceError::InvalidConfig);
        }
        let arguments_bytes = request
            .arguments
            .iter()
            .try_fold(request.executable.len(), |size, arg| add(size, arg.len()))?;
        let raw = add(
            add(arguments_bytes, environment_bytes)?,
            request.current_dir.as_os_str().len(),
        )?;
        let encoded = platform_encoded_bytes(request)?;
        let entries = add(add(request.arguments.len(), request.environment.len())?, 4)?;
        // Versioned engineering allowance for simultaneously live raw/encoded
        // clones and bounded entry/pointer tables in the native launch adapters.
        // It is an estimate of requested backing, never measured physical RAM.
        let backing = add(
            multiply(add(raw, encoded)?, 8)?,
            add(
                multiply(entries, multiply(std::mem::size_of::<usize>(), 32)?)?,
                4096,
            )?,
        )?;
        Ok(Self { backing })
    }
}

/// Same checked plan used by resource admission before the child spawn. Callers
/// reserve this bound rather than silently expanding their existing run limits.
pub fn required_ram_bytes(request: &RunRequest<'_>) -> Result<u64, ResourceError> {
    validate_io(request).map_err(|error| ResourceError::Io(error.kind()))?;
    let launch = LaunchPlan::checked(request)?;
    let output = add(request.stdout_limit, request.stderr_limit)?;
    let ram = add(
        add(
            add(output, request.stdin.map_or(0, <[u8]>::len))?,
            IO_SCRATCH_BYTES,
        )?,
        launch.backing,
    )?;
    u64::try_from(ram).map_err(|_| ResourceError::ArithmeticOverflow)
}

fn invalid_text(value: &OsStr) -> bool {
    value.as_encoded_bytes().contains(&0)
}
fn add(left: usize, right: usize) -> Result<usize, ResourceError> {
    left.checked_add(right)
        .ok_or(ResourceError::ArithmeticOverflow)
}
fn multiply(left: usize, right: usize) -> Result<usize, ResourceError> {
    left.checked_mul(right)
        .ok_or(ResourceError::ArithmeticOverflow)
}

#[cfg(unix)]
fn platform_encoded_bytes(request: &RunRequest<'_>) -> Result<usize, ResourceError> {
    let mut bytes = add(request.executable.len(), 1)?;
    for argument in request.arguments {
        bytes = add(bytes, add(argument.len(), 1)?)?;
    }
    for (key, value) in request.environment {
        bytes = add(bytes, add(add(key.len(), value.len())?, 2)?)?;
    }
    add(bytes, add(request.current_dir.as_os_str().len(), 1)?)
}

#[cfg(windows)]
fn platform_encoded_bytes(request: &RunRequest<'_>) -> Result<usize, ResourceError> {
    use std::os::windows::ffi::OsStrExt;
    let mut command_units = add(quoted_units(request.executable)?, 1)?;
    for argument in request.arguments {
        command_units = add(command_units, add(quoted_units(argument)?, 1)?)?;
    }
    if command_units > 32_767 {
        return Err(ResourceError::InvalidConfig);
    }
    let executable_units = add(request.executable.encode_wide().count(), 1)?;
    let directory_units = add(request.current_dir.as_os_str().encode_wide().count(), 1)?;
    if directory_units > 32_767 {
        return Err(ResourceError::InvalidConfig);
    }
    let mut environment_units = 1;
    for (key, value) in request.environment {
        let key_units = key.encode_wide().count();
        let value_units = value.encode_wide().count();
        if key_units > MAX_VARIABLE_UNITS || value_units > MAX_VARIABLE_UNITS {
            return Err(ResourceError::InvalidConfig);
        }
        environment_units = add(environment_units, add(add(key_units, value_units)?, 2)?)?;
    }
    environment_units = environment_units.max(2); // includes the double NUL
    if environment_units > MAX_ENV_BLOCK_UNITS {
        return Err(ResourceError::InvalidConfig);
    }
    multiply(
        add(
            add(command_units, executable_units)?,
            add(directory_units, environment_units)?,
        )?,
        2,
    )
}

#[cfg(windows)]
fn quoted_units(value: &OsStr) -> Result<usize, ResourceError> {
    use std::os::windows::ffi::OsStrExt;
    let mut length = 2; // opening/closing quotes
    let mut backslashes = 0;
    for unit in value.encode_wide() {
        if unit == u16::from(b'\\') {
            backslashes = add(backslashes, 1)?;
            continue;
        }
        let count = if unit == u16::from(b'"') {
            add(multiply(backslashes, 2)?, 2)?
        } else {
            add(backslashes, 1)?
        };
        length = add(length, count)?;
        backslashes = 0;
    }
    add(length, multiply(backslashes, 2)?)
}
