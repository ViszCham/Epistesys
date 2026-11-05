//! Explicit native integration profile for `lc631-runtime/tests/scoped_solver_native_z3.rs`.
//!
//! After reviewing and integrating the companion scoped module and patch, the
//! owning integrator can place this file in that tests directory. With the
//! workspace rooted at `scripts/lc631`, set EPISTESYS_TLCX10_NATIVE_Z3 to the
//! authorized absolute Windows executable path and run the single ignored test:
//! `cargo test -p lc631-runtime --test scoped_solver_native_z3 tlcx10_native_z3_scoped_abstract_sat -- --ignored --exact`
//! An unset, invalid, changed or non-UTF-8 path fails the opted-in test. An
//! ignored test is not an observation. Run this profile explicitly; the portable suite does not observe a backend.
//!
//! The digest is a parent-provided expected pin, not a signed receipt. The
//! registration's version string is a declaration: this test does not execute
//! `z3 --version`, validate the license, or authenticate the image at spawn.
//! Budget RAM is adapter admission, not the child's OS memory limit; disk
//! budget zero does not sandbox filesystem access.

use std::path::PathBuf;
use std::time::Duration;

use lc631_analysis::{
    build_dgcl_pipeline,
    context_lowering::lower_dgcl_context,
    logical_lowering::{
        lower_dgcl_logical, LogicProfile as AnalysisLogicProfile, LogicalLoweringBudget,
        LogicalTask,
    },
    ConditionEvidenceState, DgclBackendSet,
};
use lc631_core::{
    context::{ContextLimits, ContextScope, ContextState, SourceRevision, TaskKind},
    AuthorityRevision,
};
use lc631_runtime::{
    resource::{
        Cancellation, Deadline, ResourceAmount, ResourceBroker, ResourceKind, ResourceLimits,
        ResourceRequest,
    },
    scoped_solver::{ScopedLookupError, ScopedSolverStore},
    solver_lowering::{
        prepare_abstract_condition_satisfiability, AbstractSatisfiabilityBudget,
        AbstractSatisfiabilityDecision, AbstractSatisfiabilityInterpretation,
        PreparedAbstractSatisfiabilityQuery,
    },
};
use lc631_solvers::{
    protocol::{SolverProtocol, ValidationBasis},
    registry::{BackendId, LogicProfile, SolverRegistration, SolverRegistry, SolverResourceCaps},
    runner::{ExecutionBudget, ProcessCleanupState, ProcessRunState, SolverExecutionState},
    supervisor_adapter::hash_registered_executable,
    typed_query::SmtEmitLimits,
    LogicalResult, Satisfiability,
};

// Observed platform-local byte pins, not authenticated image provenance.
const EXPECTED_SHA256: [u8; 32] = if cfg!(windows) {
    [
        0x5f, 0x32, 0xb2, 0xd2, 0xb9, 0x32, 0x91, 0xb5, 0x6e, 0x3c, 0x10, 0xa4, 0x3b, 0x08, 0x8c,
        0xae, 0x78, 0xac, 0xc8, 0x40, 0x17, 0xeb, 0xff, 0x8f, 0xa7, 0x41, 0xa7, 0xec, 0xee, 0x6c,
        0x6c, 0xa5,
    ]
} else {
    [
        0xc4, 0x5e, 0x78, 0x7b, 0x5d, 0x1a, 0x3d, 0x94, 0x07, 0x9d, 0x1e, 0x83, 0xb9, 0x8e, 0x23,
        0xbb, 0xc8, 0x84, 0xcc, 0xaf, 0x54, 0x2e, 0xa3, 0x8c, 0x38, 0x14, 0xf3, 0x1a, 0xc2, 0xf7,
        0x91, 0x97,
    ]
};

fn task(scope_label: &str) -> (LogicalTask, ContextState) {
    let report = build_dgcl_pipeline(
        "Please verify the result if ready.",
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    let scope = SourceRevision::of(scope_label).digest();
    let goal = SourceRevision::of("declared integration-test goal").digest();
    let context = lower_dgcl_context(
        &report,
        ContextScope::declared(scope, goal, scope, scope),
        AuthorityRevision(10),
        ContextLimits {
            max_contexts: 16,
            max_premises: 4096,
            max_edits: 32,
            max_statement_bytes: 262_144,
        },
    )
    .unwrap();
    let state = context.original().state().clone();
    let task = lower_dgcl_logical(
        &report,
        &context,
        TaskKind::Reasoning,
        AnalysisLogicProfile::ClassicalPropositionalV1,
        LogicalLoweringBudget::default(),
    )
    .unwrap();
    (task, state)
}

fn caps() -> SolverResourceCaps {
    SolverResourceCaps {
        max_argv_count: 16,
        max_argv_bytes: 4096,
        max_stdin_bytes: 4096,
        max_stdout_bytes: 4096,
        max_stderr_bytes: 1024,
        max_timeout_ms: 10_000,
        max_ram_bytes: 64 * 1024 * 1024,
        max_disk_bytes: 0,
        max_cleanup_bytes: 65_536,
        max_child_count: 1,
    }
}

fn prepare(task: &LogicalTask) -> PreparedAbstractSatisfiabilityQuery<'_> {
    let decision = prepare_abstract_condition_satisfiability(
        task,
        task.conditions()[0].formula(),
        AbstractSatisfiabilityInterpretation::FreeBooleanAtoms,
        SmtEmitLimits::default(),
        AbstractSatisfiabilityBudget::default(),
    );
    let AbstractSatisfiabilityDecision::Prepared(prepared) = decision else {
        panic!("abstract condition did not prepare")
    };
    prepared
}

#[test]
#[ignore = "opt-in native Z3 integration; requires authorized executable and pinned digest"]
fn tlcx10_native_z3_scoped_abstract_sat() {
    match std::env::consts::OS {
        "windows" | "linux" => (),
        platform => panic!("native profile does not support {platform}"),
    }
    let executable = PathBuf::from(
        std::env::var_os("EPISTESYS_TLCX10_NATIVE_Z3")
            .expect("EPISTESYS_TLCX10_NATIVE_Z3 is required when this ignored test is selected"),
    );
    assert!(executable.is_absolute(), "solver path must be absolute");
    let executable_text = executable
        .to_str()
        .expect("solver path must be UTF-8 for this registry");
    let (task_a, context_a) = task("comparison A");
    let (task_b, context_b) = task("comparison B");
    let condition = task_a.conditions().first().expect("fixture condition");
    assert_eq!(condition.reported_state(), ConditionEvidenceState::Unknown);
    let condition_id = condition.formula();
    let prepared_a = prepare(&task_a);
    let prepared_b = prepare(&task_b);
    assert_ne!(context_a.revision(), context_b.revision());
    assert_eq!(
        prepared_a.binding().source_identity(),
        prepared_b.binding().source_identity()
    );
    assert_eq!(prepared_a.smt_payload(), prepared_b.smt_payload());
    assert_eq!(prepared_a.binding().formula_id(), condition_id);
    assert_eq!(
        prepared_a.condition_reported_state(),
        ConditionEvidenceState::Unknown
    );

    let limits = ResourceLimits::uniform(256 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let mut run = broker
        .begin_run(
            1,
            task_a.source_store().id().digest(),
            limits,
            Deadline::after(Duration::from_secs(60)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let mut reservation = run
        .reserve(&[
            ResourceRequest::new(
                ResourceKind::Ram,
                ResourceAmount::checked(128 * 1024 * 1024).unwrap(),
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

    {
        let lease = reservation.lease();
        let observed_hash = hash_registered_executable(&lease, &executable)
            .expect("bounded executable hash observation failed");
        assert_eq!(
            observed_hash, EXPECTED_SHA256,
            "the executable digest does not match the selected pin"
        );
        let registration = SolverRegistration::checked(
            BackendId::Z3,
            LogicProfile::SmtBooleanQfLia,
            SolverProtocol::SmtLibStatusV1,
            executable_text,
            "5.3.2",
            EXPECTED_SHA256,
            vec!["-smt2".to_owned(), "-in".to_owned()],
            caps(),
        )
        .unwrap();
        let registry = SolverRegistry::checked(vec![registration]).unwrap();
        let budget = ExecutionBudget::checked(
            10_000,
            4096,
            4096,
            1024,
            16,
            4096,
            64 * 1024 * 1024,
            0,
            65_536,
        )
        .unwrap();
        let cwd = std::env::current_dir().expect("an absolute working directory is required");
        assert!(cwd.is_absolute());
        let mut store = ScopedSolverStore::with_resources(2, &lease).unwrap();
        let receipt = store
            .execute(
                &prepared_a,
                &context_a,
                Some(context_a.scope()),
                &registry,
                &budget,
                &lease,
                &cwd,
            )
            .expect("scoped native execution failed");
        let report = receipt.report();
        assert_eq!(report.state(), SolverExecutionState::SolverReported);
        assert_eq!(report.process_state(), Some(ProcessRunState::Completed));
        assert_eq!(
            report.logical_result(),
            Some(LogicalResult::Satisfiability(Satisfiability::Sat))
        );
        assert_eq!(
            report.validation_basis(),
            Some(ValidationBasis::SolverReported)
        );
        assert!(report.stdout_fully_observed());
        assert!(report.stderr_fully_observed());
        assert!(!report.stderr_truncated());
        assert_eq!(
            report.cleanup_state(),
            ProcessCleanupState::RunnerReportedVerified
        );
        assert!(!report.proof_checked());
        assert!(!report.authority_created());
        assert!(!report.output_commit_allowed());
        let stored = store
            .lookup(
                &prepared_a,
                &context_a,
                Some(context_a.scope()),
                report.invocation_digest(),
            )
            .expect("same prepared query must be consumed");
        assert_eq!(stored.id(), receipt.insertion().id());
        assert_eq!(stored.report(), report);
        assert!(matches!(
            store.lookup(
                &prepared_b,
                &context_b,
                Some(context_b.scope()),
                report.invocation_digest()
            ),
            Err(ScopedLookupError::NotFound)
        ));
        assert!(matches!(
            store.lookup(
                &prepared_a,
                &context_b,
                Some(context_b.scope()),
                report.invocation_digest()
            ),
            Err(ScopedLookupError::ContextMismatch)
        ));
        assert_eq!(store.len(), 1);
    }
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
    assert_eq!(task_a.conditions()[0].formula(), condition_id);
    assert_eq!(
        task_a.conditions()[0].reported_state(),
        ConditionEvidenceState::Unknown
    );
    assert_eq!(
        prepared_a.condition_reported_state(),
        ConditionEvidenceState::Unknown
    );
}
