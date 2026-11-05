//! 日本語: 固定optional legacy propertyの実実行によるabsolute K1。
//! English: Absolute K1 from actual fixed optional legacy-property execution.
//! Given transport inputs/shared serde are not independent semantic or host evidence.
#![forbid(unsafe_code)]
mod admission;
mod aggregate;
mod execute;
mod identity;
mod output;
mod owner;
mod plan;
mod resources;
use crate::creativity_measurement::*;
use crate::DgclStandaloneCandidate;
use lc631_core::{
    context::{ContextRevision, ContextScope, SourceRevision},
    CandidateId,
};
use lc631_substrate::{
    resource::{ResourceClaim, ResourceError, ResourceReservationView},
    source::SourceStore,
    Digest32,
};
pub use owner::{
    observe_baseline_current, observe_breakcost_current, OwnedBreakCost, OwnedLegacyBaseline,
};
pub use plan::{bind_candidate, register, AdapterCandidate, RootDefinition};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdapterKind {
    LegacyProjectionV1,
    BoundedProjectionV2,
    DirectLegacyDtoSerializeV1,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LegacyProperty {
    JsonObjectEnvelope,
    V1StrictConsumerAcceptance,
    SourceIdentity,
    CandidateIdentity,
    NoHostSend,
    NoOutputCommit,
    NoLegacyCompletion,
    NoAuthorityCreation,
    NegotiatedSchema,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PropertyClass {
    Optional,
    Hard,
}
#[derive(Clone, Debug)]
pub struct PropertyRule {
    pub property: LegacyProperty,
    pub class: PropertyClass,
    pub weight: Rational,
}
pub struct LegacyCase<'a> {
    pub id: u64,
    pub canonical_key: &'a [u8],
    pub input: &'a DgclStandaloneCandidate,
    pub weight: &'a Rational,
}
#[derive(Clone, Copy)]
pub struct AdapterRequest<'a> {
    pub case: u64,
    pub kind: AdapterKind,
    pub requested_schema: &'a str,
}
#[derive(Clone, Copy)]
pub struct BreakCostBinding {
    pub source: SourceRevision,
    pub context: ContextRevision,
    pub scope: ContextScope,
    pub baseline: Digest32,
    pub hard_invariants: Digest32,
    pub policy: Digest32,
    pub goal_profile: Option<Digest32>,
    pub evidence_revision: Digest32,
    pub oracle: Digest32,
    pub dependency_group: Digest32,
}
#[derive(Clone, Copy)]
pub struct CandidateBinding {
    pub id: CandidateId,
    pub context: ContextRevision,
    pub candidate_digest: Digest32,
    pub target: Digest32,
    pub transform: Digest32,
}
#[derive(Clone, Copy)]
pub struct BreakCostBudget {
    pub max_cases: usize,
    pub max_input_bytes_per_case: usize,
    pub max_input_bytes_total: usize,
    pub max_candidate_text_bytes: usize,
    pub max_output_bytes_per_case: usize,
    pub max_output_bytes_per_run: usize,
    pub max_adapter_calls_per_run: usize,
    pub max_serializations_per_run: usize,
    pub max_property_checks_per_run: usize,
    pub max_soft_checks_per_run: usize,
    pub max_extra_hard_checks: usize,
    pub max_weight_bits: u32,
    pub max_intermediate_bits: u32,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalHardStatus {
    NotRejected,
    Failed,
    Pending,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NotObservedReason {
    SoftCheckBudget,
    PropertyBudget,
    ProducerFailure,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PropertyOutcome {
    Preserved,
    ConfirmedViolation,
    NotObserved(NotObservedReason),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionCounters {
    pub adapter_calls: usize,
    pub serializations: usize,
    pub property_checks: usize,
    pub soft_checks: usize,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    Resource(ResourceError),
    ForeignReservation,
    BindingMismatch,
    InvalidConfig,
    InvalidSuite,
    InvalidInput,
    NonTradeableHard,
    InputBudget,
    OutputBudget,
    NumericBudget,
    DispatchBudget,
    PropertyBudget,
    ProducerFailure,
}
#[derive(Clone, Copy)]
pub struct ExpectedInputs<'a, 'v> {
    pub source: &'a SourceStore,
    pub root: &'a RootDefinition<'a, 'v>,
    pub cases: &'a [LegacyCase<'a>],
    pub baseline: &'a OwnedLegacyBaseline<'a, 'v>,
    pub candidate: &'a AdapterCandidate<'a, 'v>,
}
const V1: &str = "epistesys-dgcl-consumer-view.v1";
const V2: &str = "epistesys-dgcl-consumer-view.v2";
const MIB: u64 = 1024 * 1024;
fn same<T>(a: &T, b: &T) -> bool {
    std::ptr::eq(a, b)
}
fn schema(kind: AdapterKind) -> &'static str {
    match kind {
        AdapterKind::BoundedProjectionV2 => V2,
        AdapterKind::LegacyProjectionV1 | AdapterKind::DirectLegacyDtoSerializeV1 => V1,
    }
}
