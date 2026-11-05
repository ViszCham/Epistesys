//! 日本語: actual B gainと実inner C occurrenceの対応。原式意味適用は未検査。
//! English: Actual B gain/inner C occurrence correspondence; original-root applicability stays unassessed.
mod current;
mod payload;
use super::{required_metrics, resources, OriginalFormula};
use crate::{bridge_gain_measurement as b, compatibility_measurement as c};
pub use current::{capture_current, with_native_current};
use lc631_analysis::creativity_measurement::{ComponentMetric, ComponentReport, RationalInterval};
use lc631_core::CandidateId;
use lc631_substrate::{
    resource::{ResourceClaim, ResourceError, ResourceReservationView},
    Digest32,
};
use std::cell::Cell;

pub struct BoolCoordinateSpec<'i> {
    pub native: c::PlanInput<'i>,
    pub composite_goal: Digest32,
    pub declared_relation: &'i [u8],
}
#[derive(Clone, Copy)]
pub struct BoolTarget<'t> {
    pub forward: &'t [c::PortMap],
    pub inverse: &'t [c::PortMap],
}
#[derive(Clone, Copy)]
pub struct NativeExpected<'n, 'p, 'i, 'v> {
    pub plan: &'n c::Plan<'i, 'v>,
    pub program: &'n c::Program<'p, 'i, 'v>,
}
#[derive(Clone, Copy)]
pub struct Owners<'g, 'a, 'b, 'o, 'p, 'i, 'v> {
    pub root: &'p b::BridgeRoot<'i, 'v>,
    pub program: &'o b::BridgeProgram<'p, 'i, 'v>,
    pub gain: &'g b::OwnedBridgeGain<'a, 'b, 'o, 'p, 'i, 'v>,
    pub native: NativeExpected<'g, 'p, 'i, 'v>,
}
pub struct NativeProgramView<'s, 'p, 'i, 'v> {
    plan: &'s c::Plan<'i, 'v>,
    program: &'s c::Program<'p, 'i, 'v>,
    input: c::PlanInput<'i>,
    forward: &'s [c::PortMap; 3],
    inverse: &'s [c::PortMap; 3],
}
pub struct OwnedBoolCorrespondence<'g, 'a, 'b, 'o, 'p, 'i, 'v> {
    coordinate: &'g BoolCoordinateSpec<'i>,
    target: BoolTarget<'g>,
    owners: Owners<'g, 'a, 'b, 'o, 'p, 'i, 'v>,
    without_bridge: [ComponentMetric; 19],
    without_bridge_len: usize,
    reads: Cell<u8>,
    retained: ResourceClaim<'v>,
}
pub struct CurrentBoolCorrespondence<'s, 'p, 'i, 'v> {
    native: NativeProgramView<'s, 'p, 'i, 'v>,
    report: &'s ComponentReport,
    without_bridge: &'s [ComponentMetric],
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompositeApplicability {
    Unassessed,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Provenance {
    ConditionalGiven,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OriginalGap {
    FullCompositeCost,
    AllRootTl,
    RootApplicability,
    OtherNativeOwners,
    CompositeHardEvidence,
}
#[derive(Debug)]
pub enum Error {
    Resource(ResourceError),
    ForeignReservation,
    ForeignNativePlan,
    ForeignNativeProgram,
    CoordinateInputMismatch,
    CoordinatePayloadMismatch,
    ForeignCoordinate,
    Bridge(b::BridgeError),
    InvalidDeclaration,
    ReadBudget,
}
const GAPS: &[OriginalGap] = &[
    OriginalGap::FullCompositeCost,
    OriginalGap::AllRootTl,
    OriginalGap::RootApplicability,
    OriginalGap::OtherNativeOwners,
    OriginalGap::CompositeHardEvidence,
];
impl<'s, 'p, 'i, 'v> NativeProgramView<'s, 'p, 'i, 'v> {
    pub fn plan(&self) -> &'s c::Plan<'i, 'v> {
        self.plan
    }
    pub fn program(&self) -> &'s c::Program<'p, 'i, 'v> {
        self.program
    }
    pub fn input(&self) -> c::PlanInput<'s> {
        self.input
    }
    pub fn forward(&self) -> &'s [c::PortMap] {
        self.forward
    }
    pub fn inverse(&self) -> &'s [c::PortMap] {
        self.inverse
    }
}
impl<'s, 'p, 'i, 'v> CurrentBoolCorrespondence<'s, 'p, 'i, 'v> {
    pub fn native_plan(&self) -> &'s c::Plan<'i, 'v> {
        self.native.plan()
    }
    pub fn native_program(&self) -> &'s c::Program<'p, 'i, 'v> {
        self.native.program()
    }
    pub fn forward(&self) -> &'s [c::PortMap] {
        self.native.forward()
    }
    pub fn inverse(&self) -> &'s [c::PortMap] {
        self.native.inverse()
    }
    pub fn bridge_report(&self) -> &'s ComponentReport {
        self.report
    }
    pub fn consumed_metric(&self) -> ComponentMetric {
        self.report.metric
    }
    pub fn native_missing_leaves(&self, formula: OriginalFormula) -> &[ComponentMetric] {
        match formula {
            OriginalFormula::F1V1 => required_metrics(formula),
            OriginalFormula::F2V1 => self.without_bridge,
        }
    }
    pub fn original_missing_leaves(&self, formula: OriginalFormula) -> &[ComponentMetric] {
        required_metrics(formula)
    }
    pub fn composite_applicability(&self) -> CompositeApplicability {
        CompositeApplicability::Unassessed
    }
    pub fn provenance(&self) -> Provenance {
        Provenance::ConditionalGiven
    }
    pub fn original_gaps(&self) -> &[OriginalGap] {
        GAPS
    }
    pub fn original_scalar(&self) -> Option<&RationalInterval> {
        None
    }
    pub fn original_formula_evaluated(&self) -> bool {
        false
    }
    pub fn recommendation(&self) -> Option<CandidateId> {
        None
    }
}
fn from_bridge(error: b::BridgeError) -> Error {
    match error {
        b::BridgeError::Resource(e) => Error::Resource(e),
        b::BridgeError::ForeignReservation => Error::ForeignReservation,
        b::BridgeError::Compatibility(c::Error::Resource(e)) => Error::Resource(e),
        b::BridgeError::Compatibility(c::Error::ForeignReservation) => Error::ForeignReservation,
        other => Error::Bridge(other),
    }
}
