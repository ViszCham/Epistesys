//! 日本語: 実有限framework署名とpositive構築archiveのP2。権限を作らない。
//! English: Actual closed signature/archive P2,not P3,source truth or authority.
//! Existing root-local work is shared; different roots still lack run-global quota.
#![forbid(unsafe_code)]
mod admission;
mod archive;
mod archive_native;
mod budget;
mod construction;
mod current;
mod identity;
mod measure;
mod plan;
mod signature;
mod types;
use super as f;
use super::super as n;
use super::{
    BoolExpr, ComponentMetric, Digest32, IndependenceStatus, InputBasis, Operator, Rational,
    ResourceClaim, ResourceError, ResourceReservationView, SemanticValidation, SmtStatusQuery,
    SourceStore, ValidatedFormalCandidate, Work,
};
pub use current::{Current, Owned};
use lc631_analysis::creativity_measurement::{
    ComponentReport, ComponentState, MeasurementBinding, MeasurementError, MeasurementGap,
    RationalInterval,
};
use lc631_substrate::measurement_numeric as numeric;
pub use plan::Root;
use std::cell::Cell;
pub use types::*;
fn framework(e: f::Error) -> Error {
    match e {
        f::Error::Resource(e) => Error::Resource(e),
        f::Error::ForeignReservation => Error::ForeignReservation,
        f::Error::WorkBudget => Error::WorkBudget,
        other => Error::Framework(other),
    }
}
fn native(e: n::Error) -> Error {
    match e {
        n::Error::Resource(e) => Error::Resource(e),
        n::Error::ForeignReservation => Error::ForeignReservation,
        n::Error::WorkBudget => Error::WorkBudget,
        n::Error::InvalidWeight => Error::InvalidWeight,
        n::Error::DuplicateFeature => Error::DuplicateFeature,
        n::Error::UniverseMismatch => Error::UniverseMismatch,
        n::Error::PayloadBudget => Error::PayloadBudget,
        n::Error::Arithmetic(MeasurementError::NumericBudget) => Error::NumericBudget,
        n::Error::Arithmetic(e) => Error::Arithmetic(e),
        other => Error::Framework(f::Error::Context(other)),
    }
}
fn ratio(n: i64, d: i64) -> Result<RationalInterval, Error> {
    RationalInterval::exact_ratio(n, d).map_err(Error::Numeric)
}
