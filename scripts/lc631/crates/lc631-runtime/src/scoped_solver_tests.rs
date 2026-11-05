//! Scoped execution contract tests; synthetic reports do not observe a solver.
//! Test-only injection calls the same scoped preflight/store path as production,
//! but the fixture runner reports synthetic observations; no real solver runs.

#[path = "../../lc631-solvers/tests/common/tlcx10_result_fixture.rs"]
mod fixture;

use crate::scoped_solver::{ScopedLookupError, ScopedRunError, ScopedSolverStore};
use crate::solver_lowering::{
    prepare_abstract_condition_satisfiability, AbstractSatisfiabilityBudget,
    AbstractSatisfiabilityDecision, AbstractSatisfiabilityInterpretation,
    PreparedAbstractSatisfiabilityQuery,
};
use lc631_analysis::{
    build_dgcl_pipeline,
    context_lowering::lower_dgcl_context,
    logical_lowering::{lower_dgcl_logical, LogicProfile, LogicalLoweringBudget, LogicalTask},
    DgclBackendSet,
};
use lc631_core::{
    context::{ContextLimits, ContextScope, ContextState, SourceRevision, TaskKind},
    AuthorityRevision,
};
use lc631_solvers::{
    protocol::ValidationBasis,
    registry::SolverRegistry,
    result_store::ResultStoreError,
    runner::{
        execute_registered, ExecutionBudget, ProcessRunState, SolverExecutionReport, SolverQuery,
    },
    typed_query::SmtEmitLimits,
    LogicalResult, Satisfiability,
};
use lc631_substrate::Digest32;
use std::cell::Cell;

#[path = "scoped_solver_resource_tests.rs"]
mod resource_tests;

#[path = "scoped_solver_current_tests.rs"]
mod current_tests;

const SOURCE: &str = "Please verify the result if ready.";

fn pin(name: &str) -> Digest32 {
    SourceRevision::of(name).digest()
}

fn make_task(comparison: Digest32, goal: Digest32, kind: TaskKind) -> (LogicalTask, ContextState) {
    let report = build_dgcl_pipeline(
        SOURCE,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    let context = lower_dgcl_context(
        &report,
        ContextScope::declared(comparison, goal, pin("probes"), pin("semantics")),
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
        kind,
        LogicProfile::ClassicalPropositionalV1,
        LogicalLoweringBudget::default(),
    )
    .unwrap();
    (task, state)
}

fn prepare(task: &LogicalTask) -> PreparedAbstractSatisfiabilityQuery<'_> {
    // The opaque ID is supplied by the real task factory.
    let formula = task.conditions().first().unwrap().formula();
    let AbstractSatisfiabilityDecision::Prepared(prepared) =
        prepare_abstract_condition_satisfiability(
            task,
            formula,
            AbstractSatisfiabilityInterpretation::FreeBooleanAtoms,
            SmtEmitLimits::default(),
            AbstractSatisfiabilityBudget::default(),
        )
    else {
        panic!("the declared public fixture must prepare")
    };
    prepared
}

fn report(
    query: &SolverQuery<'_>,
    state: ProcessRunState,
    stdout: &'static [u8],
) -> SolverExecutionReport {
    fixture::report_with(
        query,
        state,
        stdout,
        false,
        state == ProcessRunState::Completed,
    )
}

// Invoke the actual request factory with a different budget, rather than
// constructing an InvocationDigest from bytes. Both runners are fixtures.
fn changed_budget_report(query: &SolverQuery<'_>) -> SolverExecutionReport {
    let executable = std::env::current_exe().unwrap();
    let registry =
        SolverRegistry::checked(vec![fixture::registration(&executable, [0x51; 32])]).unwrap();
    let (_issuer, lease) = fixture::resource_owner(query.source());
    let view = lease.view();
    let budget =
        ExecutionBudget::checked(900, 4096, 4096, 1024, 16, 4096, 8 * 1024 * 1024, 0, 65_536)
            .unwrap();
    let mut runner = fixture::ReportedRunner {
        state: ProcessRunState::Completed,
        stdout: b"sat\n",
        stdout_truncated: false,
        cleanup_verified: true,
    };
    let result = execute_registered(&registry, query, &budget, &view, &mut runner);
    lease.close_confirmed().unwrap();
    result
}

#[test]
fn tlcx10_same_source_payload_other_context_and_goal_do_not_lookup() {
    let (a, ca) = make_task(pin("comparison A"), pin("goal A"), TaskKind::Reasoning);
    let (b, cb) = make_task(pin("comparison B"), pin("goal A"), TaskKind::Reasoning);
    let (g, cg) = make_task(pin("comparison A"), pin("goal B"), TaskKind::Reasoning);
    let pa = prepare(&a);
    let pb = prepare(&b);
    let pg = prepare(&g);
    assert_eq!(
        pa.binding().source_identity(),
        pb.binding().source_identity()
    );
    assert_eq!(pa.smt_payload(), pb.smt_payload());
    assert_eq!(pa.smt_payload(), pg.smt_payload());
    assert_ne!(ca.revision(), cb.revision());
    assert_ne!(ca.scope().goal(), cg.scope().goal());
    let mut store = ScopedSolverStore::with_capacity(3).unwrap();
    let receipt = store
        .execute_for_test(&pa, &ca, Some(ca.scope()), |q| {
            Ok(report(q, ProcessRunState::Completed, b"sat\n"))
        })
        .unwrap();
    let invocation = receipt.report().invocation_digest();
    assert_eq!(
        store
            .lookup(&pa, &ca, Some(ca.scope()), invocation)
            .unwrap()
            .id(),
        receipt.insertion().id()
    );
    assert!(matches!(
        store.lookup(&pb, &cb, Some(cb.scope()), invocation),
        Err(ScopedLookupError::NotFound)
    ));
    assert!(matches!(
        store.lookup(&pg, &cg, Some(cg.scope()), invocation),
        Err(ScopedLookupError::NotFound)
    ));
    assert!(matches!(
        store.lookup(&pa, &cb, Some(cb.scope()), invocation),
        Err(ScopedLookupError::ContextMismatch)
    ));
    assert!(matches!(
        store.lookup(&pa, &ca, Some(cg.scope()), invocation),
        Err(ScopedLookupError::GoalScopeMismatch)
    ));
    assert_eq!(store.len(), 1);
}

#[test]
fn tlcx10_same_context_payload_other_task_projection_and_formula_rejected() {
    let (reasoning, context) = make_task(pin("comparison"), pin("goal"), TaskKind::Reasoning);
    let (math, math_context) = make_task(pin("comparison"), pin("goal"), TaskKind::Mathematics);
    let a = prepare(&reasoning);
    let b = prepare(&math);
    assert_eq!(context.revision(), math_context.revision());
    assert_eq!(a.binding().source_identity(), b.binding().source_identity());
    assert_eq!(a.smt_payload(), b.smt_payload());
    assert_ne!(a.binding().task_kind(), b.binding().task_kind());
    assert_ne!(a.binding().formula_id(), b.binding().formula_id());
    let mut store = ScopedSolverStore::with_capacity(2).unwrap();
    let receipt = store
        .execute_for_test(&a, &context, Some(context.scope()), |q| {
            Ok(report(q, ProcessRunState::Completed, b"sat\n"))
        })
        .unwrap();
    assert!(matches!(
        store.lookup(
            &b,
            &math_context,
            Some(math_context.scope()),
            receipt.report().invocation_digest()
        ),
        Err(ScopedLookupError::NotFound)
    ));
    assert_eq!(store.len(), 1);
}

#[test]
fn tlcx10_empty_store_owner_and_different_invocation_are_not_replay() {
    let (task, context) = make_task(pin("comparison"), pin("goal"), TaskKind::Reasoning);
    let prepared = prepare(&task);
    let cloned = prepared.clone();
    let mut owner_a = ScopedSolverStore::with_capacity(2).unwrap();
    let mut owner_b = ScopedSolverStore::with_capacity(2).unwrap();
    let a = owner_a
        .execute_for_test(&prepared, &context, Some(context.scope()), |q| {
            Ok(report(q, ProcessRunState::Completed, b"sat\n"))
        })
        .unwrap();
    let other = changed_budget_report(&prepared.scoped_solver_query().unwrap());
    assert_ne!(a.report().invocation_digest(), other.invocation_digest());
    assert!(matches!(
        owner_b.lookup(
            &prepared,
            &context,
            Some(context.scope()),
            a.report().invocation_digest()
        ),
        Err(ScopedLookupError::NotFound)
    ));
    assert!(matches!(
        owner_a.lookup(
            &prepared,
            &context,
            Some(context.scope()),
            other.invocation_digest()
        ),
        Err(ScopedLookupError::NotFound)
    ));
    assert_eq!(prepared, cloned);
    assert!(matches!(
        owner_a.lookup(
            &cloned,
            &context,
            Some(context.scope()),
            a.report().invocation_digest()
        ),
        Err(ScopedLookupError::NotFound)
    ));
    assert_eq!(owner_a.len(), 1);
    assert!(owner_b.is_empty());
    let b = owner_b
        .execute_for_test(&cloned, &context, Some(context.scope()), |q| {
            Ok(report(q, ProcessRunState::Completed, b"sat\n"))
        })
        .unwrap();
    // Result IDs are scoped to an in-memory store, not owner capabilities.
    assert_eq!(a.insertion().id(), b.insertion().id());
    assert!(matches!(
        owner_b.lookup(
            &prepared,
            &context,
            Some(context.scope()),
            b.report().invocation_digest()
        ),
        Err(ScopedLookupError::NotFound)
    ));
    assert_eq!(
        owner_b
            .lookup(
                &cloned,
                &context,
                Some(context.scope()),
                b.report().invocation_digest()
            )
            .unwrap()
            .report(),
        b.report()
    );
}

#[test]
fn tlcx10_repeated_conflicting_reports_are_retained_and_ambiguous() {
    let (task, context) = make_task(pin("comparison"), pin("goal"), TaskKind::Reasoning);
    let prepared = prepare(&task);
    let mut store = ScopedSolverStore::with_capacity(3).unwrap();
    let sat = store
        .execute_for_test(&prepared, &context, Some(context.scope()), |q| {
            Ok(report(q, ProcessRunState::Completed, b"sat\n"))
        })
        .unwrap();
    let unsat = store
        .execute_for_test(&prepared, &context, Some(context.scope()), |q| {
            Ok(report(q, ProcessRunState::Completed, b"unsat\n"))
        })
        .unwrap();
    let repeated = store
        .execute_for_test(&prepared, &context, Some(context.scope()), |q| {
            Ok(report(q, ProcessRunState::Completed, b"sat\n"))
        })
        .unwrap();
    assert_eq!(
        sat.report().invocation_digest(),
        unsat.report().invocation_digest()
    );
    assert_ne!(sat.insertion().id(), unsat.insertion().id());
    assert_eq!(sat.report(), repeated.report());
    assert_ne!(sat.insertion().id(), repeated.insertion().id());
    assert_eq!(store.len(), 3);
    assert!(matches!(
        store.lookup(
            &prepared,
            &context,
            Some(context.scope()),
            sat.report().invocation_digest()
        ),
        Err(ScopedLookupError::Ambiguous)
    ));
    assert_eq!(
        store
            .lookup_occurrence(
                &prepared,
                &context,
                Some(context.scope()),
                sat.report().invocation_digest(),
                0
            )
            .unwrap()
            .report(),
        sat.report()
    );
    assert_eq!(
        store
            .lookup_occurrence(
                &prepared,
                &context,
                Some(context.scope()),
                sat.report().invocation_digest(),
                1
            )
            .unwrap()
            .report(),
        unsat.report()
    );
    assert_eq!(
        store
            .lookup_occurrence(
                &prepared,
                &context,
                Some(context.scope()),
                sat.report().invocation_digest(),
                2
            )
            .unwrap()
            .report(),
        repeated.report()
    );
    assert!(matches!(
        store.lookup_occurrence(
            &prepared,
            &context,
            Some(context.scope()),
            sat.report().invocation_digest(),
            3
        ),
        Err(ScopedLookupError::NotFound)
    ));
    assert_eq!(
        sat.report().validation_basis(),
        Some(ValidationBasis::SolverReported)
    );
    assert_eq!(
        unsat.report().validation_basis(),
        Some(ValidationBasis::SolverReported)
    );
    assert!(!sat.report().proof_checked());
    assert!(!unsat.report().proof_checked());
}

#[test]
fn tlcx10_unknown_and_timeout_remain_distinct_without_promotion() {
    let (task, context) = make_task(pin("comparison"), pin("goal"), TaskKind::Reasoning);
    let prepared = prepare(&task);
    let mut unknown_store = ScopedSolverStore::with_capacity(1).unwrap();
    let unknown = unknown_store
        .execute_for_test(&prepared, &context, Some(context.scope()), |q| {
            Ok(report(q, ProcessRunState::Completed, b"unknown\n"))
        })
        .unwrap();
    let observed = unknown_store
        .lookup(
            &prepared,
            &context,
            Some(context.scope()),
            unknown.report().invocation_digest(),
        )
        .unwrap()
        .report();
    assert_eq!(
        observed.logical_result(),
        Some(LogicalResult::Satisfiability(Satisfiability::Unknown))
    );
    assert_eq!(
        observed.validation_basis(),
        Some(ValidationBasis::SolverReported)
    );
    let mut timeout_store = ScopedSolverStore::with_capacity(1).unwrap();
    let timeout = timeout_store
        .execute_for_test(&prepared, &context, Some(context.scope()), |q| {
            Ok(report(q, ProcessRunState::TimedOut, b""))
        })
        .unwrap();
    let observed_timeout = timeout_store
        .lookup(
            &prepared,
            &context,
            Some(context.scope()),
            timeout.report().invocation_digest(),
        )
        .unwrap()
        .report();
    assert_eq!(
        observed_timeout.state(),
        lc631_solvers::runner::SolverExecutionState::TimedOut
    );
    assert_eq!(observed_timeout.logical_result(), None);
    assert_eq!(observed_timeout.validation_basis(), None);
    for r in [observed, observed_timeout] {
        assert!(!r.proof_checked());
        assert!(!r.authority_created());
        assert!(!r.output_commit_allowed());
    }
}

#[test]
fn tlcx10_full_capacity_and_missing_goal_stop_before_runner() {
    let (task, context) = make_task(pin("comparison"), pin("goal"), TaskKind::Reasoning);
    let prepared = prepare(&task);
    let mut store = ScopedSolverStore::with_capacity(1).unwrap();
    store
        .execute_for_test(&prepared, &context, Some(context.scope()), |q| {
            Ok(report(q, ProcessRunState::Completed, b"sat\n"))
        })
        .unwrap();
    let invoked = Cell::new(false);
    let full = store.execute_for_test(&prepared, &context, Some(context.scope()), |q| {
        invoked.set(true);
        Ok(report(q, ProcessRunState::Completed, b"sat\n"))
    });
    assert!(matches!(
        full,
        Err(ScopedRunError::StoreBeforeExecution(
            ResultStoreError::CapacityReached
        ))
    ));
    assert!(!invoked.get());
    assert_eq!(store.len(), 1);
    let mut empty = ScopedSolverStore::with_capacity(1).unwrap();
    let bad = empty.execute_for_test(&prepared, &context, None, |q| {
        invoked.set(true);
        Ok(report(q, ProcessRunState::Completed, b"sat\n"))
    });
    assert!(matches!(bad, Err(ScopedRunError::MissingGoal)));
    assert!(!invoked.get());
    assert!(empty.is_empty());
    let (_, other) = make_task(pin("comparison"), pin("other goal"), TaskKind::Reasoning);
    let bad_scope = empty.execute_for_test(&prepared, &context, Some(other.scope()), |q| {
        invoked.set(true);
        Ok(report(q, ProcessRunState::Completed, b"sat\n"))
    });
    assert!(matches!(bad_scope, Err(ScopedRunError::GoalScopeMismatch)));
    assert!(!invoked.get());
    assert!(empty.is_empty());
}

#[test]
fn tlcx10_test_only_swapped_query_report_is_returned_without_partial_insert() {
    let (task, context) = make_task(pin("comparison"), pin("goal"), TaskKind::Reasoning);
    let prepared = prepare(&task);
    let mut store = ScopedSolverStore::with_capacity(1).unwrap();
    let mut swapped = None;
    let result = store.execute_for_test(&prepared, &context, Some(context.scope()), |query| {
        let other_query = fixture::query(query.source());
        assert_ne!(query.digest(), other_query.digest());
        let foreign = report(&other_query, ProcessRunState::Completed, b"sat\n");
        swapped = Some(foreign);
        Ok(foreign)
    });
    match result {
        Err(ScopedRunError::StoreAfterExecution {
            error: ResultStoreError::BindingMismatch,
            report,
        }) => {
            assert_eq!(Some(report), swapped);
        }
        _ => panic!("mismatched report must be returned with the insertion error"),
    }
    assert!(store.is_empty());
}
