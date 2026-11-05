use lc631_solvers::protocol::{
    query_protocol_compatibility, ProtocolError, QueryProtocolCompatibility, QueryResultKind,
    SolverProtocol, ValidationBasis,
};
use lc631_solvers::registry::{
    BackendId, LogicProfile, SolverRegistration, SolverRegistry, SolverRegistryError,
    SolverResourceCaps,
};
use lc631_solvers::runner::{
    execute_registered, BinaryBindingState, ExecutionBudget, InvocationBindingState,
    InvocationDigest, LiveResourceReservation, ProcessCleanupState, ProcessFailure,
    ProcessRunState, QueryDigest, QuerySourceIdentity, ReservationFailure,
    RunnerReportedProcessObservation, SolverExecutionState, SolverProcessRunner, SolverQuery,
};
use lc631_solvers::{LogicalResult, Satisfiability};
use std::path::Path;

fn caps() -> SolverResourceCaps {
    SolverResourceCaps {
        max_argv_count: 16,
        max_argv_bytes: 4096,
        max_stdin_bytes: 4096,
        max_stdout_bytes: 4096,
        max_stderr_bytes: 1024,
        max_timeout_ms: 5000,
        max_ram_bytes: 8 * 1024 * 1024,
        max_disk_bytes: 0,
        max_cleanup_bytes: 65_536,
        max_child_count: 1,
    }
}

fn registration(
    id: BackendId,
    profile: LogicProfile,
    protocol: SolverProtocol,
) -> SolverRegistration {
    let executable = if cfg!(windows) {
        r"C:\tools\solver.exe"
    } else {
        "/usr/bin/solver"
    };
    SolverRegistration::checked(
        id,
        profile,
        protocol,
        executable,
        "5.0.0",
        [7; 32],
        vec!["--fixed-option".into()],
        caps(),
    )
    .unwrap()
}

fn registry() -> SolverRegistry {
    SolverRegistry::checked(vec![registration(
        BackendId::Z3,
        LogicProfile::SmtBooleanQfLia,
        SolverProtocol::SmtLibStatusV1,
    )])
    .unwrap()
}

fn asp_registry() -> SolverRegistry {
    SolverRegistry::checked(vec![registration(
        BackendId::Clingo,
        LogicProfile::AspFiniteGroundNormal,
        SolverProtocol::ClingoStatusV1,
    )])
    .unwrap()
}

fn query(
    backend: BackendId,
    source: QuerySourceIdentity,
    kind: QueryResultKind,
    bytes: &[u8],
) -> SolverQuery<'_> {
    SolverQuery::checked(backend, source, LogicProfile::SmtBooleanQfLia, kind, bytes).unwrap()
}

fn asp_query(source: QuerySourceIdentity, kind: QueryResultKind, bytes: &[u8]) -> SolverQuery<'_> {
    SolverQuery::checked(
        BackendId::Clingo,
        source,
        LogicProfile::AspFiniteGroundNormal,
        kind,
        bytes,
    )
    .unwrap()
}

fn budget() -> ExecutionBudget {
    ExecutionBudget::checked(1000, 4096, 4096, 1024, 16, 4096, 4 * 1024 * 1024, 0, 65_536).unwrap()
}

struct Lease {
    source: QuerySourceIdentity,
    valid: bool,
}

impl LiveResourceReservation for Lease {
    fn validate(
        &self,
        expected_source: QuerySourceIdentity,
        _budget: &ExecutionBudget,
    ) -> Result<(), ReservationFailure> {
        if !self.valid {
            return Err(ReservationFailure::Stale);
        }
        if self.source != expected_source {
            return Err(ReservationFailure::SourceMismatch);
        }
        Ok(())
    }
}

struct FakeRunner {
    calls: usize,
    output: &'static [u8],
    state: ProcessRunState,
    cleanup_verified: bool,
    truncated: bool,
    exit_code: Option<i32>,
    wrong_binding: bool,
    wrong_binary: bool,
    wrong_options: bool,
    wrong_invocation: bool,
    failure: Option<ProcessFailure>,
}

impl SolverProcessRunner for FakeRunner {
    fn run<'a>(
        &'a mut self,
        request: lc631_solvers::runner::ProcessRequest<'a>,
        _reservation: &'a dyn LiveResourceReservation,
    ) -> Result<RunnerReportedProcessObservation<'a>, ProcessFailure> {
        self.calls += 1;
        if let Some(failure) = self.failure {
            return Err(failure);
        }
        Ok(RunnerReportedProcessObservation {
            state: self.state,
            source: if self.wrong_binding {
                QuerySourceIdentity::from_bytes([8; 32])
            } else {
                request.source
            },
            query: if self.wrong_binding {
                QueryDigest::from_bytes([6; 32])
            } else {
                request.query
            },
            invocation: if self.wrong_invocation {
                InvocationDigest::from_bytes([9; 32])
            } else {
                request.invocation
            },
            observed_arguments: if self.wrong_options {
                &[]
            } else {
                request.fixed_arguments
            },
            executable_digest: if self.wrong_binary { [8; 32] } else { [7; 32] },
            exit_code: self.exit_code,
            stdout: self.output,
            stderr: &[],
            stdout_truncated: self.truncated,
            stderr_truncated: false,
            cleanup_verified: self.cleanup_verified,
        })
    }
}

fn observed_runner(output: &'static [u8]) -> FakeRunner {
    FakeRunner {
        calls: 0,
        output,
        state: ProcessRunState::Completed,
        cleanup_verified: true,
        truncated: false,
        exit_code: Some(0),
        wrong_binding: false,
        wrong_binary: false,
        wrong_options: false,
        wrong_invocation: false,
        failure: None,
    }
}

#[test]
fn tlcx10_registry_rejects_duplicate_backend_and_protocol_profile_crosswire() {
    let z3 = registration(
        BackendId::Z3,
        LogicProfile::SmtBooleanQfLia,
        SolverProtocol::SmtLibStatusV1,
    );
    assert!(matches!(
        SolverRegistry::checked(vec![z3.clone(), z3]),
        Err(SolverRegistryError::DuplicateRegistration)
    ));
    assert!(matches!(
        SolverRegistration::checked(
            BackendId::Z3,
            LogicProfile::AspFiniteGroundNormal,
            SolverProtocol::ClingoStatusV1,
            if cfg!(windows) {
                r"C:\tools\z3.exe"
            } else {
                "/usr/bin/z3"
            },
            "5.0.0",
            [7; 32],
            vec![],
            caps(),
        ),
        Err(SolverRegistryError::ProtocolProfileMismatch)
    ));
}

#[test]
fn tlcx10_registry_completeness_requires_both_declared_profiles() {
    let partial = registry();
    assert_eq!(
        partial.completeness(),
        lc631_solvers::registry::RegistryCompleteness {
            z3_qf_lia_registered: true,
            clingo_ground_asp_registered: false,
        }
    );
    assert_eq!(
        partial.require_complete_profiles(),
        Err(lc631_solvers::registry::IncompleteRegistry {
            missing_z3_qf_lia: false,
            missing_clingo_ground_asp: true,
        })
    );

    let complete = SolverRegistry::checked(vec![
        registration(
            BackendId::Z3,
            LogicProfile::SmtBooleanQfLia,
            SolverProtocol::SmtLibStatusV1,
        ),
        registration(
            BackendId::Clingo,
            LogicProfile::AspFiniteGroundNormal,
            SolverProtocol::ClingoStatusV1,
        ),
    ])
    .unwrap();
    assert!(complete.completeness().is_complete());
    assert_eq!(complete.require_complete_profiles(), Ok(()));
}

#[test]
fn tlcx10_smt_unsat_is_reported_satisfiability_not_a_proof() {
    let source = QuerySourceIdentity::from_bytes([1; 32]);
    let lease = Lease {
        source,
        valid: true,
    };
    let mut runner = observed_runner(b"unsat\n");
    let report = execute_registered(
        &registry(),
        &query(
            BackendId::Z3,
            source,
            QueryResultKind::Satisfiability,
            b"(check-sat)\n",
        ),
        &budget(),
        &lease,
        &mut runner,
    );
    assert_eq!(report.state(), SolverExecutionState::SolverReported);
    assert_eq!(
        report.logical_result(),
        Some(LogicalResult::Satisfiability(Satisfiability::Unsat))
    );
    assert_eq!(
        report.validation_basis(),
        Some(ValidationBasis::SolverReported)
    );
    assert_eq!(
        report.binary_binding_state(),
        Some(BinaryBindingState::RunnerReportedDigestMatchesRegistration)
    );
    assert_eq!(
        report.invocation_binding_state(),
        Some(InvocationBindingState::RunnerReportedInvocationDigestMatchesRegistration)
    );
    assert!(report.invocation_digest().is_some());
    assert!(
        !report.registry_completeness().clingo_ground_asp_registered,
        "a one-backend registry remains visibly incomplete"
    );
    assert!(!report.proof_checked());
}

#[test]
fn tlcx10_query_digest_binds_backend_source_profile_result_kind_and_payload() {
    let source = QuerySourceIdentity::from_bytes([1; 32]);
    let baseline = query(
        BackendId::Z3,
        source,
        QueryResultKind::Satisfiability,
        b"(check-sat)\n",
    );
    let repeated = query(
        BackendId::Z3,
        source,
        QueryResultKind::Satisfiability,
        b"(check-sat)\n",
    );
    let other_source = query(
        BackendId::Z3,
        QuerySourceIdentity::from_bytes([2; 32]),
        QueryResultKind::Satisfiability,
        b"(check-sat)\n",
    );
    let other_kind = query(
        BackendId::Z3,
        source,
        QueryResultKind::Entailment,
        b"(check-sat)\n",
    );
    let other_payload = query(
        BackendId::Z3,
        source,
        QueryResultKind::Satisfiability,
        b"(assert false)\n(check-sat)\n",
    );
    let other_backend = asp_query(source, QueryResultKind::StableModelExistence, b"a(1).\n");

    assert_eq!(baseline.digest(), repeated.digest());
    for different in [other_source, other_kind, other_payload] {
        assert_ne!(baseline.digest(), different.digest());
    }
    assert_ne!(baseline.digest(), other_backend.digest());
}

#[test]
fn tlcx10_clingo_stable_model_status_stays_solver_reported() {
    let source = QuerySourceIdentity::from_bytes([3; 32]);
    let lease = Lease {
        source,
        valid: true,
    };
    let mut runner = observed_runner(b"Answer: 1\na(1)\nSATISFIABLE\n");
    let report = execute_registered(
        &asp_registry(),
        &asp_query(source, QueryResultKind::StableModelExistence, b"a(1).\n"),
        &budget(),
        &lease,
        &mut runner,
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
    assert!(!report.proof_checked());
}

#[test]
fn tlcx10_requested_run_budget_cannot_exceed_registered_owner_caps() {
    let source = QuerySourceIdentity::from_bytes([1; 32]);
    let lease = Lease {
        source,
        valid: true,
    };
    let mut runner = observed_runner(b"sat\n");
    let oversized = ExecutionBudget::checked(
        1000,
        4096,
        4096,
        1024,
        16,
        4096,
        16 * 1024 * 1024,
        0,
        65_536,
    )
    .unwrap();
    let report = execute_registered(
        &registry(),
        &query(
            BackendId::Z3,
            source,
            QueryResultKind::Satisfiability,
            b"(check-sat)\n",
        ),
        &oversized,
        &lease,
        &mut runner,
    );
    assert_eq!(report.state(), SolverExecutionState::ResourceRejected);
    assert_eq!(report.logical_result(), None);
    assert_eq!(runner.calls, 0);
}

#[test]
fn tlcx10_status_cannot_be_reinterpreted_as_entailment() {
    let source = QuerySourceIdentity::from_bytes([1; 32]);
    let lease = Lease {
        source,
        valid: true,
    };
    let mut runner = observed_runner(b"sat\n");
    let report = execute_registered(
        &registry(),
        &query(
            BackendId::Z3,
            source,
            QueryResultKind::Entailment,
            b"(check-sat)\n",
        ),
        &budget(),
        &lease,
        &mut runner,
    );
    assert_eq!(report.state(), SolverExecutionState::UnsupportedMapping);
    assert_eq!(report.logical_result(), None);
    assert_eq!(
        runner.calls, 0,
        "unsupported result shape must be rejected before runner invocation"
    );
}

#[test]
fn tlcx10_model_proof_and_counterexample_queries_are_unsupported_preflight() {
    let source = QuerySourceIdentity::from_bytes([4; 32]);
    let lease = Lease {
        source,
        valid: true,
    };
    for kind in [
        QueryResultKind::ModelPayload,
        QueryResultKind::ProofPayload,
        QueryResultKind::CounterexamplePayload,
    ] {
        assert_eq!(
            query_protocol_compatibility(SolverProtocol::SmtLibStatusV1, kind),
            QueryProtocolCompatibility::Unsupported
        );
        let mut runner = observed_runner(b"sat\n(model ...)");
        let report = execute_registered(
            &registry(),
            &query(BackendId::Z3, source, kind, b"(check-sat)\n"),
            &budget(),
            &lease,
            &mut runner,
        );
        assert_eq!(report.state(), SolverExecutionState::UnsupportedMapping);
        assert_eq!(report.logical_result(), None);
        assert_eq!(runner.calls, 0);
    }
}

#[test]
fn tlcx10_resource_source_mismatch_prevents_child_start() {
    let source = QuerySourceIdentity::from_bytes([1; 32]);
    let lease = Lease {
        source: QuerySourceIdentity::from_bytes([2; 32]),
        valid: true,
    };
    let mut runner = observed_runner(b"sat\n");
    let report = execute_registered(
        &registry(),
        &query(
            BackendId::Z3,
            source,
            QueryResultKind::Satisfiability,
            b"(check-sat)\n",
        ),
        &budget(),
        &lease,
        &mut runner,
    );
    assert_eq!(report.state(), SolverExecutionState::ResourceRejected);
    assert_eq!(report.logical_result(), None);
    assert_eq!(runner.calls, 0);
}

#[test]
fn tlcx10_timeout_truncation_and_unknown_cleanup_have_no_logical_result() {
    let source = QuerySourceIdentity::from_bytes([1; 32]);
    let lease = Lease {
        source,
        valid: true,
    };
    for (state, cleanup_verified, truncated, expected, expected_cleanup) in [
        (
            ProcessRunState::TimedOut,
            true,
            false,
            SolverExecutionState::TimedOut,
            ProcessCleanupState::RunnerReportedVerified,
        ),
        (
            ProcessRunState::TimedOut,
            false,
            false,
            SolverExecutionState::TimedOut,
            ProcessCleanupState::RunnerReportedUnknown,
        ),
        (
            ProcessRunState::Completed,
            true,
            true,
            SolverExecutionState::OutputTruncated,
            ProcessCleanupState::RunnerReportedVerified,
        ),
        (
            ProcessRunState::Completed,
            false,
            false,
            SolverExecutionState::CleanupUnknown,
            ProcessCleanupState::RunnerReportedUnknown,
        ),
        (
            ProcessRunState::ResourceExhaustedBeforeStart,
            true,
            false,
            SolverExecutionState::ResourceRejected,
            ProcessCleanupState::RunnerReportedNotStarted,
        ),
        (
            ProcessRunState::ResourceExhaustedAfterStartUnknown,
            false,
            false,
            SolverExecutionState::StartedUnknown,
            ProcessCleanupState::RunnerReportedUnknown,
        ),
    ] {
        let mut runner = FakeRunner {
            calls: 0,
            output: b"unsat\n",
            state,
            cleanup_verified,
            truncated,
            exit_code: Some(0),
            wrong_binding: false,
            wrong_binary: false,
            wrong_options: false,
            wrong_invocation: false,
            failure: None,
        };
        let report = execute_registered(
            &registry(),
            &query(
                BackendId::Z3,
                source,
                QueryResultKind::Satisfiability,
                b"(check-sat)\n",
            ),
            &budget(),
            &lease,
            &mut runner,
        );
        assert_eq!(report.state(), expected);
        assert_eq!(report.cleanup_state(), expected_cleanup);
        assert_eq!(report.logical_result(), None);
    }
}

#[test]
fn tlcx10_wrong_binary_query_invocation_or_options_is_runner_reported_mismatch() {
    let source = QuerySourceIdentity::from_bytes([1; 32]);
    let lease = Lease {
        source,
        valid: true,
    };
    for (wrong_binding, wrong_binary, wrong_options, wrong_invocation) in [
        (true, false, false, false),
        (false, true, false, false),
        (false, false, true, false),
        (false, false, false, true),
    ] {
        let mut runner = FakeRunner {
            wrong_binding,
            wrong_binary,
            wrong_options,
            wrong_invocation,
            ..observed_runner(b"sat\n")
        };
        let report = execute_registered(
            &registry(),
            &query(
                BackendId::Z3,
                source,
                QueryResultKind::Satisfiability,
                b"(check-sat)\n",
            ),
            &budget(),
            &lease,
            &mut runner,
        );
        assert_eq!(report.state(), SolverExecutionState::BindingMismatch);
        assert_eq!(report.logical_result(), None);
    }
}

#[test]
fn tlcx10_invocation_digest_changes_with_registered_version_options_and_caps() {
    let source = QuerySourceIdentity::from_bytes([5; 32]);
    let lease = Lease {
        source,
        valid: true,
    };
    let query = query(
        BackendId::Z3,
        source,
        QueryResultKind::Satisfiability,
        b"(check-sat)\n",
    );
    let baseline_registration = registration(
        BackendId::Z3,
        LogicProfile::SmtBooleanQfLia,
        SolverProtocol::SmtLibStatusV1,
    );
    let changed_version = SolverRegistration::checked(
        BackendId::Z3,
        LogicProfile::SmtBooleanQfLia,
        SolverProtocol::SmtLibStatusV1,
        baseline_registration.executable(),
        "5.0.1",
        *baseline_registration.binary_digest(),
        baseline_registration.fixed_arguments().to_vec(),
        caps(),
    )
    .unwrap();
    let changed_options = SolverRegistration::checked(
        BackendId::Z3,
        LogicProfile::SmtBooleanQfLia,
        SolverProtocol::SmtLibStatusV1,
        baseline_registration.executable(),
        baseline_registration.version(),
        *baseline_registration.binary_digest(),
        vec!["--different-option".into()],
        caps(),
    )
    .unwrap();
    let changed_caps = SolverRegistration::checked(
        BackendId::Z3,
        LogicProfile::SmtBooleanQfLia,
        SolverProtocol::SmtLibStatusV1,
        baseline_registration.executable(),
        baseline_registration.version(),
        *baseline_registration.binary_digest(),
        baseline_registration.fixed_arguments().to_vec(),
        SolverResourceCaps {
            max_timeout_ms: caps().max_timeout_ms - 1,
            ..caps()
        },
    )
    .unwrap();

    let report_for = |registration, run_budget| {
        let registry = SolverRegistry::checked(vec![registration]).unwrap();
        let mut runner = observed_runner(b"sat\n");
        execute_registered(&registry, &query, &run_budget, &lease, &mut runner)
    };
    let baseline = report_for(baseline_registration, budget());
    let version = report_for(changed_version, budget());
    let options = report_for(changed_options, budget());
    let caps = report_for(changed_caps, budget());
    let changed_budget = report_for(
        registration(
            BackendId::Z3,
            LogicProfile::SmtBooleanQfLia,
            SolverProtocol::SmtLibStatusV1,
        ),
        ExecutionBudget::checked(900, 4096, 4096, 1024, 16, 4096, 4 * 1024 * 1024, 0, 65_536)
            .unwrap(),
    );
    let baseline_digest = baseline.invocation_digest().unwrap();
    assert_ne!(baseline_digest, version.invocation_digest().unwrap());
    assert_ne!(baseline_digest, options.invocation_digest().unwrap());
    assert_ne!(baseline_digest, caps.invocation_digest().unwrap());
    assert_ne!(baseline_digest, changed_budget.invocation_digest().unwrap());
    assert_eq!(baseline.query(), version.query());
    assert_eq!(baseline.query(), options.query());
    assert_eq!(baseline.query(), caps.query());
}

#[test]
fn tlcx10_resource_exhaustion_errors_preserve_prestart_vs_started_unknown() {
    let source = QuerySourceIdentity::from_bytes([6; 32]);
    let lease = Lease {
        source,
        valid: true,
    };
    for (failure, state, cleanup) in [
        (
            ProcessFailure::ResourceExhaustedBeforeStart,
            SolverExecutionState::ResourceRejected,
            ProcessCleanupState::RunnerReportedNotStarted,
        ),
        (
            ProcessFailure::ResourceExhaustedAfterStartUnknown,
            SolverExecutionState::StartedUnknown,
            ProcessCleanupState::RunnerReportedUnknown,
        ),
    ] {
        let mut runner = FakeRunner {
            failure: Some(failure),
            ..observed_runner(b"")
        };
        let report = execute_registered(
            &registry(),
            &query(
                BackendId::Z3,
                source,
                QueryResultKind::Satisfiability,
                b"(check-sat)\n",
            ),
            &budget(),
            &lease,
            &mut runner,
        );
        assert_eq!(report.state(), state);
        assert_eq!(report.cleanup_state(), cleanup);
        assert_eq!(report.logical_result(), None);
    }
}

#[test]
fn tlcx10_malformed_or_duplicate_protocol_status_is_not_truth() {
    assert_eq!(
        lc631_solvers::protocol::parse_status(
            SolverProtocol::SmtLibStatusV1,
            QueryResultKind::Satisfiability,
            b"sat\nunsat\n",
            64,
        ),
        Err(ProtocolError::DuplicateOrTrailingStatus)
    );
    assert_eq!(
        lc631_solvers::protocol::parse_status(
            SolverProtocol::SmtLibStatusV1,
            QueryResultKind::Entailment,
            b"sat\n",
            64,
        ),
        Err(ProtocolError::UnsupportedQueryKind)
    );
}

#[test]
fn tlcx10_registration_path_and_query_arguments_are_never_shell_text() {
    let registration = registration(
        BackendId::Z3,
        LogicProfile::SmtBooleanQfLia,
        SolverProtocol::SmtLibStatusV1,
    );
    assert!(Path::new(registration.executable()).is_absolute());
    let source = QuerySourceIdentity::from_bytes([1; 32]);
    let query = query(
        BackendId::Z3,
        source,
        QueryResultKind::Satisfiability,
        b"(check-sat)\n",
    );
    assert_eq!(query.payload(), b"(check-sat)\n");
    assert!(registration
        .fixed_arguments()
        .iter()
        .all(|argument| !argument.contains('\0')));
}
