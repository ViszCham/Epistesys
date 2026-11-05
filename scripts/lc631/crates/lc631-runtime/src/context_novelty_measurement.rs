//! 日本語: Given有限Boolean入力の実文脈距離・構築archive既知性。権限を生成しない。
//! English: Actual finite Given Boolean context/construction measurements, no authority.
//! Safe private ownership and a root-local nonrefundable ledger are selected.
//! No persistence ABI, unsafe, GPU, parallel backend, global static or speed claim.
//! Re-capturing a DIFFERENT RootPlan does not share logical-work quota: the
//! current broker has no run-global work-meter port. That gap is Unsupported,
//! not repaired by a process-global counter. Given interpretation/shared IR,
//! oracle independence, general calibration and host closure remain unresolved.
#![forbid(unsafe_code)]
mod api;
mod archive;
mod budget;
mod canonical;
mod features;
pub mod framework;
mod identity;
mod measure;
mod native_hash;
mod numeric;
mod owned;
mod plan;
mod resources;
mod validate;
use crate::creative_candidate_bridge::{
    BridgeLimits, CreativeCandidateBridge, CreativeError, GivenPremise, GoalPolicy, Operator,
    ValidatedFormalCandidate,
};
pub use api::*;
use budget::{Meter, Work};
use lc631_analysis::creativity_measurement::{
    formula::OriginalFormula, ComponentMetric, ComponentReport, ComponentState, IndependenceStatus,
    InputBasis, MeasurementBinding, MeasurementError, MeasurementGap, Rational, RationalInterval,
    SemanticValidation,
};
use lc631_core::context::{ContextRevision, ContextScope, OriginalContext};
use lc631_solvers::typed_query::{BoolExpr, BoolSymbolId, SmtStatusQuery};
use lc631_substrate::{
    resource::{ResourceClaim, ResourceError, ResourceReservationView},
    source::{SourceSpan, SourceStore},
    Digest32,
};
pub use owned::{CurrentAssessment, OwnedAssessment};
pub use plan::RootPlan;
const MIB: u64 = 1024 * 1024;
const CURRENT_WORK: u64 = 32768;
type FeatureSet = Vec<usize>;
struct CanonicalFeature {
    payload: Vec<u8>,
    weight: Rational,
}
struct Universe {
    features: Vec<CanonicalFeature>,
}
fn arithmetic(e: lc631_substrate::measurement_numeric::NumericError) -> Error {
    Error::Arithmetic(e.into())
}
fn point(n: i64, d: i64) -> Result<RationalInterval, Error> {
    RationalInterval::exact_ratio(n, d).map_err(arithmetic)
}
fn number(n: i64) -> Rational {
    Rational::from_integer(n.into())
}
