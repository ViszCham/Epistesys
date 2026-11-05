//! Explicit formal services path. The old pure solve remains a legacy contract.
//! Actual native evidence is retained; source interpretation/closure stay separate.
#![forbid(unsafe_code)]
mod decision;
mod evaluation;
mod optimization;
pub mod replay;
pub use optimization::{
    ObjectiveNormalization, ObjectiveStepState, OneStepBudget, OptimizationError,
    OwnedObjectiveStep,
};
mod execute;
pub use evaluation::{
    BooleanEvaluationState, BooleanProbeReceipt, DeclaredEvaluation, EvaluationError,
    OwnedBooleanMeasurement,
};
mod types;
pub use decision::{CanonicalRendered, OwnedCanonicalDecision, SelectionSummary};
pub use execute::solve_with_services;
pub use types::*;

// Boundary reexports: the CLI consumes these SAME types without a new solver edge.
pub use lc631_solvers::protocol::{sexpr::SExprLimits, QueryResultKind, SolverProtocol};
pub use lc631_solvers::registry::{
    BackendId, LogicProfile, SolverRegistration, SolverRegistry, SolverRegistryError,
    SolverResourceCaps,
};
pub use lc631_solvers::runner::ExecutionBudget;
pub use lc631_substrate::source::{SourceError, SourceSpan, SourceStore};
pub type CanonicalSolveRequest<'p, 'view> = CanonicalRequest<'p, 'view>;
