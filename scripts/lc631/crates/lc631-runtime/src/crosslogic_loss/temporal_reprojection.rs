//! 日本語: Given sourceからの同profile再投影案。旧query/resultは不変。
//! English: Same-profile Given-source reprojection proposal; old queries/results stay intact.
#![forbid(unsafe_code)]

use super::finite_temporal::{
    analyze_temporal_pair, FiniteBridgeDiagnostic, FiniteBridgeError, TemporalObservationRequest,
};
use super::{
    ComparisonBinding, PendingReprojection, PreparationError, ReprojectionCause,
    ReprojectionRequest,
};
use crate::finite_results::{FiniteResultError, FiniteResultId, FiniteResultStore};
use lc631_core::context::ContextConsumer;
use lc631_solvers::temporal::{EvaluationBudget, TemporalError, TemporalQuery, Truth};
use lc631_substrate::resource::{
    ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
    ResourceReservationView,
};
use lc631_substrate::Digest32;
use sha2::{Digest, Sha256};

/// Each actual owner and its current lease are supplied independently. Disjoint
/// caller dependency declarations do not establish checker independence.
#[derive(Clone, Copy)]
pub struct CurrentTemporalPair<'store, 'input, 'view, 'lease> {
    pub contexts: &'store ContextConsumer,
    pub left_store: &'store FiniteResultStore<'input, 'view>,
    pub left: TemporalObservationRequest<'input>,
    pub left_resources: &'store ResourceReservationView<'lease>,
    pub right_store: &'store FiniteResultStore<'input, 'view>,
    pub right: TemporalObservationRequest<'input>,
    pub right_resources: &'store ResourceReservationView<'lease>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReprojectionError {
    Preparation(PreparationError),
    Bridge(FiniteBridgeError),
    Store(FiniteResultError),
    Resource(ResourceError),
    Query(TemporalError),
    UnsupportedCause,
    PendingMismatch,
    PairMismatch,
    NoQueryChange,
    ForeignPreparationOwner,
    AllocationFailed,
    ArithmeticOverflow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TemporalInputBasis {
    Given,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeltaStatus {
    Proposed,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReprojectionPolicy {
    IdentityFromGivenSourceV1,
}
/// This is a structural query proposal, not an admitted ContextDelta, verified
/// correction, semantic bridge, defect/gain measurement or authority receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProposedQueryDelta {
    before: Digest32,
    after: Digest32,
}
impl ProposedQueryDelta {
    pub const fn status(self) -> DeltaStatus {
        DeltaStatus::Proposed
    }
    pub const fn before(self) -> Digest32 {
        self.before
    }
    pub const fn after(self) -> Digest32 {
        self.after
    }
}

/// Query backing drops before its retained RAM claim. Original immutable
/// requests/result IDs remain inspectable; there is no replacement AST input,
/// detached-report constructor, serialization promotion or context mutation.
pub struct PreparedTemporalReprojection<'input, 'view> {
    query: TemporalQuery,
    left: TemporalObservationRequest<'input>,
    right: TemporalObservationRequest<'input>,
    pending: PendingReprojection,
    original_truths: (Truth, Truth),
    next_binding: ComparisonBinding,
    delta: ProposedQueryDelta,
    retained: ResourceClaim<'view>,
}
impl<'input> PreparedTemporalReprojection<'input, '_> {
    pub fn query(&self) -> &TemporalQuery {
        &self.query
    }
    pub const fn original_request(&self) -> ReprojectionRequest {
        self.pending.request()
    }
    pub const fn original_truths(&self) -> (Truth, Truth) {
        self.original_truths
    }
    pub const fn original_left(&self) -> TemporalObservationRequest<'input> {
        self.left
    }
    pub const fn original_right(&self) -> TemporalObservationRequest<'input> {
        self.right
    }
    pub const fn delta(&self) -> ProposedQueryDelta {
        self.delta
    }
    pub const fn policy(&self) -> ReprojectionPolicy {
        ReprojectionPolicy::IdentityFromGivenSourceV1
    }
    pub const fn input_basis(&self) -> TemporalInputBasis {
        TemporalInputBasis::Given
    }
    pub const fn proposed_binding(&self) -> ComparisonBinding {
        self.next_binding
    }
}

/// Reconstruct one bounded query from the exact Given left query. This policy
/// does not infer which interpretation is correct. It only handles same-input,
/// same-profile disagreement; other causes remain unsupported/open.
pub fn prepare_identity_reprojection<'input, 'view>(
    pending: PendingReprojection,
    pair: &CurrentTemporalPair<'_, 'input, '_, '_>,
    resources: &'view ResourceReservationView<'_>,
) -> Result<PreparedTemporalReprojection<'input, 'view>, ReprojectionError> {
    let diagnostic = check_pair(pair, pending)?;
    resources.check().map_err(ReprojectionError::Resource)?;
    if resources.source_revision() != pair.left.binding.context.source().digest() {
        return Err(ReprojectionError::Resource(ResourceError::SourceMismatch));
    }
    let bytes = pair
        .left
        .query
        .nodes()
        .len()
        .checked_mul(std::mem::size_of::<lc631_solvers::temporal::TemporalNode>())
        .and_then(|n| n.checked_add(std::mem::size_of::<PreparedTemporalReprojection<'_, '_>>()))
        .and_then(|n| n.checked_add(512))
        .and_then(|n| u64::try_from(n).ok())
        .ok_or(ReprojectionError::ArithmeticOverflow)?;
    let retained = resources
        .claim_resources(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(bytes).map_err(ReprojectionError::Resource)?,
        )])
        .map_err(ReprojectionError::Resource)?;
    if !retained.same_reservation(pair.left_resources) {
        return Err(ReprojectionError::ForeignPreparationOwner);
    }
    let mut nodes = Vec::new();
    nodes
        .try_reserve_exact(pair.left.query.nodes().len())
        .map_err(|_| ReprojectionError::AllocationFailed)?;
    if nodes.capacity() > pair.left.query.nodes().len() {
        return Err(ReprojectionError::AllocationFailed);
    }
    nodes.extend_from_slice(pair.left.query.nodes());
    let query = TemporalQuery::checked(
        pair.right.binding.context,
        nodes,
        pair.left.query.root(),
        pair.left.query.profile(),
    )
    .map_err(ReprojectionError::Query)?;
    if query.digest() == pair.right.query.digest() {
        return Err(ReprojectionError::NoQueryChange);
    }
    let delta = ProposedQueryDelta {
        before: pair.right.query.digest(),
        after: query.digest(),
    };
    let mut next_binding = pair.right.binding;
    next_binding.translated_query = query.digest();
    next_binding.projection = projection_digest(pair, query.digest());
    retained.check().map_err(ReprojectionError::Resource)?;
    check_pair(pair, pending)?;
    Ok(PreparedTemporalReprojection {
        query,
        left: pair.left,
        right: pair.right,
        pending,
        original_truths: diagnostic.truths(),
        next_binding,
        delta,
        retained,
    })
}

/// A fresh actual result, borrowing its derived query. Only execute_prepared
/// constructs it; it contains no caller truth/status or completion flag.
pub struct ReprojectedTemporalResult<'query> {
    observation: FiniteResultId,
    query: &'query TemporalQuery,
    binding: ComparisonBinding,
}
impl ReprojectedTemporalResult<'_> {
    pub const fn observation(&self) -> FiniteResultId {
        self.observation
    }
    pub const fn binding(&self) -> ComparisonBinding {
        self.binding
    }
}

/// Execute after preparation, with the prepared owner outside the destination
/// store's query lifetime. The fresh store owns actual evaluations. Logical RAM
/// claims exclude allocator overhead/RSS; checks are not atomic revocation fences.
pub fn execute_prepared<'query, 'input: 'query>(
    prepared: &'query PreparedTemporalReprojection<'input, '_>,
    pair: &CurrentTemporalPair<'_, '_, '_, '_>,
    fresh: &mut FiniteResultStore<'query, '_>,
    budget: EvaluationBudget,
    resources: &ResourceReservationView<'_>,
) -> Result<ReprojectedTemporalResult<'query>, ReprojectionError> {
    check_prepared(prepared, pair)?;
    let observation = fresh
        .evaluate_temporal(
            &prepared.query,
            prepared.right.trace,
            prepared.next_binding.context,
            prepared.next_binding.scope,
            budget,
            resources,
        )
        .map_err(ReprojectionError::Store)?;
    // No rollback claim if a later current check fails: the actual fresh store
    // may contain a historical observation, but no current feedback token issues.
    check_prepared(prepared, pair)?;
    fresh
        .lookup_temporal_current(
            observation,
            &prepared.query,
            prepared.right.trace,
            prepared.next_binding.context,
            prepared.next_binding.scope,
            resources,
        )
        .map_err(ReprojectionError::Store)?;
    Ok(ReprojectedTemporalResult {
        observation,
        query: &prepared.query,
        binding: prepared.next_binding,
    })
}

/// Re-read old owners and fresh result under current leases before comparing.
/// A matching outcome remains SameFiniteOutcomeUnverified/shared evaluator.
pub fn consume_current<'store>(
    prepared: &PreparedTemporalReprojection<'_, '_>,
    pair: &CurrentTemporalPair<'store, '_, '_, '_>,
    result: &ReprojectedTemporalResult<'_>,
    fresh: &'store FiniteResultStore<'_, '_>,
    resources: &ResourceReservationView<'_>,
) -> Result<FiniteBridgeDiagnostic<'store>, ReprojectionError> {
    check_prepared(prepared, pair)?;
    if !std::ptr::eq(result.query, &prepared.query) || result.binding != prepared.next_binding {
        return Err(ReprojectionError::PairMismatch);
    }
    fresh
        .lookup_temporal_current(
            result.observation,
            result.query,
            prepared.right.trace,
            result.binding.context,
            result.binding.scope,
            resources,
        )
        .map_err(ReprojectionError::Store)?;
    let next = TemporalObservationRequest {
        observation: result.observation,
        query: result.query,
        trace: prepared.right.trace,
        binding: result.binding,
        dependencies: prepared.right.dependencies,
    };
    let diagnostic = analyze_temporal_pair(pair.left_store, pair.left, fresh, next)
        .map_err(ReprojectionError::Bridge)?;
    check_prepared(prepared, pair)?;
    fresh
        .lookup_temporal_current(
            result.observation,
            result.query,
            prepared.right.trace,
            result.binding.context,
            result.binding.scope,
            resources,
        )
        .map_err(ReprojectionError::Store)?;
    Ok(diagnostic)
}

fn check_prepared(
    prepared: &PreparedTemporalReprojection<'_, '_>,
    pair: &CurrentTemporalPair<'_, '_, '_, '_>,
) -> Result<(), ReprojectionError> {
    prepared.retained.check().map_err(ReprojectionError::Resource)?;
    if !prepared.retained.same_reservation(pair.left_resources) {
        return Err(ReprojectionError::ForeignPreparationOwner);
    }
    if !same_request(prepared.left, pair.left) || !same_request(prepared.right, pair.right) {
        return Err(ReprojectionError::PairMismatch);
    }
    check_pair(pair, prepared.pending)?;
    Ok(())
}
fn same_request(a: TemporalObservationRequest<'_>, b: TemporalObservationRequest<'_>) -> bool {
    a.observation == b.observation
        && a.binding == b.binding
        && std::ptr::eq(a.query, b.query)
        && std::ptr::eq(a.trace, b.trace)
        && a.dependencies == b.dependencies
}
fn check_pair<'store>(
    pair: &CurrentTemporalPair<'store, '_, '_, '_>,
    pending: PendingReprojection,
) -> Result<FiniteBridgeDiagnostic<'store>, ReprojectionError> {
    for (store, input, resources) in [
        (pair.left_store, pair.left, pair.left_resources),
        (pair.right_store, pair.right, pair.right_resources),
    ] {
        store
            .lookup_temporal_current(
                input.observation,
                input.query,
                input.trace,
                input.binding.context,
                input.binding.scope,
                resources,
            )
            .map_err(ReprojectionError::Store)?;
    }
    pending
        .request()
        .inspect_current(pair.contexts, pair.left.binding, pair.right.binding)
        .map_err(ReprojectionError::Preparation)?;
    let diagnostic = analyze_temporal_pair(pair.left_store, pair.left, pair.right_store, pair.right)
        .map_err(ReprojectionError::Bridge)?;
    let actual = diagnostic
        .reprojection()
        .map_err(ReprojectionError::Preparation)?;
    if actual != pending.request() {
        return Err(ReprojectionError::PendingMismatch);
    }
    if actual.cause() != ReprojectionCause::ReportedDisagreement {
        return Err(ReprojectionError::UnsupportedCause);
    }
    Ok(diagnostic)
}
fn projection_digest(pair: &CurrentTemporalPair<'_, '_, '_, '_>, query: Digest32) -> Digest32 {
    let mut hash = Sha256::new();
    hash.update(b"epistesys.temporal-reprojection.identity-from-given-source.v1\0");
    for digest in [
        pair.left.binding.root_query,
        pair.left.binding.projection,
        pair.right.binding.projection,
        pair.left.query.digest(),
        pair.right.query.digest(),
        query,
    ] {
        hash.update(digest.as_bytes());
    }
    Digest32::from_bytes(hash.finalize().into())
}
