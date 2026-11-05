//! Bounded structural candidate preparation; independent validation remains open.
//! 日本語: 前提編集は提案のまま保持し、観測・権限・創造性得点へ昇格しない。
//! English: premise edits remain proposals, never observations, permits or scores.
use lc631_core::context::{
    ContextAdmission, ContextConsumer, ContextDelta, ContextError, ContextRevision,
    OriginalContext, PremiseBasis, PremiseEdit,
};
use lc631_substrate::source::{SourceError, SourceSpan, SourceStore};
use lc631_substrate::Digest32;
use std::sync::Arc;

/// Declared origin only. No provider is executed or authenticated by this module.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProposalProvider {
    DeterministicOperator,
    ReportedLlm { provider_revision: Digest32 },
}

/// Only preservation and explicit mutable-premise replacement are implemented.
/// The other operations require typed logical forms and dedicated validators.
#[derive(Clone, Debug)]
pub enum CreativeOperator {
    Preserve,
    RevisePremise {
        target: Digest32,
        replacement: Arc<str>,
    },
    Recombine,
    Rebind,
    Reframe,
    Abstract,
    CounterexampleRevision,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperatorKind {
    Preserve,
    RevisePremise,
    Recombine,
    Rebind,
    Reframe,
    Abstract,
    CounterexampleRevision,
}

impl CreativeOperator {
    pub const fn kind(&self) -> OperatorKind {
        match self {
            Self::Preserve => OperatorKind::Preserve,
            Self::RevisePremise { .. } => OperatorKind::RevisePremise,
            Self::Recombine => OperatorKind::Recombine,
            Self::Rebind => OperatorKind::Rebind,
            Self::Reframe => OperatorKind::Reframe,
            Self::Abstract => OperatorKind::Abstract,
            Self::CounterexampleRevision => OperatorKind::CounterexampleRevision,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CandidateValidation {
    NeedsIndependentValidation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparationError {
    InvalidProposalLimit,
    ProposalLimit,
    AllocationFailed,
    ForeignSource,
    EmptySourceAnchor,
    Source(SourceError),
    Context(ContextError),
    UnknownParent,
    MissingPremise,
    HardInvariant,
    NoStructuralChange,
    EmptyReplacement,
    ReplacementLimit,
    UnsupportedOperator(OperatorKind),
}

/// Structural provenance, deliberately lacking a Validated/Eligible conversion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedCandidate {
    parent: ContextRevision,
    target: ContextRevision,
    anchor: SourceSpan,
    provider: ProposalProvider,
    operator: OperatorKind,
    duplicate: bool,
}

impl PreparedCandidate {
    pub const fn parent(self) -> ContextRevision {
        self.parent
    }
    pub const fn target(self) -> ContextRevision {
        self.target
    }
    pub const fn source_anchor(self) -> SourceSpan {
        self.anchor
    }
    pub const fn provider(self) -> ProposalProvider {
        self.provider
    }
    pub const fn operator(self) -> OperatorKind {
        self.operator
    }
    pub const fn is_duplicate(self) -> bool {
        self.duplicate
    }
    pub const fn validation(self) -> CandidateValidation {
        CandidateValidation::NeedsIndependentValidation
    }
}

/// Single owner of bounded core states and a separately bounded proposal ledger.
/// This preparation seam is not wired into runtime, world or a solver registry.
pub struct CreativePreparation {
    contexts: ContextConsumer,
    proposals: Vec<PreparedCandidate>,
    max_proposals: usize,
}

impl CreativePreparation {
    pub fn new(original: OriginalContext, max_proposals: usize) -> Result<Self, PreparationError> {
        if max_proposals == 0 || max_proposals > 2_048 {
            return Err(PreparationError::InvalidProposalLimit);
        }
        let mut proposals = Vec::new();
        proposals
            .try_reserve_exact(max_proposals)
            .map_err(|_| PreparationError::AllocationFailed)?;
        let contexts = ContextConsumer::new(original).map_err(PreparationError::Context)?;
        Ok(Self {
            contexts,
            proposals,
            max_proposals,
        })
    }

    pub fn contexts(&self) -> &ContextConsumer {
        &self.contexts
    }
    pub fn proposals(&self) -> &[PreparedCandidate] {
        &self.proposals
    }

    /// The anchor binds source bytes only; it does not justify replacement truth.
    pub fn prepare(
        &mut self,
        source: &SourceStore,
        anchor: SourceSpan,
        parent: ContextRevision,
        provider: ProposalProvider,
        operator: CreativeOperator,
    ) -> Result<PreparedCandidate, PreparationError> {
        if self.proposals.len() >= self.max_proposals {
            return Err(PreparationError::ProposalLimit);
        }
        if source.id().digest() != parent.source().digest() {
            return Err(PreparationError::ForeignSource);
        }
        if source
            .slice(anchor)
            .map_err(PreparationError::Source)?
            .is_empty()
        {
            return Err(PreparationError::EmptySourceAnchor);
        }
        let kind = operator.kind();
        let state = self
            .contexts
            .get(parent)
            .ok_or(PreparationError::UnknownParent)?;
        let mut edits = Vec::new();
        match operator {
            CreativeOperator::Preserve => {}
            CreativeOperator::RevisePremise {
                target,
                replacement,
            } => {
                let premise = state
                    .premises()
                    .iter()
                    .find(|p| p.id() == target)
                    .ok_or(PreparationError::MissingPremise)?;
                if !premise.changeable() {
                    return Err(PreparationError::HardInvariant);
                }
                if replacement.is_empty() {
                    return Err(PreparationError::EmptyReplacement);
                }
                if replacement.len() > self.contexts.original().limits().max_statement_bytes {
                    return Err(PreparationError::ReplacementLimit);
                }
                if replacement.as_ref() == premise.statement() {
                    return Err(PreparationError::NoStructuralChange);
                }
                edits
                    .try_reserve_exact(1)
                    .map_err(|_| PreparationError::AllocationFailed)?;
                edits.push(PremiseEdit::Replace {
                    id: target,
                    statement: replacement,
                    basis: PremiseBasis::Proposed,
                });
            }
            CreativeOperator::Recombine
            | CreativeOperator::Rebind
            | CreativeOperator::Reframe
            | CreativeOperator::Abstract
            | CreativeOperator::CounterexampleRevision => {
                return Err(PreparationError::UnsupportedOperator(kind));
            }
        }
        // Authority/scope/applicability are borrowed from checked core state,
        // never supplied by a proposal or inferred from a provider label.
        let delta = ContextDelta::proposed(
            parent,
            state.scope(),
            self.contexts.original().authority(),
            state.applicability(),
            edits,
        );
        let admission = self
            .contexts
            .admit(delta)
            .map_err(PreparationError::Context)?;
        let (target, duplicate) = match admission {
            ContextAdmission::Inserted(revision) => (revision, false),
            ContextAdmission::Duplicate(revision) => (revision, true),
        };
        let candidate = PreparedCandidate {
            parent,
            target,
            anchor,
            provider,
            operator: kind,
            duplicate,
        };
        // All proposal capacity was reserved before ContextConsumer published.
        self.proposals.push(candidate);
        Ok(candidate)
    }
}
