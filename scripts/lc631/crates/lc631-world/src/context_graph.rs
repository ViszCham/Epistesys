//! Bounded real context graph over the core admission rules.
//! This consumer retains duplicate proposal provenance without counting aliases
//! as new worlds. It issues neither evidence nor execution capabilities.
use lc631_core::context::{
    ContextAdmission, ContextConsumer, ContextDelta, ContextError, ContextRevision, ContextState,
    OriginalContext,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextGraphError {
    InvalidTransitionBudget,
    TransitionBudget,
    AllocationFailed,
    Context(ContextError),
}

pub struct ContextTransition {
    target: ContextRevision,
    duplicate: bool,
    delta: ContextDelta,
}

impl ContextTransition {
    pub const fn target(&self) -> ContextRevision {
        self.target
    }
    pub const fn is_duplicate(&self) -> bool {
        self.duplicate
    }
    pub fn delta(&self) -> &ContextDelta {
        &self.delta
    }
}

pub struct ContextGraph {
    consumer: ContextConsumer,
    transitions: Vec<ContextTransition>,
    max_transitions: usize,
}

impl ContextGraph {
    pub fn new(
        original: OriginalContext,
        max_transitions: usize,
    ) -> Result<Self, ContextGraphError> {
        if max_transitions == 0 || max_transitions > 100_000 {
            return Err(ContextGraphError::InvalidTransitionBudget);
        }
        let mut transitions = Vec::new();
        transitions
            .try_reserve_exact(max_transitions)
            .map_err(|_| ContextGraphError::AllocationFailed)?;
        let consumer = ContextConsumer::new(original).map_err(ContextGraphError::Context)?;
        Ok(Self {
            consumer,
            transitions,
            max_transitions,
        })
    }

    pub fn original(&self) -> &OriginalContext {
        self.consumer.original()
    }
    pub fn states(&self) -> &[ContextState] {
        self.consumer.states()
    }
    pub fn get(&self, revision: ContextRevision) -> Option<&ContextState> {
        self.consumer.get(revision)
    }
    pub fn transitions(&self) -> &[ContextTransition] {
        &self.transitions
    }

    pub fn propose(&mut self, delta: ContextDelta) -> Result<ContextAdmission, ContextGraphError> {
        if self.transitions.len() >= self.max_transitions {
            return Err(ContextGraphError::TransitionBudget);
        }
        if delta.edits().len() > self.original().limits().max_edits {
            return Err(ContextGraphError::Context(ContextError::BudgetExceeded));
        }
        // Copy only bounded edit records and shared Arc/source views. Reserve
        // first so the core state cannot publish before its ledger has capacity.
        let mut edits = Vec::new();
        edits
            .try_reserve_exact(delta.edits().len())
            .map_err(|_| ContextGraphError::AllocationFailed)?;
        edits.extend_from_slice(delta.edits());
        let proposal = ContextDelta::proposed(
            delta.parent(),
            delta.scope(),
            delta.authority(),
            delta.applicability(),
            edits,
        );
        let admission = self
            .consumer
            .admit(proposal)
            .map_err(ContextGraphError::Context)?;
        let (target, duplicate) = match admission {
            ContextAdmission::Inserted(revision) => (revision, false),
            ContextAdmission::Duplicate(revision) => (revision, true),
        };
        self.transitions.push(ContextTransition {
            target,
            duplicate,
            delta,
        });
        Ok(admission)
    }
}
