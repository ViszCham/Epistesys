//! Reasoning identities remain separate from execution capabilities.
//!
//! A context revision is not an authority revision:
//! ```compile_fail
//! use lc631_core::{AuthorityRevision, context::{ContextRevision, SourceRevision}};
//! let context = ContextRevision::original(SourceRevision::of("input"));
//! let _: AuthorityRevision = context;
//! ```
//! It also exposes no execution-permit issuer:
//! ```compile_fail
//! use lc631_core::context::{ContextRevision, SourceRevision};
//! let context = ContextRevision::original(SourceRevision::of("input"));
//! context.issue_execution_permit();
//! ```
use crate::AuthorityRevision;
// Preserve the public core paths while sharing one low-level identity owner.
pub use lc631_substrate::context_identity::{ContextRevision, SourceRevision};

/// Observation metadata, deliberately not a validated receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EvidenceRevision(u64);

impl EvidenceRevision {
    pub const fn observed(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReasoningRevisions {
    context: ContextRevision,
    authority: AuthorityRevision,
    evidence: EvidenceRevision,
}

impl ReasoningRevisions {
    pub const fn observed(
        context: ContextRevision,
        authority: AuthorityRevision,
        evidence: EvidenceRevision,
    ) -> Self {
        Self {
            context,
            authority,
            evidence,
        }
    }

    pub fn with_context(self, context: ContextRevision) -> Result<Self, SourceMismatch> {
        if self.context.source() != context.source() {
            return Err(SourceMismatch);
        }
        Ok(Self { context, ..self })
    }

    pub const fn context(self) -> ContextRevision {
        self.context
    }

    pub const fn authority(self) -> AuthorityRevision {
        self.authority
    }

    pub const fn evidence(self) -> EvidenceRevision {
        self.evidence
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceMismatch;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskKind {
    Mathematics,
    Reasoning,
    Explanation,
    Coding,
    Media,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MissingStage {
    ContextLowering,
    SolverPortfolio,
    Measurement,
    Selection,
    HostObservation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecisionStatus {
    NotConnected(MissingStage),
    NeedsEvidence,
    Clarify,
    Hold,
    Conflict,
}

pub use reconstruction::*;

/// Structural premise reconstruction only: no solver truth, receipt or permit.
mod reconstruction {
    use super::{ContextRevision, SourceRevision};
    use crate::AuthorityRevision;
    use lc631_substrate::source::{SourceError, SourceSpan, SourceStore};
    use lc631_substrate::Digest32;
    use sha2::{Digest, Sha256};
    use std::collections::BTreeSet;
    use std::sync::Arc;

    /// These are supplied input classifications, not independently verified evidence.
    /// A child proposal never rewrites its parent's or the original observation.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum PremiseBasis {
        Given,
        Observed,
        Assumed,
        Derived,
        Proposed,
        Refuted,
        Conflicted,
    }

    impl PremiseBasis {
        fn code(self) -> u8 {
            match self {
                Self::Given => 0,
                Self::Observed => 1,
                Self::Assumed => 2,
                Self::Derived => 3,
                Self::Proposed => 4,
                Self::Refuted => 5,
                Self::Conflicted => 6,
            }
        }

        fn is_proposal(self) -> bool {
            matches!(self, Self::Assumed | Self::Proposed)
        }
    }

    /// Scope fields pin declared registries; their hashes do not validate those registries.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct ContextScope {
        comparison: Digest32,
        goal: Digest32,
        probes: Digest32,
        semantics: Digest32,
    }

    impl ContextScope {
        pub const fn declared(
            comparison: Digest32,
            goal: Digest32,
            probes: Digest32,
            semantics: Digest32,
        ) -> Self {
            Self {
                comparison,
                goal,
                probes,
                semantics,
            }
        }

        pub const fn comparison(self) -> Digest32 {
            self.comparison
        }
        pub const fn goal(self) -> Digest32 {
            self.goal
        }
        pub const fn probes(self) -> Digest32 {
            self.probes
        }
        pub const fn semantics(self) -> Digest32 {
            self.semantics
        }

        fn hash_into(self, hash: &mut Sha256) {
            for value in [self.comparison, self.goal, self.probes, self.semantics] {
                hash.update(value.as_bytes());
            }
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct ContextPremise {
        id: Digest32,
        statement: PremiseText,
        basis: PremiseBasis,
        changeable: bool,
    }

    #[derive(Clone, Debug)]
    enum PremiseText {
        OwnedProposal(Arc<str>),
        SourceView {
            store: SourceStore,
            span: SourceSpan,
        },
    }

    impl PremiseText {
        fn text(&self) -> &str {
            match self {
                Self::OwnedProposal(text) => text,
                // Construction checks this immutable store/span pair. Neither
                // field is exposed mutably, so the index invariant is static.
                Self::SourceView { store, span } => &store.text()[span.start()..span.end()],
            }
        }
    }

    impl PartialEq for PremiseText {
        fn eq(&self, other: &Self) -> bool {
            self.text() == other.text()
        }
    }
    impl Eq for PremiseText {}

    impl ContextPremise {
        pub fn reported(
            id: Digest32,
            statement: Arc<str>,
            basis: PremiseBasis,
            changeable: bool,
        ) -> Self {
            Self {
                id,
                statement: PremiseText::OwnedProposal(statement),
                basis,
                changeable,
            }
        }

        /// A checked source view is provenance, not verified semantic evidence.
        pub fn from_source(
            id: Digest32,
            store: &SourceStore,
            span: SourceSpan,
            basis: PremiseBasis,
            changeable: bool,
        ) -> Result<Self, SourceError> {
            store.slice(span)?;
            Ok(Self {
                id,
                statement: PremiseText::SourceView {
                    store: store.clone(),
                    span,
                },
                basis,
                changeable,
            })
        }

        pub fn source_span(&self) -> Option<SourceSpan> {
            match &self.statement {
                PremiseText::OwnedProposal(_) => None,
                PremiseText::SourceView { span, .. } => Some(*span),
            }
        }

        pub const fn id(&self) -> Digest32 {
            self.id
        }
        pub fn statement(&self) -> &str {
            self.statement.text()
        }
        pub const fn basis(&self) -> PremiseBasis {
            self.basis
        }
        pub const fn changeable(&self) -> bool {
            self.changeable
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum ApplicabilityState {
        Unknown,
        Unsupported,
        Supported,
        Refuted,
        Conflict,
    }

    /// Reported applicability is retained, not promoted to validated truth.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum ContextApplicability {
        Unconditional,
        Conditional {
            scope: ContextScope,
            predicate: Digest32,
            state: ApplicabilityState,
        },
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct ContextLimits {
        pub max_contexts: usize,
        pub max_premises: usize,
        pub max_edits: usize,
        /// Logical text bytes per state, not an OS/process-wide memory guarantee.
        pub max_statement_bytes: usize,
    }

    impl ContextLimits {
        fn checked(self) -> Result<Self, ContextError> {
            if self.max_contexts == 0
                || self.max_contexts > 2_048
                || self.max_premises == 0
                || self.max_premises > 100_000
                || self.max_edits == 0
                || self.max_edits > 100_000
                || self.max_statement_bytes == 0
                || self.max_statement_bytes > 128 * 1024 * 1024
            {
                return Err(ContextError::InvalidLimits);
            }
            Ok(self)
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum PremiseEdit {
        Add(ContextPremise),
        Replace {
            id: Digest32,
            statement: Arc<str>,
            basis: PremiseBasis,
        },
        Retract {
            id: Digest32,
        },
    }

    impl PremiseEdit {
        fn target(&self) -> Digest32 {
            match self {
                Self::Add(premise) => premise.id,
                Self::Replace { id, .. } | Self::Retract { id } => *id,
            }
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct ContextDelta {
        parent: ContextRevision,
        scope: ContextScope,
        authority: AuthorityRevision,
        applicability: ContextApplicability,
        edits: Vec<PremiseEdit>,
    }

    impl ContextDelta {
        pub fn proposed(
            parent: ContextRevision,
            scope: ContextScope,
            authority: AuthorityRevision,
            applicability: ContextApplicability,
            edits: Vec<PremiseEdit>,
        ) -> Self {
            Self {
                parent,
                scope,
                authority,
                applicability,
                edits,
            }
        }

        pub fn edits(&self) -> &[PremiseEdit] {
            &self.edits
        }
        pub const fn parent(&self) -> ContextRevision {
            self.parent
        }

        pub const fn scope(&self) -> ContextScope {
            self.scope
        }
        pub const fn authority(&self) -> AuthorityRevision {
            self.authority
        }
        pub const fn applicability(&self) -> ContextApplicability {
            self.applicability
        }
    }

    /// Checked structure is not an execution capability:
    /// ```compile_fail
    /// use lc631_core::{context::ContextState, ExecutionPermit};
    /// fn promote(state: ContextState) -> ExecutionPermit { state.into() }
    /// ```
    #[derive(Clone, Debug)]
    pub struct ContextState {
        revision: ContextRevision,
        scope: ContextScope,
        premises: Arc<[ContextPremise]>,
        applicability: ContextApplicability,
        delta: Option<Arc<ContextDelta>>,
    }

    impl ContextState {
        pub const fn revision(&self) -> ContextRevision {
            self.revision
        }
        pub const fn scope(&self) -> ContextScope {
            self.scope
        }
        pub fn premises(&self) -> &[ContextPremise] {
            &self.premises
        }
        pub const fn applicability(&self) -> ContextApplicability {
            self.applicability
        }
        pub fn parent(&self) -> Option<ContextRevision> {
            self.delta.as_ref().map(|delta| delta.parent)
        }
        pub fn declared_delta(&self) -> Option<&ContextDelta> {
            self.delta.as_deref()
        }
        /// Exact canonical-text profile identity, not arbitrary semantic equivalence.
        pub const fn canonical_id(&self) -> Digest32 {
            self.revision.interpretation_digest()
        }
    }

    #[derive(Clone, Debug)]
    pub struct OriginalContext {
        state: ContextState,
        authority: AuthorityRevision,
        limits: ContextLimits,
    }

    impl OriginalContext {
        pub fn checked(
            source: SourceRevision,
            scope: ContextScope,
            authority: AuthorityRevision,
            premises: Vec<ContextPremise>,
            limits: ContextLimits,
        ) -> Result<Self, ContextError> {
            let limits = limits.checked()?;
            validate_premises(&premises, source, limits)?;
            let applicability = ContextApplicability::Unconditional;
            let identity = canonical_identity(source, scope, authority, &premises, applicability)?;
            let state = ContextState {
                revision: ContextRevision::original(source).reconstructed(*identity.as_bytes()),
                scope,
                premises: premises.into(),
                applicability,
                delta: None,
            };
            Ok(Self {
                state,
                authority,
                limits,
            })
        }

        pub const fn state(&self) -> &ContextState {
            &self.state
        }
        pub const fn authority(&self) -> AuthorityRevision {
            self.authority
        }

        pub const fn limits(&self) -> ContextLimits {
            self.limits
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum ContextError {
        InvalidLimits,
        BudgetExceeded,
        AllocationFailed,
        EmptyPremise,
        DuplicatePremise,
        DuplicateEdit,
        MissingPremise,
        HardInvariant,
        BasisPromotion,
        AuthorityMismatch,
        ScopeMismatch,
        ForeignSource,
        UnknownParent,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum ContextAdmission {
        Inserted(ContextRevision),
        Duplicate(ContextRevision),
    }

    /// Canonical core consumer; world/runtime orchestration must call this seam.
    pub struct ContextConsumer {
        original: OriginalContext,
        states: Vec<ContextState>,
    }

    impl ContextConsumer {
        pub fn new(original: OriginalContext) -> Result<Self, ContextError> {
            let mut states = Vec::new();
            states
                .try_reserve_exact(original.limits.max_contexts)
                .map_err(|_| ContextError::AllocationFailed)?;
            states.push(original.state.clone());
            Ok(Self { original, states })
        }

        pub fn original(&self) -> &OriginalContext {
            &self.original
        }
        pub fn states(&self) -> &[ContextState] {
            &self.states
        }
        pub fn get(&self, revision: ContextRevision) -> Option<&ContextState> {
            self.states.iter().find(|state| state.revision == revision)
        }

        pub fn admit(&mut self, delta: ContextDelta) -> Result<ContextAdmission, ContextError> {
            if delta.scope != self.original.state.scope {
                return Err(ContextError::ScopeMismatch);
            }
            if delta.authority != self.original.authority {
                return Err(ContextError::AuthorityMismatch);
            }
            if delta.parent.source() != self.original.state.revision.source() {
                return Err(ContextError::ForeignSource);
            }
            if let ContextApplicability::Conditional { scope, .. } = delta.applicability {
                if scope != delta.scope {
                    return Err(ContextError::ScopeMismatch);
                }
            }
            let parent = self.get(delta.parent).ok_or(ContextError::UnknownParent)?;
            let limits = self.original.limits;
            if delta.edits.len() > limits.max_edits {
                return Err(ContextError::BudgetExceeded);
            }
            let capacity = parent
                .premises
                .len()
                .checked_add(delta.edits.len())
                .ok_or(ContextError::BudgetExceeded)?;
            let mut premises = Vec::new();
            premises
                .try_reserve_exact(capacity)
                .map_err(|_| ContextError::AllocationFailed)?;
            premises.extend(parent.premises.iter().cloned());
            let mut targets = BTreeSet::new();
            for edit in &delta.edits {
                if !targets.insert(edit.target()) {
                    return Err(ContextError::DuplicateEdit);
                }
                apply_edit(&mut premises, edit)?;
            }
            validate_premises(&premises, delta.parent.source(), limits)?;
            let identity = canonical_identity(
                delta.parent.source(),
                delta.scope,
                delta.authority,
                &premises,
                delta.applicability,
            )?;
            if let Some(existing) = self
                .states
                .iter()
                .find(|state| state.canonical_id() == identity)
            {
                return Ok(ContextAdmission::Duplicate(existing.revision));
            }
            if self.states.len() >= limits.max_contexts {
                return Err(ContextError::BudgetExceeded);
            }
            let revision = ContextRevision::original(delta.parent.source())
                .reconstructed(*identity.as_bytes());
            self.states.push(ContextState {
                revision,
                scope: delta.scope,
                premises: premises.into(),
                applicability: delta.applicability,
                delta: Some(Arc::new(delta)),
            });
            Ok(ContextAdmission::Inserted(revision))
        }
    }

    fn validate_premises(
        premises: &[ContextPremise],
        source: SourceRevision,
        limits: ContextLimits,
    ) -> Result<(), ContextError> {
        if premises.len() > limits.max_premises {
            return Err(ContextError::BudgetExceeded);
        }
        let mut ids = BTreeSet::new();
        let mut bytes = 0usize;
        for premise in premises {
            if let Some(span) = premise.source_span() {
                if span.source_id().digest() != source.digest() {
                    return Err(ContextError::ForeignSource);
                }
            }
            if !ids.insert(premise.id) {
                return Err(ContextError::DuplicatePremise);
            }
            if premise.statement().is_empty() {
                return Err(ContextError::EmptyPremise);
            }
            bytes = bytes
                .checked_add(premise.statement().len())
                .ok_or(ContextError::BudgetExceeded)?;
        }
        if bytes > limits.max_statement_bytes {
            return Err(ContextError::BudgetExceeded);
        }
        Ok(())
    }

    fn apply_edit(
        premises: &mut Vec<ContextPremise>,
        edit: &PremiseEdit,
    ) -> Result<(), ContextError> {
        if let PremiseEdit::Add(premise) = edit {
            if !premise.changeable {
                return Err(ContextError::HardInvariant);
            }
            if !premise.basis.is_proposal() {
                return Err(ContextError::BasisPromotion);
            }
            if premises.iter().any(|existing| existing.id == premise.id) {
                return Err(ContextError::DuplicatePremise);
            }
            premises.push(premise.clone());
            return Ok(());
        }
        let index = premises
            .iter()
            .position(|premise| premise.id == edit.target())
            .ok_or(ContextError::MissingPremise)?;
        if !premises[index].changeable {
            return Err(ContextError::HardInvariant);
        }
        match edit {
            PremiseEdit::Replace {
                statement, basis, ..
            } => {
                if !basis.is_proposal() {
                    return Err(ContextError::BasisPromotion);
                }
                premises[index].statement = PremiseText::OwnedProposal(Arc::clone(statement));
                premises[index].basis = *basis;
            }
            PremiseEdit::Retract { .. } => {
                premises.remove(index);
            }
            PremiseEdit::Add(_) => return Err(ContextError::DuplicatePremise),
        }
        Ok(())
    }

    fn canonical_identity(
        source: SourceRevision,
        scope: ContextScope,
        authority: AuthorityRevision,
        premises: &[ContextPremise],
        applicability: ContextApplicability,
    ) -> Result<Digest32, ContextError> {
        let mut canonical = Vec::new();
        canonical
            .try_reserve_exact(premises.len())
            .map_err(|_| ContextError::AllocationFailed)?;
        canonical.extend(premises.iter().map(|premise| {
            (
                premise.statement(),
                premise.basis.code(),
                premise.changeable,
            )
        }));
        canonical.sort_unstable();
        canonical.dedup();
        let mut hash = Sha256::new();
        hash.update(b"epistesys.context-canonical-text.v1\0");
        hash.update(source.digest().as_bytes());
        scope.hash_into(&mut hash);
        hash.update(authority.0.to_le_bytes());
        let count = u64::try_from(canonical.len()).map_err(|_| ContextError::BudgetExceeded)?;
        hash.update(count.to_le_bytes());
        for (statement, basis, changeable) in canonical {
            let length =
                u64::try_from(statement.len()).map_err(|_| ContextError::BudgetExceeded)?;
            hash.update(length.to_le_bytes());
            hash.update(statement.as_bytes());
            hash.update([basis, u8::from(changeable)]);
        }
        match applicability {
            ContextApplicability::Unconditional => hash.update([0]),
            ContextApplicability::Conditional {
                predicate, state, ..
            } => {
                hash.update([1]);
                hash.update(predicate.as_bytes());
                let state_code = match state {
                    ApplicabilityState::Unknown => 0,
                    ApplicabilityState::Unsupported => 1,
                    ApplicabilityState::Supported => 2,
                    ApplicabilityState::Refuted => 3,
                    ApplicabilityState::Conflict => 4,
                };
                hash.update([state_code]);
            }
        }
        Ok(Digest32::from_bytes(hash.finalize().into()))
    }
}
