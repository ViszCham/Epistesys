//! Bounded, non-semantic S-expression syntax parser.
//!
//! The parser preserves source bytes/spans and tree shape only. It does not
//! interpret SMT-LIB commands, models, proofs, counterexamples, or truth.

mod parser;
pub use parser::parse;
pub(crate) use parser::parse_preallocated;

/// Bytes charged for simultaneously live parser arrays; excludes allocator headers.
pub(crate) fn scratch_bytes(limits: SExprLimits) -> Option<u64> {
    let bytes = std::mem::size_of::<SExprNode>()
        .checked_mul(limits.max_nodes)?
        .checked_add(std::mem::size_of::<NodeId>().checked_mul(limits.max_nodes)?)?
        .checked_add(std::mem::size_of::<ListFrame>().checked_mul(limits.max_depth)?)?;
    u64::try_from(bytes).ok()
}

const HARD_MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const HARD_MAX_NODES: usize = 200_000;
const HARD_MAX_DEPTH: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SExprLimits {
    max_input_bytes: usize,
    max_nodes: usize,
    max_depth: usize,
}

impl SExprLimits {
    pub fn checked(
        max_input_bytes: usize,
        max_nodes: usize,
        max_depth: usize,
    ) -> Result<Self, SExprError> {
        if max_input_bytes == 0
            || max_input_bytes > HARD_MAX_INPUT_BYTES
            || max_nodes == 0
            || max_nodes > HARD_MAX_NODES
            || max_depth == 0
            || max_depth > HARD_MAX_DEPTH
        {
            return Err(SExprError::InvalidLimits);
        }
        Ok(Self {
            max_input_bytes,
            max_nodes,
            max_depth,
        })
    }

    pub const fn max_input_bytes(self) -> usize {
        self.max_input_bytes
    }

    pub const fn max_nodes(self) -> usize {
        self.max_nodes
    }

    pub const fn max_depth(self) -> usize {
        self.max_depth
    }
}

impl Default for SExprLimits {
    fn default() -> Self {
        Self {
            max_input_bytes: 2 * 1024 * 1024,
            max_nodes: 65_536,
            max_depth: 128,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct NodeId(usize);

impl NodeId {
    pub const fn index(self) -> usize {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ByteSpan {
    start: usize,
    end: usize,
}

impl ByteSpan {
    const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub const fn start(self) -> usize {
        self.start
    }

    pub const fn end(self) -> usize {
        self.end
    }

    pub const fn len(self) -> usize {
        self.end - self.start
    }

    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SExprKind {
    List,
    Atom,
    StringLiteral,
    QuotedSymbol,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SExprNode {
    id: NodeId,
    kind: SExprKind,
    span: ByteSpan,
    content_span: Option<ByteSpan>,
    parent: Option<NodeId>,
    first_child: Option<NodeId>,
    next_sibling: Option<NodeId>,
}

impl SExprNode {
    pub const fn id(self) -> NodeId {
        self.id
    }

    pub const fn kind(self) -> SExprKind {
        self.kind
    }

    pub const fn span(self) -> ByteSpan {
        self.span
    }

    /// Raw interior span for strings and quoted symbols; atom content is the
    /// full token. A list has no single content span.
    pub const fn content_span(self) -> Option<ByteSpan> {
        self.content_span
    }

    pub const fn parent(self) -> Option<NodeId> {
        self.parent
    }

    pub const fn first_child(self) -> Option<NodeId> {
        self.first_child
    }

    pub const fn next_sibling(self) -> Option<NodeId> {
        self.next_sibling
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SExprError {
    InvalidLimits,
    InputTooLarge { actual: usize, limit: usize },
    InvalidUtf8 { valid_up_to: usize },
    NodeLimitExceeded,
    DepthLimitExceeded,
    AllocationFailed,
    UnexpectedClose { offset: usize },
    UnclosedList { offset: usize },
    UnclosedString { offset: usize },
    UnclosedQuotedSymbol { offset: usize },
    InvalidQuotedSymbol { offset: usize },
    MissingSeparator { offset: usize },
    InvalidToken { offset: usize },
}

/// Borrowed S-expression syntax tree with exact byte spans into the original
/// UTF-8 input. Roots may contain multiple top-level forms.
pub struct SExprDocument<'source> {
    source: &'source [u8],
    nodes: Vec<SExprNode>,
    roots: Vec<NodeId>,
}

impl<'source> SExprDocument<'source> {
    pub fn source_bytes(&self) -> &'source [u8] {
        self.source
    }

    pub fn roots(&self) -> &[NodeId] {
        &self.roots
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn node(&self, id: NodeId) -> Option<&SExprNode> {
        self.nodes.get(id.0)
    }

    pub fn raw(&self, id: NodeId) -> Option<&'source [u8]> {
        self.node(id).and_then(|node| self.raw_span(node.span))
    }

    pub fn raw_span(&self, span: ByteSpan) -> Option<&'source [u8]> {
        self.source.get(span.start..span.end)
    }

    /// List children in source order. Non-list nodes return `None`.
    pub fn children(&self, list: NodeId) -> Option<SExprChildren<'_, 'source>> {
        let node = self.node(list)?;
        if node.kind != SExprKind::List {
            return None;
        }
        Some(SExprChildren {
            document: self,
            next: node.first_child,
        })
    }
}

pub struct SExprChildren<'document, 'source> {
    document: &'document SExprDocument<'source>,
    next: Option<NodeId>,
}

impl Iterator for SExprChildren<'_, '_> {
    type Item = NodeId;

    fn next(&mut self) -> Option<Self::Item> {
        let id = self.next?;
        self.next = self.document.node(id)?.next_sibling;
        Some(id)
    }
}

#[derive(Clone, Copy)]
struct ListFrame {
    list: NodeId,
    last_child: Option<NodeId>,
}
