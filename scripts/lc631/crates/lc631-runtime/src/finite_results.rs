//! Resource-owned finite evaluations, never fabricated external-process reports.
//!
//! This initial port consumes the actual closed finite temporal evaluator. It
//! does not expose detached-report insertion, permission or completion tokens.
use lc631_core::context::{ContextRevision, ContextScope};
use lc631_solvers::temporal::{
    evaluate, EvaluationBudget, FiniteTrace, TemporalError, TemporalEvaluation, TemporalQuery,
    Truth,
};
use lc631_substrate::resource::{
    ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
    ResourceReservationView,
};
use lc631_substrate::Digest32;
use std::sync::atomic::{AtomicU64, Ordering};

const MAX_FINITE_RESULTS: usize = 4096;
static NEXT_STORE: AtomicU64 = AtomicU64::new(1);
mod ledger;
pub use ledger::{FiniteLedgerError, TemporalLedgerRequest};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FiniteResultError {
    InvalidCapacity,
    IdentityExhausted,
    CapacityReached,
    AllocationFailed,
    ArithmeticOverflow,
    ContextMismatch,
    BindingMismatch,
    NotFound,
    Evaluation(TemporalError),
    Resource(ResourceError),
}

/// Process-local store occurrence only, not a durable or authenticated receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FiniteResultId {
    owner: u64,
    index: usize,
}

struct TemporalEntry<'input> {
    query: &'input TemporalQuery,
    trace: &'input FiniteTrace,
    context: ContextRevision,
    scope: ContextScope,
    report: TemporalEvaluation,
}

/// Callers retain their immutable query/trace backing. This owner charges the
/// entry array and evaluation table, not caller inputs, allocator overhead or
/// RSS. Entries drop before the retained metadata claim.
///
/// ```compile_fail
/// use lc631_runtime::finite_results::FiniteResultStore;
/// use lc631_solvers::temporal::TemporalEvaluation;
/// fn inject(store: &mut FiniteResultStore<'_, '_>, report: TemporalEvaluation) {
///     store.append(report);
/// }
/// ```
pub struct FiniteResultStore<'input, 'view> {
    entries: Vec<TemporalEntry<'input>>,
    capacity: usize,
    owner: u64,
    source: Digest32,
    metadata: ResourceClaim<'view>,
}

impl<'input, 'view> FiniteResultStore<'input, 'view> {
    pub fn with_resources(
        capacity: usize,
        resources: &'view ResourceReservationView<'_>,
    ) -> Result<Self, FiniteResultError> {
        if capacity == 0 || capacity > MAX_FINITE_RESULTS {
            return Err(FiniteResultError::InvalidCapacity);
        }
        let bytes = std::mem::size_of::<TemporalEntry<'input>>()
            .checked_mul(capacity)
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(FiniteResultError::ArithmeticOverflow)?;
        let amount = ResourceAmount::checked(bytes).map_err(FiniteResultError::Resource)?;
        let metadata = resources
            .claim_resources(&[ResourceRequest::new(ResourceKind::Ram, amount)])
            .map_err(FiniteResultError::Resource)?;
        let owner = NEXT_STORE
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .map_err(|_| FiniteResultError::IdentityExhausted)?;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(capacity)
            .map_err(|_| FiniteResultError::AllocationFailed)?;
        if entries.capacity() > capacity {
            return Err(FiniteResultError::AllocationFailed);
        }
        Ok(Self {
            entries,
            capacity,
            owner,
            source: resources.source_revision(),
            metadata,
        })
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Source/context/scope are declared reasoning lineage, not authenticated
    /// semantics. Evaluation runs here; no external report can be injected.
    #[allow(clippy::too_many_arguments)]
    pub fn evaluate_temporal(
        &mut self,
        query: &'input TemporalQuery,
        trace: &'input FiniteTrace,
        context: ContextRevision,
        scope: ContextScope,
        budget: EvaluationBudget,
        resources: &ResourceReservationView<'_>,
    ) -> Result<FiniteResultId, FiniteResultError> {
        resources.check().map_err(FiniteResultError::Resource)?;
        self.metadata.check().map_err(FiniteResultError::Resource)?;
        if !self.metadata.same_reservation(resources) {
            return Err(FiniteResultError::Resource(ResourceError::ForeignOwner));
        }
        if context.source().digest() != self.source {
            return Err(FiniteResultError::Resource(ResourceError::SourceMismatch));
        }
        if context != query.context() {
            return Err(FiniteResultError::ContextMismatch);
        }
        if self.entries.len() == self.capacity {
            return Err(FiniteResultError::CapacityReached);
        }
        let work = claim_temporal_work(query, trace, resources)?;
        let report = evaluate(query, trace, budget).map_err(FiniteResultError::Evaluation)?;
        // Recheck cancellation/deadline before publication. This is not an
        // atomic revocation fence: cancellation may race after the check.
        // The evaluator has no external effect to roll back.
        work.check().map_err(FiniteResultError::Resource)?;
        let id = FiniteResultId {
            owner: self.owner,
            index: self.entries.len(),
        };
        self.entries.push(TemporalEntry {
            query,
            trace,
            context,
            scope,
            report,
        });
        Ok(id)
    }

    /// Historical inspection only: this does not reauthorize execution or
    /// revalidate an expired lease. Every query/input/context/scope is checked.
    pub fn lookup_temporal(
        &self,
        id: FiniteResultId,
        query: &TemporalQuery,
        trace: &FiniteTrace,
        context: ContextRevision,
        scope: ContextScope,
    ) -> Result<&TemporalEvaluation, FiniteResultError> {
        if id.owner != self.owner {
            return Err(FiniteResultError::BindingMismatch);
        }
        let entry = self
            .entries
            .get(id.index)
            .ok_or(FiniteResultError::NotFound)?;
        if context != query.context() || context != entry.context {
            return Err(FiniteResultError::ContextMismatch);
        }
        if scope != entry.scope
            || query.digest() != entry.query.digest()
            || trace.digest() != entry.trace.digest()
            || entry.report.query_digest() != query.digest()
            || entry.report.trace_digest() != trace.digest()
            || entry.report.profile_digest() != query.profile().digest()
        {
            return Err(FiniteResultError::BindingMismatch);
        }
        Ok(&entry.report)
    }

    /// Explicit time-of-call current consumption, distinct from historical
    /// inspection. Real lease-pointer identity, source and current metadata are
    /// checked before exact input lookup and again before returning the borrow.
    /// A returned immutable report can later be read historically; this is not
    /// an atomic revocation fence, renewed execution permit or authority token.
    #[allow(clippy::too_many_arguments)] // Keep exact model/context/scope and current lease explicit.
    pub fn lookup_temporal_current(
        &self,
        id: FiniteResultId,
        query: &TemporalQuery,
        trace: &FiniteTrace,
        context: ContextRevision,
        scope: ContextScope,
        resources: &ResourceReservationView<'_>,
    ) -> Result<&TemporalEvaluation, FiniteResultError> {
        self.check_current_input(query, context, resources)?;
        let report = self.lookup_temporal(id, query, trace, context, scope)?;
        self.check_current_input(query, context, resources)?;
        Ok(report)
    }

    fn check_current_input(
        &self,
        query: &TemporalQuery,
        context: ContextRevision,
        resources: &ResourceReservationView<'_>,
    ) -> Result<(), FiniteResultError> {
        resources.check().map_err(FiniteResultError::Resource)?;
        self.metadata.check().map_err(FiniteResultError::Resource)?;
        if !self.metadata.same_reservation(resources) {
            return Err(FiniteResultError::Resource(ResourceError::ForeignOwner));
        }
        if resources.source_revision() != self.source
            || context.source().digest() != self.source
            || query.context().source().digest() != self.source
        {
            return Err(FiniteResultError::Resource(ResourceError::SourceMismatch));
        }
        Ok(())
    }
}

fn claim_temporal_work<'view>(
    query: &TemporalQuery,
    trace: &FiniteTrace,
    resources: &'view ResourceReservationView<'_>,
) -> Result<ResourceClaim<'view>, FiniteResultError> {
    let bytes = query
        .nodes()
        .len()
        .checked_mul(trace.steps().len())
        .and_then(|cells| cells.checked_mul(std::mem::size_of::<Truth>()))
        .and_then(|bytes| u64::try_from(bytes).ok())
        .ok_or(FiniteResultError::ArithmeticOverflow)?;
    let amount = ResourceAmount::checked(bytes).map_err(FiniteResultError::Resource)?;
    resources
        .claim_resources(&[ResourceRequest::new(ResourceKind::Ram, amount)])
        .map_err(FiniteResultError::Resource)
}
