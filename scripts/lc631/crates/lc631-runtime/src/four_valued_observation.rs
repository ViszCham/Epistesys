//! Actual finite signed-Horn evaluation with owned resources and lineage.
//! Neither/Both remain raw values, not source truth, losses, gains or authority.
//!
//! Detached results cannot be attached:
//! ```compile_fail
//! use lc631_runtime::four_valued_observation::FourValuedObservationStore;
//! use lc631_solvers::four_valued::Evaluation;
//! fn inject(store: &mut FourValuedObservationStore<'_, '_>, result: Evaluation) {
//!     store.append(result);
//! }
//! ```
use crate::facet_ledger::{
    FacetLedger, FiniteObservationBinding, LedgerError, PropertyId, Revision, TransformId,
};
use crate::resource::ResourceBroker;
use lc631_core::context::{ContextRevision, ContextScope};
use lc631_solvers::four_valued::{self as kernel, Atom, Evaluation, Literal, Program, Rule};
use lc631_substrate::{
    resource::{
        ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
        ResourceReservationView,
    },
    Digest32,
};
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicU64, Ordering};

const MAX_CAPACITY: usize = 4096;
const MAX_STEPS: u64 = 1_000_000;
static NEXT_OWNER: AtomicU64 = AtomicU64::new(1);

/// Caller-declared immutable input, not a report or a validated source claim.
#[derive(Clone, Copy)]
pub struct FourValuedRequest<'input> {
    pub atoms: usize,
    pub facts: &'input [Literal],
    pub rules: &'input [Rule<'input>],
    pub context: ContextRevision,
    pub scope: ContextScope,
    pub query: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FourValuedObservationId {
    owner: u64,
    index: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FourValuedError {
    InvalidCapacity,
    IdentityExhausted,
    CapacityReached,
    AllocationFailed,
    ArithmeticOverflow,
    BindingMismatch,
    NotFound,
    StepLimitTooLarge,
    Evaluation(kernel::Error),
    Resource(ResourceError),
    Ledger(LedgerError),
}
struct Entry<'input> {
    request: FourValuedRequest<'input>,
    input_digest: [u8; 32],
    result_digest: [u8; 32],
    report: Evaluation,
}

/// This ephemeral owner is not a second TL ledger. It borrows original inputs
/// and owns only actual evaluations. Its broker and same-lease claims precede
/// allocation; allocator overhead, caller inputs and RSS are not certified.
pub struct FourValuedObservationStore<'input, 'view> {
    entries: Vec<Entry<'input>>,
    capacity: usize,
    owner: u64,
    source: Digest32,
    broker: &'view ResourceBroker,
    metadata: ResourceClaim<'view>,
}
impl<'input, 'view> FourValuedObservationStore<'input, 'view> {
    pub fn with_resources(
        broker: &'view ResourceBroker,
        capacity: usize,
        view: &'view ResourceReservationView<'_>,
    ) -> Result<Self, FourValuedError> {
        if capacity == 0 || capacity > MAX_CAPACITY {
            return Err(FourValuedError::InvalidCapacity);
        }
        let source = view.source_revision();
        broker
            .validate_lease(view, view.run_id(), source, ResourceKind::Ram)
            .map_err(FourValuedError::Resource)?;
        let bytes = std::mem::size_of::<Entry<'input>>()
            .checked_mul(capacity)
            .ok_or(FourValuedError::ArithmeticOverflow)?;
        let metadata = claim(view, bytes)?;
        let owner = NEXT_OWNER
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .map_err(|_| FourValuedError::IdentityExhausted)?;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(capacity)
            .map_err(|_| FourValuedError::AllocationFailed)?;
        if entries.capacity() > capacity {
            return Err(FourValuedError::AllocationFailed);
        }
        Ok(Self {
            entries,
            capacity,
            owner,
            source,
            broker,
            metadata,
        })
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn active(&self, view: &ResourceReservationView<'_>) -> Result<(), FourValuedError> {
        if !self.metadata.same_reservation(view) {
            return Err(FourValuedError::Resource(ResourceError::ForeignOwner));
        }
        self.metadata.check().map_err(FourValuedError::Resource)?;
        self.broker
            .validate_lease(view, view.run_id(), self.source, ResourceKind::Ram)
            .map_err(FourValuedError::Resource)
    }

    /// Execute here, not through caller report insertion. Budget exhaustion
    /// publishes nothing. Cancellation is checked before and after bounded
    /// pure evaluation, not as an atomic revocation fence or per-step poll.
    pub fn evaluate(
        &mut self,
        request: FourValuedRequest<'input>,
        max_steps: u64,
        view: &ResourceReservationView<'_>,
    ) -> Result<FourValuedObservationId, FourValuedError> {
        self.active(view)?;
        if request.context.source().digest() != self.source {
            return Err(FourValuedError::Resource(ResourceError::SourceMismatch));
        }
        if self.entries.len() == self.capacity {
            return Err(FourValuedError::CapacityReached);
        }
        if max_steps > MAX_STEPS {
            return Err(FourValuedError::StepLimitTooLarge);
        }
        let work = claim(view, std::mem::size_of::<Evaluation>())?;
        let program = program(request)?;
        let input_digest = input_digest(request);
        let report = program
            .evaluate(max_steps)
            .map_err(FourValuedError::Evaluation)?;
        let result_digest = result_digest(&report, input_digest)?;
        work.check().map_err(FourValuedError::Resource)?;
        self.active(view)?;
        let id = FourValuedObservationId {
            owner: self.owner,
            index: self.entries.len(),
        };
        self.entries.push(Entry {
            request,
            input_digest,
            result_digest,
            report,
        });
        Ok(id)
    }

    fn entry(
        &self,
        id: FourValuedObservationId,
        request: FourValuedRequest<'_>,
    ) -> Result<&Entry<'input>, FourValuedError> {
        if id.owner != self.owner {
            return Err(FourValuedError::BindingMismatch);
        }
        let entry = self
            .entries
            .get(id.index)
            .ok_or(FourValuedError::NotFound)?;
        let _ = program(request)?;
        if entry.request.context != request.context
            || entry.request.scope != request.scope
            || entry.input_digest != input_digest(request)
        {
            return Err(FourValuedError::BindingMismatch);
        }
        Ok(entry)
    }
    /// Historical bytes/bits only: expiry or cancellation grants no new action.
    pub fn historical(
        &self,
        id: FourValuedObservationId,
        request: FourValuedRequest<'_>,
    ) -> Result<&Evaluation, FourValuedError> {
        Ok(&self.entry(id, request)?.report)
    }
    /// Active context-branch view; full payload/scope bindings remain mandatory.
    pub fn current(
        &self,
        id: FourValuedObservationId,
        request: FourValuedRequest<'_>,
        view: &ResourceReservationView<'_>,
    ) -> Result<&Evaluation, FourValuedError> {
        self.active(view)?;
        self.historical(id, request)
    }
    pub fn binding(
        &self,
        id: FourValuedObservationId,
        request: FourValuedRequest<'_>,
    ) -> Result<FiniteObservationBinding, FourValuedError> {
        let entry = self.entry(id, request)?;
        Ok(FiniteObservationBinding {
            source: Revision(*request.context.source().digest().as_bytes()),
            context: Revision(*request.context.interpretation_digest().as_bytes()),
            query: Revision(entry.input_digest),
            result: Revision(entry.result_digest),
        })
    }

    /// Consume only into the existing canonical ledger's Unverified U facet.
    /// Logical Both/Neither are never reinterpreted as source defects or gains.
    #[allow(clippy::too_many_arguments)]
    pub fn retain_unverified(
        &self,
        id: FourValuedObservationId,
        request: FourValuedRequest<'_>,
        view: &ResourceReservationView<'_>,
        ledger: &mut FacetLedger,
        transform: TransformId,
        property: PropertyId,
        required_evidence: Revision,
        next_action: Revision,
    ) -> Result<FiniteObservationBinding, FourValuedError> {
        self.active(view)?;
        let binding = self.binding(id, request)?;
        let declared = ledger
            .transform(transform)
            .ok_or(FourValuedError::Ledger(LedgerError::UnknownTransform))?;
        if declared.scope.id.0 != *request.scope.comparison().as_bytes()
            || declared.scope.root_goal.0 != *request.scope.goal().as_bytes()
            || declared.scope.probes.0 != *request.scope.probes().as_bytes()
            || declared.scope.policy.0 != *request.scope.semantics().as_bytes()
        {
            return Err(FourValuedError::Ledger(LedgerError::ScopeMismatch));
        }
        ledger
            .retain_finite_observation(transform, binding, property, required_evidence, next_action)
            .map_err(FourValuedError::Ledger)?;
        Ok(binding)
    }
}

fn claim<'view>(
    view: &'view ResourceReservationView<'_>,
    bytes: usize,
) -> Result<ResourceClaim<'view>, FourValuedError> {
    let bytes = u64::try_from(bytes).map_err(|_| FourValuedError::ArithmeticOverflow)?;
    let amount = ResourceAmount::checked(bytes).map_err(FourValuedError::Resource)?;
    view.claim_resources(&[ResourceRequest::new(ResourceKind::Ram, amount)])
        .map_err(FourValuedError::Resource)
}
fn program(request: FourValuedRequest<'_>) -> Result<Program<'_>, FourValuedError> {
    Program::new(
        request.atoms,
        request.facts,
        request.rules,
        request.context,
        request.query,
    )
    .map_err(FourValuedError::Evaluation)
}

/// Called only after Program validated MAX_ATOMS/RULES/BODY. All integer
/// counts fit fixed u64 wire words; syntax order and both literal signs bind.
fn input_digest(request: FourValuedRequest<'_>) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"epistesys.owned-four-valued.input.v1\0");
    hash.update(kernel::PROFILE_REVISION.as_bytes());
    for digest in [
        request.context.source().digest(),
        request.context.interpretation_digest(),
        request.scope.comparison(),
        request.scope.goal(),
        request.scope.probes(),
        request.scope.semantics(),
    ] {
        hash.update(digest.as_bytes());
    }
    hash.update(request.query);
    hash.update((request.atoms as u64).to_le_bytes());
    hash.update((request.facts.len() as u64).to_le_bytes());
    for literal in request.facts {
        hash.update([literal.atom().index(), u8::from(literal.is_negative())]);
    }
    hash.update((request.rules.len() as u64).to_le_bytes());
    for rule in request.rules {
        hash.update([rule.head.atom().index(), u8::from(rule.head.is_negative())]);
        hash.update((rule.body.len() as u64).to_le_bytes());
        for literal in rule.body {
            hash.update([literal.atom().index(), u8::from(literal.is_negative())]);
        }
    }
    hash.finalize().into()
}
fn result_digest(report: &Evaluation, input: [u8; 32]) -> Result<[u8; 32], FourValuedError> {
    let mut hash = Sha256::new();
    hash.update(b"epistesys.owned-four-valued.result.v1\0");
    hash.update(input);
    hash.update(report.binding().input_digest());
    hash.update(report.steps().to_le_bytes());
    for index in 0..report.atom_count() {
        let atom = Atom::new(index as u8).map_err(FourValuedError::Evaluation)?;
        let truth = report.truth(atom).map_err(FourValuedError::Evaluation)?;
        hash.update([u8::from(truth.support()), u8::from(truth.refutation())]);
        for literal in [Literal::positive(atom), Literal::negative(atom)] {
            match report
                .dependencies(literal)
                .map_err(FourValuedError::Evaluation)?
            {
                Some(bits) => {
                    hash.update([1]);
                    hash.update(bits.to_le_bytes());
                }
                None => {
                    hash.update([0]);
                }
            }
        }
    }
    Ok(hash.finalize().into())
}
