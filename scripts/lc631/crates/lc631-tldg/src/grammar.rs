use crate::{GrammarProfileId, SourceRevision};
use lc631_core::SourceSpan;
use serde::Serialize;

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
