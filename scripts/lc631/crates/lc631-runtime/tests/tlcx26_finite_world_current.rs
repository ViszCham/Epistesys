//! 日本語: 履歴の可読性と、現在leaseで消費できることを混同しない。
//! English: Readable history is distinct from consumption under a current lease.
use lc631_core::{context::*, AuthorityRevision};
use lc631_runtime::{
    finite_results::{FiniteResultError, FiniteResultStore},
    materialized_world_bridge::*,
};
use lc631_solvers::temporal::*;
use lc631_substrate::{resource::*, Digest32};
use lc631_world::{context_graph::ContextGraph, materialized_worlds::*};
use std::{sync::Arc, time::Duration};

fn graph() -> ContextGraph {
    let source = SourceRevision::of("finite world current input");
    let pin = source.digest();
    ContextGraph::new(
        OriginalContext::checked(
            source,
            ContextScope::declared(pin, pin, pin, pin),
            AuthorityRevision(1),
            vec![ContextPremise::reported(
                pin,
                Arc::from("Given model"),
                PremiseBasis::Given,
                false,
            )],
            ContextLimits {
                max_contexts: 4,
                max_premises: 4,
                max_edits: 4,
                max_statement_bytes: 1024,
            },
        )
        .unwrap(),
        4,
    )
    .unwrap()
}
fn query(context: ContextRevision) -> TemporalQuery {
    TemporalQuery::checked(
        context,
        vec![TemporalNode::Atom(0)],
        0,
        TemporalProfile::ClosedFiniteKleeneV1,
    )
    .unwrap()
}
fn trace(truth: Truth) -> FiniteTrace {
    let pin = Digest32::from_bytes([4; 32]);
    let clock = ClockSpec::exact(pin, TimeScale::Monotonic, pin, pin);
    FiniteTrace::checked(
        vec![TraceStep::new(
            Instant::absolute(clock, 0, TimeUnit::Second).unwrap(),
            vec![truth],
        )],
        TraceClosure::Closed,
    )
    .unwrap()
}
fn lease(
    issuer: &ResourceLeaseIssuer,
    source: Digest32,
    cancel: Cancellation,
    duration: Duration,
) -> ResourceLease {
    let reported = ReportedResourceLease::checked(
        7,
        source,
        &[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(1024 * 1024).unwrap(),
        )],
        Deadline::after(duration).unwrap(),
        cancel,
    )
    .unwrap();
    issuer.issue_admitted(10, reported).unwrap()
}
fn limits() -> MaterializedLimits {
    MaterializedLimits {
        max_live: 2,
        max_attempts: 8,
        max_payload_bytes: 8192,
        max_referenced_bytes: 32768,
    }
}

#[test]
fn tlcx26_current_foreign_same_numeric_lease_cannot_launder_historical_store() {
    let graph = graph();
    let context = graph.states()[0].revision();
    let scope = graph.states()[0].scope();
    let q = query(context);
    let input = trace(Truth::True);
    let old_issuer = ResourceLeaseIssuer::new().unwrap();
    let fresh_issuer = ResourceLeaseIssuer::new().unwrap();
    let old = lease(
        &old_issuer,
        context.source().digest(),
        Cancellation::default(),
        Duration::from_secs(30),
    );
    let fresh = lease(
        &fresh_issuer,
        context.source().digest(),
        Cancellation::default(),
        Duration::from_secs(30),
    );
    let old_view = old.view();
    let fresh_view = fresh.view();
    assert_eq!(old_view.run_id(), fresh_view.run_id());
    assert_eq!(old_view.reservation_id(), fresh_view.reservation_id());
    assert_ne!(old_view.owner_id(), fresh_view.owner_id());
    let mut store = FiniteResultStore::with_resources(1, &old_view).unwrap();
    let id = store
        .evaluate_temporal(
            &q,
            &input,
            context,
            scope,
            EvaluationBudget::default(),
            &old_view,
        )
        .unwrap();
    let prepared = PreparedTemporalWorld::with_resources(&q, &input, &fresh_view).unwrap();
    let mut worlds = MaterializedWorlds::new(&graph, limits()).unwrap();
    let key = worlds.admit(prepared.world_query()).unwrap().key();
    // Historical APIs intentionally remain readable; fresh payload admission
    // alone never makes the old store's real reservation current.
    assert_eq!(
        store
            .lookup_temporal(id, &q, &input, context, scope)
            .unwrap()
            .truth(),
        Truth::True
    );
    assert_eq!(
        observe_temporal_world(&worlds, key, &prepared, &store, id, scope)
            .unwrap()
            .report()
            .truth(),
        Truth::True
    );
    assert!(matches!(
        store.lookup_temporal_current(id, &q, &input, context, scope, &fresh_view),
        Err(FiniteResultError::Resource(ResourceError::ForeignOwner))
    ));
    assert!(matches!(
        observe_temporal_world_current(&worlds, key, &prepared, &store, id, scope, &fresh_view),
        Err(WorldBridgeError::Store(FiniteResultError::Resource(
            ResourceError::ForeignOwner
        )))
    ));
}

#[test]
fn tlcx26_current_cancelled_store_with_fresh_payload_stays_historical() {
    let graph = graph();
    let context = graph.states()[0].revision();
    let scope = graph.states()[0].scope();
    let q = query(context);
    let input = trace(Truth::True);
    let cancel = Cancellation::default();
    let old_issuer = ResourceLeaseIssuer::new().unwrap();
    let fresh_issuer = ResourceLeaseIssuer::new().unwrap();
    let old = lease(
        &old_issuer,
        context.source().digest(),
        cancel.clone(),
        Duration::from_secs(30),
    );
    let fresh = lease(
        &fresh_issuer,
        context.source().digest(),
        Cancellation::default(),
        Duration::from_secs(30),
    );
    let old_view = old.view();
    let fresh_view = fresh.view();
    let mut store = FiniteResultStore::with_resources(1, &old_view).unwrap();
    let id = store
        .evaluate_temporal(
            &q,
            &input,
            context,
            scope,
            EvaluationBudget::default(),
            &old_view,
        )
        .unwrap();
    cancel.cancel();
    let prepared = PreparedTemporalWorld::with_resources(&q, &input, &fresh_view).unwrap();
    let mut worlds = MaterializedWorlds::new(&graph, limits()).unwrap();
    let key = worlds.admit(prepared.world_query()).unwrap().key();
    assert_eq!(
        observe_temporal_world(&worlds, key, &prepared, &store, id, scope)
            .unwrap()
            .report()
            .truth(),
        Truth::True
    );
    assert!(matches!(
        store.lookup_temporal_current(id, &q, &input, context, scope, &fresh_view),
        Err(FiniteResultError::Resource(ResourceError::Cancelled))
    ));
    assert!(matches!(
        observe_temporal_world_current(&worlds, key, &prepared, &store, id, scope, &fresh_view),
        Err(WorldBridgeError::Store(FiniteResultError::Resource(
            ResourceError::Cancelled
        )))
    ));
}

#[test]
fn tlcx26_current_expired_store_with_fresh_payload_stays_historical() {
    let graph = graph();
    let context = graph.states()[0].revision();
    let scope = graph.states()[0].scope();
    let q = query(context);
    let input = trace(Truth::False);
    let old_issuer = ResourceLeaseIssuer::new().unwrap();
    let old = lease(
        &old_issuer,
        context.source().digest(),
        Cancellation::default(),
        Duration::from_secs(2),
    );
    let old_view = old.view();
    let mut store = FiniteResultStore::with_resources(1, &old_view).unwrap();
    let id = store
        .evaluate_temporal(
            &q,
            &input,
            context,
            scope,
            EvaluationBudget::default(),
            &old_view,
        )
        .unwrap();
    std::thread::sleep(Duration::from_millis(2100));
    let fresh_issuer = ResourceLeaseIssuer::new().unwrap();
    let fresh = lease(
        &fresh_issuer,
        context.source().digest(),
        Cancellation::default(),
        Duration::from_secs(30),
    );
    let fresh_view = fresh.view();
    let prepared = PreparedTemporalWorld::with_resources(&q, &input, &fresh_view).unwrap();
    let mut worlds = MaterializedWorlds::new(&graph, limits()).unwrap();
    let key = worlds.admit(prepared.world_query()).unwrap().key();
    assert_eq!(
        observe_temporal_world(&worlds, key, &prepared, &store, id, scope)
            .unwrap()
            .report()
            .truth(),
        Truth::False
    );
    assert!(matches!(
        store.lookup_temporal_current(id, &q, &input, context, scope, &fresh_view),
        Err(FiniteResultError::Resource(ResourceError::DeadlineExceeded))
    ));
    assert!(matches!(
        observe_temporal_world_current(&worlds, key, &prepared, &store, id, scope, &fresh_view),
        Err(WorldBridgeError::Store(FiniteResultError::Resource(
            ResourceError::DeadlineExceeded
        )))
    ));
}

#[test]
fn tlcx26_current_consumption_rechecks_both_prepared_and_store_without_erasing_history() {
    let graph = graph();
    let context = graph.states()[0].revision();
    let scope = graph.states()[0].scope();
    let q = query(context);
    let input = trace(Truth::Unknown);
    let cancel = Cancellation::default();
    let issuer = ResourceLeaseIssuer::new().unwrap();
    let actual = lease(
        &issuer,
        context.source().digest(),
        cancel.clone(),
        Duration::from_secs(30),
    );
    let view = actual.view();
    let prepared = PreparedTemporalWorld::with_resources(&q, &input, &view).unwrap();
    let mut worlds = MaterializedWorlds::new(&graph, limits()).unwrap();
    let key = worlds.admit(prepared.world_query()).unwrap().key();
    let mut store = FiniteResultStore::with_resources(1, &view).unwrap();
    let id = store
        .evaluate_temporal(
            &q,
            &input,
            context,
            scope,
            EvaluationBudget::default(),
            &view,
        )
        .unwrap();
    let observed =
        observe_temporal_world_current(&worlds, key, &prepared, &store, id, scope, &view).unwrap();
    assert_eq!(
        observed.report_current(&view).unwrap().truth(),
        Truth::Unknown
    );
    let foreign_issuer = ResourceLeaseIssuer::new().unwrap();
    let foreign = lease(
        &foreign_issuer,
        context.source().digest(),
        Cancellation::default(),
        Duration::from_secs(30),
    );
    let foreign_view = foreign.view();
    assert!(observed.report_current(&foreign_view).is_err());
    cancel.cancel();
    assert!(observed.report_current(&view).is_err());
    assert_eq!(observed.historical_report().truth(), Truth::Unknown);
    assert_eq!(
        store
            .lookup_temporal(id, &q, &input, context, scope)
            .unwrap()
            .truth(),
        Truth::Unknown
    );
}

#[test]
fn tlcx26_current_fresh_prepared_foreign_to_store_view_and_exact_binding_cuts() {
    let graph = graph();
    let context = graph.states()[0].revision();
    let scope = graph.states()[0].scope();
    let q = query(context);
    let input = trace(Truth::True);
    let changed = trace(Truth::False);
    let issuer = ResourceLeaseIssuer::new().unwrap();
    let foreign_issuer = ResourceLeaseIssuer::new().unwrap();
    let actual = lease(
        &issuer,
        context.source().digest(),
        Cancellation::default(),
        Duration::from_secs(30),
    );
    let foreign = lease(
        &foreign_issuer,
        context.source().digest(),
        Cancellation::default(),
        Duration::from_secs(30),
    );
    let view = actual.view();
    let foreign_view = foreign.view();
    let prepared = PreparedTemporalWorld::with_resources(&q, &input, &foreign_view).unwrap();
    let mut worlds = MaterializedWorlds::new(&graph, limits()).unwrap();
    let key = worlds.admit(prepared.world_query()).unwrap().key();
    let mut store = FiniteResultStore::with_resources(1, &view).unwrap();
    let id = store
        .evaluate_temporal(
            &q,
            &input,
            context,
            scope,
            EvaluationBudget::default(),
            &view,
        )
        .unwrap();
    assert!(matches!(
        observe_temporal_world_current(&worlds, key, &prepared, &store, id, scope, &view),
        Err(WorldBridgeError::Resource(ResourceError::ForeignOwner))
    ));
    assert!(matches!(
        store.lookup_temporal_current(id, &q, &changed, context, scope, &view),
        Err(FiniteResultError::BindingMismatch)
    ));
    let other_scope = ContextScope::declared(
        Digest32::from_bytes([99; 32]),
        scope.goal(),
        scope.probes(),
        scope.semantics(),
    );
    assert!(matches!(
        store.lookup_temporal_current(id, &q, &input, context, other_scope, &view),
        Err(FiniteResultError::BindingMismatch)
    ));
}
