use super::{Digest32, Rational, RationalInterval};
use crate::joint_selection::{SelectionError, SelectionPins};
use lc631_core::{context::ContextRevision, CandidateId};
use lc631_solvers::typed_query::{BoolExpr, SmtStatusQuery};
use lc631_substrate::resource::ResourceError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DatasetRole {
    Training,
    Search,
    ExplanationValidation,
    TransferValidation,
    FinalResearchHoldout,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StepId(pub u16);
#[derive(Clone, Copy, Debug)]
pub enum Rule<'i> {
    Premise { index: u8 },
    AndElim { prior: StepId, child: u8 },
    AndIntro { priors: &'i [StepId] },
}
#[derive(Clone, Copy, Debug)]
pub struct Step<'i> {
    pub conclusion: &'i BoolExpr,
    pub rule: Rule<'i>,
}
#[derive(Clone, Copy, Debug)]
pub struct Certificate<'i> {
    pub steps: &'i [Step<'i>],
}
#[derive(Clone, Copy, Debug)]
pub enum Submission<'i> {
    Missing,
    Certificate(Certificate<'i>),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Binding {
    pub selection: SelectionPins,
    pub context: ContextRevision,
    pub suite: Digest32,
    pub oracle: Digest32,
    pub definition: Digest32,
    pub probe_set: Digest32,
    pub dependency_group: Digest32,
    pub role: DatasetRole,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateBinding {
    pub id: CandidateId,
    pub candidate_digest: Digest32,
    pub context: ContextRevision,
    pub target: Digest32,
    pub transform: Digest32,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    pub max_case_work: u64,
    pub max_run_work: u64,
    pub max_dispatches: u16,
    pub max_split_work: u64,
}
#[derive(Clone, Copy)]
pub struct ExplainCase<'i> {
    pub key: &'i [u8],
    pub theory: &'i SmtStatusQuery,
    pub goal: &'i BoolExpr,
    pub weight: &'i Rational,
    pub baseline: Submission<'i>,
}
#[derive(Clone, Copy)]
pub struct ExplainRootInput<'i> {
    pub binding: Binding,
    pub cases: &'i [ExplainCase<'i>],
    pub limits: Limits,
}
#[derive(Clone, Copy)]
pub struct ExplainResponse<'c> {
    pub key: &'c [u8],
    pub submission: Submission<'c>,
    pub prose: &'c [u8],
}
#[derive(Clone, Copy)]
pub struct ExplainCandidateInput<'c> {
    pub binding: CandidateBinding,
    pub responses: &'c [ExplainResponse<'c>],
}
#[derive(Clone, Copy)]
pub struct Expected<'i, 'c> {
    pub root: ExplainRootInput<'i>,
    pub candidate: ExplainCandidateInput<'c>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaseState {
    Pass,
    CompletedInvalid(InvalidDerivation),
    Unknown(Gap),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvalidDerivation {
    RuleMismatch,
    GoalMismatch,
    EmptyCertificate,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gap {
    MissingSubmission,
    WorkLimit,
    ProgramLimit,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NoComparable {
    ContextInconsistent,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvalidInput {
    EmptySuite,
    DuplicateCase,
    DuplicateResponse,
    UnknownCase,
    InvalidWeight,
    UndeclaredAtom,
    SizeLimit,
    DepthLimit,
    PremiseIndex,
    ChildIndex,
    ForwardReference,
    Cycle,
    InvalidArity,
    InvalidLimits,
    InvalidProse,
}
#[derive(Debug)]
pub enum Error {
    InvalidInput(InvalidInput),
    UnsupportedRole,
    UnsupportedFragment,
    NoComparable(NoComparable),
    Resource(ResourceError),
    ForeignReservation,
    ExpectedInputMismatch,
    CandidateMismatch,
    BindingMismatch,
    InvalidRows,
    WorkAdmissionExhausted,
    AllocationFailed,
    DefinitionMismatch,
    Numeric(super::numeric::NumericError),
    Measurement(lc631_analysis::creativity_measurement::MeasurementError),
    Selection(SelectionError),
}
/// Detached inspection data, never a checked owner or execution token.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnedStep {
    pub conclusion: BoolExpr,
    pub rule: OwnedRule,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OwnedRule {
    Premise { index: u8 },
    AndElim { prior: StepId, child: u8 },
    AndIntro { priors: Vec<StepId> },
}
/// Non-ZST private backing: borrowed object identity, not equal payload hashes.
pub struct ScoreNode {
    pub(crate) value: RationalInterval,
}
impl ScoreNode {
    pub(crate) fn state(state: CaseState) -> Result<Self, Error> {
        let value = match state {
            CaseState::Pass => RationalInterval::exact_ratio(1, 1),
            CaseState::CompletedInvalid(_) => RationalInterval::exact_ratio(0, 1),
            CaseState::Unknown(_) => RationalInterval::from_ratio(0, 1, 1, 1),
        }
        .map_err(Error::Numeric)?;
        Ok(Self { value })
    }
}
pub(crate) struct Outcome {
    pub state: CaseState,
    pub node: ScoreNode,
}
