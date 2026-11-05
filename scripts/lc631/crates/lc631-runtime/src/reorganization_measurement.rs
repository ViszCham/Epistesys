//! 日本語: 固定Given単項Bool roleの実再結合。source解釈と独立性は未検証。
//! English: Actual finite unary-Bool rebinding, not verified source interpretation.
//! Safe private types, staged borrows and RAII retain the invariant. No unsafe,
//! persistent ABI, new arena, no_std, SoA, parallel/async/atomic, GPU/SIMD or
//! codegen backend is justified. Finite hand tests remain central's next gate.
//! Work is conservatively charged within ONE actual plan. Recapture can create
//! another account: TaskRun-wide cross-plan enforcement remains NeedsEvidence.
//! Reframe B and existing graph/path getter changes are NOT implemented here.
#![forbid(unsafe_code)]
#![doc = concat!(
    "\nAn owner cannot be constructed from a detached report.\n```compile_fail\n",
    include_str!("../tests/tlcx22_reorganization_support/compile_fail/from_report.rs"),
    "\n```\nCaller results cannot replace actual composition or evaluation.\n```compile_fail\n",
    include_str!("../tests/tlcx22_reorganization_support/compile_fail/caller_result.rs"),
    "\n```\nA current report cannot escape its scoped borrow.\n```compile_fail\n",
    include_str!("../tests/tlcx22_reorganization_support/compile_fail/current_escape.rs"),
    "\n```\nRequest storage must outlive its evaluations.\n```compile_fail\n",
    include_str!("../tests/tlcx22_reorganization_support/compile_fail/drop_requests.rs"),
    "\n```\n",
)]
mod current;
mod dependence;
mod finite;
mod measure;
mod model;
mod plan;
mod program;
mod queries;
mod selection;

pub use current::{CurrentRebind, RebindRow};
pub use measure::{CheckRecord, OwnedRebind};
pub use model::{
    BindingInput, MapBackInput, RebindProbe, RoleId, RoleModel, RootUseInput, UnitId, UseExpr,
};
pub use plan::{Limits, RebindPlan, WorkQuotaStatus};
pub use program::PreparedRebindProgram;
pub use queries::RebindQueries;
pub use selection::{compare_current, select_current, CurrentDecision};

use lc631_analysis::creativity_measurement::MeasurementError;
use lc631_substrate::{
    measurement_numeric::NumericError,
    resource::{
        ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
        ResourceReservationView,
    },
};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckKind {
    RoleDifference,
    MapBack,
    RootUse,
}

#[derive(Debug)]
pub enum Error {
    InvalidLimits,
    InvalidModel,
    InvalidCandidate,
    DuplicateBinding,
    DanglingUse,
    UnitSetMismatch,
    MapSetMismatch,
    MissingActualUse,
    UseUnitMismatch,
    PrerequisiteFailed { unit: UnitId, check: CheckKind },
    DuplicateProbe,
    ProbeSetMismatch,
    EmptyProbeSet,
    InvalidWeight,
    ForeignSourceOwner,
    TheoryMismatch,
    RootBaselineMismatch,
    UnsupportedInteger,
    UndeclaredSymbol,
    ParameterMismatch,
    ShapeLimit,
    PayloadLimit,
    ArithmeticOverflow,
    AllocationFailed,
    ForeignReservation,
    ForeignInterval,
    ForeignReport,
    ForeignPlan,
    ForeignProgram,
    ForeignQueries,
    MetricMismatch,
    WorkBudget,
    Resource(ResourceError),
    Formal(crate::formal_boolean::Failure),
    Measurement(MeasurementError),
    Numeric(NumericError),
    Selection(crate::joint_selection::SelectionError),
}
impl From<ResourceError> for Error {
    fn from(e: ResourceError) -> Self {
        Self::Resource(e)
    }
}
impl From<crate::formal_boolean::Failure> for Error {
    fn from(e: crate::formal_boolean::Failure) -> Self {
        Self::Formal(e)
    }
}
impl From<NumericError> for Error {
    fn from(e: NumericError) -> Self {
        Self::Numeric(e)
    }
}
impl From<MeasurementError> for Error {
    fn from(e: MeasurementError) -> Self {
        Self::Measurement(e)
    }
}
impl From<crate::joint_selection::SelectionError> for Error {
    fn from(e: crate::joint_selection::SelectionError) -> Self {
        Self::Selection(e)
    }
}
fn claim<'v>(
    view: &'v ResourceReservationView<'_>,
    bytes: u64,
) -> Result<ResourceClaim<'v>, Error> {
    view.check()?;
    Ok(view.claim_resources(&[ResourceRequest::new(
        ResourceKind::Ram,
        ResourceAmount::checked(bytes)?,
    )])?)
}
fn vector<T>(capacity: usize) -> Result<Vec<T>, Error> {
    let mut out = Vec::new();
    out.try_reserve_exact(capacity)
        .map_err(|_| Error::AllocationFailed)?;
    Ok(out)
}
