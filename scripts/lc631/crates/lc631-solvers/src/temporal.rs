//! 日本語: 閉じた有限traceの評価。無限未来・因果・権限を導出しない。
//! English: Closed finite-trace evaluation, not infinite-future, causal or authority evidence.
mod evaluate;
mod formula;
mod time;
mod trace;

pub use evaluate::{evaluate, EvaluationBudget, TemporalEvaluation};
pub use formula::{TemporalNode, TemporalProfile, TemporalQuery};
pub use time::{ClockSpec, Instant, Interval, TimeScale, TimeUnit};
pub use trace::{FiniteTrace, TraceClosure, TraceStep, Truth};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TemporalError {
    EmptyTrace,
    TraceLimitExceeded,
    AtomLimitExceeded,
    InconsistentAtomWidth,
    NonIncreasingTime,
    ClockMismatch,
    UnsupportedPrecision,
    UnsupportedOpenPrefix,
    UnsupportedLoop,
    UnsupportedInfiniteProfile,
    EmptyOrReversedInterval,
    ArithmeticOverflow,
    EmptyFormula,
    FormulaLimitExceeded,
    InvalidNodeReference,
    MissingAtom,
    InvalidBudget,
    ResourceExhausted,
    AllocationFailed,
}
