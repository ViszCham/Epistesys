//! 日本語: 実Until評価→所有store→Unverified ledger。権限・保存真理は生成しない。
//! English: Actual Until evaluator -> owned store -> Unverified ledger, never authority/preservation truth.
use lc631_core::context::{ContextRevision, ContextScope, SourceRevision};
use lc631_runtime::facet_ledger::*;
use lc631_runtime::finite_results::{FiniteResultStore, TemporalLedgerRequest};
use lc631_runtime::resource::{
    Cancellation, Deadline, ResourceAmount, ResourceBroker, ResourceError, ResourceKind,
    ResourceLimits, ResourceRequest,
};
use lc631_solvers::temporal::*;
use lc631_substrate::Digest32;
use std::time::Duration;

fn scope() -> ContextScope {
    let id = Digest32::from_bytes([1; 32]);
    ContextScope::declared(id, id, id, id)
}

fn query(context: ContextRevision) -> TemporalQuery {
    TemporalQuery::checked(
        context,
        vec![
            TemporalNode::Atom(0),
            TemporalNode::Atom(1),
            TemporalNode::Until { left: 0, right: 1 },
        ],
        2,
        TemporalProfile::ClosedFiniteKleeneV1,
    )
    .unwrap()
}

fn trace(middle: Truth, origin_seconds: i64) -> FiniteTrace {
    let id = Digest32::from_bytes([2; 32]);
    let clock = ClockSpec::exact(id, TimeScale::Monotonic, id, id);
    let anchor = Instant::absolute(clock, origin_seconds, TimeUnit::Second).unwrap();
    let rows = [
        vec![Truth::True, Truth::False],
        vec![middle, Truth::False],
        vec![Truth::False, Truth::True],
    ];
    FiniteTrace::checked(
        rows.into_iter()
            .enumerate()
            .map(|(i, values)| {
                TraceStep::new(
                    Instant::relative(anchor, i as i64, TimeUnit::Second).unwrap(),
                    values,
                )
            })
            .collect(),
        TraceClosure::Closed,
    )
    .unwrap()
}

fn transform(id: u8, binding: FiniteObservationBinding) -> TransformBinding {
    let current = scope();
    let other = Revision([9; 32]);
    TransformBinding {
        id: TransformId([id; 32]),
        source: binding.source,
        target: other,
        source_context: binding.context,
        target_context: other,
        direction: Direction::Forward,
        scope: ComparisonScope {
            id: ScopeId(*current.comparison().as_bytes()),
            baseline: other,
            root_goal: Revision(*current.goal().as_bytes()),
            probes: Revision(*current.probes().as_bytes()),
            policy: Revision(*current.semantics().as_bytes()),
        },
        producer: other,
        query: binding.query,
        result: binding.result,
    }
}

#[test]
fn tlcx13_balanced_until_unknown_counterexample_and_time_shift_reach_real_consumer() {
    let source = SourceRevision::of("balanced temporal consumer");
    let context = ContextRevision::original(source);
    let query = query(context);
    let expected = [Truth::True, Truth::False, Truth::Unknown];
    let traces: Vec<FiniteTrace> = expected.iter().map(|middle| trace(*middle, 0)).collect();
    let shifted: Vec<FiniteTrace> = expected.iter().map(|middle| trace(*middle, 100)).collect();
    let limits = ResourceLimits::uniform(1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let run = broker
        .begin_run(
            13,
            source.digest(),
            limits,
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
        let mut store = FiniteResultStore::with_resources(6, &view).unwrap();
        let mut ledger = FacetLedger::new(3, 3).unwrap();
        for (i, trace) in traces.iter().enumerate() {
            let id = store
                .evaluate_temporal(
                    &query,
                    trace,
                    context,
                    scope(),
                    EvaluationBudget::checked(9).unwrap(),
                    &view,
                )
                .unwrap();
            let report = store
                .lookup_temporal(id, &query, trace, context, scope())
                .unwrap();
            assert_eq!(report.truth(), expected[i]);
            assert_eq!(
                report.counterexample().is_some(),
                expected[i] == Truth::False
            );
            if let Some(counterexample) = report.counterexample() {
                assert_eq!(counterexample, trace);
                assert_eq!(counterexample.steps().len(), 3);
            }
            let binding = store
                .temporal_binding(id, &query, trace, context, scope())
                .unwrap();
            let binding_transform = transform(20 + i as u8, binding);
            let transform_id = binding_transform.id;
            let comparison = binding_transform.scope;
            ledger.admit_transform(binding_transform).unwrap();
            store
                .retain_temporal_observation(
                    TemporalLedgerRequest {
                        observation: id,
                        query: &query,
                        trace,
                        context,
                        scope: scope(),
                        transform: transform_id,
                        property: PropertyId([3; 32]),
                        required_evidence: Revision([4; 32]),
                        next_action: Revision([5; 32]),
                    },
                    &mut ledger,
                )
                .unwrap();
            assert_eq!(ledger.for_transform(transform_id).count(), 1);
            assert_eq!(ledger.unique_defects(comparison).count(), 0);
            assert_eq!(ledger.unique_gains(comparison).count(), 0);
            assert!(matches!(
                ledger.for_transform(transform_id).next().unwrap().facet,
                Facet::Uncertainty(Gap {
                    reason: GapReason::Unverified,
                    ..
                })
            ));
            let new_id = store
                .evaluate_temporal(
                    &query,
                    &shifted[i],
                    context,
                    scope(),
                    EvaluationBudget::checked(9).unwrap(),
                    &view,
                )
                .unwrap();
            let new_report = store
                .lookup_temporal(new_id, &query, &shifted[i], context, scope())
                .unwrap();
            assert_eq!(new_report.truth(), expected[i]);
            let new_binding = store
                .temporal_binding(new_id, &query, &shifted[i], context, scope())
                .unwrap();
            assert_ne!(binding.result, new_binding.result);
            assert_eq!(binding.query, new_binding.query);
            assert!(store
                .lookup_temporal(id, &query, &shifted[i], context, scope())
                .is_err());
        }
        assert_eq!(store.len(), 6);
        assert_eq!(ledger.entries().len(), 3);
    }
    reservation.release_confirmed().unwrap();
}

#[test]
fn tlcx13_balanced_expired_wall_budget_cannot_begin_a_successful_store_run() {
    let source = SourceRevision::of("expired temporal run");
    let limits = ResourceLimits::uniform(1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let result = broker.begin_run(
        13,
        source.digest(),
        limits,
        Deadline::at(std::time::Instant::now()),
        Cancellation::default(),
    );
    assert!(matches!(result, Err(ResourceError::DeadlineExceeded)));
    assert_eq!(broker.used(ResourceKind::Ram).unwrap(), 0);
}

#[test]
fn tlcx13_balanced_unsupported_infinite_or_open_trace_cannot_enter_closed_store() {
    let context = ContextRevision::original(SourceRevision::of("unsupported temporal"));
    assert_eq!(
        TemporalQuery::checked(
            context,
            vec![TemporalNode::Constant(true)],
            0,
            TemporalProfile::InfiniteLtl
        )
        .unwrap_err(),
        TemporalError::UnsupportedInfiniteProfile
    );
    let id = Digest32::from_bytes([8; 32]);
    let clock = ClockSpec::exact(id, TimeScale::Utc, id, id);
    let step = TraceStep::new(
        Instant::absolute(clock, 0, TimeUnit::Second).unwrap(),
        vec![],
    );
    assert_eq!(
        FiniteTrace::checked(vec![step.clone()], TraceClosure::OpenPrefix).unwrap_err(),
        TemporalError::UnsupportedOpenPrefix
    );
    assert_eq!(
        FiniteTrace::checked(vec![step], TraceClosure::LoopTo(0)).unwrap_err(),
        TemporalError::UnsupportedLoop
    );
}
