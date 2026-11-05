//! 実有限modal評価の所有とledger消費。source真理・権限・完成の証拠ではない。
//! Actual finite modal evaluation ownership and ledger consumption, not source truth.
#![forbid(unsafe_code)]

use crate::facet_ledger::{
    FacetLedger, FiniteObservationBinding, LedgerError, PropertyId, Revision, TransformId,
};
use lc631_core::context::{ContextRevision, ContextScope, SourceRevision};
use lc631_solvers::finite_modal::{
    evaluate_with_lineage, CallerLineage, Error, Formula, Model, NativeObservation,
};
use lc631_substrate::resource::{
    ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
    ResourceReservationView,
};
use lc631_substrate::Digest32;
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_MODAL_OWNER: AtomicU64 = AtomicU64::new(1);
const MAX_RESULTS: usize = 4096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModalError {
    InvalidCapacity,
    CapacityReached,
    AllocationFailed,
    IdentityExhausted,
    ArithmeticOverflow,
    SourceMismatch,
    ContextMismatch,
    ScopeMismatch,
    ForeignObservation,
    NotFound,
    Evaluation(Error),
    Resource(ResourceError),
    Ledger(LedgerError),
}

/// Typed model/formula inputs, never a caller-reported evaluation status.
#[derive(Clone, Copy)]
pub struct ModalRequest<'a> {
    pub source: &'a str,
    pub context: ContextRevision,
    pub scope: ContextScope,
    pub model: Model<'a>,
    pub nodes: &'a [Formula],
    pub root: u16,
}

/// Local occurrence only. Private fields prevent constructing an injected ID.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModalObservationId {
    owner: u64,
    index: usize,
}

struct Entry {
    scope: ContextScope,
    report: NativeObservation,
    result_digest: [u8; 32],
}

/// Entries own reports privately constructed by the actual finite evaluator.
/// Immutable caller input backing is borrowed only during evaluation; retained
/// digests are content identities, not independently checked replay evidence.
///
/// ```compile_fail
/// use lc631_runtime::finite_modal_observation::ModalObservationStore;
/// use lc631_solvers::finite_modal::NativeObservation;
/// fn inject(store: &mut ModalObservationStore<'_>, report: NativeObservation) {
///     store.append(report);
/// }
/// ```
pub struct ModalObservationStore<'view> {
    entries: Vec<Entry>,
    capacity: usize,
    owner: u64,
    source: Digest32,
    metadata: ResourceClaim<'view>,
}

impl<'view> ModalObservationStore<'view> {
    pub fn with_resources(
        capacity: usize,
        resources: &'view ResourceReservationView<'_>,
    ) -> Result<Self, ModalError> {
        if capacity == 0 || capacity > MAX_RESULTS {
            return Err(ModalError::InvalidCapacity);
        }
        let bytes = capacity
            .checked_mul(std::mem::size_of::<Entry>())
            .ok_or(ModalError::ArithmeticOverflow)?;
        let metadata = claim(resources, bytes)?;
        let owner = NEXT_MODAL_OWNER
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .map_err(|_| ModalError::IdentityExhausted)?;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(capacity)
            .map_err(|_| ModalError::AllocationFailed)?;
        if entries.capacity() > capacity {
            return Err(ModalError::AllocationFailed);
        }
        Ok(Self {
            entries,
            capacity,
            owner,
            source: resources.source_revision(),
            metadata,
        })
    }
    pub(crate) fn check_current(
        &self,
        resources: &ResourceReservationView<'_>,
    ) -> Result<(), ModalError> {
        resources.check().map_err(ModalError::Resource)?;
        self.metadata.check().map_err(ModalError::Resource)?;
        if !self.metadata.same_reservation(resources) {
            return Err(ModalError::Resource(ResourceError::ForeignOwner));
        }
        if self.source != resources.source_revision() {
            return Err(ModalError::SourceMismatch);
        }
        Ok(())
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn evaluate(
        &mut self,
        request: ModalRequest<'_>,
        resources: &ResourceReservationView<'_>,
    ) -> Result<ModalObservationId, ModalError> {
        resources.check().map_err(ModalError::Resource)?;
        self.metadata.check().map_err(ModalError::Resource)?;
        if !self.metadata.same_reservation(resources) {
            return Err(ModalError::Resource(ResourceError::ForeignOwner));
        }
        if request.context.source().digest() != self.source
            || SourceRevision::of(request.source) != request.context.source()
        {
            return Err(ModalError::SourceMismatch);
        }
        if self.entries.len() == self.capacity {
            return Err(ModalError::CapacityReached);
        }
        if request.nodes.is_empty() || request.nodes.len() > 256 {
            return Err(ModalError::Evaluation(Error::LimitExceeded));
        }
        // The actual evaluator uses one u64 per formula node, not a world×node clone.
        let bytes = request
            .nodes
            .len()
            .checked_mul(std::mem::size_of::<u64>())
            .ok_or(ModalError::ArithmeticOverflow)?;
        let work = claim(resources, bytes)?;
        let query = typed_query_digest(&request);
        let report = evaluate_with_lineage(
            CallerLineage {
                context: request.context,
                query_revision: query,
            },
            request.model,
            request.nodes,
            request.root,
        )
        .map_err(ModalError::Evaluation)?;
        work.check().map_err(ModalError::Resource)?;
        let id = ModalObservationId {
            owner: self.owner,
            index: self.entries.len(),
        };
        self.entries.push(Entry {
            scope: request.scope,
            result_digest: result_digest(report),
            report,
        });
        Ok(id)
    }

    /// Historical inspection; matching declared lineage grants no fresh authority.
    pub fn lookup(
        &self,
        id: ModalObservationId,
        context: ContextRevision,
        scope: ContextScope,
    ) -> Result<&NativeObservation, ModalError> {
        Ok(&self.entry(id, context, scope)?.report)
    }
    fn entry(
        &self,
        id: ModalObservationId,
        context: ContextRevision,
        scope: ContextScope,
    ) -> Result<&Entry, ModalError> {
        if id.owner != self.owner {
            return Err(ModalError::ForeignObservation);
        }
        let entry = self.entries.get(id.index).ok_or(ModalError::NotFound)?;
        if entry.report.context() != context {
            return Err(ModalError::ContextMismatch);
        }
        if entry.scope != scope {
            return Err(ModalError::ScopeMismatch);
        }
        Ok(entry)
    }
    pub fn binding(
        &self,
        id: ModalObservationId,
        context: ContextRevision,
        scope: ContextScope,
    ) -> Result<FiniteObservationBinding, ModalError> {
        let entry = self.entry(id, context, scope)?;
        Ok(FiniteObservationBinding {
            source: Revision(entry.report.source_digest()),
            context: Revision(entry.report.interpretation_digest()),
            query: Revision(entry.report.query_digest()),
            result: Revision(entry.result_digest),
        })
    }
    /// A finite model result alone leaves source/target preservation Unverified.
    /// No D/G, numerical value, proof, permit, or completion is generated here.
    pub fn retain(
        &self,
        request: ModalLedgerRequest,
        ledger: &mut FacetLedger,
    ) -> Result<(), ModalError> {
        self.metadata.check().map_err(ModalError::Resource)?;
        let binding = self.binding(request.observation, request.context, request.scope)?;
        let transform = ledger
            .transform(request.transform)
            .ok_or(ModalError::Ledger(LedgerError::UnknownTransform))?;
        if transform.scope.id.0 != *request.scope.comparison().as_bytes()
            || transform.scope.root_goal.0 != *request.scope.goal().as_bytes()
            || transform.scope.probes.0 != *request.scope.probes().as_bytes()
            || transform.scope.policy.0 != *request.scope.semantics().as_bytes()
        {
            return Err(ModalError::ScopeMismatch);
        }
        ledger
            .retain_finite_observation(
                request.transform,
                binding,
                request.property,
                request.required_evidence,
                request.next_action,
            )
            .map_err(ModalError::Ledger)
    }
}

pub struct ModalLedgerRequest {
    pub observation: ModalObservationId,
    pub context: ContextRevision,
    pub scope: ContextScope,
    pub transform: TransformId,
    pub property: PropertyId,
    pub required_evidence: Revision,
    pub next_action: Revision,
}

fn claim<'a>(
    resources: &'a ResourceReservationView<'_>,
    bytes: usize,
) -> Result<ResourceClaim<'a>, ModalError> {
    let bytes = u64::try_from(bytes).map_err(|_| ModalError::ArithmeticOverflow)?;
    let amount = ResourceAmount::checked(bytes).map_err(ModalError::Resource)?;
    resources
        .claim_resources(&[ResourceRequest::new(ResourceKind::Ram, amount)])
        .map_err(ModalError::Resource)
}

fn typed_query_digest(request: &ModalRequest<'_>) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"epistesys.runtime.modal-query.v1\0");
    hash.update(request.context.source().digest().as_bytes());
    hash.update(request.context.interpretation_digest().as_bytes());
    for pin in [
        request.scope.comparison(),
        request.scope.goal(),
        request.scope.probes(),
        request.scope.semantics(),
    ] {
        hash.update(pin.as_bytes());
    }
    hash.update(request.root.to_le_bytes());
    hash.update((request.nodes.len() as u16).to_le_bytes());
    for node in request.nodes {
        let (tag, args) = match *node {
            Formula::Bottom => (0, [0, 0]),
            Formula::Top => (1, [0, 0]),
            Formula::Atom(a) => (2, [u16::from(a), 0]),
            Formula::Not(a) => (3, [a, 0]),
            Formula::And(a, b) => (4, [a, b]),
            Formula::Or(a, b) => (5, [a, b]),
            Formula::Implies(a, b) => (6, [a, b]),
            Formula::Box(a) => (7, [a, 0]),
            Formula::Diamond(a) => (8, [a, 0]),
            Formula::Know(a) => (9, [a, 0]),
            Formula::Ought(a) => (10, [a, 0]),
        };
        hash.update([tag]);
        for argument in args {
            hash.update(argument.to_le_bytes());
        }
    }
    hash.finalize().into()
}

fn result_digest(report: NativeObservation) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"epistesys.runtime.modal-result.v1\0");
    hash.update(report.input_digest());
    hash.update(report.model_digest());
    hash.update(report.formula_digest());
    hash.update([report.finite_world_count()]);
    hash.update(report.evaluation().true_worlds().to_le_bytes());
    hash.update(report.evaluation().false_worlds().to_le_bytes());
    hash.finalize().into()
}
