//! 日本語: 再帰型を使わず、子が前にあるbounded DAGを検査する。
//! English: Check a bounded child-before-parent DAG without recursive types.
use super::TemporalError;
use lc631_substrate::context_identity::ContextRevision;
use lc631_substrate::Digest32;
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TemporalProfile {
    ClosedFiniteKleeneV1,
    InfiniteLtl,
}

impl TemporalProfile {
    pub fn digest(self) -> Digest32 {
        let mut hash = Sha256::new();
        hash.update(b"epistesys.temporal-profile.v1\0");
        match self {
            Self::ClosedFiniteKleeneV1 => hash.update(b"closed;kleene;strong-next;strong-until;instant-nanoseconds-exact;steps=256;atoms=256;nodes=1024"),
            Self::InfiniteLtl => hash.update(b"infinite-ltl-unsupported"),
        }
        Digest32::from_bytes(hash.finalize().into())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TemporalNode {
    Constant(bool),
    Atom(u16),
    Not(u16),
    And(u16, u16),
    Or(u16, u16),
    StrongNext(u16),
    Eventually(u16),
    Always(u16),
    Until { left: u16, right: u16 },
}

impl TemporalNode {
    fn encoding(self) -> (u8, u16, u16, usize) {
        match self {
            Self::Constant(v) => (0, u16::from(v), 0, 0),
            Self::Atom(v) => (1, v, 0, 0),
            Self::Not(a) => (2, a, 0, 1),
            Self::And(a, b) => (3, a, b, 2),
            Self::Or(a, b) => (4, a, b, 2),
            Self::StrongNext(a) => (5, a, 0, 1),
            Self::Eventually(a) => (6, a, 0, 1),
            Self::Always(a) => (7, a, 0, 1),
            Self::Until { left, right } => (8, left, right, 2),
        }
    }
}

/// A structural query, not a source-language interpretation or execution permit.
#[derive(Debug)]
pub struct TemporalQuery {
    context: ContextRevision,
    nodes: Vec<TemporalNode>,
    root: u16,
    digest: Digest32,
}

impl TemporalQuery {
    pub fn checked(
        context: ContextRevision,
        nodes: Vec<TemporalNode>,
        root: u16,
        profile: TemporalProfile,
    ) -> Result<Self, TemporalError> {
        match profile {
            TemporalProfile::ClosedFiniteKleeneV1 => {}
            TemporalProfile::InfiniteLtl => return Err(TemporalError::UnsupportedInfiniteProfile),
        }
        if nodes.is_empty() {
            return Err(TemporalError::EmptyFormula);
        }
        if nodes.len() > 1024 {
            return Err(TemporalError::FormulaLimitExceeded);
        }
        if usize::from(root) >= nodes.len() {
            return Err(TemporalError::InvalidNodeReference);
        }
        let mut hash = Sha256::new();
        hash.update(b"epistesys.temporal-query.closed-kleene.v1\0");
        hash.update(context.source().digest().as_bytes());
        hash.update(context.interpretation_digest().as_bytes());
        hash.update(profile.digest().as_bytes());
        hash.update(root.to_le_bytes());
        hash.update((nodes.len() as u16).to_le_bytes());
        for (index, node) in nodes.iter().enumerate() {
            let (tag, a, b, children) = node.encoding();
            if (children >= 1 && usize::from(a) >= index)
                || (children == 2 && usize::from(b) >= index)
            {
                return Err(TemporalError::InvalidNodeReference);
            }
            if let TemporalNode::Atom(atom) = node {
                if *atom >= 256 {
                    return Err(TemporalError::AtomLimitExceeded);
                }
            }
            hash.update([tag]);
            hash.update(a.to_le_bytes());
            hash.update(b.to_le_bytes());
        }
        Ok(Self {
            context,
            nodes,
            root,
            digest: Digest32::from_bytes(hash.finalize().into()),
        })
    }
    pub const fn context(&self) -> ContextRevision {
        self.context
    }
    pub const fn digest(&self) -> Digest32 {
        self.digest
    }
    pub const fn profile(&self) -> TemporalProfile {
        TemporalProfile::ClosedFiniteKleeneV1
    }
    pub const fn root(&self) -> u16 {
        self.root
    }
    pub fn nodes(&self) -> &[TemporalNode] {
        &self.nodes
    }
}
