//! 日本語: 実ContextGraphの明示有限前提を既存四値ownerと同じfacet ledgerへ接続する。
//! English: Actual graph premises feed the existing four-valued owner and same facet ledger.
//! Source correspondence remains unverified; this is neither authority nor normal CLI registration.
#![forbid(unsafe_code)]

use crate::four_valued_observation::{
    FourValuedError, FourValuedObservationId, FourValuedObservationStore, FourValuedRequest,
};
use crate::resource::ResourceBroker;
use lc631_core::context::{ContextRevision, ContextScope, ContextState};
use lc631_solvers::four_valued::{Atom, Evaluation, Literal, Program, Rule, Truth};
use lc631_substrate::resource::{
    ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
    ResourceReservationView,
};
use lc631_world::context_graph::ContextGraph;

mod ledger;
mod plan;
pub use ledger::RecordedBranch;
pub use plan::BranchPlan;

pub const PROFILE: &str = "explicit-context-signed-horn-v1";
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceMeaning {
    SourceUnverified,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BranchError {
    UnknownContext,
    UnknownParent,
    ForeignGraph,
    ScopeMismatch,
    QueryMismatch,
    InvalidAtomCount,
    UndeclaredAtom,
    UnsupportedSyntax,
    UnsupportedBasis,
    PremiseBudget,
    AllocationFailed,
    ArithmeticOverflow,
    Resource(ResourceError),
    Observation(FourValuedError),
    Ledger(crate::facet_ledger::LedgerError),
}

/// Borrowed rule views have their own caller-held lifetime: no self-reference,
/// leaked input, detached report, or second evaluator is required.
pub struct BranchInputs<'plan, 'graph, 'view> {
    plan: &'plan BranchPlan<'graph, 'view>,
    facts: Vec<Literal>,
    rules: Vec<Rule<'plan>>,
    held: ResourceClaim<'view>,
}
impl<'plan, 'graph, 'view> BranchInputs<'plan, 'graph, 'view> {
    pub fn prepare(
        plan: &'plan BranchPlan<'graph, 'view>,
        view: &'view ResourceReservationView<'_>,
    ) -> Result<Self, BranchError> {
        plan.check(view)?;
        let count = plan.parsed.len();
        let bytes = count
            .checked_mul(std::mem::size_of::<Literal>() + std::mem::size_of::<Rule<'_>>())
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Self>()))
            .ok_or(BranchError::ArithmeticOverflow)?;
        let held = claim(view, bytes)?;
        let mut facts = Vec::new();
        let mut rules = Vec::new();
        facts
            .try_reserve_exact(count)
            .map_err(|_| BranchError::AllocationFailed)?;
        rules
            .try_reserve_exact(count)
            .map_err(|_| BranchError::AllocationFailed)?;
        if facts.capacity() > count || rules.capacity() > count {
            return Err(BranchError::AllocationFailed);
        }
        for premise in &plan.parsed {
            match premise {
                plan::Parsed::Fact(fact) => facts.push(*fact),
                plan::Parsed::Rule { body, len, head } => rules.push(Rule {
                    body: &body[..usize::from(*len)],
                    head: *head,
                }),
            }
        }
        let result = Self {
            plan,
            facts,
            rules,
            held,
        };
        let request = result.request();
        Program::new(
            request.atoms,
            request.facts,
            request.rules,
            request.context,
            request.query,
        )
        .map_err(|e| BranchError::Observation(FourValuedError::Evaluation(e)))?;
        result.check(view)?;
        Ok(result)
    }
    fn request(&self) -> FourValuedRequest<'_> {
        FourValuedRequest {
            atoms: self.plan.atoms,
            facts: &self.facts,
            rules: &self.rules,
            context: self.plan.state.revision(),
            scope: self.plan.state.scope(),
            query: self.plan.query_digest,
        }
    }
    fn check(&self, view: &ResourceReservationView<'_>) -> Result<(), BranchError> {
        self.plan.check(view)?;
        check_claim(&self.held, view)
    }
}

/// Retains the actual store and its immutable input borrow. No public constructor
/// accepts Evaluation, a reported success, or arbitrary observation identity.
/// ```compile_fail
/// use lc631_runtime::four_valued_context::BranchObservation;
/// use lc631_solvers::four_valued::Evaluation;
/// fn promote(report: Evaluation) { BranchObservation::from_report(report); }
/// ```
pub struct BranchObservation<'input, 'plan, 'graph, 'view> {
    input: &'input BranchInputs<'plan, 'graph, 'view>,
    store: FourValuedObservationStore<'input, 'view>,
    id: FourValuedObservationId,
}
impl<'input, 'plan, 'graph, 'view> BranchObservation<'input, 'plan, 'graph, 'view> {
    pub fn execute(
        input: &'input BranchInputs<'plan, 'graph, 'view>,
        broker: &'view ResourceBroker,
        max_steps: u64,
        view: &'view ResourceReservationView<'_>,
    ) -> Result<Self, BranchError> {
        input.check(view)?;
        let mut store = FourValuedObservationStore::with_resources(broker, 1, view)
            .map_err(BranchError::Observation)?;
        let id = store
            .evaluate(input.request(), max_steps, view)
            .map_err(BranchError::Observation)?;
        input.check(view)?;
        Ok(Self { input, store, id })
    }
    pub const fn source_meaning(&self) -> SourceMeaning {
        SourceMeaning::SourceUnverified
    }
    pub fn actual_evaluations(&self) -> usize {
        self.store.len()
    }
    pub fn historical_report(&self) -> Result<&Evaluation, BranchError> {
        self.store
            .historical(self.id, self.input.request())
            .map_err(BranchError::Observation)
    }
    pub fn report_current(
        &self,
        graph: &ContextGraph,
        context: ContextRevision,
        query: Atom,
        view: &ResourceReservationView<'_>,
    ) -> Result<&Evaluation, BranchError> {
        self.input.check(view)?;
        if !std::ptr::eq(graph, self.input.plan.graph) {
            return Err(BranchError::ForeignGraph);
        }
        if graph.get(context).map(ContextState::revision) != Some(self.input.plan.state.revision())
        {
            return Err(BranchError::UnknownContext);
        }
        if query != self.input.plan.query {
            return Err(BranchError::QueryMismatch);
        }
        self.store
            .current(self.id, self.input.request(), view)
            .map_err(BranchError::Observation)
    }
    pub fn truth_current(
        &self,
        graph: &ContextGraph,
        context: ContextRevision,
        query: Atom,
        view: &ResourceReservationView<'_>,
    ) -> Result<Truth, BranchError> {
        self.report_current(graph, context, query, view)?
            .truth(query)
            .map_err(|e| BranchError::Observation(FourValuedError::Evaluation(e)))
    }
}

fn claim<'view>(
    view: &'view ResourceReservationView<'_>,
    bytes: usize,
) -> Result<ResourceClaim<'view>, BranchError> {
    let bytes = u64::try_from(bytes).map_err(|_| BranchError::ArithmeticOverflow)?;
    let amount = ResourceAmount::checked(bytes).map_err(BranchError::Resource)?;
    view.claim_resources(&[ResourceRequest::new(ResourceKind::Ram, amount)])
        .map_err(BranchError::Resource)
}
fn check_claim(
    held: &ResourceClaim<'_>,
    view: &ResourceReservationView<'_>,
) -> Result<(), BranchError> {
    held.check().map_err(BranchError::Resource)?;
    if !held.same_reservation(view) {
        return Err(BranchError::Resource(ResourceError::ForeignOwner));
    }
    Ok(())
}
