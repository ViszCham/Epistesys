//! Execution termination and logical meaning are separate dimensions.
/// Process/protocol termination, not logical truth or validated evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionStatus {
    Completed,
    Timeout,
    ResourceExhausted,
    Unsupported,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Satisfiability {
    Sat,
    Unsat,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Support {
    Supported,
    Refuted,
    Neither,
    Both,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Quantification {
    Exists,
    ForAll,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TraceDomain {
    Finite,
    Infinite,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConsequenceMode {
    Brave,
    Cautious,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectiveResult {
    Bounded { lower: i64, upper: i64 },
    Unbounded,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogicalResult {
    Satisfiability(Satisfiability),
    Entailment {
        support: Support,
        models: Quantification,
    },
    ModelCheck {
        support: Support,
        traces: TraceDomain,
    },
    Inference {
        support: Support,
        mode: ConsequenceMode,
    },
    Optimization(ObjectiveResult),
    Intervention(Support),
    SpatialRelation(Support),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResultContractError {
    MissingCompletedResult,
    ResultAfterIncompleteExecution,
    ReversedObjectiveBounds,
}

/// A shape-checked solver report, not an execution receipt or a proof token.
///
/// Partial incumbents from interrupted runs need a separate candidate channel;
/// they cannot be promoted to completed logical results through this type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SolverRun {
    status: ExecutionStatus,
    result: Option<LogicalResult>,
}

impl SolverRun {
    pub fn reported(
        status: ExecutionStatus,
        result: Option<LogicalResult>,
    ) -> Result<Self, ResultContractError> {
        match (status, result) {
            (ExecutionStatus::Completed, None) => {
                return Err(ResultContractError::MissingCompletedResult);
            }
            (ExecutionStatus::Completed, Some(_)) | (_, None) => {}
            (_, Some(_)) => {
                return Err(ResultContractError::ResultAfterIncompleteExecution);
            }
        }
        if let Some(LogicalResult::Optimization(ObjectiveResult::Bounded { lower, upper })) = result
        {
            if lower > upper {
                return Err(ResultContractError::ReversedObjectiveBounds);
            }
        }
        Ok(Self { status, result })
    }

    pub const fn status(self) -> ExecutionStatus {
        self.status
    }
    pub const fn result(self) -> Option<LogicalResult> {
        self.result
    }
}
