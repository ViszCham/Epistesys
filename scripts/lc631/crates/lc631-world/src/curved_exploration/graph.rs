//! Bounded observed-transition graph, independent of geometry and semantic scoring.
#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct NodeId(pub u64);

/// Declared query-slot cost, separate from symmetric geometric distance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectedTransitionCost(u64);

impl DirectedTransitionCost {
    pub const fn query_slots(value: u64) -> Self {
        Self(value)
    }
    pub const fn slots(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReportedTransition {
    pub from: NodeId,
    pub to: NodeId,
    pub cost: DirectedTransitionCost,
    pub evidence_payload_digest: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GraphError {
    EmptyGraph,
    LimitExceeded,
    DuplicateNode,
    DuplicateEdge,
    UnknownNode,
    MissingEvidence,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BaselineDistance {
    ObservedHops(u32),
    UnknownNoObservedPath,
}

/// Single-owner data; evidence digests are reported references, not authentication.
pub struct TransitionGraph {
    nodes: BTreeSet<NodeId>,
    outgoing: BTreeMap<NodeId, Vec<ReportedTransition>>,
}

impl TransitionGraph {
    pub fn new(nodes: &[NodeId], edges: &[ReportedTransition]) -> Result<Self, GraphError> {
        if nodes.is_empty() {
            return Err(GraphError::EmptyGraph);
        }
        if nodes.len() > 2048 || edges.len() > 16384 {
            return Err(GraphError::LimitExceeded);
        }
        let node_set: BTreeSet<_> = nodes.iter().copied().collect();
        if node_set.len() != nodes.len() {
            return Err(GraphError::DuplicateNode);
        }
        let mut seen = BTreeSet::new();
        let mut outgoing: BTreeMap<NodeId, Vec<ReportedTransition>> = BTreeMap::new();
        for edge in edges {
            if !node_set.contains(&edge.from) || !node_set.contains(&edge.to) {
                return Err(GraphError::UnknownNode);
            }
            if edge.evidence_payload_digest == [0; 32] {
                return Err(GraphError::MissingEvidence);
            }
            if !seen.insert((edge.from, edge.to)) {
                return Err(GraphError::DuplicateEdge);
            }
            outgoing.entry(edge.from).or_default().push(*edge);
        }
        Ok(Self {
            nodes: node_set,
            outgoing,
        })
    }

    pub fn baseline_hops(&self, from: NodeId, to: NodeId) -> Result<BaselineDistance, GraphError> {
        if !self.nodes.contains(&from) || !self.nodes.contains(&to) {
            return Err(GraphError::UnknownNode);
        }
        let mut queue = VecDeque::from([(from, 0u32)]);
        let mut visited = BTreeSet::from([from]);
        while let Some((node, hops)) = queue.pop_front() {
            if node == to {
                return Ok(BaselineDistance::ObservedHops(hops));
            }
            if let Some(edges) = self.outgoing.get(&node) {
                for edge in edges {
                    if visited.insert(edge.to) {
                        queue.push_back((edge.to, hops + 1));
                    }
                }
            }
        }
        Ok(BaselineDistance::UnknownNoObservedPath)
    }

    pub fn directed_cost(
        &self,
        from: NodeId,
        to: NodeId,
    ) -> Result<Option<DirectedTransitionCost>, GraphError> {
        if !self.nodes.contains(&from) || !self.nodes.contains(&to) {
            return Err(GraphError::UnknownNode);
        }
        Ok(self
            .outgoing
            .get(&from)
            .and_then(|edges| edges.iter().find(|e| e.to == to))
            .map(|e| e.cost))
    }
}
