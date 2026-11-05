//! Native declarations and orthogonal states, never success/report constructors.
use super::*;
#[derive(Clone, Copy)]
pub struct AtomMap {
    pub local: BoolSymbolId,
    pub meaning: u8,
    pub grounding: SourceSpan,
}
pub struct FactFeature<'a> {
    pub formula: &'a BoolExpr,
    pub weight: &'a Rational,
}
pub struct ConstructionFeature<'a> {
    pub formula: &'a BoolExpr,
    pub weight: &'a Rational,
}
pub struct ArchiveEntry<'a> {
    pub source: &'a SourceStore,
    pub context: ContextRevision,
    pub query: &'a SmtStatusQuery,
}
#[derive(Clone, Copy)]
pub enum ArchiveInput<'a> {
    Read(&'a [ArchiveEntry<'a>]),
    Unread,
    Unavailable,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArchiveStatus {
    Complete,
    Empty,
    Unread,
    Unavailable,
    Unsupported,
    Partial,
}
#[derive(Clone, Copy)]
pub struct Limits {
    pub max_atoms: u8,
    pub max_assertions: usize,
    pub max_features: usize,
    pub max_archive_entries: usize,
    pub max_ast_nodes: usize,
    pub max_depth: usize,
    pub max_payload_bytes: usize,
    pub max_call_work: u64,
    pub max_total_work: u64,
    pub max_archive_comparisons: usize,
    pub max_weight_bits: u32,
    pub max_intermediate_bits: u32,
    pub retained_ram: u64,
    pub scratch_ram: u64,
    pub output_bytes: u64,
}
pub struct RootInput<'a> {
    pub source: &'a SourceStore,
    pub original: OriginalContext,
    pub source_query: &'a SmtStatusQuery,
    pub premises: &'a [GivenPremise<'a>],
    pub target: Digest32,
    pub goal: GoalPolicy<'a>,
    pub bridge_limits: BridgeLimits,
}
pub struct Declaration<'a> {
    pub atoms: &'a [AtomMap],
    pub facts: &'a [FactFeature<'a>],
    pub constructions: &'a [ConstructionFeature<'a>],
    pub archive: ArchiveInput<'a>,
    pub corpus_revision: Digest32,
    pub oracle_revision: Digest32,
    pub policy_revision: Digest32,
    pub limits: Limits,
}
#[derive(Clone, Copy)]
pub struct ExpectedInputs<'s, 'a, 'v> {
    pub root: &'s RootPlan<'a, 'v>,
    pub source: &'a SourceStore,
    pub source_query: &'a SmtStatusQuery,
    pub candidate: &'s ValidatedFormalCandidate<'a, 'v>,
    pub candidate_ast: &'s SmtStatusQuery,
    pub atoms: &'a [AtomMap],
    pub facts: &'a [FactFeature<'a>],
    pub constructions: &'a [ConstructionFeature<'a>],
    pub archive: ArchiveInput<'a>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Counts {
    pub archive_comparisons: usize,
    pub assignment_evaluations: u64,
    pub work_debited: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PendingExit {
    NonzeroFramework,
    CrossFrameworkSemantic,
    CrossFrameworkCompatibility,
    NormativeTnc,
    OriginalScoreSelection,
}
pub enum OriginalStatus<'a> {
    NoScalar { missing: &'a [ComponentMetric] },
}
#[derive(Debug)]
pub enum Error {
    Resource(ResourceError),
    Creative(CreativeError),
    Arithmetic(MeasurementError),
    ForeignOwner,
    ForeignReservation,
    ExpectedInputMismatch,
    SourceMismatch,
    InvalidMap,
    InvalidWeight,
    DuplicateFeature,
    UniverseMismatch,
    InvalidLimits,
    UnsupportedSignature,
    HardInvariant,
    ContextInconsistent,
    WorkBudget,
    PayloadBudget,
}
