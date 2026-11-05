//! 日本語: 実finite storeとworldの借用接続。source意味・権限は未検証。
//! English: Borrow actual finite-store results into worlds; source meaning and authority stay unverified.
#![forbid(unsafe_code)]

pub mod current_set;

use crate::finite_results::{FiniteResultError, FiniteResultId, FiniteResultStore};
use lc631_core::context::{ContextScope, ContextState};
use lc631_solvers::temporal::{
    FiniteTrace, TemporalEvaluation, TemporalNode, TemporalProfile, TemporalQuery, TimeScale, Truth,
};
use lc631_substrate::resource::{
    ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
    ResourceReservationView,
};
use lc631_world::materialized_worlds::{
    BoundWorldQuery, MaterializedError, MaterializedKey, MaterializedWorlds,
};

const PROFILE: &[u8] = b"epistesys.tlcx26.closed-finite-kleene.v1";
const MAX_PAYLOAD: usize = 128 * 1024;
// Magic, source, context, actual profile digest, root/node counts, clock,
// exact precision, closed-trace tag, canonical nanosecond tag, step/atom counts.
const HEADER_BYTES: usize = 8 + 32 + 32 + 32 + 2 + 2 + 97 + 8 + 1 + 1 + 2 + 2;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorldBridgeError {
    PayloadMismatch,
    ScopeMismatch,
    SourceMismatch,
    UnsupportedInput,
    PayloadBudget,
    AllocationFailed,
    EmptyCoverage,
    CoverageLimit,
    World(MaterializedError),
    Store(FiniteResultError),
    Resource(ResourceError),
}

/// Single canonical in-memory payload, not a persistence codec or receipt.
/// Caller-owned query/trace remain borrowed and cannot be replaced by hashes.
pub struct PreparedTemporalWorld<'input, 'view> {
    query: &'input TemporalQuery,
    trace: &'input FiniteTrace,
    payload: Vec<u8>,
    storage: ResourceClaim<'view>,
}

impl<'input, 'view> PreparedTemporalWorld<'input, 'view> {
    pub fn with_resources(
        query: &'input TemporalQuery,
        trace: &'input FiniteTrace,
        resources: &'view ResourceReservationView<'_>,
    ) -> Result<Self, WorldBridgeError> {
        resources.check().map_err(WorldBridgeError::Resource)?;
        if query.context().source().digest() != resources.source_revision() {
            return Err(WorldBridgeError::SourceMismatch);
        }
        check_input(query, trace)?;
        let bytes = payload_size(query, trace)?;
        let bytes_u64 = u64::try_from(bytes).map_err(|_| WorldBridgeError::PayloadBudget)?;
        let amount = ResourceAmount::checked(bytes_u64).map_err(WorldBridgeError::Resource)?;
        let storage = resources
            .claim_resources(&[ResourceRequest::new(ResourceKind::Ram, amount)])
            .map_err(WorldBridgeError::Resource)?;
        let mut payload = Vec::new();
        payload
            .try_reserve_exact(bytes)
            .map_err(|_| WorldBridgeError::AllocationFailed)?;
        if payload.capacity() > bytes {
            return Err(WorldBridgeError::PayloadBudget);
        }
        encode_payload(&mut payload, query, trace)?;
        if payload.len() != bytes {
            return Err(WorldBridgeError::PayloadBudget);
        }
        storage.check().map_err(WorldBridgeError::Resource)?;
        Ok(Self {
            query,
            trace,
            payload,
            storage,
        })
    }

    pub fn world_query(&self) -> BoundWorldQuery<'_> {
        BoundWorldQuery::reported(self.query.context(), PROFILE, &self.payload)
    }

    pub fn reserved_payload_bytes(&self) -> usize {
        self.payload.capacity()
    }

    fn check_current(
        &self,
        resources: &ResourceReservationView<'_>,
    ) -> Result<(), WorldBridgeError> {
        resources.check().map_err(WorldBridgeError::Resource)?;
        self.storage.check().map_err(WorldBridgeError::Resource)?;
        if self.query.context().source().digest() != resources.source_revision() {
            return Err(WorldBridgeError::SourceMismatch);
        }
        if !self.storage.same_reservation(resources) {
            return Err(WorldBridgeError::Resource(ResourceError::ForeignOwner));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceMeaning {
    NeedsEvidence,
}

/// Constructed only by exact world + store lookup, never detached status flags.
/// This borrows the live world and stored report: removal cannot race a view.
/// Historical evaluation view only: payload admission does not make the store's
/// evaluation lease current. Use the explicit current API for that check.
pub struct ObservedTemporalWorld<'a> {
    key: MaterializedKey,
    context: &'a ContextState,
    scope: ContextScope,
    query: &'a TemporalQuery,
    trace: &'a FiniteTrace,
    input: &'a [u8],
    report: &'a TemporalEvaluation,
}

impl ObservedTemporalWorld<'_> {
    pub const fn key(&self) -> MaterializedKey {
        self.key
    }
    pub const fn report(&self) -> &TemporalEvaluation {
        self.report
    }
    pub const fn context(&self) -> &ContextState {
        self.context
    }
    pub const fn source_meaning(&self) -> SourceMeaning {
        SourceMeaning::NeedsEvidence
    }
}

pub fn observe_temporal_world<'a>(
    worlds: &'a MaterializedWorlds<'_>,
    key: MaterializedKey,
    prepared: &'a PreparedTemporalWorld<'_, '_>,
    store: &'a FiniteResultStore<'_, '_>,
    id: FiniteResultId,
    scope: ContextScope,
) -> Result<ObservedTemporalWorld<'a>, WorldBridgeError> {
    // Fresh admission of this payload is separate from the historical evaluation.
    // This does not renew the store's old execution lease or issue authority.
    prepared
        .storage
        .check()
        .map_err(WorldBridgeError::Resource)?;
    let branch = worlds.get(key).map_err(WorldBridgeError::World)?;
    if branch.context().scope() != scope {
        return Err(WorldBridgeError::ScopeMismatch);
    }
    if branch.query() != prepared.world_query() {
        return Err(WorldBridgeError::PayloadMismatch);
    }
    let report = store
        .lookup_temporal(
            id,
            prepared.query,
            prepared.trace,
            branch.context().revision(),
            scope,
        )
        .map_err(WorldBridgeError::Store)?;
    Ok(ObservedTemporalWorld {
        key,
        context: branch.context(),
        scope,
        query: prepared.query,
        trace: prepared.trace,
        input: &prepared.payload,
        report,
    })
}

/// Current store/payload consumption with actual immutable backing retained.
/// MaterializedWorlds has no ResourceClaim: its key/context/payload are checked
/// structurally under an immutable borrow, not certified as a resource lease or
/// semantic/execution-coverage fact. No detached historical-view constructor.
///
/// ```compile_fail
/// use lc631_runtime::materialized_world_bridge::{
///     CurrentObservedTemporalWorld, ObservedTemporalWorld,
/// };
/// fn inject(old: ObservedTemporalWorld<'_>) {
///     CurrentObservedTemporalWorld::from_historical(old);
/// }
/// ```
pub struct CurrentObservedTemporalWorld<'a, 'input, 'view, 'world> {
    historical: ObservedTemporalWorld<'a>,
    worlds: &'a MaterializedWorlds<'world>,
    prepared: &'a PreparedTemporalWorld<'input, 'view>,
    store: &'a FiniteResultStore<'input, 'view>,
    id: FiniteResultId,
}

impl CurrentObservedTemporalWorld<'_, '_, '_, '_> {
    /// Explicit history remains readable after cancellation/expiry. It cannot
    /// be used to renew the store, mint a current wrapper or create authority.
    pub const fn historical_report(&self) -> &TemporalEvaluation {
        self.historical.report
    }

    /// Revalidate both original claims at each current consumption. Checks
    /// observe their current state only; cancellation may race after return.
    pub fn report_current(
        &self,
        resources: &ResourceReservationView<'_>,
    ) -> Result<&TemporalEvaluation, WorldBridgeError> {
        self.check_current(resources)?;
        Ok(self.historical.report)
    }

    pub const fn source_meaning(&self) -> SourceMeaning {
        SourceMeaning::NeedsEvidence
    }

    fn check_current(
        &self,
        resources: &ResourceReservationView<'_>,
    ) -> Result<(), WorldBridgeError> {
        self.prepared.check_current(resources)?;
        let branch = self
            .worlds
            .get(self.historical.key)
            .map_err(WorldBridgeError::World)?;
        if branch.context().scope() != self.historical.scope {
            return Err(WorldBridgeError::ScopeMismatch);
        }
        if branch.context().revision() != self.historical.context.revision()
            || branch.query() != self.prepared.world_query()
        {
            return Err(WorldBridgeError::PayloadMismatch);
        }
        let actual = self
            .store
            .lookup_temporal_current(
                self.id,
                self.prepared.query,
                self.prepared.trace,
                branch.context().revision(),
                self.historical.scope,
                resources,
            )
            .map_err(WorldBridgeError::Store)?;
        if !std::ptr::eq(actual, self.historical.report) {
            return Err(WorldBridgeError::PayloadMismatch);
        }
        self.prepared.check_current(resources)?;
        Ok(())
    }
}

/// Additive current port. Historical observe/lookup signatures and meanings
/// remain unchanged. Native store and prepared-payload ownership are checked
/// before historical exact binding and again before publishing this wrapper.
#[allow(clippy::too_many_arguments)] // Actual world, inputs, native result and lease are distinct boundaries.
pub fn observe_temporal_world_current<'a, 'input, 'view, 'world>(
    worlds: &'a MaterializedWorlds<'world>,
    key: MaterializedKey,
    prepared: &'a PreparedTemporalWorld<'input, 'view>,
    store: &'a FiniteResultStore<'input, 'view>,
    id: FiniteResultId,
    scope: ContextScope,
    resources: &ResourceReservationView<'_>,
) -> Result<CurrentObservedTemporalWorld<'a, 'input, 'view, 'world>, WorldBridgeError> {
    prepared.check_current(resources)?;
    store
        .lookup_temporal_current(
            id,
            prepared.query,
            prepared.trace,
            prepared.query.context(),
            scope,
            resources,
        )
        .map_err(WorldBridgeError::Store)?;
    let historical = observe_temporal_world(worlds, key, prepared, store, id, scope)?;
    let current = CurrentObservedTemporalWorld {
        historical,
        worlds,
        prepared,
        store,
        id,
    };
    current.check_current(resources)?;
    Ok(current)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogicalDistinctness {
    DifferentOnGivenTrace,
    SameOutcomeOnly,
    Unconfirmed,
    Incomparable,
}

/// A counterexample to equality on one declared trace is not a universal
/// equivalence test or independent confirmation of source interpretation.
pub fn compare_observed(
    a: &ObservedTemporalWorld<'_>,
    b: &ObservedTemporalWorld<'_>,
) -> LogicalDistinctness {
    if a.scope != b.scope
        || a.context.revision() != b.context.revision()
        || a.query.profile() != b.query.profile()
        || a.trace != b.trace
    {
        return LogicalDistinctness::Incomparable;
    }
    match (a.report.truth(), b.report.truth()) {
        (Truth::Unknown, _) | (_, Truth::Unknown) => LogicalDistinctness::Unconfirmed,
        (Truth::True, Truth::False) | (Truth::False, Truth::True) => {
            LogicalDistinctness::DifferentOnGivenTrace
        }
        (Truth::True, Truth::True) | (Truth::False, Truth::False) => {
            LogicalDistinctness::SameOutcomeOnly
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObservedWorldCoverage {
    pub executed_live: usize,
    pub unknown_outcomes: usize,
    pub source_meaning: SourceMeaning,
}

/// Count actual consumed evaluations once per complete input, never per opaque
/// world/receipt ID or duplicate store occurrence. No history is retained here.
pub fn observed_coverage(
    observations: &[ObservedTemporalWorld<'_>],
) -> Result<ObservedWorldCoverage, WorldBridgeError> {
    if observations.is_empty() {
        return Err(WorldBridgeError::EmptyCoverage);
    }
    if observations.len() > 2048 {
        return Err(WorldBridgeError::CoverageLimit);
    }
    let mut executed_live = 0;
    let mut unknown_outcomes = 0;
    for (index, observation) in observations.iter().enumerate() {
        let duplicate = observations[..index]
            .iter()
            .any(|prior| prior.scope == observation.scope && prior.input == observation.input);
        if !duplicate {
            executed_live += 1;
            if observation.report.truth() == Truth::Unknown {
                unknown_outcomes += 1;
            }
        }
    }
    Ok(ObservedWorldCoverage {
        executed_live,
        unknown_outcomes,
        source_meaning: SourceMeaning::NeedsEvidence,
    })
}

fn payload_size(query: &TemporalQuery, trace: &FiniteTrace) -> Result<usize, WorldBridgeError> {
    let nodes = query
        .nodes()
        .len()
        .checked_mul(5)
        .ok_or(WorldBridgeError::PayloadBudget)?;
    let steps = trace
        .atom_count()
        .checked_add(16)
        .and_then(|step| step.checked_mul(trace.steps().len()))
        .ok_or(WorldBridgeError::PayloadBudget)?;
    let total = HEADER_BYTES
        .checked_add(nodes)
        .and_then(|bytes| bytes.checked_add(steps))
        .ok_or(WorldBridgeError::PayloadBudget)?;
    if total > MAX_PAYLOAD {
        return Err(WorldBridgeError::PayloadBudget);
    }
    Ok(total)
}

fn check_input(query: &TemporalQuery, trace: &FiniteTrace) -> Result<(), WorldBridgeError> {
    match query.profile() {
        TemporalProfile::ClosedFiniteKleeneV1 => {}
        TemporalProfile::InfiniteLtl => return Err(WorldBridgeError::UnsupportedInput),
    }
    if query.nodes().is_empty()
        || query.nodes().len() > 1024
        || trace.steps().is_empty()
        || trace.steps().len() > 256
        || trace.atom_count() > 256
    {
        return Err(WorldBridgeError::UnsupportedInput);
    }
    if trace
        .steps()
        .iter()
        .any(|step| step.instant().clock().precision_nanoseconds() != 0)
    {
        return Err(WorldBridgeError::UnsupportedInput);
    }
    // FiniteTrace::checked is the only constructor: it rejects OpenPrefix and
    // LoopTo and validates same clock, atom width and strictly increasing times.
    // Instant::nanoseconds() is the canonical exact integer unit, not raw input units.
    Ok(())
}

fn encode_payload(
    out: &mut Vec<u8>,
    query: &TemporalQuery,
    trace: &FiniteTrace,
) -> Result<(), WorldBridgeError> {
    let node_count =
        u16::try_from(query.nodes().len()).map_err(|_| WorldBridgeError::PayloadBudget)?;
    let step_count =
        u16::try_from(trace.steps().len()).map_err(|_| WorldBridgeError::PayloadBudget)?;
    let atom_count =
        u16::try_from(trace.atom_count()).map_err(|_| WorldBridgeError::PayloadBudget)?;
    out.extend_from_slice(b"EPIW26T1");
    out.extend_from_slice(query.context().source().digest().as_bytes());
    out.extend_from_slice(query.context().interpretation_digest().as_bytes());
    out.extend_from_slice(query.profile().digest().as_bytes());
    out.extend_from_slice(&query.root().to_le_bytes());
    out.extend_from_slice(&node_count.to_le_bytes());
    for node in query.nodes() {
        encode_node(out, *node);
    }
    // FiniteTrace's checked nonempty/same-clock invariant is preserved.
    let clock = trace.steps()[0].instant().clock();
    out.extend_from_slice(clock.clock().as_bytes());
    out.push(match clock.scale() {
        TimeScale::Monotonic => 0,
        TimeScale::Utc => 1,
        TimeScale::Tai => 2,
    });
    out.extend_from_slice(clock.origin().as_bytes());
    out.extend_from_slice(clock.reference().as_bytes());
    out.extend_from_slice(&clock.precision_nanoseconds().to_le_bytes());
    out.extend_from_slice(&[0, 0]); // Closed extent, canonical exact nanoseconds.
    out.extend_from_slice(&step_count.to_le_bytes());
    out.extend_from_slice(&atom_count.to_le_bytes());
    for step in trace.steps() {
        out.extend_from_slice(&step.instant().nanoseconds().to_le_bytes());
        for truth in step.values() {
            out.push(match truth {
                Truth::False => 0,
                Truth::Unknown => 1,
                Truth::True => 2,
            });
        }
    }
    Ok(())
}

fn encode_node(out: &mut Vec<u8>, node: TemporalNode) {
    let (tag, a, b) = match node {
        TemporalNode::Constant(value) => (0, u16::from(value), 0),
        TemporalNode::Atom(atom) => (1, atom, 0),
        TemporalNode::Not(child) => (2, child, 0),
        TemporalNode::And(a, b) => (3, a, b),
        TemporalNode::Or(a, b) => (4, a, b),
        TemporalNode::StrongNext(child) => (5, child, 0),
        TemporalNode::Eventually(child) => (6, child, 0),
        TemporalNode::Always(child) => (7, child, 0),
        TemporalNode::Until { left, right } => (8, left, right),
    };
    out.push(tag);
    out.extend_from_slice(&a.to_le_bytes());
    out.extend_from_slice(&b.to_le_bytes());
}
