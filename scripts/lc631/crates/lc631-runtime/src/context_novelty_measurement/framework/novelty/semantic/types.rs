use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Side { Source, Target }
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatusMap { CanonicalForcingV1 }
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BridgeMap { ActualProfileMapV1 }
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BridgeRule { RootGoalUseV1, MappedRootForcingV1 }
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalStatus { Supported, Refuted, Undetermined }
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Status { Complete, Partial, NoComparable, Undefined }
#[derive(Clone, Copy)]
pub struct SemanticProbe<'a> {
    pub role: f::QueryRole,
    pub formula: &'a f::BoolExpr,
    pub weight: &'a Rational,
}
#[derive(Clone, Copy)]
pub struct BridgeProbe<'a> {
    pub role: f::QueryRole,
    pub rule: BridgeRule,
    pub weight: &'a Rational,
}
#[derive(Clone, Copy)]
pub struct Registration<'a> {
    pub semantic: &'a [SemanticProbe<'a>],
    pub bridges: &'a [BridgeProbe<'a>],
    pub status_map: Option<StatusMap>,
    pub bridge_map: Option<BridgeMap>,
    pub definition_revision: Digest32,
    pub oracle_revision: Digest32,
    pub policy_revision: Digest32,
    pub limits: Limits,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    pub max_probes: usize,
    pub max_bridge_probes: usize,
    pub max_assessments: usize,
    pub max_current_checks: usize,
    pub max_native_dispatches: usize,
    pub max_lowered_nodes: usize,
    pub max_payload_bytes: usize,
    pub max_weight_bits: u32,
    pub max_intermediate_bits: u32,
    pub plan_ram: u64,
    pub assessment_ram: u64,
    pub scratch_ram: u64,
    pub output_bytes: u64,
    pub precision: u32,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Counts {
    pub required_not_dispatches: usize,
    pub native_not_dispatches: usize,
    pub native_not_store_entries: usize,
}
#[derive(Debug)]
pub enum Error {
    AlreadyGenerated,
    DuplicateProbe,
    InvalidProbe,
    RequiredSemanticProbe,
    InvalidWeight,
    InvalidLimits,
    NumericBudget,
    PayloadBudget,
    LoweringBudget,
    RamBudget,
    OutputBudget,
    WorkBudget,
    AssessmentBudget,
    CurrentBudget,
    ForeignRoot,
    ForeignReservation,
    ExpectedInputMismatch,
    NoScalar,
    Numeric(numeric::NumericError),
    Arithmetic(MeasurementError),
    Novelty(m::Error),
    Resource(ResourceError),
}
