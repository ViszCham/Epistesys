//! 日本語: 固定4caseの実Boolean bridge測定。Givenと共有検査器の境界を保持する。
//! English: Actual four-case Boolean bridge measurement; Given/shared-checker boundaries remain.
//! This opt-in interface produces no Grant, candidate eligibility or host-delivery evidence.
#![forbid(unsafe_code)]
mod admission;
mod consumer;
mod evaluation;
mod frame;
mod identity;
mod plan;
mod program;
pub use consumer::{next_use_current, use_current, OwnedUse};
pub use evaluation::{measure, OwnedCompatibility};
use lc631_analysis::creativity_measurement::ComponentReport;
use lc631_core::context::{ContextRevision, ContextScope};
use lc631_solvers::{
    protocol::sexpr::SExprError,
    smt::ModelValidationError,
    typed_query::{BoolSymbolId, SmtStatusQuery},
};
use lc631_substrate::{
    resource::{
        ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
        ResourceReservationView,
    },
    source::SourceStore,
    Digest32,
};
pub use plan::Plan;
pub use program::Program;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Assignment {
    pub symbol: BoolSymbolId,
    pub value: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortMap {
    pub source: BoolSymbolId,
    pub target: BoolSymbolId,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProbeCase<'a> {
    pub input: &'a [Assignment],
    pub weight: u32,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContractPins {
    pub definition: Digest32,
    pub oracle: Digest32,
    pub policy: Digest32,
    pub baseline: Digest32,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    pub max_work: u64,
    pub max_frame_bytes: usize,
    pub max_program_steps: u32,
}
#[derive(Clone, Copy)]
pub struct PlanInput<'a> {
    pub source: &'a SourceStore,
    pub target: &'a SourceStore,
    pub source_context: ContextRevision,
    pub target_context: ContextRevision,
    pub scope: ContextScope,
    pub root_goal: &'a SmtStatusQuery,
    pub source_theory: &'a SmtStatusQuery,
    pub target_theory: &'a SmtStatusQuery,
    pub source_use: &'a SmtStatusQuery,
    pub target_use: &'a SmtStatusQuery,
    pub source_hard: BoolSymbolId,
    pub target_hard: BoolSymbolId,
    pub expected_mapping: &'a [PortMap],
    pub cases: &'a [ProbeCase<'a>],
    pub pins: ContractPins,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stage {
    Roundtrip,
    Use,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Dispatch {
    pub case: usize,
    pub through: Stage,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Check {
    Pass,
    Fail,
    Unknown,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gap {
    NotDispatched,
    WorkBudget,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HardState {
    Pass,
    Fail,
    NeedsEvidence,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NextUse {
    Ready,
    NeedsProbe { case: usize },
    RejectedCompatibility,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UseOutcome {
    Executed {
        case: usize,
        source_value: bool,
        target_value: bool,
    },
    NeedsProbe {
        case: usize,
    },
    RejectedCompatibility,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Unsupported {
    Integer,
    UseFormula,
    BridgeProfile,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidInput,
    InvalidLimits,
    BindingMismatch,
    ForeignReservation,
    ProgramBudget,
    WorkBudget,
    HardInvariantFailed,
    NoExecutedUse,
    Unsupported(Unsupported),
    Resource(ResourceError),
    Model(ModelValidationError),
    Syntax(SExprError),
    AllocationFailed,
}

/// Public report data only: no owner constructor consumes this structure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Lineage {
    pub source_context: ContextRevision,
    pub target_context: ContextRevision,
    pub root_goal_revision: Digest32,
    pub oracle: Digest32,
    pub baseline: Digest32,
    pub forward_program: Digest32,
    pub inverse_program: Digest32,
    pub source_store: Digest32,
    pub target_store: Digest32,
    pub queries: [Digest32; 5],
    pub declared_inputs: [Digest32; 4],
    pub output_frames: [Option<Digest32>; 4],
    pub inverse_frames: [Option<Digest32>; 4],
    pub source_model_frames: [Option<Digest32>; 4],
    pub target_model_frames: [Option<Digest32>; 4],
    pub expected_mapping: Digest32,
    pub plan: Digest32,
    pub definition: Digest32,
    pub policy: Digest32,
    pub budget_series: Digest32,
}
type Frame = [Assignment; 3];
#[derive(Clone, Copy)]
struct Assessment {
    typed: Check,
    roundtrip: Check,
    used: Check,
    hard: HardState,
    source: Option<bool>,
    target: Option<bool>,
    gap: Option<Gap>,
    output: Option<Digest32>,
    inverse: Option<Digest32>,
    source_wire: Option<Digest32>,
    target_wire: Option<Digest32>,
}
impl Assessment {
    fn missing() -> Self {
        Self {
            typed: Check::Unknown,
            roundtrip: Check::Unknown,
            used: Check::Unknown,
            hard: HardState::NeedsEvidence,
            source: None,
            target: None,
            gap: Some(Gap::NotDispatched),
            output: None,
            inverse: None,
            source_wire: None,
            target_wire: None,
        }
    }
    fn success(self) -> Check {
        if self.hard == HardState::Fail
            || [self.typed, self.roundtrip, self.used].contains(&Check::Fail)
        {
            Check::Fail
        } else if [self.typed, self.roundtrip, self.used]
            .iter()
            .all(|c| *c == Check::Pass)
        {
            Check::Pass
        } else {
            Check::Unknown
        }
    }
}
/// Internally obtained frames/models, with read-only arithmetic report getters.
/// This type has no detached-report insertion path.
pub struct Trial<'i> {
    assessment: Assessment,
    input: Option<Frame>,
    output: Option<Frame>,
    inverse: Option<Frame>,
    source_model: Option<frame::ModelFrame<'i>>,
    target_model: Option<frame::ModelFrame<'i>>,
}
impl Trial<'_> {
    pub fn type_check(&self) -> Check {
        self.assessment.typed
    }
    pub fn roundtrip_check(&self) -> Check {
        self.assessment.roundtrip
    }
    pub fn use_check(&self) -> Check {
        self.assessment.used
    }
    pub fn source_value(&self) -> Option<bool> {
        self.assessment.source
    }
    pub fn target_value(&self) -> Option<bool> {
        self.assessment.target
    }
    pub fn gap(&self) -> Option<Gap> {
        self.assessment.gap
    }
}
fn claim<'v>(
    view: &'v ResourceReservationView<'_>,
    ram: u64,
    output: u64,
) -> Result<ResourceClaim<'v>, Error> {
    let request = |kind, n| {
        ResourceAmount::checked(n)
            .map(|amount| ResourceRequest::new(kind, amount))
            .map_err(Error::Resource)
    };
    if output == 0 {
        view.claim_resources(&[request(ResourceKind::Ram, ram)?])
    } else {
        view.claim_resources(&[
            request(ResourceKind::Ram, ram)?,
            request(ResourceKind::Output, output)?,
        ])
    }
    .map_err(Error::Resource)
}
fn same_owner<T, U>(a: &T, b: &U) -> bool {
    // Sized owner occurrence only, never persisted, ranked, or dereferenced through a cast.
    std::ptr::eq((a as *const T).cast::<()>(), (b as *const U).cast::<()>())
}
fn hard_state(values: &[Assessment; 4]) -> HardState {
    if values.iter().any(|v| v.hard == HardState::Fail) {
        HardState::Fail
    } else if values.iter().all(|v| v.hard == HardState::Pass) {
        HardState::Pass
    } else {
        HardState::NeedsEvidence
    }
}
fn readiness(order: &[usize; 4], values: &[Assessment; 4]) -> NextUse {
    if values.iter().any(|v| v.success() == Check::Fail) {
        return NextUse::RejectedCompatibility;
    }
    for &case in order {
        if values[case].success() == Check::Unknown {
            return NextUse::NeedsProbe { case };
        }
    }
    NextUse::Ready
}
