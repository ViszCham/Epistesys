#![forbid(unsafe_code)]

pub const PROGRAM_ANALYSIS_SCHEMA: &str = "lc631-program-analysis.v1";

mod assembly;
mod cargo_world;
mod corpus;
mod model;
mod pipeline;
mod process;
mod python;
mod python_script;
mod release;
mod rust;
mod validation;

pub use model::*;
pub use pipeline::{analyze, analyze_path, analyze_with_receipts, AnalysisRequest};
pub use process::{run_bounded_tool, BoundedToolRequest, BoundedToolResult, ToolRunState};
pub use release::{
    evaluation_payload_digest, expected_v630_baseline_digest, release_component_scope,
    stage_completion_payload_digest,
};
pub use validation::{validation_receipt_payload_digest, validation_receipt_scope};
