//! 日本語: 固定Givenモデルを既存有限Boolean evaluatorで実評価するredundancy owner。
//! source意味・独立性・権限とは別で、通常CLIや全F2の閉鎖ではない。
//! English: Actual redundancy ownership over a fixed Given model using the existing finite Boolean evaluator.
//! Source meaning, independence and authority remain separate; normal CLI/full F2 closure is not asserted.
#![forbid(unsafe_code)]

use crate::formal_boolean;
use lc631_analysis::creativity_measurement::{ComponentReport, RationalInterval};
use lc631_core::{
    context::{ContextRevision, ContextScope},
    CandidateId,
};
use lc631_solvers::typed_query::{BoolExpr, BoolSymbolId, SmtStatusQuery};
use lc631_substrate::{
    resource::{ResourceClaim, ResourceError},
    source::{SourceSpan, SourceStore},
    Digest32,
};
use std::cell::Cell;

mod candidate;
mod current;
mod pairs;
mod produce;
mod resources;
mod root;
mod syntax;
pub use current::compare_current;
use resources::{ChainClaim, Counters};
use syntax::Stats;

pub const PROFILE: &str = "CR-REDUNDANCY/finite-boolean-fixed-inventory-v1";

/// 日本語: slot/atom番号はlookup用であり、意味同値の証拠ではない。
/// English: Slot/atom numbers are lookup handles, never evidence of semantic equivalence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SlotId(pub u16);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RootAtom(pub u8);

/// 日本語: rootが先に固定するGiven role。保護属性やcandidateの名前とは別。
/// enum名からsource roleを推論しない。同種の警告でも異なる役割文脈が必要なら
/// この限定catalogでは未対応であり、同名variantへ黙って統合しない。
/// English: Given roles fixed by the root, separate from protection and candidate labels.
/// Enum names do not infer source roles. Distinct role contexts beyond this
/// restricted catalog are unsupported, never merged because their kind has one name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticRole {
    Premise,
    Conclusion,
    Precondition,
    ActionWarning,
    ResultWarning,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Protection {
    /// 日本語: 同じglobal map後のbaseline payload不変。Γ下で同値でもp→¬¬pを拒否。
    /// English: Baseline payload unchanged after the same global map; reject p→¬¬p even if Γ-equivalent.
    HardUnchanged,
    Mutable,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SlotSpec {
    pub slot: SlotId,
    pub role: SemanticRole,
    pub protection: Protection,
}

/// 日本語: groundingは原文へのGiven対応であり、propositionの真理を認証しない。
/// root.model.source.slice(grounding)で元sourceを検査する。baseline.sourceとの
/// 異表現対応をsource hash一致から作らない。
/// English: Grounding is a Given source correspondence, not authenticated proposition truth.
/// Admission checks root.model.source.slice(grounding), not an arbitrary
/// baseline source. Equal source hashes do not create representation correspondence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AtomMeaningInput {
    pub atom: RootAtom,
    pub grounding: SourceSpan,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AtomMapEntry {
    pub local: BoolSymbolId,
    pub meaning: RootAtom,
}

/// 日本語: Noneは未抽出。false式や空inventoryへ変換しない。
/// role追加のcompile-fail例は中央export後の実行待ちで、behavior REDの観測ではない。
/// English: None is unextracted, never a false formula or an empty inventory.
/// This role-invention compile-fail example awaits central export and doctest
/// execution; it is not an observed behavioral RED.
/// ```compile_fail
/// use lc631_runtime::redundancy_measurement::{SemanticRole, SlotId, UnitInput};
/// let _unit = UnitInput {
///     slot: SlotId(0),
///     formula: None,
///     role: SemanticRole::Conclusion,
/// };
/// ```
#[derive(Clone, Copy, Debug)]
pub struct UnitInput<'input> {
    pub slot: SlotId,
    pub formula: Option<&'input BoolExpr>,
}

/// 日本語: 全明示typed artifactの入力。complete boolやcaller classを受け取らない。
/// English: Whole explicit typed-artifact input, without a completeness bool or caller classes.
pub struct SemanticArtifactInput<'input> {
    pub source: &'input SourceStore,
    pub context: ContextRevision,
    pub units: &'input [UnitInput<'input>],
}

/// 日本語: 実際のΓ/namespaceの借用。両native側はこの同じqueryを使う。
/// English: Borrow the actual Γ/namespace; both native Request sides use this same query.
pub struct ModelInput<'input> {
    pub source: &'input SourceStore,
    pub context: ContextRevision,
    pub theory: &'input SmtStatusQuery,
    pub atom_map: &'input [AtomMapEntry],
    pub unconstrained: formal_boolean::UnconstrainedPolicy,
}

/// 日本語: rootのslots/roles/意味map/変更policyはcandidate bindingより先に固定する。
/// slots=[]かつbaseline.units=[]の検査済み閉入力だけが有効な空caseとなる。
/// English: Fix root slots/roles/meaning map/change policy before binding candidates.
/// Only an admitted closed slots=[] and baseline.units=[] input qualifies as valid empty.
pub struct RootInput<'input> {
    pub scope: ContextScope,
    pub baseline: &'input SemanticArtifactInput<'input>,
    pub slots: &'input [SlotSpec],
    pub atom_meanings: &'input [AtomMeaningInput],
    pub model: &'input ModelInput<'input>,
}

/// 日本語: Mutable slotへの宣言差分。実before/after対応とroot policyを後で検査する。
/// English: Declared mutable-slot edit; actual before/after references and root policy must be checked.
#[derive(Clone, Copy, Debug)]
pub struct MutableChangeInput<'input> {
    pub slot: SlotId,
    pub before: &'input BoolExpr,
    pub after: &'input BoolExpr,
}

/// 日本語: retained上限はroot/bound/pair/owned全chainの合計。stage別上限ではない。
/// 総workは同じroot/runでPairPlanを作り直しても復活しない。
/// English: Retained RAM caps the COMBINED root/bound/pair/owned live chain,
/// not each stage. Total work never resets per call or recreated PairPlan in the
/// same root/run scope. Recapturing a separate RootPlan is NOT a run-global work quota.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    pub max_atoms: u8,
    pub max_units: u16,
    pub max_unit_nodes: u16,
    pub max_theory_nodes: u16,
    pub max_depth: u16,
    pub max_call_work: u64,
    pub max_total_work: u64,
    pub max_retained_ram: u64,
    pub max_scratch_ram: u64,
    pub max_output_bytes: u64,
}
/// 日本語: 有限profile上限。全最大値の組合せの完走を約束しない。
/// English: Finite profile caps, without promising completion for every maximum combination.
pub const PROFILE_LIMITS: Limits = Limits {
    max_atoms: 8,
    max_units: 16,
    max_unit_nodes: 64,
    max_theory_nodes: 64,
    max_depth: 32,
    max_call_work: 1_000_000,
    max_total_work: 8_000_000,
    max_retained_ram: 128 * 1024,
    max_scratch_ram: 64 * 1024,
    max_output_bytes: 64 * 1024,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gap {
    UnextractedArtifact,
    UnextractedUnit(SlotId),
    CorrespondencePending,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RedundancyState {
    Measured(RationalInterval),
    Unknown(Gap),
    Unsupported(formal_boolean::Failure),
    NoComparable(formal_boolean::NoComparable),
    ResourceExhausted,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidLimits,
    AtomLimit,
    UnitLimit,
    NodeLimit,
    DepthLimit,
    InvalidInventory,
    DuplicateSlot,
    MissingSlot,
    ForeignSlot,
    InvalidAtomMap,
    UnsupportedProtectedRewrite,
    MissingDelta,
    InvalidDelta,
    BindingMismatch,
    ScopeMismatch,
    SourceMismatch,
    ForeignReservation,
    PerCallBudget,
    TotalWorkBudget,
    AllocationFailed,
    ArithmeticOverflow,
    Conflict,
    Formal(formal_boolean::Failure),
    Resource(ResourceError),
}
/// 日本語: Equalは同scopeのRの算術tieであり、candidate意味同値や優越ではない。
/// English: Equal is an arithmetic R tie in scope, not candidate semantic equivalence or dominance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Comparison {
    /// 日本語: 左ownerのRが右ownerのRより小さい。
    /// English: The left owner's R is lower than the right owner's R.
    Lower,
    Equal,
    /// 日本語: 左ownerのRが右ownerのRより大きい。
    /// English: The left owner's R is higher than the right owner's R.
    Higher,
    NoRecommendation,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeCounts {
    pub consistency_calls: u64,
    pub pair_calls: u64,
    pub pairs_required: u64,
    pub ast_visits: u64,
    /// 日本語: 実測でなく、線形map探索の保守的なmodeled上限。
    /// English: Conservative modeled linear-map comparison allowance, not observed lookups.
    pub map_comparisons: u64,
    /// Cumulative nonrefundable modeled root-work snapshot at production time;
    /// may include other prepared candidates, not this owner's exclusive usage.
    pub charged_work: u64,
}

/// 日本語: 同じownerの全子stageでwork/retainedを共有。別RootPlanの再captureはrun全体quotaでない。
/// English: All child stages share this owner's work/retained counters. A new RootPlan has no run-global quota.
pub struct RootRedundancyPlan<'input, 'view> {
    input: &'input RootInput<'input>,
    limits: Limits,
    native_symbols: Vec<(BoolSymbolId, BoolSymbolId)>,
    consistency_true: BoolExpr,
    theory_stats: Stats,
    baseline_gap: Option<Gap>,
    baseline_digest: Digest32,
    model_digest: Digest32,
    comparison_digest: Digest32,
    budget: Counters,
    retained: ResourceClaim<'view>,
}
pub struct BoundCandidateUnits<'plan, 'input, 'view> {
    root: &'plan RootRedundancyPlan<'input, 'view>,
    candidate: CandidateId,
    artifact: Option<&'input SemanticArtifactInput<'input>>,
    formulas: [Option<&'input BoolExpr>; 16],
    stats: [Stats; 16],
    gap: Option<Gap>,
    digest: Digest32,
    retained: ChainClaim<'plan, 'view>,
}
/// 日本語: work予約は返金しない。produceは一回だけで、同じplanの再評価を拒否する。
/// English: Work reservations never refund. A plan dispatches once and refuses repeated production.
pub struct PairPlan<'candidate, 'plan, 'input, 'view> {
    candidate: &'candidate BoundCandidateUnits<'plan, 'input, 'view>,
    requests: Vec<formal_boolean::Request<'candidate>>,
    pairs: Vec<(usize, usize)>,
    costs: Vec<pairs::CallCost>,
    pairs_required: u64,
    digest: Digest32,
    started: Cell<bool>,
    retained: ChainClaim<'candidate, 'view>,
}
pub struct OwnedRedundancy<'pairs, 'candidate, 'plan, 'input, 'view> {
    pair_plan: &'pairs PairPlan<'candidate, 'plan, 'input, 'view>,
    observations: Vec<formal_boolean::Evaluation<'pairs>>,
    counts: NativeCounts,
    state: RedundancyState,
    report: ComponentReport,
    retained: ChainClaim<'pairs, 'view>,
}
