//! Application entry contracts. Unconnected stages never report completion.
#![forbid(unsafe_code)]

pub mod adversarial_measurement;
pub mod breakcost_selection;
pub mod bridge_gain_measurement;
pub mod calibration;
mod candidates;
pub mod canonical_original_pending;
pub mod canonical_solve;
pub mod capability_measurement;
pub mod causal_spatial_observation;
pub mod checked_tl_reduction;
pub mod compatibility_measurement;
pub mod composite_original;
pub mod connection_audit;
mod connections;
pub mod contextual_distillation;
pub mod context_novelty_measurement;
pub mod contextual_repair;
pub mod contextual_resume;
pub mod creative_candidate_bridge;
pub mod crosslogic_loss;
pub mod description_measurement;
pub mod design_measurement;
pub mod explicit_formal;
pub mod explanation_measurement;
pub mod facet_ledger;
pub mod facet_measurement_bridge;
pub mod finite_modal_observation;
/// Actual finite checker consumption, distinct from external-process reports.
pub mod finite_results;
pub mod formal_boolean;
pub mod formal_measurement;
pub mod four_valued_observation;
pub mod four_valued_context;
pub mod goal_suite_measurement;
pub mod handoff;
pub mod joint_selection;
/// Bounded checkpoint-journal inspection; not a completion/restoration API.
pub mod journal;
pub mod materialized_world_bridge;
pub mod memory_measurement;
pub mod observation_bridge;
pub mod original_score_pending;
pub mod path_preservation;
pub mod prediction_measurement;
pub mod product_correspondence;
pub mod product_cost;
pub mod recommendation_output;
pub mod reconstruction_measurement;
pub mod redundancy_measurement;
pub mod reorganization_measurement;
pub mod resource;
pub mod scheduled_solver;
pub mod scoped_solver;
#[cfg(test)]
mod scoped_solver_tests;
pub mod semantic_accelerator;
pub mod solve_preparation;
pub mod solver_lowering;
pub mod spatial_preservation;
pub mod spatiotemporal_selection;
pub mod structural_gain_measurement;
pub mod temporal_preservation;
pub mod temporal_smt_point;
pub mod transfer_measurement;
pub mod turn_cache;
// Keep the store implementation private; only the bounded journal surface is
// exposed through `journal`. The main solver path remains explicitly unconnected.
#[allow(dead_code)]
pub(crate) mod store;

pub use candidates::{CandidateSet, CandidateSetError};
pub use connections::{connection_contracts, ConnectionContract, ConnectionState};

use lc631_core::context::{
    ContextRevision, DecisionStatus, MissingStage, SourceMismatch, SourceRevision, TaskKind,
};

pub struct SolveRequest<'a> {
    source: &'a str,
    context: ContextRevision,
    task: TaskKind,
}

impl<'a> SolveRequest<'a> {
    pub fn checked(
        source: &'a str,
        context: ContextRevision,
        task: TaskKind,
    ) -> Result<Self, SourceMismatch> {
        if SourceRevision::of(source) != context.source() {
            return Err(SourceMismatch);
        }
        Ok(Self {
            source,
            context,
            task,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeReport {
    pub context: ContextRevision,
    pub task: TaskKind,
    pub status: DecisionStatus,
    pub source_bytes: usize,
}

pub fn solve(request: SolveRequest<'_>) -> RuntimeReport {
    RuntimeReport {
        context: request.context,
        task: request.task,
        status: DecisionStatus::NotConnected(MissingStage::ContextLowering),
        source_bytes: request.source.len(),
    }
}
