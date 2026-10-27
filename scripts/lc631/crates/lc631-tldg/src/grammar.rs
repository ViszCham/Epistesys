use crate::{GrammarProfileId, SourceRevision};
use lc631_core::SourceSpan;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenKind {
    Word,
    Number,
    Whitespace,
    Operator,
    Delimiter,
    Quote,
    Symbol,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Token {
    pub id: u32,
    pub span: SourceSpan,
    pub kind: TokenKind,
    pub surface: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct TokenLattice {
    pub tokens: Vec<Token>,
    pub competing_segmentations: Vec<Vec<u32>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RegionCandidate {
    pub span: SourceSpan,
    pub profile: GrammarProfileId,
    pub evidence: String,
    pub embedded: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct RegionLattice {
    pub candidates: Vec<RegionCandidate>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GrammarProduction {
    pub id: u32,
    pub name: String,
    pub profile: GrammarProfileId,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct UnifiedGrammarIr {
    pub productions: Vec<GrammarProduction>,
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct SyntaxNodeId(pub u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SyntaxNodeKind {
    Document,
    Token,
    Group,
    Error,
    Implicit,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SyntaxNode {
    pub id: SyntaxNodeId,
    pub spans: Vec<SourceSpan>,
    pub kind: SyntaxNodeKind,
    pub profile_candidates: Vec<GrammarProfileId>,
    pub origin: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationKind {
    Contains,
    Sequence,
    Scope,
    BindingCandidate,
    DependencyCandidate,
    EmbeddedIn,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationState {
    Verified,
    Candidate,
    Unknown,
    Incomparable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SyntaxRelation {
    pub from: SyntaxNodeId,
    pub to: SyntaxNodeId,
    pub kind: RelationKind,
    pub state: RelationState,
    pub evidence: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PackedDerivation {
    pub id: u32,
    pub root: SyntaxNodeId,
    pub production_ids: Vec<u32>,
    pub status: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintKind {
    Binding,
    Scope,
    Dependency,
    TypeOrRole,
    EffectOrSpeechAct,
    Temporal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintState {
    Verified,
    Candidate,
    Unknown,
    Incomparable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ConstraintEdge {
    pub from: SyntaxNodeId,
    pub to: SyntaxNodeId,
    pub kind: ConstraintKind,
    pub state: ConstraintState,
    pub source: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct TypedConstraintGraph {
    pub edges: Vec<ConstraintEdge>,
    /// Explicit cross-graph correspondence; token ordinals are not syntax IDs.
    pub token_bindings: Vec<TokenSyntaxBinding>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TokenSyntaxBinding {
    pub token_id: u32,
    pub syntax_node: SyntaxNodeId,
    pub source_span: SourceSpan,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum ConstraintBindingError {
    DuplicateIdentity,
    BudgetExceeded,
    IncompleteMapping,
    InvalidSourceSpan,
    UnknownSyntaxNode,
    EndpointMismatch,
}

impl TypedConstraintGraph {
    pub fn validate_source_bindings(
        &self,
        source: &str,
        tokens: &TokenLattice,
        syntax: &UnifiedSyntaxHypergraph,
    ) -> Result<(), ConstraintBindingError> {
        let node_budget = crate::Dg1Budget::default().max_syntax_nodes;
        if tokens.tokens.len() > node_budget
            || syntax.nodes.len() > node_budget
            || syntax.relations.len() > node_budget * 2
        {
            return Err(ConstraintBindingError::BudgetExceeded);
        }
        let tokens_by_id = tokens
            .tokens
            .iter()
            .map(|token| (token.id, token))
            .collect::<BTreeMap<_, _>>();
        let nodes_by_id = syntax
            .nodes
            .iter()
            .map(|node| (node.id, node))
            .collect::<BTreeMap<_, _>>();
        let bindings_by_token = self
            .token_bindings
            .iter()
            .map(|binding| (binding.token_id, binding))
            .collect::<BTreeMap<_, _>>();
        let mapped_nodes = self
            .token_bindings
            .iter()
            .map(|binding| binding.syntax_node)
            .collect::<BTreeSet<_>>();
        if tokens_by_id.len() != tokens.tokens.len()
            || nodes_by_id.len() != syntax.nodes.len()
            || bindings_by_token.len() != self.token_bindings.len()
            || mapped_nodes.len() != self.token_bindings.len()
        {
            return Err(ConstraintBindingError::DuplicateIdentity);
        }
        if syntax.relations.iter().any(|edge| {
            !nodes_by_id.contains_key(&edge.from) || !nodes_by_id.contains_key(&edge.to)
        }) {
            return Err(ConstraintBindingError::UnknownSyntaxNode);
        }
        if syntax
            .nodes
            .iter()
            .flat_map(|node| &node.spans)
            .any(|span| source.get(span.start..span.end).is_none())
        {
            return Err(ConstraintBindingError::InvalidSourceSpan);
        }
        if self.token_bindings.len() != tokens.tokens.len()
            || self.edges.len() != tokens.tokens.len().saturating_sub(1)
        {
            return Err(ConstraintBindingError::IncompleteMapping);
        }
        let mut source_cursor = 0;
        for token in &tokens.tokens {
            let binding = bindings_by_token
                .get(&token.id)
                .ok_or(ConstraintBindingError::IncompleteMapping)?;
            let node = nodes_by_id
                .get(&binding.syntax_node)
                .ok_or(ConstraintBindingError::UnknownSyntaxNode)?;
            if token.span.start != source_cursor
                || token.span.end <= token.span.start
                || token.span != binding.source_span
                || source.get(token.span.start..token.span.end) != Some(token.surface.as_str())
                || node.kind != SyntaxNodeKind::Token
                || node.spans.as_slice() != [token.span]
                || node.origin != "source_anchored_surface_token"
            {
                return Err(ConstraintBindingError::InvalidSourceSpan);
            }
            source_cursor = token.span.end;
        }
        // Generic parser derivations claim a full lexical projection. The
        // separate same-AST distillation profile has no lexical derivation and
        // intentionally carries an empty token lattice instead.
        if (!syntax.derivations.is_empty() || !tokens.tokens.is_empty())
            && source_cursor != source.len()
        {
            return Err(ConstraintBindingError::IncompleteMapping);
        }
        for (pair, edge) in tokens.tokens.windows(2).zip(&self.edges) {
            if edge.from != bindings_by_token[&pair[0].id].syntax_node
                || edge.to != bindings_by_token[&pair[1].id].syntax_node
                || edge.state == ConstraintState::Verified
            {
                return Err(ConstraintBindingError::EndpointMismatch);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum ParseDefect {
    AmbiguousRegionBoundary(SourceSpan),
    CompetingDerivations,
    ErrorRecoveryInsertedNode(SourceSpan),
    UnmatchedDelimiter(SourceSpan),
    UnknownRegion(SourceSpan),
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct UnifiedSyntaxHypergraph {
    pub nodes: Vec<SyntaxNode>,
    pub relations: Vec<SyntaxRelation>,
    pub derivations: Vec<PackedDerivation>,
    pub defects: Vec<ParseDefect>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DeepGrammarArtifact {
    pub source: SourceRevision,
    pub boundary_roundtrip: String,
    pub regions: RegionLattice,
    pub tokens: TokenLattice,
    pub grammar: UnifiedGrammarIr,
    pub syntax: UnifiedSyntaxHypergraph,
    pub constraints: TypedConstraintGraph,
    pub claim_boundary: String,
}
