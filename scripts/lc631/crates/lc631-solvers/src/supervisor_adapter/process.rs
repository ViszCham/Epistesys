//! One-shot, bounded solver process runner with explicitly charged output views.
use super::executable::{hash_file_contents, reserve_hash_buffer};
use super::resources::{
    map_prestart_resource_error, map_reservation_failure, map_supervisor_error,
};
use crate::runner::{
    LiveResourceReservation, ProcessFailure, ProcessRunState, RunnerReportedProcessObservation,
    SolverProcessRunner,
};
use lc631_process_supervisor::{
    required_ram_bytes, run_bounded_with_resources_observed, RunRequest,
};
use lc631_substrate::resource::{
    ResourceAmount, ResourceClaim, ResourceKind, ResourceRequest, ResourceReservationView,
};
use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::time::Duration;

const ADAPTER_BUFFER_METADATA_BYTES: usize = 4096;

pub(crate) struct BoundedSolverRunner<'view, 'lease> {
    reservation: &'view ResourceReservationView<'lease>,
    working_directory: &'view Path,
    arguments: Vec<OsString>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    _retained_output_claim: Option<ResourceClaim<'view>>,
}

impl<'view, 'lease> BoundedSolverRunner<'view, 'lease> {
    pub(crate) fn new(
        reservation: &'view ResourceReservationView<'lease>,
        working_directory: &'view Path,
    ) -> Self {
        Self {
            reservation,
            working_directory,
            arguments: Vec::new(),
            stdout: Vec::new(),
            stderr: Vec::new(),
            _retained_output_claim: None,
        }
    }

    pub(crate) fn stdout(&self) -> &[u8] {
        &self.stdout
    }

    fn reserve_and_prepare_buffers(
        &mut self,
        request: &crate::runner::ProcessRequest<'_>,
    ) -> Result<(), ProcessFailure> {
        let stdout_limit = request.budget.max_stdout_bytes();
        let stderr_limit = request.budget.max_stderr_bytes();
        let output_bytes = stdout_limit
            .checked_add(stderr_limit)
            .ok_or(ProcessFailure::ResourceExhaustedBeforeStart)?;
        let argument_bytes = request
            .fixed_arguments
            .iter()
            .try_fold(0_usize, |total, arg| {
                let encoded = encoded_os_string_bytes(arg)?;
                total
                    .checked_add(encoded)
                    .ok_or(ProcessFailure::ResourceExhaustedBeforeStart)
            })?;
        let argument_metadata = request
            .fixed_arguments
            .len()
            .checked_mul(std::mem::size_of::<OsString>())
            .ok_or(ProcessFailure::ResourceExhaustedBeforeStart)?;
        let retained_ram = output_bytes
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(argument_bytes.checked_mul(2)?))
            .and_then(|bytes| bytes.checked_add(argument_metadata))
            .and_then(|bytes| bytes.checked_add(ADAPTER_BUFFER_METADATA_BYTES))
            .ok_or(ProcessFailure::ResourceExhaustedBeforeStart)?;
        let retained_ram = u64::try_from(retained_ram)
            .map_err(|_| ProcessFailure::ResourceExhaustedBeforeStart)?;
        let retained_output = u64::try_from(output_bytes)
            .map_err(|_| ProcessFailure::ResourceExhaustedBeforeStart)?;
        let ram_amount = ResourceAmount::checked(retained_ram)
            .map_err(|_| ProcessFailure::ResourceExhaustedBeforeStart)?;
        let output_amount = ResourceAmount::checked(retained_output)
            .map_err(|_| ProcessFailure::ResourceExhaustedBeforeStart)?;
        let claim = self
            .reservation
            .claim_resources(&[
                ResourceRequest::new(ResourceKind::Ram, ram_amount),
                ResourceRequest::new(ResourceKind::Output, output_amount),
            ])
            .map_err(map_prestart_resource_error)?;
        // Retain the charge before any fallible allocation so partially
        // prepared buffers remain accounted until this runner is dropped.
        self._retained_output_claim = Some(claim);

        self.arguments.clear();
        self.arguments
            .try_reserve_exact(request.fixed_arguments.len())
            .map_err(|_| ProcessFailure::ResourceExhaustedBeforeStart)?;
        if self.arguments.capacity() > request.fixed_arguments.len().saturating_mul(2).max(1) {
            return Err(ProcessFailure::ResourceExhaustedBeforeStart);
        }
        for argument in request.fixed_arguments {
            self.arguments.push(OsString::from(argument));
        }

        self.stdout.clear();
        self.stdout
            .try_reserve_exact(stdout_limit)
            .map_err(|_| ProcessFailure::ResourceExhaustedBeforeStart)?;
        self.stderr.clear();
        self.stderr
            .try_reserve_exact(stderr_limit)
            .map_err(|_| ProcessFailure::ResourceExhaustedBeforeStart)?;
        if self.stdout.capacity() > stdout_limit.saturating_mul(2)
            || self.stderr.capacity() > stderr_limit.saturating_mul(2)
        {
            return Err(ProcessFailure::ResourceExhaustedBeforeStart);
        }
        Ok(())
    }
}

impl SolverProcessRunner for BoundedSolverRunner<'_, '_> {
    fn run<'call>(
        &'call mut self,
        request: crate::runner::ProcessRequest<'call>,
        reservation: &'call dyn LiveResourceReservation,
    ) -> Result<RunnerReportedProcessObservation<'call>, ProcessFailure> {
        reservation
            .validate(request.source, &request.budget)
            .map_err(map_reservation_failure)?;
        self.reservation
            .check()
            .map_err(map_prestart_resource_error)?;

        let executable = Path::new(request.executable);
        let hash_buffer_claim = reserve_hash_buffer(self.reservation)?;
        let actual_digest = hash_file_contents(self.reservation, executable)?;
        drop(hash_buffer_claim);
        if actual_digest != *request.expected_executable_digest {
            return Ok(RunnerReportedProcessObservation {
                state: ProcessRunState::NotStarted,
                source: request.source,
                query: request.query,
                invocation: request.invocation,
                observed_arguments: &[],
                executable_digest: actual_digest,
                exit_code: None,
                stdout: &[],
                stderr: &[],
                stdout_truncated: false,
                stderr_truncated: false,
                cleanup_verified: true,
            });
        }

        self.reserve_and_prepare_buffers(&request)?;
        let run_request = RunRequest {
            executable: OsStr::new(request.executable),
            arguments: &self.arguments,
            current_dir: self.working_directory,
            environment: &[],
            stdin: Some(request.stdin),
            timeout: Duration::from_millis(request.budget.timeout_ms()),
            stdout_limit: request.budget.max_stdout_bytes(),
            stderr_limit: request.budget.max_stderr_bytes(),
        };
        let process_ram = required_ram_bytes(&run_request)
            .map_err(|_| ProcessFailure::ResourceExhaustedBeforeStart)?;
        if process_ram > request.budget.ram_bytes() {
            return Err(ProcessFailure::ResourceExhaustedBeforeStart);
        }

        let observed = match run_bounded_with_resources_observed(run_request, self.reservation) {
            Ok(observed) => observed,
            Err(error) => return Err(map_supervisor_error(error)),
        };
        if observed.cancelled() {
            drop(observed);
            return Err(ProcessFailure::Cancelled);
        }

        let outcome = observed.outcome();
        if outcome.stdout.len() > self.stdout.capacity()
            || outcome.stderr.len() > self.stderr.capacity()
        {
            drop(observed);
            return Err(ProcessFailure::ResourceExhaustedAfterStartUnknown);
        }
        self.stdout.extend_from_slice(&outcome.stdout);
        self.stderr.extend_from_slice(&outcome.stderr);
        let state = if outcome.timed_out {
            ProcessRunState::TimedOut
        } else if outcome.exit_code.is_some() {
            ProcessRunState::Completed
        } else {
            ProcessRunState::Failed
        };
        let exit_code = outcome.exit_code;
        let stdout_truncated = outcome.stdout_truncated;
        let stderr_truncated = outcome.stderr_truncated;
        let cleanup_verified = observed.cleanup_complete();
        drop(observed);

        Ok(self.observation(
            request,
            state,
            actual_digest,
            exit_code,
            &self.stdout,
            &self.stderr,
            stdout_truncated,
            stderr_truncated,
            cleanup_verified,
        ))
    }
}

impl BoundedSolverRunner<'_, '_> {
    #[allow(clippy::too_many_arguments)]
    fn observation<'call>(
        &'call self,
        request: crate::runner::ProcessRequest<'call>,
        state: ProcessRunState,
        executable_digest: [u8; 32],
        exit_code: Option<i32>,
        stdout: &'call [u8],
        stderr: &'call [u8],
        stdout_truncated: bool,
        stderr_truncated: bool,
        cleanup_verified: bool,
    ) -> RunnerReportedProcessObservation<'call> {
        RunnerReportedProcessObservation {
            state,
            source: request.source,
            query: request.query,
            invocation: request.invocation,
            observed_arguments: request.fixed_arguments,
            executable_digest,
            exit_code,
            stdout,
            stderr,
            stdout_truncated,
            stderr_truncated,
            cleanup_verified,
        }
    }
}

fn encoded_os_string_bytes(value: &str) -> Result<usize, ProcessFailure> {
    #[cfg(windows)]
    {
        value
            .encode_utf16()
            .count()
            .checked_mul(std::mem::size_of::<u16>())
            .ok_or(ProcessFailure::ResourceExhaustedBeforeStart)
    }
    #[cfg(not(windows))]
    {
        Ok(value.len())
    }
}
