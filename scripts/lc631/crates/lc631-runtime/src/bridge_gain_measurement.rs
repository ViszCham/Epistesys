//! 日本語: Given Boolean interface間の実Use ownerからのみCR-BRIDGEを測る。
//! English: Measure CR-BRIDGE only from actual Use owners between Given Boolean interfaces.
//! Closed version IDs are not binary hashes, authenticity attestations, Grants or theory equivalence.
//!
//! Private roots cannot expose the native Plan:
//! ```compile_fail
//! use lc631_runtime::bridge_gain_measurement::BridgeRoot;
//! fn leak(root: &BridgeRoot<'_, '_>) { let _ = &root.native; }
//! ```
//! Caller success or reports cannot construct actual owners:
//! ```compile_fail
//! use lc631_runtime::bridge_gain_measurement::BridgeUseBatch;
//! fn inject() { BridgeUseBatch::from_success(true); }
//! ```
//! ```compile_fail
//! use lc631_runtime::bridge_gain_measurement::OwnedBridgeGain;
//! use lc631_analysis::creativity_measurement::ComponentReport;
//! fn inject(report: ComponentReport) { OwnedBridgeGain::from_report(report); }
//! ```
//! ```compile_fail
//! use lc631_runtime::bridge_gain_measurement::BridgeUseBatch;
//! let _: BridgeUseBatch<'_, '_, '_, '_, '_> = serde_json::from_str("{}").unwrap();
//! ```
//! A gain's borrowed batch must remain alive (early-drop rejection):
//! ```compile_fail
//! use lc631_runtime::bridge_gain_measurement::*;
//! use lc631_substrate::resource::ResourceReservationView;
//! fn early<'b, 'o, 'p, 'i, 'v>(root: &'p BridgeRoot<'i, 'v>,
//!     program: &BridgeProgram<'p, 'i, 'v>,
//!     base: BridgeUseBatch<'b, 'o, 'p, 'i, 'v>,
//!     candidate: BridgeUseBatch<'b, 'o, 'p, 'i, 'v>,
//!     view: &'v ResourceReservationView<'_>) {
//!     let gain = measure_pair(root, &base, &candidate, view).unwrap();
//!     drop(base);
//!     let _ = gain.report_current(root, program, view);
//! }
//! ```
#![forbid(unsafe_code)]
mod execute;
mod measure;
mod plan;
use crate::compatibility_measurement as c;
pub use execute::{BridgeBranch, BridgeUseBatch, BridgeUseFailure};
use lc631_analysis::creativity_measurement::{ComponentReport, MeasurementError, RationalInterval};
use lc631_substrate::{
    resource::{
        ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
        ResourceReservationView,
    },
    Digest32,
};
pub use measure::{measure_pair, OwnedBridgeGain};
pub use plan::{BridgeProgram, BridgeRoot};

#[derive(Clone, Copy)]
pub struct ProgramSpec<'i> {
    pub forward: &'i [c::PortMap],
    pub inverse: &'i [c::PortMap],
}
pub enum EndpointPair {
    InputRegisters3ToControlPorts3V1,
}
pub struct Limits {
    pub max_work: u64,
}
pub struct BridgeRootInput<'i> {
    pub endpoints: EndpointPair,
    pub compatibility: c::PlanInput<'i>,
    pub frozen_baseline: ProgramSpec<'i>,
    pub limits: Limits,
}
#[derive(Debug)]
pub enum BridgeError {
    InvalidLimits,
    OracleMismatch,
    BindingMismatch,
    ForeignReservation,
    RoleMismatch,
    MissingPreparation,
    MissingExecution { case: usize },
    HardInvariantFailed,
    WorkBudget,
    AllocationFailed,
    Compatibility(c::Error),
    Resource(ResourceError),
    Measurement(MeasurementError),
}
fn from_c(error: c::Error) -> BridgeError {
    match error {
        c::Error::Resource(e) => BridgeError::Resource(e),
        c::Error::ForeignReservation => BridgeError::ForeignReservation,
        c::Error::HardInvariantFailed => BridgeError::HardInvariantFailed,
        c::Error::WorkBudget => BridgeError::WorkBudget,
        other => BridgeError::Compatibility(other),
    }
}
fn claim<'v>(
    view: &'v ResourceReservationView<'_>,
    bytes: u64,
) -> Result<ResourceClaim<'v>, BridgeError> {
    view.claim_resources(&[ResourceRequest::new(
        ResourceKind::Ram,
        ResourceAmount::checked(bytes).map_err(BridgeError::Resource)?,
    )])
    .map_err(BridgeError::Resource)
}
fn check_claim(
    retained: &ResourceClaim<'_>,
    view: &ResourceReservationView<'_>,
) -> Result<(), BridgeError> {
    view.check().map_err(BridgeError::Resource)?;
    retained.check().map_err(BridgeError::Resource)?;
    if !retained.same_reservation(view) {
        return Err(BridgeError::ForeignReservation);
    }
    Ok(())
}
