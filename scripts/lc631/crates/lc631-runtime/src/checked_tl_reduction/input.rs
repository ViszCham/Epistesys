//! Actual finite-model input constructors only. Declarations remain Given.
pub use lc631_solvers::causal_spatial::{FrameId, Rectangle, SpatialRelation};
pub use lc631_solvers::temporal::{
    ClockSpec, EvaluationBudget, FiniteTrace, Instant, TemporalNode, TemporalProfile,
    TemporalQuery, TimeScale, TimeUnit, TraceClosure, TraceStep, Truth,
};
