#[path = "common/tlcx10_result_fixture.rs"]
mod fixture;

use fixture::{
    budget, query, query_with_payload, registration, report_with, resource_owner, source,
    CancelledRunner,
};
use lc631_solvers::protocol::ValidationBasis;
use lc631_solvers::registry::SolverRegistry;
use lc631_solvers::result_store::{
    ResultStore, ResultStoreError, ResultStoreLookupError, SolverResultBinding,
};
use lc631_solvers::runner::{
    execute_registered, ProcessCleanupState, ProcessRunState, SolverExecutionState, SolverQuery,
};
use lc631_solvers::supervisor_adapter::append_report_for_query;
use lc631_solvers::{LogicalResult, Satisfiability};

#[test]
fn tlcx10_store_handoff_retains_solver_report_without_truth_promotion() {
    let query = query(source());
    let report = report_with(&query, ProcessRunState::Completed, b"sat\n", false, true);
    let mut store = ResultStore::with_capacity(2).unwrap();

    let receipt = append_report_for_query(&mut store, &query, report);
    assert_eq!(receipt.report(), report);
    let insertion = receipt.insertion().unwrap();
    assert_eq!(insertion.sequence(), 1);
    let stored = store
        .lookup(SolverResultBinding::for_query(
            &query,
            report.invocation_digest(),
        ))
        .unwrap();
    assert_eq!(
        stored.report().state(),
        SolverExecutionState::SolverReported
    );
    assert_eq!(
        stored.report().logical_result(),
        Some(LogicalResult::Satisfiability(Satisfiability::Sat))
    );
    assert_eq!(
        stored.report().validation_basis(),
        Some(ValidationBasis::SolverReported)
    );
    assert!(stored.report().stdout_fully_observed());
    assert!(stored.report().stderr_fully_observed());
    assert!(!stored.report().proof_checked());
    assert!(!stored.report().authority_created());
    assert!(!stored.report().output_commit_allowed());
    assert_eq!(stored.id(), insertion.id());
}

#[test]
fn tlcx10_store_handoff_keeps_solver_unknown_as_reported_unknown() {
    let query = query(source());
    let report = report_with(
        &query,
        ProcessRunState::Completed,
        b"unknown\n",
        false,
        true,
    );
    let mut store = ResultStore::with_capacity(1).unwrap();
    assert!(append_report_for_query(&mut store, &query, report)
        .insertion()
        .is_ok());

    let stored = store
        .lookup(SolverResultBinding::for_query(
            &query,
            report.invocation_digest(),
        ))
        .unwrap();
    assert_eq!(
        stored.report().state(),
        SolverExecutionState::SolverReported
    );
    assert_eq!(
        stored.report().logical_result(),
        Some(LogicalResult::Satisfiability(Satisfiability::Unknown))
    );
    assert_eq!(
        stored.report().validation_basis(),
        Some(ValidationBasis::SolverReported)
    );
    assert!(stored.report().stdout_fully_observed());
    assert!(stored.report().stderr_fully_observed());
    assert!(!stored.report().proof_checked());
}

#[test]
fn tlcx10_store_scope_mismatch_returns_the_original_report() {
    let report_query = query(source());
    let current_query = query_with_payload(source(), b"(assert true)\n(check-sat)\n");
    let report = report_with(
        &report_query,
        ProcessRunState::Completed,
        b"sat\n",
        false,
        true,
    );
    let mut store = ResultStore::with_capacity(2).unwrap();

    let receipt = append_report_for_query(&mut store, &current_query, report);
    assert_eq!(receipt.report(), report);
    assert_eq!(receipt.insertion(), Err(ResultStoreError::BindingMismatch));
    assert!(store.is_empty());
}

#[test]
fn tlcx10_store_preserves_timeout_truncation_and_cleanup_unknown() {
    let timeout_query = query(source());
    let truncation_query = query_with_payload(source(), b"(assert false)\n(check-sat)\n");
    let cleanup_query = query_with_payload(source(), b"(assert true)\n(check-sat)\n");
    let timeout = report_with(&timeout_query, ProcessRunState::TimedOut, b"", false, false);
    let truncated = report_with(
        &truncation_query,
        ProcessRunState::Completed,
        b"sat\n",
        true,
        true,
    );
    let cleanup_unknown = report_with(
        &cleanup_query,
        ProcessRunState::Completed,
        b"sat\n",
        false,
        false,
    );
    let mut store = ResultStore::with_capacity(4).unwrap();
    assert!(append_report_for_query(&mut store, &timeout_query, timeout)
        .insertion()
        .is_ok());
    assert!(
        append_report_for_query(&mut store, &truncation_query, truncated)
            .insertion()
            .is_ok()
    );
    assert!(
        append_report_for_query(&mut store, &cleanup_query, cleanup_unknown)
            .insertion()
            .is_ok()
    );

    let timeout_stored = store
        .lookup(SolverResultBinding::for_query(
            &timeout_query,
            timeout.invocation_digest(),
        ))
        .unwrap();
    assert_eq!(
        timeout_stored.report().state(),
        SolverExecutionState::TimedOut
    );
    assert_eq!(timeout_stored.report().logical_result(), None);
    assert_eq!(timeout_stored.report().validation_basis(), None);
    assert!(!timeout_stored.report().stdout_fully_observed());
    assert!(!timeout_stored.report().stderr_fully_observed());

    let truncated_stored = store
        .lookup(SolverResultBinding::for_query(
            &truncation_query,
            truncated.invocation_digest(),
        ))
        .unwrap();
    assert_eq!(
        truncated_stored.report().state(),
        SolverExecutionState::OutputTruncated
    );
    assert!(truncated_stored.report().stdout_truncated());
    assert!(!truncated_stored.report().stdout_fully_observed());
    assert!(truncated_stored.report().stderr_fully_observed());
    assert_eq!(truncated_stored.report().logical_result(), None);

    let cleanup_stored = store
        .lookup(SolverResultBinding::for_query(
            &cleanup_query,
            cleanup_unknown.invocation_digest(),
        ))
        .unwrap();
    assert_eq!(
        cleanup_stored.report().state(),
        SolverExecutionState::CleanupUnknown
    );
    assert!(!cleanup_stored.report().stdout_fully_observed());
    assert!(!cleanup_stored.report().stderr_fully_observed());
    assert_eq!(cleanup_stored.report().logical_result(), None);
}

#[test]
fn tlcx10_store_handoff_preserves_unsupported_mapping() {
    let source = source();
    let query = SolverQuery::checked(
        lc631_solvers::registry::BackendId::Z3,
        source,
        lc631_solvers::registry::LogicProfile::SmtBooleanQfLia,
        lc631_solvers::protocol::QueryResultKind::ModelPayload,
        b"(check-sat)\n",
    )
    .unwrap();
    let report = report_with(&query, ProcessRunState::Completed, b"sat\n", false, true);
    let mut store = ResultStore::with_capacity(1).unwrap();
    assert!(append_report_for_query(&mut store, &query, report)
        .insertion()
        .is_ok());

    let stored = store
        .lookup(SolverResultBinding::for_query(
            &query,
            report.invocation_digest(),
        ))
        .unwrap();
    assert_eq!(
        stored.report().state(),
        SolverExecutionState::UnsupportedMapping
    );
    assert_eq!(stored.report().logical_result(), None);
    assert_eq!(stored.report().validation_basis(), None);
    assert!(!stored.report().stdout_fully_observed());
    assert!(!stored.report().stderr_fully_observed());
}

#[test]
fn tlcx10_store_handoff_marks_prior_invocation_stale() {
    let query = query(source());
    let report = report_with(&query, ProcessRunState::Completed, b"sat\n", false, true);
    let current = SolverResultBinding::for_query(&query, report.invocation_digest());
    let mut store = ResultStore::with_capacity(2).unwrap();
    assert!(append_report_for_query(&mut store, &query, report)
        .insertion()
        .is_ok());

    let stale = SolverResultBinding::for_query(
        &query,
        Some(lc631_solvers::runner::InvocationDigest::from_bytes(
            [0xff; 32],
        )),
    );
    assert_eq!(
        store.lookup(stale),
        Err(ResultStoreLookupError::StaleBinding)
    );
    assert_eq!(
        store.lookup(current).unwrap().report().state(),
        SolverExecutionState::SolverReported
    );
}

#[test]
fn tlcx10_store_keeps_divergent_repeat_reports_and_marks_lookup_ambiguous() {
    let query = query(source());
    let sat = report_with(&query, ProcessRunState::Completed, b"sat\n", false, true);
    let unsat = report_with(&query, ProcessRunState::Completed, b"unsat\n", false, true);
    let binding = SolverResultBinding::for_query(&query, sat.invocation_digest());
    let mut store = ResultStore::with_capacity(2).unwrap();

    let first = append_report_for_query(&mut store, &query, sat)
        .insertion()
        .unwrap();
    let second = append_report_for_query(&mut store, &query, unsat)
        .insertion()
        .unwrap();
    assert_eq!(first.sequence(), 1);
    assert_eq!(second.sequence(), 2);
    assert_ne!(first.id(), second.id());
    assert_eq!(
        store.lookup(binding),
        Err(ResultStoreLookupError::AmbiguousResults)
    );

    let outcomes: Vec<_> = store
        .results_for(binding)
        .map(|stored| stored.report().logical_result())
        .collect();
    assert_eq!(outcomes.len(), 2);
    assert!(outcomes.contains(&Some(LogicalResult::Satisfiability(Satisfiability::Sat))));
    assert!(outcomes.contains(&Some(LogicalResult::Satisfiability(Satisfiability::Unsat))));
}

#[test]
fn tlcx10_store_failure_returns_the_terminal_report_and_does_not_evict() {
    let first_query = query(source());
    let second_query = query_with_payload(source(), b"(assert false)\n(check-sat)\n");
    let first = report_with(
        &first_query,
        ProcessRunState::Completed,
        b"sat\n",
        false,
        true,
    );
    let second = report_with(&second_query, ProcessRunState::TimedOut, b"", false, false);
    let mut store = ResultStore::with_capacity(1).unwrap();
    assert!(append_report_for_query(&mut store, &first_query, first)
        .insertion()
        .is_ok());

    let receipt = append_report_for_query(&mut store, &second_query, second);
    assert_eq!(receipt.report(), second);
    assert_eq!(receipt.insertion(), Err(ResultStoreError::CapacityReached));
    assert_eq!(receipt.report().state(), SolverExecutionState::TimedOut);
    assert_eq!(receipt.report().logical_result(), None);
    assert_eq!(store.len(), 1);
}

#[test]
fn tlcx10_cancelled_report_is_stored_as_cancelled_with_unknown_cleanup() {
    let executable = std::env::current_exe().unwrap();
    let source = source();
    let (_issuer, lease) = resource_owner(source);
    let report = {
        let view = lease.view();
        let registry =
            SolverRegistry::checked(vec![registration(&executable, [0x51; 32])]).unwrap();
        let mut runner = CancelledRunner;
        execute_registered(&registry, &query(source), &budget(0), &view, &mut runner)
    };
    let query = query(source);
    let mut store = ResultStore::with_capacity(1).unwrap();
    assert!(append_report_for_query(&mut store, &query, report)
        .insertion()
        .is_ok());

    let stored = store
        .lookup(SolverResultBinding::for_query(
            &query,
            report.invocation_digest(),
        ))
        .unwrap();
    assert_eq!(stored.report().state(), SolverExecutionState::Cancelled);
    assert_eq!(
        stored.report().cleanup_state(),
        ProcessCleanupState::RunnerReportedUnknown
    );
    assert!(!stored.report().stdout_fully_observed());
    assert!(!stored.report().stderr_fully_observed());
    assert_eq!(stored.report().logical_result(), None);
    lease.close_confirmed().unwrap();
}
