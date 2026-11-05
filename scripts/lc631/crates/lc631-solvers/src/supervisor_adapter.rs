//! Bounded process-supervisor adapter for registered solver requests.
//!
//! This facade reports process observations only. It does not authenticate
//! executed images, sandbox filesystem/network access, or validate models/proofs.

mod executable;
mod process;
mod resources;
// Concrete in-crate producers may retain the runner's actual bounded output;
// this is not a public report-injection or process-authentication surface.
pub(crate) use process::BoundedSolverRunner;

pub use executable::hash_registered_executable;

use crate::registry::SolverRegistry;
use crate::result_store::{ResultStore, ResultStoreError, ResultStoreInsert};
use crate::runner::{
    execute_registered_native, ExecutionBudget, SolverExecutionReport, SolverQuery,
};
use lc631_substrate::resource::ResourceReservationView;
use std::path::Path;

/// Adapter setup failures remain distinct from solver execution results.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupervisorAdapterError {
    RelativeWorkingDirectory,
    DiskQuotaUnsupported,
    /// No process was dispatched and no process report was constructed.
    StoreBeforeExecution(ResultStoreError),
}

/// One process report plus its bounded in-memory result-store insertion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SupervisedStoreReceipt {
    report: SolverExecutionReport,
    insertion: Result<ResultStoreInsert, ResultStoreError>,
}

impl SupervisedStoreReceipt {
    pub const fn report(self) -> SolverExecutionReport {
        self.report
    }

    pub const fn insertion(self) -> Result<ResultStoreInsert, ResultStoreError> {
        self.insertion
    }
}

/// Execute one registered query through the bounded process supervisor.
///
/// This private-runner entry point uses the same live resource view for
/// preflight and process admission. It starts at most one child and passes an
/// empty environment. A nonzero disk budget is unsupported; zero is not an OS
/// filesystem sandbox.
pub fn execute_supervised(
    registry: &SolverRegistry,
    query: &SolverQuery<'_>,
    budget: &ExecutionBudget,
    reservation: &ResourceReservationView<'_>,
    working_directory: &Path,
) -> Result<SolverExecutionReport, SupervisorAdapterError> {
    if !working_directory.is_absolute() {
        return Err(SupervisorAdapterError::RelativeWorkingDirectory);
    }
    if budget.disk_bytes() != 0 {
        return Err(SupervisorAdapterError::DiskQuotaUnsupported);
    }

    let mut runner = process::BoundedSolverRunner::new(reservation, working_directory);
    Ok(execute_registered_native(
        registry,
        query,
        budget,
        reservation,
        &mut runner,
        false,
    ))
}

mod owned_payload;
pub use owned_payload::{
    execute_supervised_owned_payload_into_store, OwnedPayloadError, OwnedPayloadObservation,
    OwnedPayloadStore, OwnedPayloadStoreInitError,
};
#[cfg(test)]
pub(crate) use owned_payload::{finish_owned_for_test, owned_test_hooks};

/// Reserve one store slot before dispatch, then execute one query and append
/// its terminal report. A preflight failure is NoRun, not ResourceRejected or
/// a fabricated solver report. The returned identity remains runner-reported,
/// not a validated solver fact. On append failure the exact report remains in the receipt next
/// to the typed insertion error, so the terminal state is not lost.
pub fn execute_supervised_into_store(
    registry: &SolverRegistry,
    query: &SolverQuery<'_>,
    budget: &ExecutionBudget,
    reservation: &ResourceReservationView<'_>,
    working_directory: &Path,
    store: &mut ResultStore,
) -> Result<SupervisedStoreReceipt, SupervisorAdapterError> {
    store
        .try_reserve_append()
        .map_err(SupervisorAdapterError::StoreBeforeExecution)?;
    let report = execute_supervised(registry, query, budget, reservation, working_directory)?;
    Ok(append_report_for_query(store, query, report))
}

/// Consume an already-produced report through the same query-bound store seam.
/// This is useful for adapters that own process execution elsewhere.
pub fn append_report_for_query(
    store: &mut ResultStore,
    query: &SolverQuery<'_>,
    report: SolverExecutionReport,
) -> SupervisedStoreReceipt {
    SupervisedStoreReceipt {
        report,
        insertion: store.append_for_query(query, report),
    }
}
