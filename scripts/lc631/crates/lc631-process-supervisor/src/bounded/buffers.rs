//! Fallible captured-byte backing prepared before the process spawn boundary.
use super::RunRequest;
use std::io;

pub(super) const IO_SCRATCH_BYTES: usize = 32_768;

pub(super) struct PreparedIoBuffers {
    pub(super) stdout: Vec<u8>,
    pub(super) stderr: Vec<u8>,
    pub(super) stdin: Option<Vec<u8>>,
}
impl PreparedIoBuffers {
    pub(super) fn prepare(request: &RunRequest<'_>) -> io::Result<Self> {
        let stdout = reserve(request.stdout_limit)?;
        let stderr = reserve(request.stderr_limit)?;
        let stdin = request
            .stdin
            .map(|input| {
                let mut bytes = reserve(input.len())?;
                bytes.extend_from_slice(input);
                Ok::<_, io::Error>(bytes)
            })
            .transpose()?;
        Ok(Self {
            stdout,
            stderr,
            stdin,
        })
    }

    pub(super) fn validate(&self, request: &RunRequest<'_>) -> io::Result<()> {
        let stdin_matches = match (request.stdin, &self.stdin) {
            (None, None) => true,
            (Some(expected), Some(actual)) => expected == actual.as_slice(),
            (None, Some(_)) | (Some(_), None) => false,
        };
        if !self.stdout.is_empty()
            || !self.stderr.is_empty()
            || !stdin_matches
            || self.stdout.capacity() < request.stdout_limit
            || self.stderr.capacity() < request.stderr_limit
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "supervisor_unprepared_io_buffers",
            ));
        }
        let declared = request
            .stdout_limit
            .checked_add(request.stderr_limit)
            .and_then(|bytes| bytes.checked_add(request.stdin.map_or(0, <[u8]>::len)))
            .and_then(|bytes| bytes.checked_add(IO_SCRATCH_BYTES));
        let actual = self
            .stdout
            .capacity()
            .checked_add(self.stderr.capacity())
            .and_then(|bytes| bytes.checked_add(self.stdin.as_ref().map_or(0, Vec::capacity)))
            // Bound the two explicit 8KiB read scratch frames as well. Allocator
            // headers and OS thread stacks are not measured by this contract.
            .and_then(|bytes| bytes.checked_add(16_384));
        if !matches!((actual, declared), (Some(actual), Some(declared)) if actual <= declared) {
            return Err(io::Error::new(
                io::ErrorKind::OutOfMemory,
                "supervisor_io_backing_exceeds_reservation",
            ));
        }
        Ok(())
    }
}

fn reserve(limit: usize) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(limit).map_err(|_| {
        io::Error::new(
            io::ErrorKind::OutOfMemory,
            "supervisor_preallocation_failed",
        )
    })?;
    Ok(bytes)
}
