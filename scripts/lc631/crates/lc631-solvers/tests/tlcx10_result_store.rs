use lc631_solvers::protocol::{QueryResultKind, SolverProtocol, ValidationBasis};
use lc631_solvers::registry::{
    BackendId, LogicProfile, SolverRegistration, SolverRegistry, SolverResourceCaps,
};
use lc631_solvers::result_store::{
    ResultStore, ResultStoreError, ResultStoreLookupError, SolverResultBinding,
};
use lc631_solvers::runner::{
    execute_registered, ExecutionBudget, InvocationDigest, LiveResourceReservation, ProcessFailure,
    ProcessRequest, ProcessRunState, QueryDigest, QuerySourceIdentity,
    RunnerReportedProcessObservation, SolverExecutionReport, SolverExecutionState,
    SolverProcessRunner, SolverQuery,
};
use lc631_solvers::Satisfiability;

const SOURCE: QuerySourceIdentity = QuerySourceIdentity::from_bytes([0x2a; 32]);
const fn large_output(last: u8) -> [u8; 5000] {
    let mut output = [b'a'; 5000];
    output[4999] = last;
    output
}
static OVERSIZE_A: [u8; 5000] = large_output(b'a');
static OVERSIZE_B: [u8; 5000] = large_output(b'b');

struct CurrentReservation;

impl LiveResourceReservation for CurrentReservation {
    fn validate(
        &self,
        _expected_source: QuerySourceIdentity,
        _budget: &ExecutionBudget,
    ) -> Result<(), lc631_solvers::runner::ReservationFailure> {
        Ok(())
    }
}

struct FixedRunner {
    status: &'static [u8],
    process_state: ProcessRunState,
    cleanup_verified: bool,
}

impl SolverProcessRunner for FixedRunner {
    fn run<'a>(
        &'a mut self,
        request: ProcessRequest<'a>,
        _reservation: &'a dyn LiveResourceReservation,
    ) -> Result<RunnerReportedProcessObservation<'a>, ProcessFailure> {
        Ok(RunnerReportedProcessObservation {
            state: self.process_state,
            source: request.source,
            query: request.query,
            invocation: request.invocation,
            observed_arguments: request.fixed_arguments,
            executable_digest: *request.expected_executable_digest,
            exit_code: (self.process_state == ProcessRunState::Completed).then_some(0),
            stdout: self.status,
            stderr: b"",
            stdout_truncated: false,
            stderr_truncated: false,
            cleanup_verified: self.cleanup_verified,
        })
    }
}

fn registry() -> SolverRegistry {
    // This is a synthetic runner fixture, never a real executable invocation.
    // Keep registry path admission intact on both supported host platforms.
    let executable = if cfg!(windows) {
        r"C:\EpistesysTests\z3.exe"
    } else {
        "/epistesys-tests/z3"
    };
    let caps = SolverResourceCaps {
        max_argv_count: 8,
        max_argv_bytes: 1024,
        max_stdin_bytes: 4096,
        max_stdout_bytes: 4096,
        max_stderr_bytes: 1024,
        max_timeout_ms: 5000,
        max_ram_bytes: 16 * 1024 * 1024,
        max_disk_bytes: 0,
        max_cleanup_bytes: 65_536,
        max_child_count: 1,
    };
    let registration = SolverRegistration::checked(
        BackendId::Z3,
        LogicProfile::SmtBooleanQfLia,
        SolverProtocol::SmtLibStatusV1,
        executable,
        "fixture-only-not-a-real-solver",
        [0x51; 32],
        Vec::new(),
        caps,
    )
    .unwrap();
    SolverRegistry::checked(vec![registration]).unwrap()
}

fn query(source: QuerySourceIdentity) -> SolverQuery<'static> {
    SolverQuery::checked(
        BackendId::Z3,
        source,
        LogicProfile::SmtBooleanQfLia,
        QueryResultKind::Satisfiability,
        b"(check-sat)\n",
    )
    .unwrap()
}

fn budget() -> ExecutionBudget {
    ExecutionBudget::checked(1000, 4096, 4096, 1024, 8, 1024, 8 * 1024 * 1024, 0, 65_536).unwrap()
}

fn report(
    query: &SolverQuery<'_>,
    output: &'static [u8],
    state: ProcessRunState,
) -> SolverExecutionReport {
    report_with_cleanup(query, output, state, state == ProcessRunState::Completed)
}

fn report_with_cleanup(
    query: &SolverQuery<'_>,
    output: &'static [u8],
    state: ProcessRunState,
    cleanup_verified: bool,
) -> SolverExecutionReport {
    let mut runner = FixedRunner {
        status: output,
        process_state: state,
        cleanup_verified,
    };
    execute_registered(
        &registry(),
        query,
        &budget(),
        &CurrentReservation,
        &mut runner,
    )
}

fn binding(query: &SolverQuery<'_>, invocation: Option<InvocationDigest>) -> SolverResultBinding {
    SolverResultBinding::for_query(query, invocation)
}

#[test]
fn tlcx10_stores_reported_status_without_promoting_it_to_proof_or_truth() {
    let query = query(SOURCE);
    let report = report(&query, b"sat\n", ProcessRunState::Completed);
    let mut store = ResultStore::with_capacity(4).unwrap();
    let inserted = store
        .append(binding(&query, report.invocation_digest()), report)
        .unwrap();

    let id = inserted.id();
    let stored = store
        .lookup(binding(&query, report.invocation_digest()))
        .unwrap();
    assert_eq!(stored.id(), id);
    assert_eq!(stored.sequence(), inserted.sequence());
    assert_eq!(
        stored.report().state(),
        SolverExecutionState::SolverReported
    );
    assert!(stored.report().stdout_fully_observed());
    assert!(stored.report().stderr_fully_observed());
    assert_eq!(
        stored.report().logical_result(),
        Some(lc631_solvers::LogicalResult::Satisfiability(
            Satisfiability::Sat
        ))
    );
    assert_eq!(
        stored.report().validation_basis(),
        Some(ValidationBasis::SolverReported)
    );
    assert!(!stored.report().proof_checked());
    assert!(!stored.report().authority_created());
    assert!(!stored.report().output_commit_allowed());
}

#[test]
fn tlcx10_scope_mismatch_is_rejected_without_mutating_the_store() {
    let query = query(SOURCE);
    let report = report(&query, b"sat\n", ProcessRunState::Completed);
    let wrong_scope = SolverResultBinding::from_parts(
        QuerySourceIdentity::from_bytes([0x7a; 32]),
        query.digest(),
        report.invocation_digest(),
    );
    let mut store = ResultStore::with_capacity(4).unwrap();

    assert_eq!(
        store.append(wrong_scope, report),
        Err(ResultStoreError::BindingMismatch)
    );
    assert_eq!(store.len(), 0);
}

#[test]
fn tlcx10_invocation_mismatch_is_rejected_without_rebinding_the_report() {
    let query = query(SOURCE);
    let report = report(&query, b"sat\n", ProcessRunState::Completed);
    let wrong_invocation =
        SolverResultBinding::for_query(&query, Some(InvocationDigest::from_bytes([0x99; 32])));
    let mut store = ResultStore::with_capacity(4).unwrap();

    assert_eq!(
        store.append(wrong_invocation, report),
        Err(ResultStoreError::BindingMismatch)
    );
    assert_eq!(store.len(), 0);
}

#[test]
fn tlcx10_incomplete_execution_is_preserved_without_a_logical_result() {
    let query = query(SOURCE);
    let report = report(&query, &OVERSIZE_A, ProcessRunState::TimedOut);
    let mut store = ResultStore::with_capacity(4).unwrap();
    store
        .append(binding(&query, report.invocation_digest()), report)
        .unwrap();

    let stored = store
        .lookup(binding(&query, report.invocation_digest()))
        .unwrap();
    assert_eq!(stored.report().state(), SolverExecutionState::TimedOut);
    assert_eq!(stored.report().logical_result(), None);
    assert_eq!(stored.report().validation_basis(), None);
    assert!(!stored.report().stdout_fully_observed());
    assert!(!stored.report().stderr_fully_observed());
}

#[test]
fn tlcx10_cleanup_unknown_does_not_claim_complete_stream_observation() {
    let query = query(SOURCE);
    let report = report_with_cleanup(&query, b"sat\n", ProcessRunState::Completed, false);
    assert_eq!(report.state(), SolverExecutionState::CleanupUnknown);
    assert!(!report.stdout_fully_observed());
    assert!(!report.stderr_fully_observed());
}

#[test]
fn tlcx10_identical_replay_is_retained_as_a_distinct_observation() {
    let query = query(SOURCE);
    let first = report(&query, b"sat\n", ProcessRunState::Completed);
    let replay = report(&query, b"sat\n", ProcessRunState::Completed);
    let current = binding(&query, first.invocation_digest());
    let mut store = ResultStore::with_capacity(4).unwrap();
    let first_insert = store.append(current, first).unwrap();
    let replay_insert = store.append(current, replay).unwrap();

    assert_ne!(first_insert.id(), replay_insert.id());
    assert_eq!(first_insert.sequence(), 1);
    assert_eq!(replay_insert.sequence(), 2);
    assert_eq!(store.len(), 2);
    assert_eq!(
        store.lookup(current),
        Err(ResultStoreLookupError::AmbiguousResults)
    );
    assert!(store.lookup_by_id(first_insert.id()).is_some());
    assert!(store.lookup_by_id(replay_insert.id()).is_some());
}

#[test]
fn tlcx10_old_invocation_is_not_returned_for_a_current_binding() {
    let query = query(SOURCE);
    let report = report(&query, b"sat\n", ProcessRunState::Completed);
    let current = binding(&query, report.invocation_digest());
    let mut store = ResultStore::with_capacity(4).unwrap();
    store.append(current, report).unwrap();

    let stale = binding(&query, Some(InvocationDigest::from_bytes([0xff; 32])));
    assert_eq!(
        store.lookup(stale),
        Err(ResultStoreLookupError::StaleBinding)
    );
}

#[test]
fn tlcx10_divergent_results_are_both_retained_and_never_selected_implicitly() {
    let query = query(SOURCE);
    let first = report(&query, b"sat\n", ProcessRunState::Completed);
    let second = report(&query, b"unsat\n", ProcessRunState::Completed);
    let current = binding(&query, first.invocation_digest());
    let mut store = ResultStore::with_capacity(4).unwrap();
    let first_insert = store.append(current, first).unwrap();
    let second_insert = store.append(current, second).unwrap();
    assert_ne!(first_insert.id(), second_insert.id());
    assert_eq!(store.len(), 2);
    assert_eq!(
        store.lookup(current),
        Err(ResultStoreLookupError::AmbiguousResults)
    );
    assert_eq!(
        store
            .lookup_by_id(first_insert.id())
            .unwrap()
            .report()
            .logical_result(),
        Some(lc631_solvers::LogicalResult::Satisfiability(
            Satisfiability::Sat
        ))
    );
    assert_eq!(
        store
            .lookup_by_id(second_insert.id())
            .unwrap()
            .report()
            .logical_result(),
        Some(lc631_solvers::LogicalResult::Satisfiability(
            Satisfiability::Unsat
        ))
    );
}

#[test]
fn tlcx10_distinct_same_length_stdout_remains_separately_addressable() {
    let query = query(SOURCE);
    // Both outputs parse to the same SMT status and have equal length, but
    // their exact source bytes differ. The store must keep the raw-output
    // witness in its report identity instead of collapsing parser-equivalent
    // byte streams.
    let first = report(&query, b"sat\n", ProcessRunState::Completed);
    let second = report(&query, b"sat ", ProcessRunState::Completed);
    assert_eq!(first.logical_result(), second.logical_result());
    assert_eq!(first.stdout_bytes(), second.stdout_bytes());
    assert_ne!(first.stdout_digest(), second.stdout_digest());
    let current = binding(&query, first.invocation_digest());
    let mut store = ResultStore::with_capacity(4).unwrap();
    let first_insert = store.append(current, first).unwrap();
    let second_insert = store.append(current, second).unwrap();
    assert_ne!(first_insert.id(), second_insert.id());
    assert_ne!(
        store
            .lookup_by_id(first_insert.id())
            .unwrap()
            .report_fingerprint(),
        store
            .lookup_by_id(second_insert.id())
            .unwrap()
            .report_fingerprint()
    );
    assert_eq!(
        store.lookup(current),
        Err(ResultStoreLookupError::AmbiguousResults)
    );
}

#[test]
fn tlcx10_early_timeout_keeps_bounded_prefix_digest_non_authoritative() {
    let query = query(SOURCE);
    let first = report(&query, &OVERSIZE_A, ProcessRunState::TimedOut);
    let second = report(&query, &OVERSIZE_B, ProcessRunState::TimedOut);
    assert_eq!(first.state(), SolverExecutionState::TimedOut);
    assert_eq!(first.stdout_bytes(), second.stdout_bytes());
    // Only the bounded prefix is hashed, so the digest is explicitly not a
    // complete identity for either oversized byte stream.
    assert_eq!(first.stdout_digest(), second.stdout_digest());
    assert!(!first.stdout_fully_observed());
    let current = binding(&query, first.invocation_digest());
    let mut store = ResultStore::with_capacity(4).unwrap();
    let first_insert = store.append(current, first).unwrap();
    let second_insert = store.append(current, second).unwrap();
    assert_ne!(first_insert.id(), second_insert.id());
    assert_eq!(store.len(), 2);
    assert_eq!(
        store.lookup(current),
        Err(ResultStoreLookupError::AmbiguousResults)
    );

    let completed_oversize = report(&query, &OVERSIZE_A, ProcessRunState::Completed);
    assert_eq!(
        completed_oversize.state(),
        SolverExecutionState::OutputTruncated
    );
    assert!(!completed_oversize.stdout_fully_observed());
}

#[test]
fn tlcx10_capacity_exhaustion_does_not_evict_prior_result() {
    let first_query = query(SOURCE);
    let second_query = query(QuerySourceIdentity::from_bytes([0x2b; 32]));
    let first = report(&first_query, b"sat\n", ProcessRunState::Completed);
    let second = report(&second_query, b"unsat\n", ProcessRunState::Completed);
    let first_binding = binding(&first_query, first.invocation_digest());
    let mut store = ResultStore::with_capacity(1).unwrap();
    store.append(first_binding, first).unwrap();

    assert_eq!(
        store.append(binding(&second_query, second.invocation_digest()), second),
        Err(ResultStoreError::CapacityReached)
    );
    assert_eq!(store.len(), 1);
    assert_eq!(
        store.lookup(first_binding).unwrap().report().state(),
        SolverExecutionState::SolverReported
    );
}

#[test]
fn tlcx10_invalid_store_capacity_is_rejected_before_allocation() {
    assert!(matches!(
        ResultStore::with_capacity(0),
        Err(ResultStoreError::InvalidCapacity)
    ));
    assert!(matches!(
        ResultStore::with_capacity(4097),
        Err(ResultStoreError::InvalidCapacity)
    ));
}

#[test]
fn tlcx10_binding_identity_contains_source_query_and_optional_invocation() {
    let query = query(SOURCE);
    let first = binding(&query, None);
    let second = binding(&query, Some(InvocationDigest::from_bytes([0x11; 32])));
    assert_ne!(first, second);
    assert_ne!(QueryDigest::from_bytes([0x10; 32]), query.digest());
}
