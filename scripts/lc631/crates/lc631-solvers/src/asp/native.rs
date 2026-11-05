//! Native-only Clingo producer. No public constructor accepts reports or raw output.
//! Owners retain their lease charges until their bytes and result metadata drop.
use super::{AspError, AspEvaluation, AspProgram};
use crate::protocol::SolverProtocol;
use crate::registry::{SolverRegistry, SolverRegistryError};
use crate::result_store::{
    ResultStore, ResultStoreError, ResultStoreInsert, SolverResultId, StoredSolverResult,
};
use crate::runner::{
    execute_registered_native, ExecutionBudget, QuerySourceIdentity, SolverExecutionReport,
    SolverExecutionState, SolverQuery,
};
use crate::supervisor_adapter::BoundedSolverRunner;
use lc631_substrate::resource::{
    ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
    ResourceReservationView,
};
use std::path::Path;

/// Metadata owner with no report-insertion API. Reads do not renew lifecycle state.
pub struct NativeAspStore<'view> {
    store: ResultStore,
    source: QuerySourceIdentity,
    claim: ResourceClaim<'view>,
}

#[derive(Debug)]
pub enum NativeAspSetupError {
    Resource(ResourceError),
    Store(ResultStoreError),
    Registry(SolverRegistryError),
    Program(AspError),
    RelativeDirectory,
    DiskQuotaUnsupported,
    UnsupportedMapping,
}

impl<'view> NativeAspStore<'view> {
    pub fn with_resources(
        capacity: usize,
        view: &'view ResourceReservationView<'_>,
    ) -> Result<Self, NativeAspSetupError> {
        let _ = ResultStore::with_capacity(capacity).map_err(NativeAspSetupError::Store)?;
        let bytes = std::mem::size_of::<StoredSolverResult>()
            .checked_mul(capacity)
            .and_then(|n| u64::try_from(n).ok())
            .ok_or(NativeAspSetupError::Resource(
                ResourceError::ArithmeticOverflow,
            ))?;
        let claim = view
            .claim_resources(&[ResourceRequest::new(
                ResourceKind::Ram,
                ResourceAmount::checked(bytes).map_err(NativeAspSetupError::Resource)?,
            )])
            .map_err(NativeAspSetupError::Resource)?;
        let mut store = ResultStore::with_capacity(capacity).map_err(NativeAspSetupError::Store)?;
        store
            .try_reserve_backing()
            .map_err(NativeAspSetupError::Store)?;
        Ok(Self {
            store,
            source: QuerySourceIdentity::from_bytes(*view.source_revision().as_bytes()),
            claim,
        })
    }
    pub fn len(&self) -> usize {
        self.store.len()
    }
    pub fn is_empty(&self) -> bool {
        self.store.is_empty()
    }
    pub fn lookup_by_id(&self, id: SolverResultId) -> Option<&StoredSolverResult> {
        self.store.lookup_by_id(id)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeAspRejection {
    Process,
    Store(ResultStoreError),
    Asp(AspError),
}

/// Actual process reports and complete/partial captured stdout survive semantic rejection.
/// No Clone/Deserialize/raw-report constructor; dropping bytes precedes refunding claims.
pub struct NativeAspOutcome<'view> {
    stdout: Vec<u8>,
    evaluation: Result<AspEvaluation, NativeAspRejection>,
    report: SolverExecutionReport,
    insertion: Result<ResultStoreInsert, ResultStoreError>,
    _claim: ResourceClaim<'view>,
}

impl NativeAspOutcome<'_> {
    pub fn stdout(&self) -> &[u8] {
        &self.stdout
    }
    pub const fn report(&self) -> SolverExecutionReport {
        self.report
    }
    pub const fn insertion(&self) -> Result<ResultStoreInsert, ResultStoreError> {
        self.insertion
    }
    pub const fn evaluation(&self) -> &Result<AspEvaluation, NativeAspRejection> {
        &self.evaluation
    }
}

struct PreparedBuffers<'view> {
    stdout: Vec<u8>,
    claim: ResourceClaim<'view>,
}

fn prepare_buffers<'view>(
    program: &AspProgram,
    budget: &ExecutionBudget,
    view: &'view ResourceReservationView<'_>,
) -> Result<PreparedBuffers<'view>, ResourceError> {
    // Cover query emitter growth, hashing, bounded parser and finite semantic workspace.
    // Fixed parser limits: <=16K atom-line records, <=256 answer records, <=16 atoms.
    let ram = program
        .limits
        .output
        .checked_mul(2)
        .and_then(|n| n.checked_add(budget.max_stdout_bytes()))
        .and_then(|n| n.checked_add(1024 * 1024))
        .and_then(|n| u64::try_from(n).ok())
        .ok_or(ResourceError::ArithmeticOverflow)?;
    let output =
        u64::try_from(budget.max_stdout_bytes()).map_err(|_| ResourceError::ArithmeticOverflow)?;
    let claim = view.claim_resources(&[
        ResourceRequest::new(ResourceKind::Ram, ResourceAmount::checked(ram)?),
        ResourceRequest::new(ResourceKind::Output, ResourceAmount::checked(output)?),
    ])?;
    let mut stdout = Vec::new();
    stdout
        .try_reserve_exact(budget.max_stdout_bytes())
        .map_err(|_| ResourceError::AllocationFailed)?;
    if stdout.capacity() > budget.max_stdout_bytes() {
        return Err(ResourceError::AllocationFailed);
    }
    Ok(PreparedBuffers { stdout, claim })
}

/// Only this entry produces an owner: concrete native runner, internally appended report,
/// exact captured bytes and checked context-bound semantic reconstruction.
/// Caller-supplied executable/module pins remain declarations, not process authentication.
pub fn execute_native_asp<'view>(
    program: &AspProgram,
    registry: &SolverRegistry,
    budget: &ExecutionBudget,
    view: &'view ResourceReservationView<'_>,
    cwd: &Path,
    store: &mut NativeAspStore<'_>,
) -> Result<NativeAspOutcome<'view>, NativeAspSetupError> {
    if !cwd.is_absolute() {
        return Err(NativeAspSetupError::RelativeDirectory);
    }
    if budget.disk_bytes() != 0 {
        return Err(NativeAspSetupError::DiskQuotaUnsupported);
    }
    store.claim.check().map_err(NativeAspSetupError::Resource)?;
    if !store.claim.same_reservation(view) {
        return Err(NativeAspSetupError::Resource(ResourceError::ForeignOwner));
    }
    let source =
        QuerySourceIdentity::from_bytes(*program.binding.context().source().digest().as_bytes());
    if store.source != source || view.source_revision().as_bytes() != source.as_bytes() {
        return Err(NativeAspSetupError::Resource(ResourceError::SourceMismatch));
    }
    let retained = prepare_buffers(program, budget, view).map_err(NativeAspSetupError::Resource)?;
    let prepared = program.prepare().map_err(NativeAspSetupError::Program)?;
    let query = prepared
        .solver_query()
        .map_err(NativeAspSetupError::Program)?;
    let registration = registry
        .resolve_for_query(query.backend(), query.profile(), query.result_kind())
        .map_err(NativeAspSetupError::Registry)?;
    if registration.protocol() != SolverProtocol::ClingoStatusV1 {
        return Err(NativeAspSetupError::UnsupportedMapping);
    }
    store
        .store
        .try_reserve_append()
        .map_err(NativeAspSetupError::Store)?;
    let mut runner = BoundedSolverRunner::new(view, cwd);
    let report = execute_registered_native(registry, &query, budget, view, &mut runner, false);
    let outcome = finish_native_asp(program, &query, report, runner.stdout(), retained, store);
    drop(runner);
    Ok(outcome)
}

// The only production caller above supplies a concrete runner's actual bytes.
// cfg(test) callers exercise failure cuts; they do not observe a native process.
fn finish_native_asp<'view>(
    program: &AspProgram,
    query: &SolverQuery<'_>,
    report: SolverExecutionReport,
    captured: &[u8],
    mut retained: PreparedBuffers<'view>,
    store: &mut NativeAspStore<'_>,
) -> NativeAspOutcome<'view> {
    let insertion = store.store.append_for_query(query, report);
    let fits = captured.len() <= retained.stdout.capacity();
    // Even an internal faulty capture cannot cause uncharged Vec growth.
    retained
        .stdout
        .extend_from_slice(&captured[..captured.len().min(retained.stdout.capacity())]);
    let evaluation = if !fits {
        Err(NativeAspRejection::Process)
    } else {
        match insertion {
            Err(error) => Err(NativeAspRejection::Store(error)),
            Ok(insert) => match store.store.lookup_by_id(insert.id()) {
                None => Err(NativeAspRejection::Process),
                Some(stored)
                    if report.state() == SolverExecutionState::SolverReported
                        && report.stderr_fully_observed() =>
                {
                    program
                        .reconstruct_stored(stored, &retained.stdout)
                        .map_err(NativeAspRejection::Asp)
                }
                Some(_) => Err(NativeAspRejection::Process),
            },
        }
    };
    NativeAspOutcome {
        stdout: retained.stdout,
        evaluation,
        report,
        insertion,
        _claim: retained.claim,
    }
}

#[cfg(test)]
#[path = "native_tests.rs"]
mod tests;
