//! Resource-view validation and explicit process-supervisor error mapping.
use crate::runner::{
    ExecutionBudget, LiveResourceReservation, ProcessFailure, QuerySourceIdentity,
    ReservationFailure,
};
use lc631_process_supervisor::ResourceRunError;
use lc631_substrate::resource::{ResourceError, ResourceKind, ResourceReservationView};

impl LiveResourceReservation for ResourceReservationView<'_> {
    fn validate(
        &self,
        expected_source: QuerySourceIdentity,
        budget: &ExecutionBudget,
    ) -> Result<(), ReservationFailure> {
        self.check().map_err(map_reservation_resource_error)?;
        if self.source_revision()
            != lc631_substrate::Digest32::from_bytes(*expected_source.as_bytes())
        {
            return Err(ReservationFailure::SourceMismatch);
        }
        require_amount(self, ResourceKind::Child, 1)?;
        require_amount(self, ResourceKind::Cleanup, 1)?;
        require_amount(self, ResourceKind::Ram, budget.ram_bytes())?;
        let output_bytes = budget
            .max_stdout_bytes()
            .checked_add(budget.max_stderr_bytes())
            .and_then(|bytes| bytes.checked_mul(2))
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(ReservationFailure::Unknown)?;
        require_amount(self, ResourceKind::Output, output_bytes)?;
        if budget.disk_bytes() > 0 {
            require_amount(self, ResourceKind::Disk, budget.disk_bytes())?;
        }
        Ok(())
    }
}

fn require_amount(
    reservation: &ResourceReservationView<'_>,
    kind: ResourceKind,
    amount: u64,
) -> Result<(), ReservationFailure> {
    let available = reservation
        .amount(kind)
        .map_err(map_reservation_resource_error)?;
    if available < amount {
        return Err(ReservationFailure::Insufficient);
    }
    Ok(())
}

pub(super) fn map_reservation_failure(failure: ReservationFailure) -> ProcessFailure {
    match failure {
        ReservationFailure::Cancelled => ProcessFailure::Cancelled,
        ReservationFailure::DeadlineExceeded => ProcessFailure::DeadlineExceeded,
        ReservationFailure::Insufficient => ProcessFailure::ResourceExhaustedBeforeStart,
        ReservationFailure::SourceMismatch => ProcessFailure::Failed,
        ReservationFailure::Stale | ReservationFailure::Unknown => ProcessFailure::NotStarted,
    }
}

fn map_reservation_resource_error(error: ResourceError) -> ReservationFailure {
    match error {
        ResourceError::Cancelled => ReservationFailure::Cancelled,
        ResourceError::DeadlineExceeded => ReservationFailure::DeadlineExceeded,
        ResourceError::SourceMismatch => ReservationFailure::SourceMismatch,
        ResourceError::Exhausted(_) | ResourceError::Unreserved(_) => {
            ReservationFailure::Insufficient
        }
        ResourceError::StaleLease | ResourceError::CleanupUnverified | ResourceError::Poisoned => {
            ReservationFailure::Stale
        }
        _ => ReservationFailure::Unknown,
    }
}

pub(super) fn map_prestart_resource_error(error: ResourceError) -> ProcessFailure {
    match error {
        ResourceError::Cancelled => ProcessFailure::Cancelled,
        ResourceError::DeadlineExceeded => ProcessFailure::DeadlineExceeded,
        ResourceError::Exhausted(_)
        | ResourceError::Unreserved(_)
        | ResourceError::AllocationFailed => ProcessFailure::ResourceExhaustedBeforeStart,
        _ => ProcessFailure::NotStarted,
    }
}

pub(super) fn map_supervisor_error(error: ResourceRunError) -> ProcessFailure {
    match error {
        ResourceRunError::NotStarted(std::io::ErrorKind::OutOfMemory) => {
            ProcessFailure::ResourceExhaustedBeforeStart
        }
        ResourceRunError::NotStarted(_) => ProcessFailure::NotStarted,
        ResourceRunError::Resource(ResourceError::Cancelled) => ProcessFailure::Cancelled,
        ResourceRunError::Resource(ResourceError::DeadlineExceeded) => {
            ProcessFailure::DeadlineExceeded
        }
        ResourceRunError::Resource(ResourceError::Io(_)) => ProcessFailure::StartedUnknown,
        ResourceRunError::Resource(
            ResourceError::Exhausted(_)
            | ResourceError::Unreserved(_)
            | ResourceError::AllocationFailed,
        ) => ProcessFailure::ResourceExhaustedBeforeStart,
        ResourceRunError::Resource(_) => ProcessFailure::NotStarted,
    }
}
