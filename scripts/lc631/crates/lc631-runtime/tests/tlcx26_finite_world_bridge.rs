use lc631_core::{context::*, AuthorityRevision};
use lc631_runtime::finite_results::FiniteResultStore;
use lc631_runtime::materialized_world_bridge::*;
use lc631_solvers::temporal::*;
use lc631_substrate::resource::*;
use lc631_substrate::Digest32;
use lc631_world::context_graph::ContextGraph;
use lc631_world::materialized_worlds::*;
use std::{sync::Arc, time::Duration};

fn graph() -> ContextGraph {
    let source = SourceRevision::of("world temporal input");
    let pin = source.digest();
    ContextGraph::new(
        OriginalContext::checked(
            source,
            ContextScope::declared(pin, pin, pin, pin),
            AuthorityRevision(1),
            vec![ContextPremise::reported(
                pin,
                Arc::from("given model"),
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

fn trace(value: Truth) -> FiniteTrace {
    let pin = Digest32::from_bytes([4; 32]);
    let clock = ClockSpec::exact(pin, TimeScale::Monotonic, pin, pin);
    FiniteTrace::checked(
        vec![TraceStep::new(
            Instant::absolute(clock, 0, TimeUnit::Second).unwrap(),
            vec![value],
        )],
        TraceClosure::Closed,
    )
    .unwrap()
}

fn query(context: ContextRevision, negate: bool) -> TemporalQuery {
    let nodes = if negate {
        vec![TemporalNode::Atom(0), TemporalNode::Not(0)]
    } else {
        vec![TemporalNode::Atom(0)]
    };
    let root = (nodes.len() - 1) as u16;
    TemporalQuery::checked(context, nodes, root, TemporalProfile::ClosedFiniteKleeneV1).unwrap()
}

fn limits() -> MaterializedLimits {
    MaterializedLimits {
        max_live: 2,
        max_attempts: 8,
        max_payload_bytes: 8192,
        max_referenced_bytes: 32768,
    }
}

fn with_resources(f: impl FnOnce(&ResourceReservationView<'_>)) {
    let issuer = ResourceLeaseIssuer::new().unwrap();
    let reported = ReportedResourceLease::checked(
        1,
        SourceRevision::of("world temporal input").digest(),
        &[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(1024 * 1024).unwrap(),
        )],
        Deadline::after(Duration::from_secs(30)).unwrap(),
        Cancellation::default(),
    )
    .unwrap();
    let lease = issuer.issue_admitted(1, reported).unwrap();
    f(&lease.view());
}

#[test]
fn tlcx26_actual_finite_store_world_and_distinctness_consumer() {
    let graph = graph();
    let context = graph.states()[0].revision();
    let scope = graph.states()[0].scope();
    let query_a = query(context, false);
    let query_b = query(context, true);
    let input = trace(Truth::True);
    with_resources(|resources| {
        let prepared_a =
            PreparedTemporalWorld::with_resources(&query_a, &input, resources).unwrap();
        // Independent field/length fixture for one Atom node and one step:
        // fixed header219 + node5 + canonical timestamp16 + one truth byte.
        let payload = prepared_a.world_query().query_bytes();
        assert_eq!(payload.len(), 241);
        assert_eq!(&payload[..8], b"EPIW26T1");
        assert_eq!(&payload[72..104], query_a.profile().digest().as_bytes());
        assert_eq!(&payload[210..218], &0u64.to_le_bytes());
        assert_eq!(&payload[218..220], &[0, 0]);
        let prepared_b =
            PreparedTemporalWorld::with_resources(&query_b, &input, resources).unwrap();
        let mut worlds = MaterializedWorlds::new(&graph, limits()).unwrap();
        let key_a = worlds.admit(prepared_a.world_query()).unwrap().key();
        let key_b = worlds.admit(prepared_b.world_query()).unwrap().key();
        let mut store = FiniteResultStore::with_resources(2, resources).unwrap();
        let a = store
            .evaluate_temporal(
                &query_a,
                &input,
                context,
                scope,
                EvaluationBudget::default(),
                resources,
            )
            .unwrap();
        let b = store
            .evaluate_temporal(
                &query_b,
                &input,
                context,
                scope,
                EvaluationBudget::default(),
                resources,
            )
            .unwrap();
        let a = observe_temporal_world(&worlds, key_a, &prepared_a, &store, a, scope).unwrap();
        let b = observe_temporal_world(&worlds, key_b, &prepared_b, &store, b, scope).unwrap();
        assert_eq!(a.report().truth(), Truth::True);
        assert_eq!(b.report().truth(), Truth::False);
        assert_eq!(
            compare_observed(&a, &b),
            LogicalDistinctness::DifferentOnGivenTrace
        );
        assert_eq!(a.source_meaning(), SourceMeaning::NeedsEvidence);
        let coverage = observed_coverage(&[a, b]).unwrap();
        assert_eq!(coverage.executed_live, 2);
        assert_eq!(worlds.coverage().semantic_distinct_unconfirmed, 2);
        assert!(matches!(
            worlds.coverage().base,
            BaseCoverage::Insufficient { actual: 2, .. }
        ));
        assert_eq!(
            worlds.coverage().execution,
            ExecutionCoverage::Unobserved {
                reported_completed: 0
            }
        );
    });
}

#[test]
fn tlcx26_store_cut_and_swapped_trace_cannot_create_observed_world() {
    let graph = graph();
    let context = graph.states()[0].revision();
    let scope = graph.states()[0].scope();
    let query = query(context, false);
    let input = trace(Truth::True);
    let changed = trace(Truth::False);
    with_resources(|resources| {
        let prepared = PreparedTemporalWorld::with_resources(&query, &input, resources).unwrap();
        let changed = PreparedTemporalWorld::with_resources(&query, &changed, resources).unwrap();
        let mut worlds = MaterializedWorlds::new(&graph, limits()).unwrap();
        let key = worlds.admit(prepared.world_query()).unwrap().key();
        let mut store = FiniteResultStore::with_resources(1, resources).unwrap();
        let empty = FiniteResultStore::with_resources(1, resources).unwrap();
        let id = store
            .evaluate_temporal(
                &query,
                &input,
                context,
                scope,
                EvaluationBudget::default(),
                resources,
            )
            .unwrap();
        assert!(matches!(
            observe_temporal_world(&worlds, key, &prepared, &empty, id, scope),
            Err(WorldBridgeError::Store(_))
        ));
        assert!(matches!(
            observe_temporal_world(&worlds, key, &changed, &store, id, scope),
            Err(WorldBridgeError::PayloadMismatch)
        ));
        let foreign = ContextScope::declared(
            Digest32::from_bytes([8; 32]),
            scope.goal(),
            scope.probes(),
            scope.semantics(),
        );
        assert!(matches!(
            observe_temporal_world(&worlds, key, &prepared, &store, id, foreign),
            Err(WorldBridgeError::ScopeMismatch)
        ));
        worlds.remove(key).unwrap();
        assert!(matches!(
            observe_temporal_world(&worlds, key, &prepared, &store, id, scope),
            Err(WorldBridgeError::World(MaterializedError::StaleKey))
        ));
    });
}

#[test]
fn tlcx26_unknown_and_duplicate_observations_do_not_inflate_coverage() {
    let graph = graph();
    let context = graph.states()[0].revision();
    let scope = graph.states()[0].scope();
    let query = query(context, false);
    let input = trace(Truth::Unknown);
    with_resources(|resources| {
        let prepared = PreparedTemporalWorld::with_resources(&query, &input, resources).unwrap();
        let mut worlds = MaterializedWorlds::new(&graph, limits()).unwrap();
        let key = worlds.admit(prepared.world_query()).unwrap().key();
        let mut store = FiniteResultStore::with_resources(1, resources).unwrap();
        let id = store
            .evaluate_temporal(
                &query,
                &input,
                context,
                scope,
                EvaluationBudget::default(),
                resources,
            )
            .unwrap();
        let a = observe_temporal_world(&worlds, key, &prepared, &store, id, scope).unwrap();
        let b = observe_temporal_world(&worlds, key, &prepared, &store, id, scope).unwrap();
        assert_eq!(compare_observed(&a, &b), LogicalDistinctness::Unconfirmed);
        assert_eq!(a.report().truth(), Truth::Unknown);
        assert_eq!(observed_coverage(&[a, b]).unwrap().executed_live, 1);
    });
}

#[test]
fn tlcx26_plain_bytes_and_reported_completion_are_not_store_observations() {
    let graph = graph();
    let context = graph.states()[0].revision();
    let scope = graph.states()[0].scope();
    let query = query(context, false);
    let input = trace(Truth::True);
    with_resources(|resources| {
        let prepared = PreparedTemporalWorld::with_resources(&query, &input, resources).unwrap();
        let mut worlds = MaterializedWorlds::new(&graph, limits()).unwrap();
        let key = worlds
            .admit(BoundWorldQuery::reported(context, b"wrong-profile", b"sat"))
            .unwrap()
            .key();
        let mut store = FiniteResultStore::with_resources(1, resources).unwrap();
        let id = store
            .evaluate_temporal(
                &query,
                &input,
                context,
                scope,
                EvaluationBudget::default(),
                resources,
            )
            .unwrap();
        assert!(matches!(
            observe_temporal_world(&worlds, key, &prepared, &store, id, scope),
            Err(WorldBridgeError::PayloadMismatch)
        ));
        assert_eq!(observed_coverage(&[]), Err(WorldBridgeError::EmptyCoverage));
    });
}

#[test]
fn tlcx26_payload_admission_and_current_cancellation_are_separate_from_history() {
    let graph = graph();
    let context = graph.states()[0].revision();
    let scope = graph.states()[0].scope();
    let query = query(context, false);
    let input = trace(Truth::True);
    let issuer = ResourceLeaseIssuer::new().unwrap();
    let cancellation = Cancellation::default();
    let reported = |bytes| {
        ReportedResourceLease::checked(
            7,
            context.source().digest(),
            &[ResourceRequest::new(
                ResourceKind::Ram,
                ResourceAmount::checked(bytes).unwrap(),
            )],
            Deadline::after(Duration::from_secs(30)).unwrap(),
            cancellation.clone(),
        )
        .unwrap()
    };
    let small = issuer.issue_admitted(9, reported(100)).unwrap();
    assert!(matches!(
        PreparedTemporalWorld::with_resources(&query, &input, &small.view()),
        Err(WorldBridgeError::Resource(ResourceError::Exhausted(
            ResourceKind::Ram
        )))
    ));
    let lease = issuer.issue_admitted(10, reported(1024 * 1024)).unwrap();
    let resources = lease.view();
    let prepared = PreparedTemporalWorld::with_resources(&query, &input, &resources).unwrap();
    let mut worlds = MaterializedWorlds::new(&graph, limits()).unwrap();
    let key = worlds.admit(prepared.world_query()).unwrap().key();
    let mut store = FiniteResultStore::with_resources(1, &resources).unwrap();
    let id = store
        .evaluate_temporal(
            &query,
            &input,
            context,
            scope,
            EvaluationBudget::default(),
            &resources,
        )
        .unwrap();
    cancellation.cancel();
    assert!(matches!(
        observe_temporal_world(&worlds, key, &prepared, &store, id, scope),
        Err(WorldBridgeError::Resource(ResourceError::Cancelled))
    ));
    assert_eq!(
        store
            .lookup_temporal(id, &query, &input, context, scope)
            .unwrap()
            .truth(),
        Truth::True
    );
}
