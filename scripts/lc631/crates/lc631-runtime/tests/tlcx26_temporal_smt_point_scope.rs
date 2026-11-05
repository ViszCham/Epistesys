//! 日本語: 実同一入力でもCandidate comparisonだけの差替えを拒否する独立負例。
//! English: Reject only the candidate comparison swap, with genuine current owners.
use lc631_core::context::{ContextRevision, ContextScope, SourceRevision};
use lc631_runtime::{
    finite_results::FiniteResultStore, formal_boolean as f, formal_measurement as m, resource::*,
    temporal_smt_point::*,
};
use lc631_solvers::{
    temporal::*,
    typed_query::{BoolExpr, BoolSymbolId, SmtEmitLimits, SmtStatusQuery},
};
use lc631_substrate::{
    measurement_numeric::RationalInterval,
    source::{SourceSpan, SourceStore},
    Digest32,
};
use std::{sync::Arc, time::Duration};

fn d(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}

#[test]
fn tlcx26_point_foreign_candidate_comparison_cannot_join_current_native_scope() {
    let source = SourceStore::new(Arc::from("Given point scope attack"), 1024).unwrap();
    let revision = SourceRevision::of(source.text());
    let context = ContextRevision::original(revision);
    let scope = ContextScope::declared(d(1), d(2), d(3), d(4));
    let symbol = BoolSymbolId::new(7);
    let predicate = BoolExpr::Variable(symbol);
    let theory = SmtStatusQuery::checked(
        vec![symbol],
        vec![],
        vec![predicate.clone()],
        SmtEmitLimits::default(),
    )
    .unwrap();
    let symbols = [(symbol, symbol)];
    let mut formal = f::Request {
        source: &theory,
        target: &theory,
        source_predicate: &predicate,
        target_predicate: &predicate,
        symbols: &symbols,
        unconstrained: f::UnconstrainedPolicy::ExplicitlyGiven,
        binding: f::Binding {
            source: revision,
            target: revision,
            source_context: context,
            target_context: context,
            probe_set: d(3),
            probe_map: d(0),
        },
    };
    formal.binding.probe_map = f::probe_map_digest(&formal, 100_000).unwrap();
    let one = RationalInterval::exact_ratio(1, 1).unwrap();
    let probes = [m::FixedProbe {
        identity: b"point-scope",
        weight: one.lower(),
        template: &formal,
    }];
    let work = m::WorkBudget {
        max_work: 100_000,
        max_payload_bytes: 1_000_000,
    };
    let plan = m::ProbePlan::checked(
        m::PlanScope {
            comparison: scope.comparison(),
            definition: d(5),
            probe_set: scope.probes(),
            source: revision,
            context,
        },
        &theory,
        &probes,
        work,
    )
    .unwrap();
    let valid = m::Candidate::given(d(6), scope.comparison(), &theory, revision, context).unwrap();
    // Same candidate ID, target object, source revision and context. ONLY the
    // comparison identity changes; no native report, lease or numeric spoof.
    let foreign = m::Candidate::given(d(6), d(99), &theory, revision, context).unwrap();
    assert!(matches!(
        plan.measure(&foreign, m::Direction::Soundness, &[], work),
        Err(m::BridgeError::ScopeMismatch)
    ));
    let query = TemporalQuery::checked(
        context,
        vec![TemporalNode::Atom(0)],
        0,
        TemporalProfile::ClosedFiniteKleeneV1,
    )
    .unwrap();
    let clock = ClockSpec::exact(d(10), TimeScale::Monotonic, d(11), d(12));
    let trace = FiniteTrace::checked(
        vec![TraceStep::new(
            Instant::absolute(clock, 0, TimeUnit::Second).unwrap(),
            vec![Truth::True],
        )],
        TraceClosure::Closed,
    )
    .unwrap();
    let limits = ResourceLimits::uniform(16 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let run = broker
        .begin_run(
            2601,
            revision.digest(),
            limits,
            Deadline::after(Duration::from_secs(30)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let reservation = run
        .reserve(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(8 * 1024 * 1024).unwrap(),
        )])
        .unwrap();
    let view = reservation.lease();
    let mut store = FiniteResultStore::with_resources(1, &view).unwrap();
    let id = store
        .evaluate_temporal(
            &query,
            &trace,
            context,
            scope,
            EvaluationBudget::default(),
            &view,
        )
        .unwrap();
    let map = [AtomPair {
        temporal: 0,
        formal: symbol,
    }];
    let request = PointRequest {
        source: &source,
        span: SourceSpan::checked(&source, 0, source.text().len()).unwrap(),
        plan: &plan,
        candidate: &valid,
        probe: 0,
        native: NativePoint {
            store: &store,
            id,
            query: &query,
            trace: &trace,
            scope,
        },
        map: &map,
    };
    let control = evaluate_point(request, &view, PointBudget::checked(10_000).unwrap()).unwrap();
    assert_eq!(
        control.report_current(&view).unwrap().native_truth(),
        Truth::True
    );
    drop(control);
    let swapped = PointRequest {
        candidate: &foreign,
        ..request
    };
    assert!(
        matches!(
            evaluate_point(swapped, &view, PointBudget::checked(10_000).unwrap()),
            Err(PointError::ScopeBinding)
        ),
        "a foreign Candidate comparison must not inherit the plan/native comparison"
    );
}
