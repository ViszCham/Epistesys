//! Actual finite evaluator -> retained store, not an external process receipt.
use lc631_core::context::{ContextRevision, ContextScope, SourceRevision};
use lc631_runtime::finite_results::{FiniteResultError, FiniteResultStore};
use lc631_runtime::resource::{
    Cancellation, Deadline, ResourceAmount, ResourceBroker, ResourceKind, ResourceLimits,
    ResourceRequest,
};
use lc631_solvers::temporal::{
    ClockSpec, EvaluationBudget, FiniteTrace, Instant, TemporalNode, TemporalProfile,
    TemporalQuery, TimeScale, TimeUnit, TraceClosure, TraceStep, Truth,
};
use lc631_substrate::resource::{ReportedResourceLease, ResourceError, ResourceLeaseIssuer};
use lc631_substrate::Digest32;
use std::time::Duration;

fn scope() -> ContextScope {
    let id = Digest32::from_bytes([3; 32]);
    ContextScope::declared(id, id, id, id)
}

fn query(context: ContextRevision) -> TemporalQuery {
    TemporalQuery::checked(
        context,
        vec![TemporalNode::Atom(0), TemporalNode::Always(0)],
        1,
        TemporalProfile::ClosedFiniteKleeneV1,
    )
    .unwrap()
}

fn trace(truth: Truth) -> FiniteTrace {
    let id = Digest32::from_bytes([4; 32]);
    let clock = ClockSpec::exact(id, TimeScale::Monotonic, id, id);
    FiniteTrace::checked(
        vec![TraceStep::new(
            Instant::absolute(clock, 0, TimeUnit::Second).unwrap(),
            vec![truth],
        )],
        TraceClosure::Closed,
    )
    .unwrap()
}

#[test]
fn tlcx13_finite_store_consumes_real_result_and_rejects_lineage_swaps() {
    let source = SourceRevision::of("temporal store input");
    let context = ContextRevision::original(source);
    let query = query(context);
    let trace = trace(Truth::False);
    let cap = ResourceLimits::uniform(1024 * 1024).unwrap();
    let broker = ResourceBroker::new(cap).unwrap();
    let run = broker
        .begin_run(
            1,
            source.digest(),
            cap,
            Deadline::after(Duration::from_secs(30)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let mut reservation = run
        .reserve(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(1024 * 1024).unwrap(),
        )])
        .unwrap();
    {
        let view = reservation.lease();
        let mut store = FiniteResultStore::with_resources(2, &view).unwrap();
        assert!(store.is_empty());
        let receipt = store
            .evaluate_temporal(
                &query,
                &trace,
                context,
                scope(),
                EvaluationBudget::default(),
                &view,
            )
            .unwrap();
        let observed = store
            .lookup_temporal(receipt, &query, &trace, context, scope())
            .unwrap();
        assert_eq!(observed.truth(), Truth::False);
        assert_eq!(observed.counterexample().unwrap().steps().len(), 1);
        assert_eq!(observed.query_digest(), query.digest());
        assert_temporal_ledger_consumption(&store, receipt, &query, &trace, context);
        let changed = context.reconstructed([9; 32]);
        assert!(matches!(
            store.lookup_temporal(receipt, &query, &trace, changed, scope()),
            Err(FiniteResultError::ContextMismatch)
        ));
        let changed_trace = crate::trace(Truth::True);
        assert!(matches!(
            store.lookup_temporal(receipt, &query, &changed_trace, context, scope()),
            Err(FiniteResultError::BindingMismatch)
        ));
        let wrong_scope = ContextScope::declared(
            source.digest(),
            source.digest(),
            source.digest(),
            source.digest(),
        );
        assert!(matches!(
            store.lookup_temporal(receipt, &query, &trace, context, wrong_scope),
            Err(FiniteResultError::BindingMismatch)
        ));
        assert_eq!(store.len(), 1);
    }
    reservation.release_confirmed().unwrap();
}

fn assert_temporal_ledger_consumption(
    store: &FiniteResultStore<'_, '_>,
    observation: lc631_runtime::finite_results::FiniteResultId,
    query: &TemporalQuery,
    trace: &FiniteTrace,
    context: ContextRevision,
) {
    use lc631_runtime::facet_ledger::*;
    use lc631_runtime::finite_results::{FiniteLedgerError, TemporalLedgerRequest};
    let binding = store
        .temporal_binding(observation, query, trace, context, scope())
        .unwrap();
    let rev = Revision([7; 32]);
    let transform = TransformId([8; 32]);
    let mut ledger = FacetLedger::new(1, 2).unwrap();
    ledger
        .admit_transform(TransformBinding {
            id: transform,
            source: binding.source,
            target: rev,
            source_context: binding.context,
            target_context: rev,
            direction: Direction::Forward,
            scope: ComparisonScope {
                id: ScopeId(*scope().comparison().as_bytes()),
                baseline: rev,
                root_goal: Revision(*scope().goal().as_bytes()),
                probes: Revision(*scope().probes().as_bytes()),
                policy: Revision(*scope().semantics().as_bytes()),
            },
            producer: rev,
            query: binding.query,
            result: binding.result,
        })
        .unwrap();
    let request = |current_scope| TemporalLedgerRequest {
        observation,
        query,
        trace,
        context,
        scope: current_scope,
        transform,
        property: PropertyId([9; 32]),
        required_evidence: rev,
        next_action: rev,
    };
    assert_eq!(
        store
            .retain_temporal_observation(request(scope()), &mut ledger)
            .unwrap(),
        binding
    );
    assert!(matches!(
        ledger.entries()[0].facet,
        Facet::Uncertainty(Gap {
            reason: GapReason::Unverified,
            ..
        })
    ));
    assert_eq!(
        ledger.unique_defects(ledger.transforms()[0].scope).count(),
        0
    );
    assert_eq!(ledger.unique_gains(ledger.transforms()[0].scope).count(), 0);
    let foreign = ContextScope::declared(
        Digest32::from_bytes([10; 32]),
        scope().goal(),
        scope().probes(),
        scope().semantics(),
    );
    assert!(matches!(
        store.retain_temporal_observation(request(foreign), &mut ledger),
        Err(FiniteLedgerError::Store(FiniteResultError::BindingMismatch))
    ));
    assert_eq!(ledger.entries().len(), 1);
}

#[test]
fn tlcx13_finite_store_unknown_capacity_and_cancellation_are_not_success() {
    let source = SourceRevision::of("unknown finite store input");
    let context = ContextRevision::original(source);
    let query = query(context);
    let trace = trace(Truth::Unknown);
    let cancellation = Cancellation::default();
    let cap = ResourceLimits::uniform(1024 * 1024).unwrap();
    let broker = ResourceBroker::new(cap).unwrap();
    let run = broker
        .begin_run(
            2,
            source.digest(),
            cap,
            Deadline::after(Duration::from_secs(30)).unwrap(),
            cancellation.clone(),
        )
        .unwrap();
    let mut reservation = run
        .reserve(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(1024 * 1024).unwrap(),
        )])
        .unwrap();
    {
        let view = reservation.lease();
        let mut store = FiniteResultStore::with_resources(2, &view).unwrap();
        let receipt = store
            .evaluate_temporal(
                &query,
                &trace,
                context,
                scope(),
                EvaluationBudget::default(),
                &view,
            )
            .unwrap();
        assert_eq!(
            store
                .lookup_temporal(receipt, &query, &trace, context, scope())
                .unwrap()
                .truth(),
            Truth::Unknown
        );
        store
            .evaluate_temporal(
                &query,
                &trace,
                context,
                scope(),
                EvaluationBudget::default(),
                &view,
            )
            .unwrap();
        assert!(matches!(
            store.evaluate_temporal(
                &query,
                &trace,
                context,
                scope(),
                EvaluationBudget::default(),
                &view
            ),
            Err(FiniteResultError::CapacityReached)
        ));
        let mut free_slot = FiniteResultStore::with_resources(1, &view).unwrap();
        cancellation.cancel();
        assert!(matches!(
            free_slot.evaluate_temporal(
                &query,
                &trace,
                context,
                scope(),
                EvaluationBudget::default(),
                &view
            ),
            Err(FiniteResultError::Resource(ResourceError::Cancelled))
        ));
        assert!(free_slot.is_empty());
        // Historical lookup is not fresh execution or authority restoration.
        assert_eq!(
            store
                .lookup_temporal(receipt, &query, &trace, context, scope())
                .unwrap()
                .truth(),
            Truth::Unknown
        );
    }
    reservation.release_confirmed().unwrap();
}

#[test]
fn tlcx13_finite_store_numeric_lease_collision_is_not_an_owner_match() {
    let source = SourceRevision::of("same numeric lease");
    let context = ContextRevision::original(source);
    let query = query(context);
    let trace = trace(Truth::True);
    // This explicitly admitted test lease is not an OS/global quota assertion.
    let issuer = ResourceLeaseIssuer::new().unwrap();
    let reported = ReportedResourceLease::checked(
        7,
        source.digest(),
        &[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(64 * 1024).unwrap(),
        )],
        Deadline::after(Duration::from_secs(30)).unwrap(),
        Cancellation::default(),
    )
    .unwrap();
    let lease_a = issuer.issue_admitted(9, reported.clone()).unwrap();
    let lease_b = issuer.issue_admitted(9, reported).unwrap();
    let view_a = lease_a.view();
    let view_b = lease_b.view();
    assert_eq!(
        (view_a.owner_id(), view_a.run_id(), view_a.reservation_id()),
        (view_b.owner_id(), view_b.run_id(), view_b.reservation_id())
    );
    let mut store_a = FiniteResultStore::with_resources(1, &view_a).unwrap();
    let store_b = FiniteResultStore::with_resources(1, &view_a).unwrap();
    assert!(matches!(
        store_a.evaluate_temporal(
            &query,
            &trace,
            context,
            scope(),
            EvaluationBudget::default(),
            &view_b
        ),
        Err(FiniteResultError::Resource(ResourceError::ForeignOwner))
    ));
    assert!(store_a.is_empty());
    let id = store_a
        .evaluate_temporal(
            &query,
            &trace,
            context,
            scope(),
            EvaluationBudget::default(),
            &view_a,
        )
        .unwrap();
    assert!(matches!(
        store_b.lookup_temporal(id, &query, &trace, context, scope()),
        Err(FiniteResultError::BindingMismatch)
    ));
}

#[test]
fn tlcx13_finite_store_budget_failure_is_not_inserted_and_work_is_refunded() {
    let source = SourceRevision::of("finite work denial");
    let context = ContextRevision::original(source);
    let small = query(context);
    let small_trace = trace(Truth::True);
    let large = TemporalQuery::checked(
        context,
        vec![TemporalNode::Constant(true); 1024],
        1023,
        TemporalProfile::ClosedFiniteKleeneV1,
    )
    .unwrap();
    let id = Digest32::from_bytes([4; 32]);
    let clock = ClockSpec::exact(id, TimeScale::Monotonic, id, id);
    let large_trace = FiniteTrace::checked(
        (0..256)
            .map(|n| {
                TraceStep::new(
                    Instant::absolute(clock, n, TimeUnit::Second).unwrap(),
                    vec![Truth::True],
                )
            })
            .collect(),
        TraceClosure::Closed,
    )
    .unwrap();
    let issuer = ResourceLeaseIssuer::new().unwrap();
    let reported = ReportedResourceLease::checked(
        8,
        source.digest(),
        &[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(64 * 1024).unwrap(),
        )],
        Deadline::after(Duration::from_secs(30)).unwrap(),
        Cancellation::default(),
    )
    .unwrap();
    let lease = issuer.issue_admitted(10, reported).unwrap();
    let view = lease.view();
    let mut store = FiniteResultStore::with_resources(2, &view).unwrap();
    assert!(matches!(
        store.evaluate_temporal(
            &large,
            &large_trace,
            context,
            scope(),
            EvaluationBudget::default(),
            &view
        ),
        Err(FiniteResultError::Resource(ResourceError::Exhausted(
            ResourceKind::Ram
        )))
    ));
    assert!(matches!(
        store.evaluate_temporal(
            &small,
            &small_trace,
            context,
            scope(),
            EvaluationBudget::checked(0).unwrap(),
            &view
        ),
        Err(FiniteResultError::Evaluation(
            lc631_solvers::temporal::TemporalError::ResourceExhausted
        ))
    ));
    assert!(store.is_empty());
    let id = store
        .evaluate_temporal(
            &small,
            &small_trace,
            context,
            scope(),
            EvaluationBudget::default(),
            &view,
        )
        .unwrap();
    assert_eq!(
        store
            .lookup_temporal(id, &small, &small_trace, context, scope())
            .unwrap()
            .truth(),
        Truth::True
    );
}
