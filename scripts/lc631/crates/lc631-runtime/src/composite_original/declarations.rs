//! 日本語: Given入力のみ。宣言からsource意味/適用/保存証拠を作らない。
//! English: Given inputs only; declarations create no semantic applicability or preservation evidence.
use lc631_analysis::creativity_measurement::ComponentMetric;
use lc631_substrate::{
    resource::ResourceError,
    source::{SourceSpan, SourceStore},
    Digest32,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Coordinate {
    Hypothesis,
    Product,
    BoolPort,
    Graph,
    Units,
    RolesFrames,
    Certificates,
    Forecasts,
    Schedule,
    Transport,
    Coding,
    Conversion,
    WholeComposite,
}
pub(super) const PHYSICAL: [Coordinate; 12] = [
    Coordinate::Hypothesis,
    Coordinate::Product,
    Coordinate::BoolPort,
    Coordinate::Graph,
    Coordinate::Units,
    Coordinate::RolesFrames,
    Coordinate::Certificates,
    Coordinate::Forecasts,
    Coordinate::Schedule,
    Coordinate::Transport,
    Coordinate::Coding,
    Coordinate::Conversion,
];
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoordinateDeclaration {
    pub coordinate: Coordinate,
    pub schema: Digest32,
}
#[derive(Clone, Copy)]
pub struct GivenCoordinate<'i> {
    pub coordinate: Coordinate,
    pub schema: Digest32,
    pub payload: &'i [u8],
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodingBaselineProfile {
    AlwaysNoneFourV1,
    WrappingAddFourV1,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApplicabilityReason {
    UnregisteredRootRelation,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApplicabilityClaim<'i> {
    GivenConditional(&'i [u8]),
    Unsupported(ApplicabilityReason),
}
#[derive(Clone, Copy)]
pub struct LeafApplicability<'i> {
    pub metric: ComponentMetric,
    pub coordinate: Coordinate,
    pub root_goal: Digest32,
    pub regime: Digest32,
    pub claim: ApplicabilityClaim<'i>,
}
#[derive(Clone, Copy)]
pub struct RootSpec<'i> {
    pub source: Option<&'i SourceStore>,
    pub question: Option<SourceSpan>,
    pub comparison: Digest32,
    pub root_goal: Digest32,
    pub baseline: Digest32,
    pub baseline_profile: CodingBaselineProfile,
    pub full_cost_profile: Digest32,
    pub total_tl_policy: Digest32,
    pub coordinates: &'i [CoordinateDeclaration],
    pub applicability: &'i [LeafApplicability<'i>],
}
#[derive(Clone, Copy)]
pub struct CandidateSpec<'i> {
    pub coordinates: &'i [GivenCoordinate<'i>],
    pub baseline_profile: CodingBaselineProfile,
}
#[derive(Clone, Copy)]
pub struct ExpectedPending<'c, 'r, 'i, 'v> {
    pub root: &'r super::ConditionalRootPlan<'i, 'v>,
    pub candidate: &'c super::ConditionalCandidate<'r, 'i, 'v>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConditionalProvenance {
    GivenUnvalidated,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CausalValidation {
    Unassessed,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HardStatus {
    NeedsEvidence,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MissingDependency {
    FullCompositeCost,
    CompositeTotalTl,
    ActualHardEvidence,
    NativeOwnerCorrespondence,
    SemanticApplicability,
}
#[derive(Debug)]
pub enum Error {
    SourceMissing,
    MissingApplicability(ComponentMetric),
    ApplicabilityGoalMismatch(ComponentMetric),
    MissingCoordinate(Coordinate),
    CoordinateSchemaMismatch(Coordinate),
    BaselineProfileMismatch,
    ForeignRoot,
    ForeignCandidate,
    ForeignReservation,
    Resource(ResourceError),
    InvalidDeclaration,
    DuplicateCoordinate,
    DuplicateApplicability,
}
