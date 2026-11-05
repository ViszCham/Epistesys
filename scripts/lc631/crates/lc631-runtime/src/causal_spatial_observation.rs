//! Actual finite causal/spatial observations -> the existing canonical U ledger.
//! The bounded ephemeral batch is not another ledger, permission issuer or proof store.
use crate::facet_ledger::{
    FacetLedger, FiniteObservationBinding, LedgerError, PropertyId, Revision, TransformId,
};
use lc631_core::context::{ContextRevision, ContextScope};
use lc631_solvers::causal_spatial::{
    self as kernel, Binding, CausalResult, Evaluation, FrameId, Profile, Query, Rectangle, Report,
    Scm, SpatialRelation, Termination, UnknownReason,
};
use lc631_substrate::resource::{
    ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
    ResourceReservationView,
};
use lc631_substrate::Digest32;
use sha2::{Digest, Sha256};

const MAX_REPORTS: usize = 256;

/// Inputs only; no supplied status, result or authorization flag is accepted.
pub enum Request<'model, 'input> {
    Causal {
        model: &'model Scm<'input>,
        query: Query,
        context: ContextRevision,
        scope: ContextScope,
    },
    Spatial {
        left: Option<Rectangle>,
        right: Option<Rectangle>,
        relation: SpatialRelation,
        frame: FrameId,
        context: ContextRevision,
        scope: ContextScope,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Input {
    Causal {
        model: [u8; 32],
        query: Query,
    },
    Spatial {
        left: Option<Rectangle>,
        right: Option<Rectangle>,
        relation: SpatialRelation,
        frame: FrameId,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Key {
    context: ContextRevision,
    scope: ContextScope,
    input: Input,
}

impl Request<'_, '_> {
    fn key(&self) -> Result<Key, ObservationError> {
        match self {
            Self::Causal {
                model,
                query,
                context,
                scope,
            } => Ok(Key {
                context: *context,
                scope: *scope,
                input: Input::Causal {
                    model: model.model_digest(),
                    query: *query,
                },
            }),
            Self::Spatial {
                left,
                right,
                relation,
                frame,
                context,
                scope,
            } => {
                for rect in [left, right].into_iter().flatten() {
                    if rect.frame() != *frame {
                        return Err(ObservationError::Kernel(kernel::Error::FrameMismatch));
                    }
                }
                Ok(Key {
                    context: *context,
                    scope: *scope,
                    input: Input::Spatial {
                        left: *left,
                        right: *right,
                        relation: *relation,
                        frame: *frame,
                    },
                })
            }
        }
    }

    fn evaluate(&self) -> Result<Payload, ObservationError> {
        match self {
            Self::Causal {
                model,
                query,
                context,
                ..
            } => model
                .evaluate(*context, *query)
                .map(Payload::Causal)
                .map_err(ObservationError::Kernel),
            Self::Spatial {
                left,
                right,
                relation,
                context,
                ..
            } => kernel::evaluate_spatial(*context, *left, *right, *relation)
                .map(Payload::Spatial)
                .map_err(ObservationError::Kernel),
        }
    }
}

/// Content-bound local observation ID, not a durable/authenticated receipt ID.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObservationId(Digest32);

enum Payload {
    Causal(Report<CausalResult>),
    Spatial(Report<bool>),
}

impl Payload {
    fn binding(&self) -> Binding {
        match self {
            Self::Causal(r) => r.binding(),
            Self::Spatial(r) => r.binding(),
        }
    }
    fn profile(&self) -> Profile {
        match self {
            Self::Causal(r) => r.profile(),
            Self::Spatial(r) => r.profile(),
        }
    }
    fn termination(&self) -> Termination {
        match self {
            Self::Causal(r) => r.termination(),
            Self::Spatial(r) => r.termination(),
        }
    }
}

pub struct Observation {
    id: ObservationId,
    key: Key,
    payload: Payload,
    binding: FiniteObservationBinding,
}

impl Observation {
    pub const fn id(&self) -> ObservationId {
        self.id
    }
    pub fn kernel_binding(&self) -> Binding {
        self.payload.binding()
    }
    pub const fn scope(&self) -> ContextScope {
        self.key.scope
    }
    pub const fn ledger_binding(&self) -> FiniteObservationBinding {
        self.binding
    }
    pub fn causal(&self) -> Option<&Report<CausalResult>> {
        match &self.payload {
            Payload::Causal(r) => Some(r),
            Payload::Spatial(_) => None,
        }
    }
    pub fn spatial(&self) -> Option<&Report<bool>> {
        match &self.payload {
            Payload::Spatial(r) => Some(r),
            Payload::Causal(_) => None,
        }
    }
    pub const fn claim_boundary(&self) -> &'static str {
        "declared finite model/frame observation, not external truth, preservation, independent evidence, permission, proof or completion"
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ObservationError {
    InvalidCapacity,
    CapacityReached,
    DuplicateObservation,
    AllocationFailed,
    ArithmeticOverflow,
    BindingMismatch,
    NotFound,
    Kernel(kernel::Error),
    Resource(ResourceError),
    Ledger(LedgerError),
}

#[derive(Clone, Copy)]
pub struct LedgerTarget {
    pub transform: TransformId,
    pub property: PropertyId,
    pub required_evidence: Revision,
    pub next_action: Revision,
}

/// Only actual evaluator calls can append. Entries drop before their RAM claim.
pub struct CausalSpatialObservations<'view> {
    entries: Vec<Observation>,
    capacity: usize,
    source: Digest32,
    metadata: ResourceClaim<'view>,
}

impl<'view> CausalSpatialObservations<'view> {
    pub fn with_resources(
        capacity: usize,
        resources: &'view ResourceReservationView<'_>,
    ) -> Result<Self, ObservationError> {
        if capacity == 0 || capacity > MAX_REPORTS {
            return Err(ObservationError::InvalidCapacity);
        }
        let bytes = std::mem::size_of::<Observation>()
            .checked_mul(capacity)
            .and_then(|n| u64::try_from(n).ok())
            .ok_or(ObservationError::ArithmeticOverflow)?;
        let metadata = resources
            .claim_resources(&[ResourceRequest::new(
                ResourceKind::Ram,
                ResourceAmount::checked(bytes).map_err(ObservationError::Resource)?,
            )])
            .map_err(ObservationError::Resource)?;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(capacity)
            .map_err(|_| ObservationError::AllocationFailed)?;
        if entries.capacity() > capacity {
            return Err(ObservationError::AllocationFailed);
        }
        Ok(Self {
            entries,
            capacity,
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

    fn current(&self, resources: &ResourceReservationView<'_>) -> Result<(), ObservationError> {
        resources.check().map_err(ObservationError::Resource)?;
        self.metadata.check().map_err(ObservationError::Resource)?;
        if !self.metadata.same_reservation(resources) {
            return Err(ObservationError::Resource(ResourceError::ForeignOwner));
        }
        Ok(())
    }

    pub fn evaluate(
        &mut self,
        request: &Request<'_, '_>,
        resources: &ResourceReservationView<'_>,
    ) -> Result<ObservationId, ObservationError> {
        self.current(resources)?;
        let key = request.key()?;
        if key.context.source().digest() != self.source {
            return Err(ObservationError::Resource(ResourceError::SourceMismatch));
        }
        if self.entries.len() == self.capacity {
            return Err(ObservationError::CapacityReached);
        }
        // The kernel uses bounded stack state (8 variables, 256 worlds), no heap.
        let workspace = resources
            .claim_resources(&[ResourceRequest::new(
                ResourceKind::Ram,
                ResourceAmount::checked(4096).map_err(ObservationError::Resource)?,
            )])
            .map_err(ObservationError::Resource)?;
        let payload = request.evaluate()?;
        let actual = payload.binding();
        if actual.context() != key.context {
            return Err(ObservationError::BindingMismatch);
        }
        if let Input::Causal { model, .. } = key.input {
            if model != actual.model() {
                return Err(ObservationError::BindingMismatch);
            }
        }
        let digest = result_digest(key, &payload);
        let id = ObservationId(Digest32::from_bytes(digest));
        if self.entries.iter().any(|entry| entry.id == id) {
            return Err(ObservationError::DuplicateObservation);
        }
        // Fresh publication check, not an atomic revocation fence. The finite
        // evaluator has no external action to roll back if cancellation races.
        workspace.check().map_err(ObservationError::Resource)?;
        let binding = FiniteObservationBinding {
            source: Revision(actual.source()),
            context: Revision(*key.context.interpretation_digest().as_bytes()),
            query: Revision(actual.query()),
            result: Revision(digest),
        };
        self.entries.push(Observation {
            id,
            key,
            payload,
            binding,
        });
        Ok(id)
    }

    /// Exact input lookup is historical; no cancellation/expiry is renewed here.
    pub fn lookup(
        &self,
        id: ObservationId,
        request: &Request<'_, '_>,
    ) -> Result<&Observation, ObservationError> {
        let key = request.key()?;
        let entry = self
            .entries
            .iter()
            .find(|entry| entry.id == id)
            .ok_or(ObservationError::NotFound)?;
        if entry.key != key {
            return Err(ObservationError::BindingMismatch);
        }
        Ok(entry)
    }

    /// Current U-only consumer. Caller separately owns/reserves the canonical ledger.
    pub fn retain_current(
        &self,
        id: ObservationId,
        request: &Request<'_, '_>,
        target: LedgerTarget,
        ledger: &mut FacetLedger,
        resources: &ResourceReservationView<'_>,
    ) -> Result<FiniteObservationBinding, ObservationError> {
        self.current(resources)?;
        let entry = self.lookup(id, request)?;
        let scope = ledger
            .transform(target.transform)
            .ok_or(ObservationError::Ledger(LedgerError::UnknownTransform))?
            .scope;
        let declared = entry.scope();
        if scope.id.0 != *declared.comparison().as_bytes()
            || scope.root_goal.0 != *declared.goal().as_bytes()
            || scope.probes.0 != *declared.probes().as_bytes()
            || scope.policy.0 != *declared.semantics().as_bytes()
        {
            return Err(ObservationError::Ledger(LedgerError::ScopeMismatch));
        }
        ledger
            .retain_finite_observation(
                target.transform,
                entry.binding,
                target.property,
                target.required_evidence,
                target.next_action,
            )
            .map_err(ObservationError::Ledger)?;
        Ok(entry.binding)
    }
}

fn result_digest(key: Key, payload: &Payload) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"epistesys.tlcx16.owned-observation.v1\0");
    let binding = payload.binding();
    for bytes in [
        binding.source(),
        *key.context.interpretation_digest().as_bytes(),
        binding.query(),
        binding.model(),
        *key.scope.comparison().as_bytes(),
        *key.scope.goal().as_bytes(),
        *key.scope.probes().as_bytes(),
        *key.scope.semantics().as_bytes(),
    ] {
        hash.update(bytes);
    }
    hash.update([match payload.profile() {
        Profile::BooleanAcyclicScmV1 => 0,
        Profile::ClosedIntegerRectangles2dV1 => 1,
    }]);
    hash.update([match payload.termination() {
        Termination::Completed => 0,
        Termination::NeedsEvidence => 1,
        Termination::ResourceExhausted => 2,
    }]);
    if let Input::Spatial { frame, .. } = key.input {
        hash.update(frame.0);
    }
    match payload {
        Payload::Causal(report) => match report.outcome() {
            Evaluation::Unknown(reason) => hash_unknown(&mut hash, reason),
            Evaluation::Computed(value) => {
                hash.update([0]);
                hash.update(value.true_worlds.to_le_bytes());
                hash.update(value.false_worlds.to_le_bytes());
                hash.update(value.work_used.to_le_bytes());
                for witness in [value.true_witness, value.false_witness] {
                    match witness {
                        None => hash.update([0]),
                        Some(w) => hash.update([1, w.exogenous, w.factual, w.action]),
                    }
                }
            }
        },
        Payload::Spatial(report) => match report.outcome() {
            Evaluation::Unknown(reason) => hash_unknown(&mut hash, reason),
            Evaluation::Computed(value) => hash.update([0, u8::from(value)]),
        },
    }
    hash.finalize().into()
}

fn hash_unknown(hash: &mut Sha256, reason: UnknownReason) {
    hash.update([1]);
    hash.update(match reason {
        UnknownReason::MissingEquation(variable) => [0, variable.0],
        UnknownReason::MissingWorlds => [1, 0],
        UnknownReason::NoCompatibleWorld => [2, 0],
        UnknownReason::WorkBudgetExhausted => [3, 0],
        UnknownReason::MissingSpatialObject => [4, 0],
    });
}
