//! Raw Given inputs and explicit nonnumeric causes; checked tokens are elsewhere.
use crate::{
    contextual_distillation::{consumer::DependencyPath, ContextualDistillation},
    joint_selection::SelectionPins,
    structural_gain_measurement::{ClosedGraph, Edge},
};
use lc631_core::context::*;
use lc631_substrate::{
    measurement_numeric::NumericError, resource::ResourceError, source::SourceStore, Digest32,
};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameProfile {
    DirectEdgeV1,
    CheckedPathMacroV1,
    Unregistered(u16),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OracleId {
    ExpandOneOuterStepToBasePathV1,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RootGoal {
    Reachability(Edge),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceBasis {
    Given,
    Unobserved,
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProbeId(pub u16);
#[derive(Clone, Copy)]
pub struct MacroInput<'i, 's> {
    pub edge: Edge,
    pub path: Option<&'i DependencyPath<'s>>,
}
#[derive(Clone, Copy)]
pub struct FrameSpec<'i, 's> {
    pub profile: FrameProfile,
    pub declaration: &'i str,
    pub macros: &'i [MacroInput<'i, 's>],
}
#[derive(Clone, Copy)]
pub struct PerspectiveQuery {
    pub id: ProbeId,
    pub edge: Edge,
    pub weight: u32,
}
#[derive(Clone, Copy)]
pub struct QueryText<'i> {
    pub query: ProbeId,
    pub text: &'i str,
}
#[derive(Clone, Copy)]
pub struct ExactRootInput<'i, 's, 'v> {
    pub source: &'i SourceStore,
    pub original: &'i OriginalContext,
    pub base: &'i ClosedGraph<'i>,
    pub family: &'i super::FrameFamily<'i, 'v>,
    pub state: &'s ContextualDistillation,
    pub root_goal: &'i RootGoal,
    pub frame_premise: Digest32,
    pub baseline_frame: &'i FrameSpec<'i, 's>,
    pub baseline_terms: &'i [QueryText<'i>],
    pub perspectives: &'i [PerspectiveQuery],
    pub pins: SelectionPins,
    pub source_basis: SourceBasis,
}
#[derive(Clone, Copy)]
pub struct FrameTransitionInput<'i, 's> {
    pub parent: &'i ContextState,
    pub child: &'i ContextState,
    pub delta: &'i ContextDelta,
    pub frame: &'i FrameSpec<'i, 's>,
    pub state: &'s ContextualDistillation,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PerspectiveOutcome {
    KnownPass,
    KnownFailure,
    Unknown,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stage {
    Parse,
    Lower,
    FrameEvaluate,
    PaidMap,
    RootOracle,
    Source,
    CurrentResource,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Cause {
    NoDirectBaseEdge,
    NoMacroEntry,
    WorkExhausted,
    SourceUnobserved,
    UnsupportedProfile,
    ForeignPlan,
    RootMismatch,
    InvalidDerivation,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParseCause {
    OutOfFrameLanguage,
    NonCanonical,
    InvalidTerm,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParseRule {
    DirectEdgeAtomV1,
    CheckedMacroCallV1,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LowerRule {
    DirectEdgeReferenceV1,
    CheckedPathReferenceV1,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvalRule {
    DirectEdgeV1,
    CheckedPathMacroV1,
}
#[derive(Clone, Copy)]
pub struct CauseRecord {
    pub(super) query: ProbeId,
    pub(super) stage: Stage,
    pub(super) cause: Cause,
}
impl CauseRecord {
    pub fn query(&self) -> ProbeId {
        self.query
    }
    pub fn stage(&self) -> Stage {
        self.stage
    }
    pub fn cause(&self) -> Cause {
        self.cause
    }
}
#[derive(Debug)]
pub enum Error {
    Context(ContextError),
    Resource(ResourceError),
    Numeric(NumericError),
    Relation(crate::contextual_distillation::consumer::RelationConsumerHold),
    Selection(crate::joint_selection::SelectionError),
    Parse(ParseCause),
    NeedsEvidence(Cause),
    Unsupported(Cause),
    NoComparable(Cause),
    FramePayloadMismatch,
    FrameDeltaMismatch,
    ForeignDeltaOwner,
    ForeignSourceOwner,
    ForeignPathState,
    MissingPathOwner,
    MapBackEndpoints,
    MissingPaidMapOwner,
    InvalidProbeSet,
    InvalidWeight,
    InvalidLimits,
    InvalidRoot,
    AllocationFailed,
    ArithmeticOverflow,
    PayloadLimit,
    ForeignReservation,
    ForeignReport,
    ForeignInterval,
    ForeignOwner,
    WorkBudget,
}
impl From<ResourceError> for Error {
    fn from(e: ResourceError) -> Self {
        Self::Resource(e)
    }
}
impl From<NumericError> for Error {
    fn from(e: NumericError) -> Self {
        Self::Numeric(e)
    }
}
impl From<crate::joint_selection::SelectionError> for Error {
    fn from(e: crate::joint_selection::SelectionError) -> Self {
        Self::Selection(e)
    }
}
