//! Public declarations are requests, not checked owners or success reports.
use super::*;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Kernel {
    ClassicalPointV1,
    PersistentChain2V1,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Contract {
    ClassicalPointV1,
    PersistentChain2V1,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Package {
    C,
    Kc,
    Ki,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SignatureRole {
    AtomForcing,
    Bottom,
    Top,
    And,
    Or,
    PositiveRules,
    Domain,
    Negation,
    Consequence,
    AxiomSuite,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum GrammarOperator {
    Constant,
    Variable,
    Not,
    And,
    Or,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeOperator {
    Constant,
    Atom,
    ProfiledNot,
    And,
    Or,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Duty {
    Optional,
    Required,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum QueryRole {
    GoalEcho,
    Placement,
    BridgeLaw,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SubjectKind {
    Original,
    Generated,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Attribution {
    JointContextAndFramework,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MapRule {
    ConstantEmbeddingRootRetractionV1,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PartitionPolicy {
    GivenTestOnly6_1_1,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetricState {
    PendingLaterPacket,
}
#[derive(Clone, Copy)]
pub struct ProfileRegistration<'a> {
    pub kernel: Kernel,
    pub contract: Contract,
    pub display_name: &'a str,
}
#[derive(Clone, Copy)]
pub struct RoleBinding {
    pub kernel: Kernel,
    pub role: SignatureRole,
    pub package: Package,
}
#[derive(Clone, Copy)]
pub struct OperatorMap {
    pub source: GrammarOperator,
    pub target: NativeOperator,
}
#[derive(Clone, Copy)]
pub struct AtomCorrespondence {
    pub meaning: u8,
    pub point_slot: u8,
    pub chain_slot: u8,
}
#[derive(Clone, Copy)]
pub struct PackageWeight<'a> {
    pub package: Package,
    pub weight: &'a Rational,
}
#[derive(Clone, Copy)]
pub struct Probe<'a> {
    pub role: QueryRole,
    pub formula: &'a BoolExpr,
    pub duty: Duty,
}
#[derive(Clone, Copy)]
pub struct MapDeclaration<'a> {
    pub atoms: &'a [AtomCorrespondence],
    pub operators: &'a [OperatorMap],
    pub point_world: u8,
    pub chain_root: u8,
    pub chain_future: u8,
    pub rule: MapRule,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CellKey {
    pub kernel: Kernel,
    pub subject: SubjectKind,
}
#[derive(Clone, Copy)]
pub struct CellPair {
    pub source: CellKey,
    pub target: CellKey,
    pub attribution: Attribution,
}
#[derive(Clone, Copy)]
pub struct Limits {
    pub max_atoms: u8,
    pub max_models: usize,
    pub max_cases: usize,
    pub max_queries: usize,
    pub max_lowered_nodes: usize,
    pub max_native_dispatches_per_case: usize,
    pub max_current_checks: usize,
    pub plan_ram: u64,
    pub case_ram: u64,
    pub scratch_ram: u64,
    pub output_bytes: u64,
}
#[derive(Clone, Copy)]
pub struct Declaration<'a> {
    pub root_scope: ContextScope,
    pub root_goal_revision: Digest32,
    pub authority_revision: AuthorityRevision,
    pub profiles: &'a [ProfileRegistration<'a>],
    pub partition: &'a [RoleBinding],
    pub universe: &'a [PackageWeight<'a>],
    pub partition_policy: PartitionPolicy,
    pub map: MapDeclaration<'a>,
    pub probes: &'a [Probe<'a>],
    pub allowed_operator: Operator,
    pub comparison: CellPair,
    pub definition_revision: Digest32,
    pub map_revision: Digest32,
    pub limits: Limits,
}
#[derive(Clone, Copy)]
pub enum Subject<'s, 'a, 'v> {
    Original,
    Generated(&'s ValidatedFormalCandidate<'a, 'v>),
}
#[derive(Clone, Copy)]
pub struct ExpectedInputs<'s, 'a, 'v> {
    pub root: &'s FrameworkRoot<'a, 'v>,
    pub declaration: &'a Declaration<'a>,
    pub source: &'a SourceStore,
    pub source_query: &'a SmtStatusQuery,
    pub subject_query: &'s SmtStatusQuery,
    pub subject: Subject<'s, 'a, 'v>,
    pub kernel: Kernel,
}
#[derive(Debug)]
pub enum Error {
    RegistryIncomplete,
    InvalidPartition,
    ProfileContractMismatch,
    InvalidMap,
    RootScopeMismatch,
    AlreadyGenerated,
    ForeignRoot,
    ForeignCandidate,
    ExpectedInputMismatch,
    ForeignReservation,
    HardInvariant,
    ContextInconsistent,
    UnsupportedProfile,
    UnsupportedOperator,
    NativeCurrentUnavailable,
    InvalidLimits,
    ModelBudget,
    LoweringBudget,
    NativeBudget,
    CaseBudget,
    CurrentBudget,
    WorkBudget,
    RamBudget,
    OutputBudget,
    PayloadBudget,
    Resource(ResourceError),
    Context(super::super::Error),
    Modal(ModalError),
}
pub(super) const KERNELS: [Kernel; 2] = [Kernel::ClassicalPointV1, Kernel::PersistentChain2V1];
pub(super) const ROLES: [SignatureRole; 10] = [
    SignatureRole::AtomForcing,
    SignatureRole::Bottom,
    SignatureRole::Top,
    SignatureRole::And,
    SignatureRole::Or,
    SignatureRole::PositiveRules,
    SignatureRole::Domain,
    SignatureRole::Negation,
    SignatureRole::Consequence,
    SignatureRole::AxiomSuite,
];
pub(super) const OPERATORS: [(GrammarOperator, NativeOperator); 5] = [
    (GrammarOperator::Constant, NativeOperator::Constant),
    (GrammarOperator::Variable, NativeOperator::Atom),
    (GrammarOperator::Not, NativeOperator::ProfiledNot),
    (GrammarOperator::And, NativeOperator::And),
    (GrammarOperator::Or, NativeOperator::Or),
];
