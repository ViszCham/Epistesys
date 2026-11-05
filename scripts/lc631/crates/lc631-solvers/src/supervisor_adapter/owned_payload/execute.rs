//! Pre-admitted native execution, exact-byte copy, and binding checks.
use super::super::{process, SupervisorAdapterError};
use super::{OwnedPayloadError, OwnedPayloadObservation, OwnedPayloadStore};
use crate::protocol::SolverProtocol;
use crate::registry::SolverRegistry;
use crate::runner::{
    execute_registered_native, output_witness_digest, ExecutionBudget, SolverExecutionReport,
    SolverExecutionState, SolverQuery,
};
use lc631_substrate::resource::{
    ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
    ResourceReservationView,
};
use sha2::{Digest, Sha256};
use std::path::Path;

struct RetainedBuffer<'view> {
    bytes: Vec<u8>,
    // The backing bytes drop before the charge.
    claim: ResourceClaim<'view>,
}

fn prepare_retained_buffer<'view>(
    reservation: &'view ResourceReservationView<'_>,
    max: usize,
) -> Result<RetainedBuffer<'view>, ResourceError> {
    let amount = ResourceAmount::checked(
        u64::try_from(max).map_err(|_| ResourceError::ArithmeticOverflow)?,
    )?;
    let claim = reservation.claim_resources(&[
        ResourceRequest::new(ResourceKind::Ram, amount),
        ResourceRequest::new(ResourceKind::Output, amount),
    ])?;
    #[cfg(test)]
    owned_test_hooks::before_copy_allocation();
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(max)
        .map_err(|_| ResourceError::AllocationFailed)?;
    if bytes.capacity() > max {
        return Err(ResourceError::AllocationFailed);
    }
    Ok(RetainedBuffer { bytes, claim })
}

fn check_owned_store(
    query: &SolverQuery<'_>,
    reservation: &ResourceReservationView<'_>,
    store: &OwnedPayloadStore<'_>,
) -> Result<(), ResourceError> {
    store.claim.check()?;
    if !store.claim.same_reservation(reservation) {
        return Err(ResourceError::ForeignOwner);
    }
    if store.source != query.source()
        || reservation.source_revision().as_bytes() != query.source().as_bytes()
    {
        return Err(ResourceError::SourceMismatch);
    }
    Ok(())
}

fn preparation_error(error: ResourceError) -> OwnedPayloadError {
    match error {
        ResourceError::AllocationFailed | ResourceError::ArithmeticOverflow => {
            OwnedPayloadError::AllocationFailed
        }
        error => OwnedPayloadError::Resource(error),
    }
}

/// Shared production/test finalizer. It does not mint an owner or append a
/// record until the actual report and complete captured bytes pass every check.
#[allow(clippy::result_large_err)]
fn finish_owned_payload<'view>(
    query: &SolverQuery<'_>,
    budget: &ExecutionBudget,
    report: SolverExecutionReport,
    captured: &[u8],
    mut retained: RetainedBuffer<'view>,
    store: &mut OwnedPayloadStore<'_>,
) -> Result<OwnedPayloadObservation<'view>, OwnedPayloadError> {
    if report.state() != SolverExecutionState::PayloadWireObserved
        || !report.stdout_fully_observed()
        || !report.stderr_fully_observed()
        || report.source() != query.source()
        || report.query() != query.digest()
        || report.invocation_digest().is_none()
    {
        return Err(OwnedPayloadError::Report(report));
    }
    let Some(summary) = report.payload_wire_summary() else {
        return Err(OwnedPayloadError::Report(report));
    };
    let max = budget.max_stdout_bytes();
    if captured.len() > max
        || captured.len() != report.stdout_bytes()
        || u32::try_from(captured.len()).ok() != Some(summary.stdout_len())
        || output_witness_digest(b"stdout", captured, max, false) != report.stdout_digest()
        || report.payload_wire_status() != Some(summary.status())
    {
        return Err(OwnedPayloadError::Report(report));
    }
    let mut hash = Sha256::new();
    hash.update(b"epistesys.tlcx.smt-payload-wire.v2\0");
    let Ok(length) = u64::try_from(captured.len()) else {
        return Err(OwnedPayloadError::Report(report));
    };
    hash.update(length.to_le_bytes());
    hash.update(captured);
    let digest: [u8; 32] = hash.finalize().into();
    let span = summary.payload_span();
    let (Ok(start), Ok(end)) = (usize::try_from(span.start()), usize::try_from(span.end())) else {
        return Err(OwnedPayloadError::Report(report));
    };
    if digest != summary.stdout_digest() || start > end || end > captured.len() {
        return Err(OwnedPayloadError::Report(report));
    }
    retained.bytes.extend_from_slice(captured);
    let insertion = store
        .store
        .append_for_query(query, report)
        .map_err(|error| OwnedPayloadError::StoreAfterExecution { report, error })?;
    let RetainedBuffer { bytes, claim } = retained;
    Ok(OwnedPayloadObservation {
        stdout: bytes,
        report,
        insertion,
        _claim: claim,
    })
}

/// Unit-only injection into the same finalizer. This is absent in production.
#[cfg(test)]
#[allow(clippy::result_large_err)]
pub(crate) fn finish_owned_for_test<'view>(
    query: &SolverQuery<'_>,
    budget: &ExecutionBudget,
    reservation: &'view ResourceReservationView<'_>,
    store: &mut OwnedPayloadStore<'_>,
    report: SolverExecutionReport,
    captured: &[u8],
) -> Result<OwnedPayloadObservation<'view>, OwnedPayloadError> {
    check_owned_store(query, reservation, store).map_err(OwnedPayloadError::Resource)?;
    store
        .store
        .try_reserve_append()
        .map_err(OwnedPayloadError::StoreBeforeExecution)?;
    let retained = prepare_retained_buffer(reservation, budget.max_stdout_bytes())
        .map_err(preparation_error)?;
    finish_owned_payload(query, budget, report, captured, retained, store)
}

/// Runs only the registered V2 query through the concrete native runner.
/// Every fallible storage reservation occurs before dispatch. No external
/// report or raw stdout can be attached to this producer.
#[allow(clippy::too_many_arguments, clippy::result_large_err)]
pub fn execute_supervised_owned_payload_into_store<'view>(
    registry: &SolverRegistry,
    query: &SolverQuery<'_>,
    budget: &ExecutionBudget,
    reservation: &'view ResourceReservationView<'_>,
    working_directory: &Path,
    store: &mut OwnedPayloadStore<'_>,
) -> Result<OwnedPayloadObservation<'view>, OwnedPayloadError> {
    if !working_directory.is_absolute() {
        return Err(OwnedPayloadError::Adapter(
            SupervisorAdapterError::RelativeWorkingDirectory,
        ));
    }
    if budget.disk_bytes() != 0 {
        return Err(OwnedPayloadError::Adapter(
            SupervisorAdapterError::DiskQuotaUnsupported,
        ));
    }
    check_owned_store(query, reservation, store).map_err(OwnedPayloadError::Resource)?;
    let registration = registry
        .resolve_for_query(query.backend(), query.profile(), query.result_kind())
        .map_err(OwnedPayloadError::Registry)?;
    if registration.protocol() != SolverProtocol::SmtLibPayloadWireV2
        || crate::protocol::query_protocol_compatibility(
            registration.protocol(),
            query.result_kind(),
        ) != crate::protocol::QueryProtocolCompatibility::Supported
    {
        return Err(OwnedPayloadError::UnsupportedMapping);
    }
    store
        .store
        .try_reserve_append()
        .map_err(OwnedPayloadError::StoreBeforeExecution)?;
    let retained = prepare_retained_buffer(reservation, budget.max_stdout_bytes())
        .map_err(preparation_error)?;
    let mut runner = process::BoundedSolverRunner::new(reservation, working_directory);
    #[cfg(test)]
    owned_test_hooks::before_native_dispatch();
    let report = execute_registered_native(registry, query, budget, reservation, &mut runner, true);
    let result = finish_owned_payload(query, budget, report, runner.stdout(), retained, store);
    drop(runner);
    result
}

#[cfg(test)]
pub(crate) mod owned_test_hooks {
    use std::cell::Cell;
    std::thread_local! {
        static COPY_ALLOCATION: Cell<usize> = const { Cell::new(0) };
        static NATIVE_DISPATCH: Cell<usize> = const { Cell::new(0) };
    }
    pub(crate) fn reset() {
        COPY_ALLOCATION.with(|v| v.set(0));
        NATIVE_DISPATCH.with(|v| v.set(0));
    }
    /// Per-thread test checkpoints, not a global allocator/process audit.
    pub(crate) fn counters() -> (usize, usize) {
        (
            COPY_ALLOCATION.with(Cell::get),
            NATIVE_DISPATCH.with(Cell::get),
        )
    }
    pub(super) fn before_copy_allocation() {
        COPY_ALLOCATION.with(|v| v.set(v.get() + 1));
    }
    pub(super) fn before_native_dispatch() {
        NATIVE_DISPATCH.with(|v| v.set(v.get() + 1));
    }
}
