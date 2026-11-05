//! Solver contract types. Actual backend execution is not connected yet.
#![forbid(unsafe_code)]

pub mod asp;
pub mod causal_spatial;
pub mod finite_modal;
pub mod four_valued;
mod logical;
#[cfg(test)]
mod owned_payload_tests;
pub mod protocol;
pub mod registry;
pub mod result_store;
pub mod runner;
pub mod smt;
pub mod supervisor_adapter;
pub mod temporal;
pub mod typed_query;

pub use logical::{
    ConsequenceMode, ExecutionStatus, LogicalResult, ObjectiveResult, Quantification,
    ResultContractError, Satisfiability, SolverRun, Support, TraceDomain,
};
