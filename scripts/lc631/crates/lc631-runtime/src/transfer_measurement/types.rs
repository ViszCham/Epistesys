use super::*;
use lc631_solvers::typed_query::{BoolExpr, SmtStatusQuery};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Program {
    ExtractV1,
    ComposeV1,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Protocol {
    pub demonstrations: u16,
    pub adaptive_updates: u16,
}
#[derive(Clone, Copy)]
pub struct TransferCase<'i> {
    pub key: &'i [u8],
    pub theory: &'i SmtStatusQuery,
    pub goal: &'i BoolExpr,
    pub weight: &'i Rational,
}
#[derive(Clone, Copy)]
pub struct SplitEntry<'i> {
    pub key: &'i [u8],
    pub family: &'i [u8],
    pub clone_family: &'i [u8],
    pub role: e::DatasetRole,
    pub source_payload: &'i [u8],
    pub theory: &'i SmtStatusQuery,
    pub goal: &'i BoolExpr,
}
#[derive(Clone, Copy)]
pub struct TransferRootInput<'i> {
    pub binding: e::Binding,
    pub cases: &'i [TransferCase<'i>],
    pub registry: &'i [SplitEntry<'i>],
    pub split_revision: Digest32,
    pub normalizer_revision: Digest32,
    pub baseline: Program,
    pub protocol: Protocol,
    pub limits: e::Limits,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransferCandidateInput {
    pub binding: e::CandidateBinding,
    pub program: Program,
    pub protocol: Protocol,
}
#[derive(Clone, Copy)]
pub struct Expected<'i> {
    pub root: TransferRootInput<'i>,
    pub candidate: TransferCandidateInput,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaseState {
    Pass,
    NoDerivationInThisProgram,
    Unknown(Gap),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gap {
    WorkLimit,
    ProgramLimit,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SplitLeakage {
    Family,
    CanonicalContent,
    SourceContent,
    CloneFamily,
}
#[derive(Debug)]
pub enum Error {
    InvalidInput(e::InvalidInput),
    UnsupportedRole,
    UnsupportedProtocol,
    ProtocolMismatch,
    NormalizerMismatch,
    SplitLeakage(SplitLeakage),
    NoComparable(e::NoComparable),
    Resource(lc631_substrate::resource::ResourceError),
    ForeignReservation,
    ExpectedInputMismatch,
    CandidateMismatch,
    BindingMismatch,
    InvalidRows,
    WorkAdmissionExhausted,
    AllocationFailed,
    DefinitionMismatch,
    UnsupportedFragment,
    SplitBudgetExhausted,
    Numeric(e::numeric::NumericError),
    Measurement(lc631_analysis::creativity_measurement::MeasurementError),
    Selection(crate::joint_selection::SelectionError),
}
pub(super) struct Outcome {
    pub state: CaseState,
    pub node: ScoreNode,
    pub certificate: Option<Vec<e::OwnedStep>>,
}
