use lc631_solvers::{
    ConsequenceMode, ExecutionStatus, LogicalResult, ObjectiveResult, Quantification,
    ResultContractError, Satisfiability, SolverRun, Support, TraceDomain,
};

fn reported_results() -> [LogicalResult; 7] {
    [
        LogicalResult::Satisfiability(Satisfiability::Sat),
        LogicalResult::Entailment {
            support: Support::Supported,
            models: Quantification::ForAll,
        },
        LogicalResult::ModelCheck {
            support: Support::Supported,
            traces: TraceDomain::Finite,
        },
        LogicalResult::Inference {
            support: Support::Supported,
            mode: ConsequenceMode::Cautious,
        },
        LogicalResult::Optimization(ObjectiveResult::Bounded { lower: 1, upper: 3 }),
        LogicalResult::Intervention(Support::Supported),
        LogicalResult::SpatialRelation(Support::Supported),
    ]
}

#[test]
fn tlcx01_incomplete_execution_cannot_carry_a_logical_result() {
    for status in [
        ExecutionStatus::Timeout,
        ExecutionStatus::ResourceExhausted,
        ExecutionStatus::Unsupported,
        ExecutionStatus::Failed,
    ] {
        for result in reported_results() {
            assert_eq!(
                SolverRun::reported(status, Some(result)),
                Err(ResultContractError::ResultAfterIncompleteExecution),
            );
        }
    }
}

#[test]
fn tlcx01_timeout_cannot_report_unsat() {
    assert_eq!(
        SolverRun::reported(
            ExecutionStatus::Timeout,
            Some(LogicalResult::Satisfiability(Satisfiability::Unsat)),
        ),
        Err(ResultContractError::ResultAfterIncompleteExecution),
    );
}

#[test]
fn tlcx01_completed_execution_requires_a_result_but_not_a_known_answer() {
    assert_eq!(
        SolverRun::reported(ExecutionStatus::Completed, None),
        Err(ResultContractError::MissingCompletedResult),
    );
    let unknown = LogicalResult::Satisfiability(Satisfiability::Unknown);
    let run = SolverRun::reported(ExecutionStatus::Completed, Some(unknown)).unwrap();
    assert_eq!(run.status(), ExecutionStatus::Completed);
    assert_eq!(run.result(), Some(unknown));
}

#[test]
fn tlcx01_stopped_executions_retain_their_exact_status() {
    for status in [
        ExecutionStatus::Timeout,
        ExecutionStatus::ResourceExhausted,
        ExecutionStatus::Unsupported,
        ExecutionStatus::Failed,
    ] {
        let run = SolverRun::reported(status, None).unwrap();
        assert_eq!(run.status(), status);
        assert_eq!(run.result(), None);
    }
}

#[test]
fn tlcx01_optimization_rejects_reversed_bounds_without_arithmetic() {
    for (lower, upper) in [(9, 2), (i64::MAX, i64::MIN)] {
        assert_eq!(
            SolverRun::reported(
                ExecutionStatus::Completed,
                Some(LogicalResult::Optimization(ObjectiveResult::Bounded {
                    lower,
                    upper,
                })),
            ),
            Err(ResultContractError::ReversedObjectiveBounds),
        );
    }
}

#[test]
fn tlcx01_optimization_retains_bounds_unbounded_and_unknown() {
    for objective in [
        ObjectiveResult::Bounded { lower: 0, upper: 0 },
        ObjectiveResult::Bounded {
            lower: i64::MIN,
            upper: i64::MAX,
        },
        ObjectiveResult::Unbounded,
        ObjectiveResult::Unknown,
    ] {
        let result = LogicalResult::Optimization(objective);
        let run = SolverRun::reported(ExecutionStatus::Completed, Some(result)).unwrap();
        assert_eq!(run.result(), Some(result));
    }
}

#[test]
fn tlcx01_satisfiability_is_not_entailment() {
    assert_ne!(
        LogicalResult::Satisfiability(Satisfiability::Sat),
        LogicalResult::Entailment {
            support: Support::Supported,
            models: Quantification::ForAll,
        },
    );
}

#[test]
fn tlcx01_query_semantics_remain_distinct() {
    assert_ne!(Quantification::Exists, Quantification::ForAll);
    assert_ne!(TraceDomain::Finite, TraceDomain::Infinite);
    assert_ne!(ConsequenceMode::Brave, ConsequenceMode::Cautious);
    assert_ne!(Support::Neither, Support::Refuted);
    assert_ne!(Support::Both, Support::Neither);
}

#[test]
fn tlcx01_all_declared_result_families_have_a_reported_positive_path() {
    let results = reported_results();
    assert_eq!(results.len(), 7, "the positive fixture must not be empty");
    for result in results {
        let report = SolverRun::reported(ExecutionStatus::Completed, Some(result)).unwrap();
        assert_eq!(report.result(), Some(result));
    }
}
