//! 実有限導出検査と固定K2。Given source/goal、Unassessed、SharedDependencyを保持。
//! Actual finite certificate checking, not gold, learned ability, full F2 or authority.
//! Work counters are ROOT-SERIES LOCAL; recaptured roots share no global counter.
#![forbid(unsafe_code)]
pub(crate) mod kernel;
mod measure;
mod plan;
mod selection;
mod types;
pub(crate) use lc631_analysis::creativity_measurement::{
    paired_gain, ComponentMetric, ComponentReport, ComponentState, IndependenceStatus, InputBasis,
    MeasurementBinding, ProbePair, SemanticValidation,
};
pub(crate) use lc631_core::measurement_numeric::{self as numeric, Rational, RationalInterval};
pub(crate) use lc631_substrate::{
    resource::{ResourceClaim, ResourceReservationView},
    Digest32,
};
pub use measure::{measure_explain, OwnedExplainGain};
pub use plan::{admit_explain, capture_explain, ExplainCandidate, ExplainRoot};
pub use selection::{select_explain_current, ExplainRow, OwnedExplainSelection};
pub(crate) use sha2::{Digest, Sha256};
pub use types::*;

pub const ROOT_RAM: u64 = 2 * 1024 * 1024;
pub const CANDIDATE_RAM: u64 = ROOT_RAM;
pub const GAIN_RAM: u64 = ROOT_RAM;
pub const SCRATCH_RAM: u64 = 4 * 1024 * 1024;
pub const OUTPUT_BYTES: u64 = 64 * 1024;
pub fn definition_revision() -> Digest32 {
    kernel::digest(b"finite-formal-v1.4/CR-EXPLAIN/ConjunctionCertificateV1/actual-forward.v1")
}

#[doc = "Current owners cannot be constructed from detached reports."]
#[doc = "```compile_fail"]
#[doc = include_str!("../tests/tlcx22_explain_transfer_support/compile_fail/from_report.rs")]
#[doc = "```"]
#[doc = "A local owner cannot lend static report references."]
#[doc = "```compile_fail"]
#[doc = include_str!("../tests/tlcx22_explain_transfer_support/compile_fail/lifetime_escape.rs")]
#[doc = "```"]
pub struct OwnerConstructionBoundary;
