//! Bounded QF_LIA model reevaluation, not source truth or proof checking.
//! The checked result borrows the exact typed query that was evaluated.
mod evaluate;
mod model;
pub mod native_model;
pub mod native_probe;
mod optimization;

pub use model::{validate_model, CheckedModel, ModelValidationError};
pub use optimization::{ObjectiveDirection, OptimizationProbe, OptimizationProbeError};
