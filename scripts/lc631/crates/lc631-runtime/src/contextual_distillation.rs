//! Preparatory finite-relation distillation; no solver/geometry/host observation.
//! Input relations are Given, not authenticated facts or independent gold.
#![forbid(unsafe_code)]

use lc631_core::context::{ContextRevision, ContextScope};
use lc631_substrate::Digest32;
use sha2::{Digest, Sha256};

/// Revalidation and bounded path consumption of advisory next-query requests.
pub mod consumer;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RelationNode(pub u8);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RelationEdit {
    pub from: RelationNode,
    pub to: RelationNode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelationProposal {
    pub context: ContextRevision,
    pub scope: ContextScope,
    pub edit: RelationEdit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BasisAvailability {
    Given,
    Unavailable,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelationValidation {
    ExistingMembership,
    FiniteDerived,
    Refuted,
    NeedsEvidence,
    Unsupported,
    Incomparable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidationBasis {
    /// Reachability of supplied edges only; not a native solver result.
    GivenFiniteReachability,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DistillationError {
    InvalidLimits,
    InvalidRelation,
    CyclicBasis,
    DuplicateProposal,
    DuplicateBasisRelation,
    EmptyBatch,
    EpochBudget,
    ProposalBudget,
    RelationBudget,
    AllocationFailed,
}

/// Bounded, immutable validator input. No method writes proposals into its basis.
#[derive(Debug)]
pub struct ImmutableRelationBasis {
    context: ContextRevision,
    scope: ContextScope,
    nodes: u8,
    edges: Vec<RelationEdit>,
    closure: [u64; 64],
    availability: BasisAvailability,
    digest: Digest32,
}

impl ImmutableRelationBasis {
    pub fn checked(
        context: ContextRevision,
        scope: ContextScope,
        nodes: u8,
        edges: &[RelationEdit],
        availability: BasisAvailability,
    ) -> Result<Self, DistillationError> {
        if nodes == 0 || nodes > 64 || edges.len() > 4_032 {
            return Err(DistillationError::InvalidLimits);
        }
        let mut canonical = bounded_copy(edges)?;
        canonical.sort_unstable();
        if canonical.windows(2).any(|w| w[0] == w[1]) {
            return Err(DistillationError::DuplicateBasisRelation);
        }
        let mut closure = [0u64; 64];
        for edit in &canonical {
            check_edit(*edit, nodes)?;
            closure[usize::from(edit.from.0)] |= 1u64 << edit.to.0;
        }
        // Bitset Warshall closure is separate from proposal generation.
        for k in 0..nodes {
            for i in 0..nodes {
                if closure[usize::from(i)] & (1u64 << k) != 0 {
                    closure[usize::from(i)] |= closure[usize::from(k)];
                }
            }
        }
        if (0..nodes).any(|n| closure[usize::from(n)] & (1u64 << n) != 0) {
            return Err(DistillationError::CyclicBasis);
        }
        let digest = basis_digest(context, scope, nodes, &canonical, availability);
        Ok(Self {
            context,
            scope,
            nodes,
            edges: canonical,
            closure,
            availability,
            digest,
        })
    }

    pub const fn digest(&self) -> Digest32 {
        self.digest
    }

    pub const fn context(&self) -> ContextRevision {
        self.context
    }
    pub const fn scope(&self) -> ContextScope {
        self.scope
    }
    pub const fn node_count(&self) -> u8 {
        self.nodes
    }
    pub fn edges(&self) -> &[RelationEdit] {
        &self.edges
    }

    pub fn validate(&self, proposal: RelationProposal) -> RelationValidation {
        if proposal.context != self.context || proposal.scope != self.scope {
            return RelationValidation::Incomparable;
        }
        if check_edit(proposal.edit, self.nodes).is_err() {
            return RelationValidation::Refuted;
        }
        match self.availability {
            BasisAvailability::Unavailable => return RelationValidation::NeedsEvidence,
            BasisAvailability::Unsupported => return RelationValidation::Unsupported,
            BasisAvailability::Given => {}
        }
        if self.edges.binary_search(&proposal.edit).is_ok() {
            return RelationValidation::ExistingMembership;
        }
        let RelationEdit { from, to } = proposal.edit;
        if self.closure[usize::from(from.0)] & (1u64 << to.0) != 0 {
            RelationValidation::FiniteDerived
        } else {
            // Refuted means absent in this complete finite reachability model.
            // It does not refute an open-world relation outside this profile.
            RelationValidation::Refuted
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DistillationLimits {
    pub max_epochs: u16,
    pub max_proposals: usize,
    pub max_relations: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EpochStop {
    ContentChanged,
    NoProgress,
    EpochBudget,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelationOutcome {
    pub edit: RelationEdit,
    pub status: RelationValidation,
}

/// A request to inspect newly derived structure, never an executable solver query.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NextRelationQuery {
    pub context: ContextRevision,
    pub scope: ContextScope,
    pub state_revision: Digest32,
    pub basis_digest: Digest32,
    pub relation: RelationEdit,
    pub basis: ValidationBasis,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelationEpochReport {
    pub epoch: u16,
    pub before: Digest32,
    pub after: Digest32,
    pub added: usize,
    pub outcomes: Vec<RelationOutcome>,
    pub next_query: Option<NextRelationQuery>,
    pub stop: EpochStop,
}

impl RelationEpochReport {
    pub fn residuals(&self) -> impl Iterator<Item = &RelationOutcome> {
        self.outcomes.iter().filter(|outcome| {
            !matches!(
                outcome.status,
                RelationValidation::ExistingMembership | RelationValidation::FiniteDerived
            )
        })
    }
}

/// A bounded local state owner. Context, source, scope and basis stay immutable.
pub struct ContextualDistillation {
    basis: ImmutableRelationBasis,
    limits: DistillationLimits,
    relations: Vec<RelationEdit>,
    revision: Digest32,
    epochs: u16,
}

impl ContextualDistillation {
    pub fn new(
        basis: ImmutableRelationBasis,
        limits: DistillationLimits,
    ) -> Result<Self, DistillationError> {
        if limits.max_epochs == 0
            || limits.max_epochs > 64
            || limits.max_proposals == 0
            || limits.max_proposals > 128
            || limits.max_relations == 0
            || limits.max_relations > 4_032
            || basis.edges.len() > limits.max_relations
        {
            return Err(DistillationError::InvalidLimits);
        }
        let relations = bounded_copy(&basis.edges)?;
        let revision = state_digest(basis.digest, &relations);
        Ok(Self {
            basis,
            limits,
            relations,
            revision,
            epochs: 0,
        })
    }

    pub const fn state_revision(&self) -> Digest32 {
        self.revision
    }
    pub const fn epochs_used(&self) -> u16 {
        self.epochs
    }
    pub fn relations(&self) -> &[RelationEdit] {
        &self.relations
    }
    pub const fn basis(&self) -> &ImmutableRelationBasis {
        &self.basis
    }

    pub fn epoch(
        &mut self,
        proposals: &[RelationProposal],
    ) -> Result<RelationEpochReport, DistillationError> {
        self.check_batch(proposals)?;
        let mut next = bounded_copy(&self.relations)?;
        let mut outcomes = Vec::new();
        outcomes
            .try_reserve_exact(proposals.len())
            .map_err(|_| DistillationError::AllocationFailed)?;
        let mut first_added = None;
        for proposal in proposals {
            let status = self.basis.validate(*proposal);
            if status == RelationValidation::FiniteDerived && !next.contains(&proposal.edit) {
                if next.len() >= self.limits.max_relations {
                    return Err(DistillationError::RelationBudget);
                }
                next.try_reserve(1)
                    .map_err(|_| DistillationError::AllocationFailed)?;
                next.push(proposal.edit);
                // Canonical order, not proposal arrival order, picks the work item.
                first_added = Some(
                    first_added.map_or(proposal.edit, |old: RelationEdit| old.min(proposal.edit)),
                );
            }
            outcomes.push(RelationOutcome {
                edit: proposal.edit,
                status,
            });
        }
        next.sort_unstable();
        let added = next.len() - self.relations.len();
        let before = self.revision;
        let after = state_digest(self.basis.digest, &next);
        let epoch = self.epochs;
        let next_epoch = epoch + 1; // max_epochs <= 64, checked before entry.
        let next_query = first_added.map(|relation| NextRelationQuery {
            context: self.basis.context,
            scope: self.basis.scope,
            state_revision: after,
            basis_digest: self.basis.digest,
            relation,
            basis: ValidationBasis::GivenFiniteReachability,
        });
        let stop = if added == 0 {
            EpochStop::NoProgress
        } else if next_epoch == self.limits.max_epochs {
            EpochStop::EpochBudget
        } else {
            EpochStop::ContentChanged
        };
        // Publish only after every validation/allocation succeeds.
        self.relations = next;
        self.revision = after;
        self.epochs = next_epoch;
        Ok(RelationEpochReport {
            epoch,
            before,
            after,
            added,
            outcomes,
            next_query,
            stop,
        })
    }

    fn check_batch(&self, proposals: &[RelationProposal]) -> Result<(), DistillationError> {
        if self.epochs >= self.limits.max_epochs {
            return Err(DistillationError::EpochBudget);
        }
        if proposals.is_empty() {
            return Err(DistillationError::EmptyBatch);
        }
        if proposals.len() > self.limits.max_proposals {
            return Err(DistillationError::ProposalBudget);
        }
        for (i, proposal) in proposals.iter().enumerate() {
            check_edit(proposal.edit, self.basis.nodes)?;
            // A relation is scoped by its context and comparison contract.
            // Preserve foreign proposals for explicit Incomparable outcomes.
            if proposals[..i].contains(proposal) {
                return Err(DistillationError::DuplicateProposal);
            }
        }
        Ok(())
    }
}

fn check_edit(edit: RelationEdit, nodes: u8) -> Result<(), DistillationError> {
    if edit.from.0 >= nodes || edit.to.0 >= nodes || edit.from == edit.to {
        Err(DistillationError::InvalidRelation)
    } else {
        Ok(())
    }
}

fn bounded_copy<T: Copy>(input: &[T]) -> Result<Vec<T>, DistillationError> {
    let mut out = Vec::new();
    out.try_reserve_exact(input.len())
        .map_err(|_| DistillationError::AllocationFailed)?;
    out.extend_from_slice(input);
    Ok(out)
}

fn basis_digest(
    context: ContextRevision,
    scope: ContextScope,
    nodes: u8,
    edges: &[RelationEdit],
    availability: BasisAvailability,
) -> Digest32 {
    let mut hash = Sha256::new();
    hash.update(b"epistesys.tlcx27.given-finite-reachability.v1\0");
    for digest in [
        context.source().digest(),
        context.interpretation_digest(),
        scope.comparison(),
        scope.goal(),
        scope.probes(),
        scope.semantics(),
    ] {
        hash.update(digest.as_bytes());
    }
    let availability_tag = match availability {
        BasisAvailability::Given => 0,
        BasisAvailability::Unavailable => 1,
        BasisAvailability::Unsupported => 2,
    };
    hash.update([nodes, availability_tag]);
    hash_relations(&mut hash, edges);
    Digest32::from_bytes(hash.finalize().into())
}

fn state_digest(basis: Digest32, relations: &[RelationEdit]) -> Digest32 {
    let mut hash = Sha256::new();
    hash.update(b"epistesys.tlcx27.relation-state.v1\0");
    hash.update(basis.as_bytes());
    hash_relations(&mut hash, relations);
    Digest32::from_bytes(hash.finalize().into())
}

fn hash_relations(hash: &mut Sha256, relations: &[RelationEdit]) {
    // Validated domain <=64 gives at most 4032 relations; no usize wire identity.
    hash.update((relations.len() as u16).to_le_bytes());
    for relation in relations {
        hash.update([relation.from.0, relation.to.0]);
    }
}
