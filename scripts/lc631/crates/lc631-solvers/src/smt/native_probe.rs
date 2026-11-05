//! One actual strict-improvement query, never a full optimization certificate.
//!
//! Detached arithmetic models cannot serve as native incumbents:
//! ```compile_fail
//! use lc631_solvers::smt::{CheckedModel, native_model::NativeCheckedModel};
//! fn replace<'q, 'v>(model: CheckedModel<'q>) -> NativeCheckedModel<'q, 'v> { model }
//! ```
//! Context cannot be changed through the returned outcome:
//! ```compile_fail
//! use lc631_substrate::context_identity::ContextRevision;
//! use lc631_solvers::smt::native_probe::NativeProbeOutcome;
//! fn rewrite(outcome: &mut NativeProbeOutcome<'_, '_, '_>, context: ContextRevision) {
//!     outcome.context = context;
//! }
//! ```
use super::{
    native_model::NativeCheckedModel, ObjectiveDirection, OptimizationProbe, OptimizationProbeError,
};
use crate::registry::SolverRegistry;
use crate::result_store::{ResultStore, ResultStoreError, ResultStoreInsert, StoredSolverResult};
use crate::runner::{ExecutionBudget, SolverExecutionReport, SolverExecutionState};
use crate::supervisor_adapter::{execute_supervised_into_store, SupervisorAdapterError};
use crate::typed_query::{BoolExpr, IntExpr, IntSymbolId, PreparedSmtStatusQuery, QueryEmitError};
use crate::{LogicalResult, Satisfiability};
use lc631_substrate::context_identity::ContextRevision;
use lc631_substrate::resource::{
    ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
    ResourceReservationView,
};
use std::path::Path;

pub mod bounded;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeProbeStatus {
    SolverReportedNoStrictImprovement,
    SolverReportedImprovementExists,
    Unresolved { execution: SolverExecutionState },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Unboundedness {
    NotEstablished,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeProbeError {
    SourceMismatch,
    Resource(ResourceError),
    Probe(OptimizationProbeError),
    Query(QueryEmitError),
    StoreBeforeExecution(ResultStoreError),
    Adapter(SupervisorAdapterError),
}

/// The sole constructor performs native execution. Every terminal report is
/// retained, including append failure. No caller report or status is accepted.
pub struct NativeProbeOutcome<'incumbent, 'query, 'view> {
    context: ContextRevision,
    probe: OptimizationProbe<'incumbent, 'query>,
    prepared: PreparedSmtStatusQuery,
    report: SolverExecutionReport,
    insertion: Result<ResultStoreInsert, ResultStoreError>,
    store: ResultStore,
    // Owned AST, emitted bytes and store backing drop before their charges.
    _clone_claim: ResourceClaim<'view>,
    _emission_claim: ResourceClaim<'view>,
    _store_claim: ResourceClaim<'view>,
}
impl NativeProbeOutcome<'_, '_, '_> {
    pub const fn context(&self) -> ContextRevision {
        self.context
    }
    pub fn probe(&self) -> &OptimizationProbe<'_, '_> {
        &self.probe
    }
    pub fn prepared(&self) -> &PreparedSmtStatusQuery {
        &self.prepared
    }
    pub const fn report(&self) -> SolverExecutionReport {
        self.report
    }
    pub const fn insertion(&self) -> Result<ResultStoreInsert, ResultStoreError> {
        self.insertion
    }
    pub fn stored(&self) -> Option<&StoredSolverResult> {
        self.insertion
            .ok()
            .and_then(|entry| self.store.lookup_by_id(entry.id()))
    }
    pub const fn unboundedness(&self) -> Unboundedness {
        Unboundedness::NotEstablished
    }
    pub fn status(&self) -> NativeProbeStatus {
        let unresolved = NativeProbeStatus::Unresolved {
            execution: self.report.state(),
        };
        let Ok(query) = self.prepared.solver_query() else {
            return unresolved;
        };
        let Some(stored) = self.stored() else {
            return unresolved;
        };
        if self.report.source() != query.source()
            || self.report.query() != query.digest()
            || self.report.invocation_digest().is_none()
            || stored.report() != self.report
            || self.report.state() != SolverExecutionState::SolverReported
        {
            return unresolved;
        }
        match self.report.logical_result() {
            Some(LogicalResult::Satisfiability(Satisfiability::Unsat)) => {
                NativeProbeStatus::SolverReportedNoStrictImprovement
            }
            Some(LogicalResult::Satisfiability(Satisfiability::Sat)) => {
                NativeProbeStatus::SolverReportedImprovementExists
            }
            _ => unresolved,
        }
    }
}

fn claim<'view>(
    view: &'view ResourceReservationView<'_>,
    bytes: usize,
) -> Result<ResourceClaim<'view>, NativeProbeError> {
    let amount = u64::try_from(bytes)
        .map_err(|_| NativeProbeError::Resource(ResourceError::ArithmeticOverflow))?;
    view.claim_resources(&[ResourceRequest::new(
        ResourceKind::Ram,
        ResourceAmount::checked(amount).map_err(NativeProbeError::Resource)?,
    )])
    .map_err(NativeProbeError::Resource)
}

/// Run one native status query using only a native-checked incumbent and the
/// same actual lease pointer. No new model, gap, full loop or unbounded proof.
/// Clone accounting uses the admitted AST node cap and conservative structural
/// headers; nested standard Clone allocation/OOM, allocator headers and RSS
/// remain unverified. No unsafe allocator workaround is introduced.
#[allow(clippy::too_many_arguments)]
pub fn execute_native_probe<'incumbent, 'query, 'view>(
    registry: &SolverRegistry,
    incumbent: &'incumbent NativeCheckedModel<'query, '_>,
    objective: IntSymbolId,
    direction: ObjectiveDirection,
    budget: &ExecutionBudget,
    reservation: &'view ResourceReservationView<'_>,
    working_directory: &Path,
) -> Result<NativeProbeOutcome<'incumbent, 'query, 'view>, NativeProbeError> {
    incumbent
        .check_reservation(reservation)
        .map_err(NativeProbeError::Resource)?;
    let context = incumbent.context();
    let source = context.source().digest();
    if source != reservation.source_revision() {
        return Err(NativeProbeError::SourceMismatch);
    }
    let limits = incumbent.model().query().limits();
    let per_node = std::mem::size_of::<BoolExpr>()
        + std::mem::size_of::<IntExpr>()
        + 4 * std::mem::size_of::<Vec<usize>>();
    let clone_bytes = limits
        .max_nodes()
        .checked_add(3)
        .and_then(|n| n.checked_mul(per_node))
        .and_then(|n| n.checked_mul(2))
        .and_then(|n| n.checked_add(4096))
        .ok_or(NativeProbeError::Resource(
            ResourceError::ArithmeticOverflow,
        ))?;
    let clone_claim = claim(reservation, clone_bytes)?;
    let emission_bytes =
        limits
            .max_output_bytes()
            .checked_mul(2)
            .ok_or(NativeProbeError::Resource(
                ResourceError::ArithmeticOverflow,
            ))?;
    let emission_claim = claim(reservation, emission_bytes)?;
    let store_claim = claim(reservation, std::mem::size_of::<StoredSolverResult>())?;
    let mut store =
        ResultStore::with_capacity(1).map_err(NativeProbeError::StoreBeforeExecution)?;
    store
        .try_reserve_backing()
        .map_err(NativeProbeError::StoreBeforeExecution)?;
    let probe = OptimizationProbe::checked(incumbent.model(), objective, direction)
        .map_err(NativeProbeError::Probe)?;
    let prepared = probe
        .better_query()
        .emit(crate::runner::QuerySourceIdentity::from_bytes(
            *source.as_bytes(),
        ))
        .map_err(NativeProbeError::Query)?;
    let query = prepared.solver_query().map_err(NativeProbeError::Query)?;
    let receipt = execute_supervised_into_store(
        registry,
        &query,
        budget,
        reservation,
        working_directory,
        &mut store,
    )
    .map_err(NativeProbeError::Adapter)?;
    Ok(NativeProbeOutcome {
        context,
        probe,
        prepared,
        report: receipt.report(),
        insertion: receipt.insertion(),
        store,
        _clone_claim: clone_claim,
        _emission_claim: emission_claim,
        _store_claim: store_claim,
    })
}
