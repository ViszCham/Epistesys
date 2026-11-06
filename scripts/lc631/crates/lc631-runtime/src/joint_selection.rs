//! Preparatory reported-measurement selection, not semantic validation or authority.
//! TLCX-10 and the canonical TLCX-17/29 producer/decision edges remain required.
#![forbid(unsafe_code)]

mod components;
/// Resource-owned two-direction formal preservation; source interpretation remains unverified.
pub mod formal;
mod frontier;
mod types;

pub use components::{
    project_assessment_reported, project_component_reported, ReportedComponentAxis,
};
pub use frontier::{compare_rows, select_joint_reported};
pub(crate) use frontier::{validate_policy, validate_rows};
pub use types::*;
