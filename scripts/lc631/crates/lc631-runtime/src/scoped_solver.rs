//! In-memory abstract SAT execution and scoped result ownership.
//!
//! Runner-reported observations only: no semantic validation, proof,
//! authenticated executable identity, durable replay, or authority.

use std::path::Path;

use lc631_core::context::{ContextRevision, ContextScope, ContextState};
use lc631_solvers::{
    registry::SolverRegistry,
    result_store::{ResultStore, ResultStoreError, ResultStoreInsert, StoredSolverResult},
    runner::{
        ExecutionBudget, InvocationDigest, QuerySourceIdentity, SolverExecutionReport, SolverQuery,
    },
    supervisor_adapter::{execute_supervised, SupervisorAdapterError},
    typed_query::QueryEmitError,
};
use lc631_substrate::resource::{
    ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
    ResourceReservationView,
};

use crate::solver_lowering::PreparedAbstractSatisfiabilityQuery;

mod current;
pub use current::ScopedCurrentLookupError;

#[derive(Debug)]
struct ScopedEntry<'prepared, 'task> {
    prepared: &'prepared PreparedAbstractSatisfiabilityQuery<'task>,
    context: ContextRevision,
    scope: ContextScope,
    invocation: Option<InvocationDigest>,
    insertion: ResultStoreInsert,
}

/// The store and sidecar are private and owned together. Each sidecar borrows
/// the actual immutable prepared object, so its whole projection stays alive.
///
/// Detached queries and reports have no scoped insertion method:
/// ```compile_fail
/// use lc631_runtime::scoped_solver::ScopedSolverStore;
/// use lc631_solvers::runner::{SolverExecutionReport, SolverQuery};
/// fn attach(store: &mut ScopedSolverStore<'_, '_>, query: &SolverQuery<'_>, report: SolverExecutionReport) {
///     let _ = store.append_for_query(query, report);
/// }
/// ```
/// The unaccounted fixture constructor is unavailable to production callers:
/// ```compile_fail
/// use lc631_runtime::scoped_solver::ScopedSolverStore;
/// let _ = ScopedSolverStore::with_capacity(1);
/// ```
/// The owned raw store is private:
/// ```compile_fail
/// use lc631_runtime::scoped_solver::ScopedSolverStore;
/// fn raw(store: &mut ScopedSolverStore<'_, '_>) { let _ = &mut store.store; }
/// ```
/// Report injection exists only in unit-test builds and is not a public API:
/// ```compile_fail
/// use lc631_core::context::ContextState;
/// use lc631_runtime::{scoped_solver::ScopedSolverStore, solver_lowering::PreparedAbstractSatisfiabilityQuery};
/// fn inject<'a, 't>(store: &mut ScopedSolverStore<'a, 't>, prepared: &'a PreparedAbstractSatisfiabilityQuery<'t>, context: &ContextState) {
///     let _ = store.execute_for_test(prepared, context, Some(context.scope()), |_| unreachable!());
/// }
/// ```
pub struct ScopedSolverStore<'prepared, 'task> {
    store: ResultStore,
    entries: Vec<ScopedEntry<'prepared, 'task>>,
    resource_source: Option<QuerySourceIdentity>,
    // Fields drop in declaration order: both owned Vecs drop before this claim.
    metadata_claim: Option<ResourceClaim<'prepared>>,
}

impl std::fmt::Debug for ScopedSolverStore<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScopedSolverStore")
            .field("len", &self.entries.len())
            .field("capacity", &self.store.capacity_limit())
            .field("metadata_claim_retained", &self.metadata_claim.is_some())
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScopedStoreInitError {
    Store(ResultStoreError),
    Resource(ResourceError),
    ArithmeticOverflow,
    SidecarAllocation,
}

// Keep the child's sole report inline on storage failure: allocating a Box on
// this error path could fail and discard the observation. The fixed-size
// report is intentionally not replaced with a success flag or missing payload.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScopedRunError {
    Resource(ResourceError),
    ContextMismatch,
    MissingGoal,
    GoalScopeMismatch,
    QueryEmission(QueryEmitError),
    SidecarAllocation,
    StoreBeforeExecution(ResultStoreError),
    Supervisor(SupervisorAdapterError),
    /// The child may have run; retain its exact report even on storage failure.
    StoreAfterExecution {
        error: ResultStoreError,
        report: SolverExecutionReport,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScopedLookupError {
    ContextMismatch,
    MissingGoal,
    GoalScopeMismatch,
    QueryEmission(QueryEmitError),
    NotFound,
    Ambiguous,
    InternalStoreMismatch,
}

#[derive(Clone, Copy, Debug)]
pub struct ScopedRunReceipt {
    report: SolverExecutionReport,
    insertion: ResultStoreInsert,
}

impl ScopedRunReceipt {
    pub const fn report(self) -> SolverExecutionReport {
        self.report
    }
    pub const fn insertion(self) -> ResultStoreInsert {
        self.insertion
    }
}

impl<'prepared, 'task> ScopedSolverStore<'prepared, 'task> {
    /// Account for the fixed backing arrays before allocating or executing.
    /// The charge excludes allocator headers, object stack bytes, and RSS.
    pub fn with_resources(
        capacity: usize,
        reservation: &'prepared ResourceReservationView<'_>,
    ) -> Result<Self, ScopedStoreInitError> {
        let _ = ResultStore::with_capacity(capacity).map_err(ScopedStoreInitError::Store)?;
        let per_entry = std::mem::size_of::<ScopedEntry<'prepared, 'task>>()
            .checked_add(std::mem::size_of::<StoredSolverResult>())
            .ok_or(ScopedStoreInitError::ArithmeticOverflow)?;
        let bytes = per_entry
            .checked_mul(capacity)
            .and_then(|n| u64::try_from(n).ok())
            .ok_or(ScopedStoreInitError::ArithmeticOverflow)?;
        let amount = ResourceAmount::checked(bytes).map_err(ScopedStoreInitError::Resource)?;
        let claim = reservation
            .claim_resources(&[ResourceRequest::new(ResourceKind::Ram, amount)])
            .map_err(ScopedStoreInitError::Resource)?;
        let mut store =
            ResultStore::with_capacity(capacity).map_err(ScopedStoreInitError::Store)?;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(capacity)
            .map_err(|_| ScopedStoreInitError::SidecarAllocation)?;
        if entries.capacity() > capacity {
            return Err(ScopedStoreInitError::SidecarAllocation);
        }
        store
            .try_reserve_backing()
            .map_err(ScopedStoreInitError::Store)?;
        Ok(Self {
            store,
            entries,
            resource_source: Some(QuerySourceIdentity::from_bytes(
                *reservation.source_revision().as_bytes(),
            )),
            metadata_claim: Some(claim),
        })
    }

    /// Unaccounted fixtures are confined to this crate's unit-test build.
    #[cfg(test)]
    pub(crate) fn with_capacity(capacity: usize) -> Result<Self, ResultStoreError> {
        Ok(Self {
            store: ResultStore::with_capacity(capacity)?,
            entries: Vec::new(),
            resource_source: None,
            metadata_claim: None,
        })
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Native execution only; there is no public detached-report insertion.
    // Keep the post-execution report inline: boxing on failure could itself
    // allocate and lose the only observation. Arguments are actual seams.
    #[allow(clippy::too_many_arguments, clippy::result_large_err)]
    pub fn execute(
        &mut self,
        prepared: &'prepared PreparedAbstractSatisfiabilityQuery<'task>,
        context: &ContextState,
        goal_scope: Option<ContextScope>,
        registry: &SolverRegistry,
        budget: &ExecutionBudget,
        reservation: &ResourceReservationView<'_>,
        working_directory: &Path,
    ) -> Result<ScopedRunReceipt, ScopedRunError> {
        reservation.check().map_err(ScopedRunError::Resource)?;
        if let Some(claim) = &self.metadata_claim {
            if !claim.same_reservation(reservation) {
                return Err(ScopedRunError::Resource(ResourceError::ForeignOwner));
            }
        }
        self.execute_with(prepared, context, goal_scope, |query| {
            execute_supervised(registry, query, budget, reservation, working_directory)
        })
    }

    #[allow(clippy::result_large_err)]
    fn execute_with<F>(
        &mut self,
        prepared: &'prepared PreparedAbstractSatisfiabilityQuery<'task>,
        context: &ContextState,
        goal_scope: Option<ContextScope>,
        run: F,
    ) -> Result<ScopedRunReceipt, ScopedRunError>
    where
        F: FnOnce(&SolverQuery<'_>) -> Result<SolverExecutionReport, SupervisorAdapterError>,
    {
        if let Some(claim) = &self.metadata_claim {
            claim.check().map_err(ScopedRunError::Resource)?;
            if self.resource_source != Some(prepared.binding().source_identity()) {
                return Err(ScopedRunError::Resource(ResourceError::SourceMismatch));
            }
        }
        if context.revision() != prepared.binding().context_revision() {
            return Err(ScopedRunError::ContextMismatch);
        }
        let goal_scope = goal_scope.ok_or(ScopedRunError::MissingGoal)?;
        if goal_scope != context.scope() {
            return Err(ScopedRunError::GoalScopeMismatch);
        }
        let query = prepared
            .scoped_solver_query()
            .map_err(ScopedRunError::QueryEmission)?;
        self.store
            .try_reserve_append()
            .map_err(ScopedRunError::StoreBeforeExecution)?;
        self.entries
            .try_reserve(1)
            .map_err(|_| ScopedRunError::SidecarAllocation)?;
        let report = run(&query).map_err(ScopedRunError::Supervisor)?;
        let insertion = self
            .store
            .append_for_query(&query, report)
            .map_err(|error| ScopedRunError::StoreAfterExecution { error, report })?;
        self.entries.push(ScopedEntry {
            prepared,
            context: context.revision(),
            scope: context.scope(),
            invocation: report.invocation_digest(),
            insertion,
        });
        Ok(ScopedRunReceipt { report, insertion })
    }

    /// Test-only injection; the production API always calls the native adapter.
    #[cfg(test)]
    #[allow(clippy::result_large_err)]
    pub(crate) fn execute_for_test<F>(
        &mut self,
        prepared: &'prepared PreparedAbstractSatisfiabilityQuery<'task>,
        context: &ContextState,
        goal_scope: Option<ContextScope>,
        run: F,
    ) -> Result<ScopedRunReceipt, ScopedRunError>
    where
        F: FnOnce(&SolverQuery<'_>) -> Result<SolverExecutionReport, SupervisorAdapterError>,
    {
        self.execute_with(prepared, context, goal_scope, run)
    }

    /// The same immutable prepared object retains query payload,
    /// interpretation, bindings, atoms, guards and residual provenance.
    /// Invocation identifies a declared profile, not a unique execution.
    /// Historical reads do not recheck the original lease deadline/cancellation.
    pub fn lookup(
        &self,
        prepared: &PreparedAbstractSatisfiabilityQuery<'_>,
        context: &ContextState,
        goal_scope: Option<ContextScope>,
        invocation: Option<InvocationDigest>,
    ) -> Result<&StoredSolverResult, ScopedLookupError> {
        if context.revision() != prepared.binding().context_revision() {
            return Err(ScopedLookupError::ContextMismatch);
        }
        let goal_scope = goal_scope.ok_or(ScopedLookupError::MissingGoal)?;
        if goal_scope != context.scope() {
            return Err(ScopedLookupError::GoalScopeMismatch);
        }
        let query = prepared
            .scoped_solver_query()
            .map_err(ScopedLookupError::QueryEmission)?;
        let mut matching = self.entries.iter().filter(|entry| {
            entry.context == context.revision()
                && entry.scope == context.scope()
                && entry.invocation == invocation
                && std::ptr::eq(entry.prepared, prepared)
        });
        let entry = matching.next().ok_or(ScopedLookupError::NotFound)?;
        if matching.next().is_some() {
            return Err(ScopedLookupError::Ambiguous);
        }
        let stored = self
            .store
            .lookup_by_id(entry.insertion.id())
            .ok_or(ScopedLookupError::InternalStoreMismatch)?;
        let binding = stored.binding();
        if binding.source() != query.source()
            || binding.query() != query.digest()
            || binding.invocation() != invocation
        {
            return Err(ScopedLookupError::InternalStoreMismatch);
        }
        Ok(stored)
    }

    /// Inspect a particular matching occurrence in this owner, in append
    /// order. The ordinal is local to this store and this exact lineage; it
    /// is not a cross-store ID, durable handle, or invocation identity.
    pub fn lookup_occurrence(
        &self,
        prepared: &PreparedAbstractSatisfiabilityQuery<'_>,
        context: &ContextState,
        goal_scope: Option<ContextScope>,
        invocation: Option<InvocationDigest>,
        occurrence: usize,
    ) -> Result<&StoredSolverResult, ScopedLookupError> {
        if context.revision() != prepared.binding().context_revision() {
            return Err(ScopedLookupError::ContextMismatch);
        }
        let goal_scope = goal_scope.ok_or(ScopedLookupError::MissingGoal)?;
        if goal_scope != context.scope() {
            return Err(ScopedLookupError::GoalScopeMismatch);
        }
        let query = prepared
            .scoped_solver_query()
            .map_err(ScopedLookupError::QueryEmission)?;
        let entry = self
            .entries
            .iter()
            .filter(|entry| {
                entry.context == context.revision()
                    && entry.scope == context.scope()
                    && entry.invocation == invocation
                    && std::ptr::eq(entry.prepared, prepared)
            })
            .nth(occurrence)
            .ok_or(ScopedLookupError::NotFound)?;
        let stored = self
            .store
            .lookup_by_id(entry.insertion.id())
            .ok_or(ScopedLookupError::InternalStoreMismatch)?;
        let binding = stored.binding();
        if binding.source() != query.source()
            || binding.query() != query.digest()
            || binding.invocation() != invocation
        {
            return Err(ScopedLookupError::InternalStoreMismatch);
        }
        Ok(stored)
    }
}
