//! 日本語: scope未指定の空理論をGivenな無制約理論へ暗黙昇格しない。
//! English: A missing-scope empty theory is not implicitly a Given unconstrained theory.
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
fn missing_scope_case(source_empty: bool, target_empty: bool) {
    assert!(source_empty || target_empty);
    let source = SourceStore::new(
        Arc::from("Given identity with explicitly missing theory scope"),
        1024,
    )
    .unwrap();
    let revision = SourceRevision::of(source.text());
    let context = ContextRevision::original(revision);
    let scope = ContextScope::declared(d(1), d(2), d(3), d(4));
    let symbol = BoolSymbolId::new(7);
    let predicate = BoolExpr::Variable(symbol);
    let src = SmtStatusQuery::checked(
        vec![symbol],
        vec![],
        if source_empty {
            vec![]
        } else {
            vec![predicate.clone()]
        },
        SmtEmitLimits::default(),
    )
    .unwrap();
    let target = SmtStatusQuery::checked(
        vec![symbol],
        vec![],
        if target_empty {
            vec![]
        } else {
            vec![predicate.clone()]
        },
        SmtEmitLimits::default(),
    )
    .unwrap();
    let symbols = [(symbol, symbol)];
    let mut request = f::Request {
        source: &src,
        target: &target,
        source_predicate: &predicate,
        target_predicate: &predicate,
        symbols: &symbols,
        unconstrained: f::UnconstrainedPolicy::MissingScope,
        binding: f::Binding {
            source: revision,
            target: revision,
            source_context: context,
            target_context: context,
            probe_set: d(3),
            probe_map: d(0),
        },
    };
    request.binding.probe_map = f::probe_map_digest(&request, 100_000).unwrap();
    // Existing canonical formal semantics explicitly retain NoComparable.
    // The new point relation must not erase that declared missing scope.
    assert!(matches!(
        f::evaluate(&request, 100_000).unwrap(),
        f::Evaluation::NoComparable {
            reason: f::NoComparable::MissingTheoryScope,
            ..
        }
    ));
    let one = RationalInterval::exact_ratio(1, 1).unwrap();
    let fixed = [m::FixedProbe {
        identity: b"missing-scope-p",
        weight: one.lower(),
        template: &request,
    }];
    let work = m::WorkBudget {
        max_work: 100_000,
        max_payload_bytes: 1_000_000,
    };
    let plan = m::ProbePlan::checked(
        m::PlanScope {
            comparison: d(1),
            definition: d(5),
            probe_set: d(3),
            source: revision,
            context,
        },
        &src,
        &fixed,
        work,
    )
    .unwrap();
    let candidate = m::Candidate::given(d(6), d(1), &target, revision, context).unwrap();
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
    let limits = ResourceLimits::uniform(8 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let run = broker
        .begin_run(
            26,
            revision.digest(),
            limits,
            Deadline::after(Duration::from_secs(30)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let reservation = run
        .reserve(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(4 * 1024 * 1024).unwrap(),
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
    let point = PointRequest {
        source: &source,
        span: SourceSpan::checked(&source, 0, source.text().len()).unwrap(),
        plan: &plan,
        candidate: &candidate,
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
    assert!(matches!(
        evaluate_point(point, &view, PointBudget::checked(10_000).unwrap()),
        Err(PointError::NeedsEvidence(_))
    ));
    // The missing formal scope does not delete or rewrite genuine native history.
    assert_eq!(
        store
            .lookup_temporal(id, &query, &trace, context, scope)
            .unwrap()
            .truth(),
        Truth::True
    );
}
#[test]
fn tlcx26_point_missing_source_scope_is_not_given_unconstrained_scope() {
    missing_scope_case(true, false);
}
#[test]
fn tlcx26_point_missing_target_scope_is_not_given_empty_theory() {
    missing_scope_case(false, true);
}
#[test]
fn tlcx26_point_both_missing_scopes_stay_non_comparable() {
    missing_scope_case(true, true);
}
