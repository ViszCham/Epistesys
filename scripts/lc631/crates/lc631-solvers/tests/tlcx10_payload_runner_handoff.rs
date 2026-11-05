#[path = "common/tlcx10_result_fixture.rs"]
mod fixture;

use fixture::{budget, caps, resource_owner, source};
use lc631_solvers::protocol::payload::PayloadShape;
use lc631_solvers::protocol::sexpr::SExprError;
use lc631_solvers::protocol::{
    query_protocol_compatibility, QueryProtocolCompatibility, QueryResultKind, SolverProtocol,
    ValidationBasis,
};
use lc631_solvers::registry::{BackendId, LogicProfile, SolverRegistration, SolverRegistry};
use lc631_solvers::result_store::{ResultStore, ResultStoreLookupError, SolverResultBinding};
use lc631_solvers::runner::{
    execute_registered, LiveResourceReservation, ProcessFailure, ProcessRequest, ProcessRunState,
    RunnerReportedProcessObservation, SolverExecutionReport, SolverExecutionState,
    SolverProcessRunner, SolverQuery,
};
use lc631_solvers::{LogicalResult, Satisfiability};
use std::path::Path;

fn registry(backend: BackendId, profile: LogicProfile, protocol: SolverProtocol) -> SolverRegistry {
    SolverRegistry::checked(vec![registration(backend, profile, protocol, [0x62; 32])]).unwrap()
}

fn registration(
    backend: BackendId,
    profile: LogicProfile,
    protocol: SolverProtocol,
    digest: [u8; 32],
) -> SolverRegistration {
    let executable = std::env::current_exe().unwrap();
    SolverRegistration::checked(
        backend,
        profile,
        protocol,
        executable.to_str().expect("current executable is UTF-8"),
        "fixture-only-not-a-real-solver",
        digest,
        Vec::new(),
        caps(),
    )
    .unwrap()
}

fn query(
    backend: BackendId,
    profile: LogicProfile,
    result_kind: QueryResultKind,
    input: &'static [u8],
) -> SolverQuery<'static> {
    SolverQuery::checked(backend, source(), profile, result_kind, input).unwrap()
}

struct CountingRunner {
    calls: usize,
    state: ProcessRunState,
    stdout: &'static [u8],
    stdout_truncated: bool,
    stderr_truncated: bool,
    cleanup_verified: bool,
}

impl SolverProcessRunner for CountingRunner {
    fn run<'a>(
        &'a mut self,
        request: ProcessRequest<'a>,
        _reservation: &'a dyn LiveResourceReservation,
    ) -> Result<RunnerReportedProcessObservation<'a>, ProcessFailure> {
        self.calls += 1;
        Ok(RunnerReportedProcessObservation {
            state: self.state,
            source: request.source,
            query: request.query,
            invocation: request.invocation,
            observed_arguments: request.fixed_arguments,
            executable_digest: *request.expected_executable_digest,
            exit_code: (self.state == ProcessRunState::Completed).then_some(0),
            stdout: self.stdout,
            stderr: b"",
            stdout_truncated: self.stdout_truncated,
            stderr_truncated: self.stderr_truncated,
            cleanup_verified: self.cleanup_verified,
        })
    }
}

fn report_with(
    query: &SolverQuery<'_>,
    protocol: SolverProtocol,
    state: ProcessRunState,
    stdout: &'static [u8],
    stdout_truncated: bool,
    cleanup_verified: bool,
) -> SolverExecutionReport {
    let registry = registry(query.backend(), query.profile(), protocol);
    let (_issuer, lease) = resource_owner(query.source());
    let view = lease.view();
    let mut runner = CountingRunner {
        calls: 0,
        state,
        stdout,
        stdout_truncated,
        stderr_truncated: false,
        cleanup_verified,
    };
    let report = execute_registered(&registry, query, &budget(0), &view, &mut runner);
    lease.close_confirmed().unwrap();
    report
}

fn report_with_invocation_fixture(
    query: &SolverQuery<'_>,
    protocol: SolverProtocol,
) -> SolverExecutionReport {
    let executable = if cfg!(windows) {
        Path::new(r"C:\tools\fixture.exe")
    } else {
        Path::new("/usr/bin/fixture")
    };
    let registration = SolverRegistration::checked(
        query.backend(),
        query.profile(),
        protocol,
        executable.to_str().unwrap(),
        "fixture-only-not-a-real-solver",
        [0x62; 32],
        Vec::new(),
        caps(),
    )
    .unwrap();
    let registry = SolverRegistry::checked(vec![registration]).unwrap();
    let (_issuer, lease) = resource_owner(query.source());
    let view = lease.view();
    let mut runner = CountingRunner {
        calls: 0,
        state: ProcessRunState::Completed,
        stdout: b"sat\n",
        stdout_truncated: false,
        stderr_truncated: false,
        cleanup_verified: true,
    };
    let report = execute_registered(&registry, query, &budget(0), &view, &mut runner);
    lease.close_confirmed().unwrap();
    report
}

fn decode_hex32(value: &str) -> [u8; 32] {
    let mut output = [0_u8; 32];
    let (pairs, remainder) = value.as_bytes().as_chunks::<2>();
    assert!(remainder.is_empty());
    assert_eq!(pairs.len(), output.len());
    for (index, pair) in pairs.iter().enumerate() {
        let pair = std::str::from_utf8(pair).unwrap();
        output[index] = u8::from_str_radix(pair, 16).unwrap();
    }
    output
}

#[test]
fn tlcx10_payload_runner_reports_bounded_model_syntax_without_logical_truth() {
    let query = query(
        BackendId::Z3,
        LogicProfile::SmtBooleanQfLia,
        QueryResultKind::ModelPayload,
        b"(model (define-fun x () Int 1))",
    );
    let report = report_with(
        &query,
        SolverProtocol::SmtLibPayloadV1,
        ProcessRunState::Completed,
        b"(model (define-fun x () Int 1))",
        false,
        true,
    );

    assert_eq!(report.state(), SolverExecutionState::PayloadSyntaxObserved);
    assert_eq!(report.logical_result(), None);
    assert_eq!(report.validation_basis(), None);
    let summary = report.payload_syntax_summary().unwrap();
    assert_eq!(summary.requested_shape(), PayloadShape::Model);
    assert_eq!(summary.root_count(), 1);
    assert_eq!(summary.node_count(), 8);
    assert_eq!(report.payload_syntax_error(), None);
    assert_eq!(report.clingo_output_summary(), None);
    assert!(!report.proof_checked());
    assert!(!report.authority_created());
    assert!(!report.output_commit_allowed());

    let mut store = ResultStore::with_capacity(1).unwrap();
    let insertion = store.append_for_query(&query, report).unwrap();
    let binding = SolverResultBinding::for_query(&query, report.invocation_digest());
    let stored = store.lookup(binding).unwrap();
    assert_eq!(
        stored.report().state(),
        SolverExecutionState::PayloadSyntaxObserved
    );
    assert_eq!(stored.id(), insertion.id());
    assert_eq!(stored.report_fingerprint().len(), 32);
}

#[test]
fn tlcx10_malformed_payload_syntax_is_rejected_without_logical_result() {
    let query = query(
        BackendId::Z3,
        LogicProfile::SmtBooleanQfLia,
        QueryResultKind::ProofPayload,
        b"(proof (asserted (= x 1))",
    );
    let report = report_with(
        &query,
        SolverProtocol::SmtLibPayloadV1,
        ProcessRunState::Completed,
        b"(proof (asserted (= x 1))",
        false,
        true,
    );

    assert_eq!(report.state(), SolverExecutionState::PayloadSyntaxRejected);
    assert_eq!(report.logical_result(), None);
    assert_eq!(report.validation_basis(), None);
    assert_eq!(
        report.payload_syntax_error(),
        Some(
            lc631_solvers::protocol::payload::PayloadSyntaxError::Syntax(
                SExprError::UnclosedList { offset: 0 }
            )
        )
    );

    let mut store = ResultStore::with_capacity(1).unwrap();
    store.append_for_query(&query, report).unwrap();
    let stored = store
        .lookup(SolverResultBinding::for_query(
            &query,
            report.invocation_digest(),
        ))
        .unwrap();
    assert_eq!(
        stored.report().state(),
        SolverExecutionState::PayloadSyntaxRejected
    );
    assert_eq!(stored.report().logical_result(), None);
}

#[test]
fn tlcx10_payload_protocol_pairings_are_preflighted_without_child_dispatch() {
    for (backend, profile, protocol, result_kind) in [
        (
            BackendId::Z3,
            LogicProfile::SmtBooleanQfLia,
            SolverProtocol::SmtLibStatusV1,
            QueryResultKind::ModelPayload,
        ),
        (
            BackendId::Z3,
            LogicProfile::SmtBooleanQfLia,
            SolverProtocol::SmtLibPayloadV1,
            QueryResultKind::Satisfiability,
        ),
        (
            BackendId::Clingo,
            LogicProfile::AspFiniteGroundNormal,
            SolverProtocol::ClingoStatusV1,
            QueryResultKind::ModelPayload,
        ),
    ] {
        assert_eq!(
            query_protocol_compatibility(protocol, result_kind),
            QueryProtocolCompatibility::Unsupported
        );
        let query = query(backend, profile, result_kind, b"(model)");
        let registry = registry(backend, profile, protocol);
        let (_issuer, lease) = resource_owner(query.source());
        let view = lease.view();
        let mut runner = CountingRunner {
            calls: 0,
            state: ProcessRunState::Completed,
            stdout: b"(model)",
            stdout_truncated: false,
            stderr_truncated: false,
            cleanup_verified: true,
        };
        let report = execute_registered(&registry, &query, &budget(0), &view, &mut runner);
        assert_eq!(report.state(), SolverExecutionState::UnsupportedMapping);
        assert_eq!(report.logical_result(), None);
        assert_eq!(runner.calls, 0);
        lease.close_confirmed().unwrap();
    }
}

#[test]
fn tlcx10_registry_routes_same_profile_by_query_kind_and_counts_only_status_profile() {
    let dual = SolverRegistry::checked(vec![
        registration(
            BackendId::Z3,
            LogicProfile::SmtBooleanQfLia,
            SolverProtocol::SmtLibStatusV1,
            [0x62; 32],
        ),
        registration(
            BackendId::Z3,
            LogicProfile::SmtBooleanQfLia,
            SolverProtocol::SmtLibPayloadV1,
            [0x63; 32],
        ),
    ])
    .unwrap();

    assert_eq!(
        dual.resolve(BackendId::Z3, LogicProfile::SmtBooleanQfLia),
        Err(lc631_solvers::registry::SolverRegistryError::AmbiguousRegistration)
    );
    assert_eq!(
        dual.resolve_for_query(
            BackendId::Z3,
            LogicProfile::SmtBooleanQfLia,
            QueryResultKind::Satisfiability,
        )
        .unwrap()
        .protocol(),
        SolverProtocol::SmtLibStatusV1
    );
    assert_eq!(
        dual.resolve_for_query(
            BackendId::Z3,
            LogicProfile::SmtBooleanQfLia,
            QueryResultKind::ModelPayload,
        )
        .unwrap()
        .protocol(),
        SolverProtocol::SmtLibPayloadV1
    );
    assert_eq!(
        dual.resolve_for_query(
            BackendId::Z3,
            LogicProfile::SmtBooleanQfLia,
            QueryResultKind::Entailment,
        ),
        Err(lc631_solvers::registry::SolverRegistryError::UnsupportedMapping)
    );

    let payload_only = SolverRegistry::checked(vec![registration(
        BackendId::Z3,
        LogicProfile::SmtBooleanQfLia,
        SolverProtocol::SmtLibPayloadV1,
        [0x64; 32],
    )])
    .unwrap();
    assert!(!payload_only.completeness().z3_qf_lia_registered);
}

#[test]
fn tlcx10_registry_executes_status_and_payload_on_disjoint_protocols() {
    let dual = SolverRegistry::checked(vec![
        registration(
            BackendId::Z3,
            LogicProfile::SmtBooleanQfLia,
            SolverProtocol::SmtLibStatusV1,
            [0x62; 32],
        ),
        registration(
            BackendId::Z3,
            LogicProfile::SmtBooleanQfLia,
            SolverProtocol::SmtLibPayloadV1,
            [0x63; 32],
        ),
    ])
    .unwrap();
    let status_query = query(
        BackendId::Z3,
        LogicProfile::SmtBooleanQfLia,
        QueryResultKind::Satisfiability,
        b"(check-sat)\n",
    );
    let (_issuer, status_lease) = resource_owner(status_query.source());
    let status_view = status_lease.view();
    let mut status_runner = CountingRunner {
        calls: 0,
        state: ProcessRunState::Completed,
        stdout: b"sat\n",
        stdout_truncated: false,
        stderr_truncated: false,
        cleanup_verified: true,
    };
    let status = execute_registered(
        &dual,
        &status_query,
        &budget(0),
        &status_view,
        &mut status_runner,
    );
    assert_eq!(status.state(), SolverExecutionState::SolverReported);
    assert_eq!(
        status.logical_result(),
        Some(LogicalResult::Satisfiability(Satisfiability::Sat))
    );
    assert_eq!(status_runner.calls, 1);
    status_lease.close_confirmed().unwrap();

    let payload_query = query(
        BackendId::Z3,
        LogicProfile::SmtBooleanQfLia,
        QueryResultKind::ModelPayload,
        b"(get-model)\n",
    );
    let (_issuer, payload_lease) = resource_owner(payload_query.source());
    let payload_view = payload_lease.view();
    let mut payload_runner = CountingRunner {
        calls: 0,
        state: ProcessRunState::Completed,
        stdout: b"(model)",
        stdout_truncated: false,
        stderr_truncated: false,
        cleanup_verified: true,
    };
    let payload = execute_registered(
        &dual,
        &payload_query,
        &budget(0),
        &payload_view,
        &mut payload_runner,
    );
    assert_eq!(payload.state(), SolverExecutionState::PayloadSyntaxObserved);
    assert_eq!(payload.logical_result(), None);
    assert_eq!(payload.validation_basis(), None);
    assert_eq!(payload_runner.calls, 1);
    payload_lease.close_confirmed().unwrap();

    let unsupported_query = query(
        BackendId::Z3,
        LogicProfile::SmtBooleanQfLia,
        QueryResultKind::Entailment,
        b"(assert false)\n",
    );
    let (_issuer, unsupported_lease) = resource_owner(unsupported_query.source());
    let unsupported_view = unsupported_lease.view();
    let mut unsupported_runner = CountingRunner {
        calls: 0,
        state: ProcessRunState::Completed,
        stdout: b"sat\n",
        stdout_truncated: false,
        stderr_truncated: false,
        cleanup_verified: true,
    };
    let unsupported = execute_registered(
        &dual,
        &unsupported_query,
        &budget(0),
        &unsupported_view,
        &mut unsupported_runner,
    );
    assert_eq!(
        unsupported.state(),
        SolverExecutionState::UnsupportedMapping
    );
    assert_eq!(unsupported_runner.calls, 0);
    unsupported_lease.close_confirmed().unwrap();
}

#[test]
fn tlcx10_status_v1_remains_status_only_and_preserves_solver_reported_unknown() {
    assert_eq!(
        query_protocol_compatibility(
            SolverProtocol::SmtLibStatusV1,
            QueryResultKind::Satisfiability
        ),
        QueryProtocolCompatibility::Supported
    );
    assert_eq!(
        query_protocol_compatibility(
            SolverProtocol::SmtLibStatusV1,
            QueryResultKind::ModelPayload
        ),
        QueryProtocolCompatibility::Unsupported
    );

    let query = query(
        BackendId::Z3,
        LogicProfile::SmtBooleanQfLia,
        QueryResultKind::Satisfiability,
        b"(check-sat)\n",
    );
    let report = report_with(
        &query,
        SolverProtocol::SmtLibStatusV1,
        ProcessRunState::Completed,
        b"unknown\n",
        false,
        true,
    );
    assert_eq!(report.state(), SolverExecutionState::SolverReported);
    assert_eq!(
        report.logical_result(),
        Some(LogicalResult::Satisfiability(Satisfiability::Unknown))
    );
    assert_eq!(report.payload_syntax_summary(), None);
    assert_eq!(report.clingo_output_summary(), None);
    assert_eq!(
        report.validation_basis(),
        Some(ValidationBasis::SolverReported)
    );
}

#[test]
fn tlcx10_clingo_answer_spans_and_atom_line_counts_remain_solver_reported() {
    let query = query(
        BackendId::Clingo,
        LogicProfile::AspFiniteGroundNormal,
        QueryResultKind::StableModelExistence,
        b"a(1).\n",
    );
    let stdout = b"Answer: 1\na(1)\nb(2)\nAnswer: 2\nc(3)\nSATISFIABLE\nModels : 2\n";
    let report = report_with(
        &query,
        SolverProtocol::ClingoStatusV1,
        ProcessRunState::Completed,
        stdout,
        false,
        true,
    );

    assert_eq!(report.state(), SolverExecutionState::SolverReported);
    assert_eq!(
        report.logical_result(),
        Some(LogicalResult::Satisfiability(Satisfiability::Sat))
    );
    assert_eq!(
        report.validation_basis(),
        Some(ValidationBasis::SolverReported)
    );
    let summary = report.clingo_output_summary().unwrap();
    assert_eq!(summary.answer_count(), 2);
    assert_eq!(summary.atom_line_count(), 3);
    assert_eq!(summary.recorded_answer_span_count(), 2);
    assert_eq!(summary.answer_span(0).unwrap().number(), 1);
    assert_eq!(summary.answer_span(0).unwrap().span().start, 0);
    assert_eq!(summary.answer_span(1).unwrap().number(), 2);
    assert!(!summary.spans_truncated());
    assert_eq!(report.payload_syntax_summary(), None);
    assert!(!report.proof_checked());
}

#[test]
fn tlcx10_clingo_truncated_status_does_not_produce_status_or_span_summary() {
    let query = query(
        BackendId::Clingo,
        LogicProfile::AspFiniteGroundNormal,
        QueryResultKind::StableModelExistence,
        b"a(1).\n",
    );
    let report = report_with(
        &query,
        SolverProtocol::ClingoStatusV1,
        ProcessRunState::Completed,
        b"Answer: 1\na(1)\nSATISFIABLE\n",
        true,
        true,
    );
    assert_eq!(report.state(), SolverExecutionState::OutputTruncated);
    assert_eq!(report.logical_result(), None);
    assert_eq!(report.clingo_output_summary(), None);
}

#[test]
fn tlcx10_result_fingerprint_binds_fixed_syntax_summaries() {
    let query = query(
        BackendId::Z3,
        LogicProfile::SmtBooleanQfLia,
        QueryResultKind::ModelPayload,
        b"(check-sat)",
    );
    let first = report_with(
        &query,
        SolverProtocol::SmtLibPayloadV1,
        ProcessRunState::Completed,
        b"(model)",
        false,
        true,
    );
    let second = report_with(
        &query,
        SolverProtocol::SmtLibPayloadV1,
        ProcessRunState::Completed,
        b"(model x)",
        false,
        true,
    );
    let mut store = ResultStore::with_capacity(2).unwrap();
    store.append_for_query(&query, first).unwrap();
    store.append_for_query(&query, second).unwrap();
    let binding = SolverResultBinding::for_query(&query, first.invocation_digest());
    assert_eq!(
        store.lookup(binding),
        Err(ResultStoreLookupError::AmbiguousResults)
    );
    let fingerprints: Vec<_> = store
        .results_for(binding)
        .map(|entry| *entry.report_fingerprint())
        .collect();
    assert_eq!(fingerprints.len(), 2);
    assert_ne!(fingerprints[0], fingerprints[1]);
}

#[test]
fn tlcx10_clingo_summary_fingerprint_changes_with_answer_spans() {
    let query = query(
        BackendId::Clingo,
        LogicProfile::AspFiniteGroundNormal,
        QueryResultKind::StableModelExistence,
        b"a(1).\n",
    );
    let first = report_with(
        &query,
        SolverProtocol::ClingoStatusV1,
        ProcessRunState::Completed,
        b"Answer: 1\na(1)\nSATISFIABLE\n",
        false,
        true,
    );
    let second = report_with(
        &query,
        SolverProtocol::ClingoStatusV1,
        ProcessRunState::Completed,
        b"Answer: 1\na(1)\nAnswer: 2\na(2)\nSATISFIABLE\n",
        false,
        true,
    );
    let mut store = ResultStore::with_capacity(2).unwrap();
    store.append_for_query(&query, first).unwrap();
    store.append_for_query(&query, second).unwrap();
    let binding = SolverResultBinding::for_query(&query, first.invocation_digest());
    let fingerprints: Vec<_> = store
        .results_for(binding)
        .map(|entry| *entry.report_fingerprint())
        .collect();
    assert_eq!(fingerprints.len(), 2);
    assert_ne!(fingerprints[0], fingerprints[1]);
}

#[test]
fn tlcx10_smt_status_v1_invocation_digest_keeps_legacy_golden() {
    let query = query(
        BackendId::Z3,
        LogicProfile::SmtBooleanQfLia,
        QueryResultKind::Satisfiability,
        b"(check-sat)\n",
    );
    let report = report_with_invocation_fixture(&query, SolverProtocol::SmtLibStatusV1);
    let expected = if cfg!(windows) {
        decode_hex32("3c30b125e306b80997712f92564d095756df5151ec86c23d793ad824c5401770")
    } else {
        decode_hex32("fc437875ac2abfc825798c56a1168fb5eddebfe50d609b42286ff51dfcf48422")
    };
    assert_eq!(report.invocation_digest().unwrap().as_bytes(), &expected);
}
