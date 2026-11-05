#![allow(clippy::drop_non_drop)] // End borrowed reservation views before releasing their owners.

use lc631_runtime::resource::{
    Cancellation, Deadline, ResourceAmount, ResourceBroker, ResourceKind, ResourceLimits,
    ResourceRequest,
};
use lc631_solvers::protocol::{QueryResultKind, SolverProtocol, ValidationBasis};
use lc631_solvers::registry::{
    BackendId, LogicProfile, SolverRegistration, SolverRegistry, SolverResourceCaps,
};
use lc631_solvers::result_store::{ResultStore, SolverResultBinding};
use lc631_solvers::runner::{
    ExecutionBudget, QuerySourceIdentity, SolverExecutionState, SolverQuery,
};
use lc631_solvers::supervisor_adapter::{
    execute_supervised_into_store, hash_registered_executable,
};
use lc631_solvers::{LogicalResult, Satisfiability};
use std::path::Path;
use std::time::Duration;

const SOLVER_CHILD: &str = env!("CARGO_BIN_EXE_tlcx10_solver_child");

fn source() -> QuerySourceIdentity {
    QuerySourceIdentity::from_bytes([0x2a; 32])
}

fn caps() -> SolverResourceCaps {
    SolverResourceCaps {
        max_argv_count: 16,
        max_argv_bytes: 4096,
        max_stdin_bytes: 4096,
        max_stdout_bytes: 4096,
        max_stderr_bytes: 1024,
        max_timeout_ms: 5000,
        max_ram_bytes: 16 * 1024 * 1024,
        max_disk_bytes: 0,
        max_cleanup_bytes: 65_536,
        max_child_count: 1,
    }
}

fn budget() -> ExecutionBudget {
    ExecutionBudget::checked(1000, 4096, 4096, 1024, 16, 4096, 8 * 1024 * 1024, 0, 65_536).unwrap()
}

fn resource_owner(
    source: QuerySourceIdentity,
) -> (
    ResourceBroker,
    lc631_runtime::resource::RunResources,
    lc631_runtime::resource::Reservation,
) {
    let limits = ResourceLimits::uniform(64 * 1024 * 1024).unwrap();
    let owner = ResourceBroker::new(limits).unwrap();
    let run = owner
        .begin_run(
            1,
            lc631_substrate::Digest32::from_bytes(*source.as_bytes()),
            limits,
            Deadline::after(Duration::from_secs(30)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let reservation = run
        .reserve(&[
            ResourceRequest::new(
                ResourceKind::Ram,
                ResourceAmount::checked(32 * 1024 * 1024).unwrap(),
            ),
            ResourceRequest::new(
                ResourceKind::Output,
                ResourceAmount::checked(2 * 1024 * 1024).unwrap(),
            ),
            ResourceRequest::new(ResourceKind::Child, ResourceAmount::checked(1).unwrap()),
            ResourceRequest::new(
                ResourceKind::Cleanup,
                ResourceAmount::checked(1024 * 1024).unwrap(),
            ),
        ])
        .unwrap();
    (owner, run, reservation)
}

#[test]
fn tlcx10_supervisor_runs_a_bounded_test_child_but_keeps_status_runner_reported() {
    // This verifies the process-supervisor adapter path only. The child emits
    // a fixed status and is explicitly not a Z3 backend or semantic oracle.
    let executable = Path::new(SOLVER_CHILD);
    let source = source();
    let (_owner, _run, mut reservation) = resource_owner(source);
    let report = {
        let view = reservation.lease();
        let binary_digest = hash_registered_executable(&view, executable).unwrap();
        let registration = SolverRegistration::checked(
            BackendId::Z3,
            LogicProfile::SmtBooleanQfLia,
            SolverProtocol::SmtLibStatusV1,
            executable.to_str().unwrap(),
            "test-fixture-only",
            binary_digest,
            Vec::new(),
            caps(),
        )
        .unwrap();
        let registry = SolverRegistry::checked(vec![registration]).unwrap();
        let query = SolverQuery::checked(
            BackendId::Z3,
            source,
            LogicProfile::SmtBooleanQfLia,
            QueryResultKind::Satisfiability,
            b"(check-sat)\n",
        )
        .unwrap();

        let mut store = ResultStore::with_capacity(2).unwrap();
        let receipt = execute_supervised_into_store(
            &registry,
            &query,
            &budget(),
            &view,
            &std::env::current_dir().unwrap(),
            &mut store,
        )
        .unwrap();
        let insertion = receipt.insertion().unwrap();
        let stored = store
            .lookup(SolverResultBinding::for_query(
                &query,
                receipt.report().invocation_digest(),
            ))
            .unwrap();
        assert_eq!(stored.id(), insertion.id());
        assert_eq!(stored.report(), receipt.report());
        (receipt.report(), stored.report())
    };

    let (report, stored_report) = report;
    assert_eq!(report.state(), SolverExecutionState::SolverReported);
    assert_eq!(stored_report, report);
    assert_eq!(
        report.logical_result(),
        Some(LogicalResult::Satisfiability(Satisfiability::Sat))
    );
    assert_eq!(
        report.validation_basis(),
        Some(ValidationBasis::SolverReported)
    );
    assert_eq!(report.stdout_bytes(), 4);
    assert!(!report.proof_checked());
    assert!(!report.authority_created());
    assert!(!report.output_commit_allowed());
    reservation.release_confirmed().unwrap();
}
