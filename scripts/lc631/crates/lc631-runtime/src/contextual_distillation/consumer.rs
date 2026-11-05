//! Consume advisory relation queries only after current-state and path checks.
//! Separate algorithms share the Given basis: no independent gold or truth claim.
#![forbid(unsafe_code)]

use super::{ContextualDistillation, NextRelationQuery, RelationEdit, RelationNode};
use super::{RelationProposal, RelationValidation};

/// Explicit relation materialization to fixed structural probes/current selection.
pub mod measurement;

/// Logical edge examinations, not elapsed-time or process-wide enforcement.
pub const MAX_RELATION_RECHECK_EDGE_VISITS: u32 = 262_144;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelationConsumerHold {
    ForeignContext,
    ForeignScope,
    ForeignBasis,
    StaleState,
    InvalidRelation,
    NotDerivedRelation,
    RelationNotAdmitted,
    Refuted,
    NeedsEvidence,
    Unsupported,
    Incomparable,
    InvalidBudget,
    ResourceExhausted,
    PathMissing,
    WitnessMismatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelationInputBasis {
    Given,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelationIndependence {
    SharedBasisUnassessed,
}

/// A borrow-bound check over a supplied finite DAG; not an execution capability.
/// The caller cannot construct the token by changing public report fields.
/// ```compile_fail
/// use lc631_runtime::contextual_distillation::consumer::CheckedRelationQuery;
/// let checked = CheckedRelationQuery { };
/// ```
/// State cannot advance while a token will still be consumed:
/// ```compile_fail
/// use lc631_runtime::contextual_distillation::{ContextualDistillation, NextRelationQuery};
/// use lc631_runtime::contextual_distillation::consumer::{revalidate_next_query, consume_dependency_path};
/// fn stale(state: &mut ContextualDistillation, request: NextRelationQuery) {
///     let token = revalidate_next_query(state, request, 1000).unwrap();
///     let _ = state.epoch(&[]);
///     let _ = consume_dependency_path(token);
/// }
/// ```
pub struct CheckedRelationQuery<'state> {
    state: &'state ContextualDistillation,
    request: NextRelationQuery,
    nodes: [RelationNode; 64],
    length: u8,
    edge_visits: u32,
}

/// Recheck the current state and independently walk its immutable base edges.
/// This checks content; it does not authenticate that a public request was issued
/// by a specific epoch or that the supplied edges model the intended semantics.
pub fn revalidate_next_query(
    state: &ContextualDistillation,
    request: NextRelationQuery,
    max_edge_visits: u32,
) -> Result<CheckedRelationQuery<'_>, RelationConsumerHold> {
    check_binding(state, request)?;
    if max_edge_visits > MAX_RELATION_RECHECK_EDGE_VISITS {
        return Err(RelationConsumerHold::InvalidBudget);
    }
    match state.basis().validate(RelationProposal {
        context: request.context,
        scope: request.scope,
        edit: request.relation,
    }) {
        RelationValidation::FiniteDerived => {}
        RelationValidation::ExistingMembership => {
            return Err(RelationConsumerHold::NotDerivedRelation)
        }
        RelationValidation::Refuted => return Err(RelationConsumerHold::Refuted),
        RelationValidation::NeedsEvidence => return Err(RelationConsumerHold::NeedsEvidence),
        RelationValidation::Unsupported => return Err(RelationConsumerHold::Unsupported),
        RelationValidation::Incomparable => return Err(RelationConsumerHold::Incomparable),
    }
    if state.relations().binary_search(&request.relation).is_err() {
        return Err(RelationConsumerHold::RelationNotAdmitted);
    }
    let (nodes, length, edge_visits) = walk_base_edges(state, request.relation, max_edge_visits)?;
    Ok(CheckedRelationQuery {
        state,
        request,
        nodes,
        length,
        edge_visits,
    })
}

fn check_binding(
    state: &ContextualDistillation,
    request: NextRelationQuery,
) -> Result<(), RelationConsumerHold> {
    if request.context != state.basis().context() {
        return Err(RelationConsumerHold::ForeignContext);
    }
    if request.scope != state.basis().scope() {
        return Err(RelationConsumerHold::ForeignScope);
    }
    if request.basis_digest != state.basis().digest() {
        return Err(RelationConsumerHold::ForeignBasis);
    }
    if request.state_revision != state.state_revision() {
        return Err(RelationConsumerHold::StaleState);
    }
    let relation = request.relation;
    if relation.from.0 >= state.basis().node_count()
        || relation.to.0 >= state.basis().node_count()
        || relation.from == relation.to
    {
        return Err(RelationConsumerHold::InvalidRelation);
    }
    Ok(())
}

fn walk_base_edges(
    state: &ContextualDistillation,
    relation: RelationEdit,
    limit: u32,
) -> Result<([RelationNode; 64], u8, u32), RelationConsumerHold> {
    // Queue/predecessors are fixed-size, separate from the producer's Warshall
    // closure. A node is enqueued at most once; indices therefore stay <64.
    let mut queue = [0u8; 64];
    let mut predecessors = [u8::MAX; 64];
    let mut seen = [false; 64];
    queue[0] = relation.from.0;
    seen[usize::from(relation.from.0)] = true;
    let (mut head, mut tail, mut visits) = (0usize, 1usize, 0u32);
    while head < tail {
        let node = queue[head];
        head += 1;
        for edge in state.basis().edges() {
            if visits >= limit {
                return Err(RelationConsumerHold::ResourceExhausted);
            }
            visits += 1; // At most 64*4032, below the u32 domain.
            if edge.from.0 != node || seen[usize::from(edge.to.0)] {
                continue;
            }
            predecessors[usize::from(edge.to.0)] = node;
            seen[usize::from(edge.to.0)] = true;
            if edge.to == relation.to {
                let (path, length) = trace_path(relation, &predecessors)?;
                return Ok((path, length, visits));
            }
            queue[tail] = edge.to.0;
            tail += 1;
        }
    }
    Err(RelationConsumerHold::PathMissing)
}

fn trace_path(
    relation: RelationEdit,
    predecessors: &[u8; 64],
) -> Result<([RelationNode; 64], u8), RelationConsumerHold> {
    let mut path = [RelationNode(0); 64];
    let mut length = 0usize;
    let mut current = relation.to.0;
    loop {
        if length == path.len() {
            return Err(RelationConsumerHold::WitnessMismatch);
        }
        path[length] = RelationNode(current);
        length += 1;
        if current == relation.from.0 {
            break;
        }
        current = *predecessors
            .get(usize::from(current))
            .ok_or(RelationConsumerHold::WitnessMismatch)?;
        if current == u8::MAX {
            return Err(RelationConsumerHold::WitnessMismatch);
        }
    }
    path[..length].reverse();
    Ok((path, length as u8)) // length<=64 by the checked fixed array above.
}

/// A concrete consumer result retaining the state borrow and Given input gap.
pub struct DependencyPath<'state> {
    checked: CheckedRelationQuery<'state>,
}

impl DependencyPath<'_> {
    pub fn nodes(&self) -> &[RelationNode] {
        &self.checked.nodes[..usize::from(self.checked.length)]
    }
    pub const fn request(&self) -> NextRelationQuery {
        self.checked.request
    }
    pub const fn edge_visits(&self) -> u32 {
        self.checked.edge_visits
    }
    pub const fn input_basis(&self) -> RelationInputBasis {
        RelationInputBasis::Given
    }
    pub const fn independence(&self) -> RelationIndependence {
        RelationIndependence::SharedBasisUnassessed
    }
}

/// Consume the checked query as an inspectable path of actual immutable edges.
/// No future lowerer may treat this path as native solver or condition evidence.
pub fn consume_dependency_path(
    checked: CheckedRelationQuery<'_>,
) -> Result<DependencyPath<'_>, RelationConsumerHold> {
    let nodes = &checked.nodes[..usize::from(checked.length)];
    if nodes.len() < 3
        || nodes.first() != Some(&checked.request.relation.from)
        || nodes.last() != Some(&checked.request.relation.to)
        || nodes.windows(2).any(|pair| {
            !checked.state.basis().edges().contains(&RelationEdit {
                from: pair[0],
                to: pair[1],
            })
        })
    {
        return Err(RelationConsumerHold::WitnessMismatch);
    }
    Ok(DependencyPath { checked })
}
