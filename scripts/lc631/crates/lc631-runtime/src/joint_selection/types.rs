use lc631_analysis::creativity_measurement::RationalInterval;
use lc631_core::context::{ContextRevision, ContextScope, SourceRevision};
use lc631_core::CandidateId;
use lc631_substrate::Digest32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AxisKind {
    Goal(Digest32),
    ReconstructionLoss,
    SemanticLoss,
    TemporalLoss,
    SpatialLoss,
    CoverageGain,
    CompressionGain,
    BridgeGain,
    PredictionGain,
    Preserve,
    Rebind,
    Reframe,
    ExplainGain,
    DesignGain,
    TransferGain,
    SolveGain,
    RedundancyCost,
    BreakCost,
    CounterexampleCost,
    MigrationCost,
}

impl AxisKind {
    pub const fn maximize(self) -> bool {
        match self {
            Self::Goal(_)
            | Self::CoverageGain
            | Self::CompressionGain
            | Self::BridgeGain
            | Self::PredictionGain
            | Self::Preserve
            | Self::Rebind
            | Self::Reframe
            | Self::ExplainGain
            | Self::DesignGain
            | Self::TransferGain
            | Self::SolveGain => true,
            Self::ReconstructionLoss
            | Self::SemanticLoss
            | Self::TemporalLoss
            | Self::SpatialLoss
            | Self::RedundancyCost
            | Self::BreakCost
            | Self::CounterexampleCost
            | Self::MigrationCost => false,
        }
    }
}

/// Fixed registry references supplied by the caller; digests do not verify payloads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectionPins {
    pub source: SourceRevision,
    pub scope: ContextScope,
    pub baseline: Digest32,
    pub hard_invariants: Digest32,
    pub policy: Digest32,
    pub goal_profile: Option<Digest32>,
    pub evidence_revision: Digest32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReportedGate {
    Pass,
    Fail,
    NeedsEvidence,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MissingAxis {
    Unknown,
    NotApplicable,
    Incomparable,
    Undefined,
    InvalidConfig,
    ResourceExhausted,
    Unsupported,
    Timeout,
    Conflict,
    Uncalibrated,
    MissingEvaluator,
}

/// Arithmetic exactness and semantic checking are deliberately separate.
pub enum AxisValue<'a> {
    Exact(&'a RationalInterval),
    Bounded(&'a RationalInterval),
    Estimated(&'a RationalInterval),
    Unavailable(MissingAxis),
}

impl AxisValue<'_> {
    pub fn interval(&self) -> Option<&RationalInterval> {
        match self {
            Self::Exact(value) | Self::Bounded(value) | Self::Estimated(value) => Some(value),
            Self::Unavailable(_) => None,
        }
    }
}

/// All borrowed payloads remain available for the caller's four-facet ledger.
pub struct JointCandidate<'a> {
    pub id: CandidateId,
    pub context: ContextRevision,
    pub pins: SelectionPins,
    pub hard_gate: ReportedGate,
    pub map_back: ReportedGate,
    pub axes: &'a [AxisValue<'a>],
    pub total_loss: AxisValue<'a>,
    /// Full caller-reported canonical typed content, obligations, policy/evidence
    /// lineage bytes; exclude opaque IDs/order/timestamps. Not a hash surrogate.
    pub canonical_key: &'a [u8],
    /// Distinct observation metadata and D/Delta/U/G bindings are retained by ref.
    pub measurement_binding: Digest32,
}

pub struct SelectionPolicy<'a> {
    pub pins: SelectionPins,
    pub axes: &'a [AxisKind],
    /// Explicit permutation for multiple goals. None permits only one goal.
    pub representative_order: Option<&'a [usize]>,
    /// Caller-declared overlap when Goal and Solve share an observation primitive.
    pub declared_goal_solve_overlap: bool,
    pub max_candidates: usize,
    pub max_canonical_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchCoverage {
    EvaluatedSet,
    BudgetExhausted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PairRelation {
    LeftDominates,
    RightDominates,
    EquivalentInScope,
    SameBounds,
    Unresolved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Disposition {
    Frontier,
    Dominated { by: usize },
    HardFailure,
    MapBackFailure,
    NeedsEvidence,
    OtherScope,
    Duplicate { representative: usize },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecommendationState {
    OperationalRepresentative,
    NoGoalProfile,
    NoGoalObservation,
    MissingRepresentativePolicy,
    NeedsEvidence,
    NoEligibleCandidate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectionOptimality {
    BestAmongEvaluated,
    BudgetLimited,
    Incomparable,
}

pub struct JointSelectionReport<'a, 'v> {
    pub pins: SelectionPins,
    pub candidates: &'a [JointCandidate<'v>],
    /// One record per input, including aliases, pending and other-scope rows.
    pub dispositions: Vec<Disposition>,
    pub frontier: Vec<usize>,
    pub recommendation: Option<usize>,
    /// Every other input is retained, including dominated/rejected/alias rows.
    pub alternatives: Vec<usize>,
    pub recommendation_state: RecommendationState,
    pub optimality: SelectionOptimality,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectionError {
    InvalidLimits,
    EmptyCandidates,
    ResourceExhausted,
    DuplicateCandidateId,
    InvalidAxes,
    InvalidRepresentativeOrder,
    MissingCanonicalKey,
    AxisCountMismatch,
    InexactExactLabel,
    OutOfRange,
    AllocationFailed,
}
