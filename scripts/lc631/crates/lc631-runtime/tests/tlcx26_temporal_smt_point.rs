//! 日本語: 一時点Boolean instanceの限定接続。一般同値・source真理ではない。
//! English: Bounded one-point Boolean instance connection, never general/source equivalence.
use lc631_core::{context::*, AuthorityRevision};
use lc631_runtime::{
    finite_results::FiniteResultStore, formal_boolean as f, formal_measurement as m,
    materialized_world_bridge::*, resource::*, temporal_smt_point::*,
};
use lc631_solvers::{
    smt::ModelValidationError,
    temporal::*,
    typed_query::{BoolExpr, BoolSymbolId, SmtEmitLimits, SmtStatusQuery},
};
use lc631_substrate::{
    measurement_numeric::RationalInterval,
    source::{SourceSpan, SourceStore},
    Digest32,
};
use lc631_world::{context_graph::ContextGraph, materialized_worlds::*};
use std::{sync::Arc, time::Duration};
fn d(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}
struct Case<'a> {
    request: PointRequest<'a, 'a, 'a>,
    view: &'a ResourceReservationView<'a>,
    graph: &'a ContextGraph,
    cancel: Cancellation,
}
fn with_case(
    negated: bool,
    constant_predicate: bool,
    truth: Truth,
    body: impl for<'a> FnOnce(Case<'a>),
) {
    let source = SourceStore::new(Arc::from("Given closed point Boolean source"), 1024).unwrap();
    let revision = SourceRevision::of(source.text());
    let scope = ContextScope::declared(d(1), d(2), d(3), d(4));
    let graph = ContextGraph::new(
        OriginalContext::checked(
            revision,
            scope,
            AuthorityRevision(1),
            vec![ContextPremise::reported(
                d(8),
                Arc::from("Given point interpretation"),
                PremiseBasis::Given,
                false,
            )],
            ContextLimits {
                max_contexts: 2,
                max_premises: 2,
                max_edits: 2,
                max_statement_bytes: 1024,
            },
        )
        .unwrap(),
        2,
    )
    .unwrap();
    let context = graph.states()[0].revision();
    let symbol = BoolSymbolId::new(7); // IDs need not equal temporal atom index0.
    let variable = BoolExpr::Variable(symbol);
    let predicate = if constant_predicate {
        BoolExpr::Constant(true)
    } else {
        variable.clone()
    };
    let assertion = if negated {
        BoolExpr::Not(Box::new(variable))
    } else {
        variable
    };
    let theory = SmtStatusQuery::checked(
        vec![symbol],
        vec![],
        vec![assertion],
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
        identity: b"point-predicate",
        weight: one.lower(),
        template: &formal,
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
        &theory,
        &probes,
        work,
    )
    .unwrap();
    let candidate = m::Candidate::given(d(6), d(1), &theory, revision, context).unwrap();
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
            vec![truth],
        )],
        TraceClosure::Closed,
    )
    .unwrap();
    let cap = ResourceLimits::uniform(16 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(cap).unwrap();
    let cancel = Cancellation::default();
    let run = broker
        .begin_run(
            26,
            revision.digest(),
            cap,
            Deadline::after(Duration::from_secs(30)).unwrap(),
            cancel.clone(),
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
    body(Case {
        request: PointRequest {
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
        },
        view: &view,
        graph: &graph,
        cancel,
    });
}

#[test]
fn tlcx26_point_counterexample_native_true_cannot_satisfy_negated_smt_theory() {
    with_case(true, false, Truth::True, |case| {
        assert!(matches!(
            evaluate_point(
                case.request,
                case.view,
                PointBudget::checked(10_000).unwrap()
            ),
            Err(PointError::NotModel(ModelValidationError::AssertionFalse {
                index: 0
            }))
        ));
    });
}
#[test]
fn tlcx26_point_equal_scalar_does_not_replace_exact_predicate_correspondence() {
    with_case(false, true, Truth::True, |case| {
        // Temporal p and formal constant true agree here, not in general.
        assert!(matches!(
            evaluate_point(
                case.request,
                case.view,
                PointBudget::checked(10_000).unwrap()
            ),
            Err(PointError::PredicateMismatch)
        ));
    });
}
#[test]
fn tlcx26_point_unknown_and_incomplete_map_never_become_false_or_zero() {
    with_case(false, false, Truth::Unknown, |case| {
        assert!(matches!(
            evaluate_point(
                case.request,
                case.view,
                PointBudget::checked(10_000).unwrap()
            ),
            Err(PointError::NeedsEvidence(MissingPoint::UnknownAtom(0)))
        ));
    });
    with_case(false, false, Truth::True, |case| {
        let absent = PointRequest {
            map: &[],
            ..case.request
        };
        assert!(matches!(
            evaluate_point(absent, case.view, PointBudget::checked(10_000).unwrap()),
            Err(PointError::Unsupported(UnsupportedPoint::IncompleteMap))
        ));
    });
}
#[test]
fn tlcx26_point_actual_native_store_and_trace_assigned_model_remain_bound() {
    with_case(false, false, Truth::True, |case| {
        let request = case.request;
        let point =
            evaluate_point(request, case.view, PointBudget::checked(10_000).unwrap()).unwrap();
        let report = point.report_current(case.view).unwrap();
        assert_eq!(report.native_truth(), Truth::True);
        assert_eq!(report.assertions_checked(), 1);
        assert_eq!(report.kind(), PointKind::GivenClosedPointBooleanInstance);
        assert_eq!(report.source_meaning(), SourceMeaning::NeedsEvidence);
        let prepared = PreparedTemporalWorld::with_resources(
            request.native.query,
            request.native.trace,
            case.view,
        )
        .unwrap();
        let mut worlds = MaterializedWorlds::new(
            case.graph,
            MaterializedLimits {
                max_live: 1,
                max_attempts: 1,
                max_payload_bytes: 1024,
                max_referenced_bytes: 4096,
            },
        )
        .unwrap();
        let key = worlds.admit(prepared.world_query()).unwrap().key();
        let observed = observe_temporal_world_current(
            &worlds,
            key,
            &prepared,
            request.native.store,
            request.native.id,
            request.native.scope,
            case.view,
        )
        .unwrap();
        point
            .check_world_current(&observed, request.plan, request.candidate, case.view)
            .unwrap();
        let cap = ResourceLimits::uniform(16 * 1024 * 1024).unwrap();
        let broker = ResourceBroker::new(cap).unwrap();
        let run = broker
            .begin_run(
                26,
                request.source.id().digest(),
                cap,
                Deadline::after(Duration::from_secs(30)).unwrap(),
                Cancellation::default(),
            )
            .unwrap();
        let foreign = run
            .reserve(&[ResourceRequest::new(
                ResourceKind::Ram,
                ResourceAmount::checked(8 * 1024 * 1024).unwrap(),
            )])
            .unwrap();
        assert!(point.report_current(&foreign.lease()).is_err());
        case.cancel.cancel();
        assert!(point.report_current(case.view).is_err());
        assert!(point
            .check_world_current(&observed, request.plan, request.candidate, case.view)
            .is_err());
        assert_eq!(point.historical_report().native_truth(), Truth::True);
        assert_eq!(observed.historical_report().truth(), Truth::True);
    });
}

#[test]
fn tlcx26_point_multistep_temporal_operator_and_work_caps_are_not_boolean_equivalence() {
    with_case(false, false, Truth::True, |case| {
        let request = case.request;
        assert!(matches!(
            evaluate_point(request, case.view, PointBudget::checked(1).unwrap()),
            Err(PointError::WorkBudget)
        ));
        let query = TemporalQuery::checked(
            request.native.query.context(),
            vec![TemporalNode::Atom(0), TemporalNode::Eventually(0)],
            1,
            TemporalProfile::ClosedFiniteKleeneV1,
        )
        .unwrap();
        let mut store = FiniteResultStore::with_resources(1, case.view).unwrap();
        let id = store
            .evaluate_temporal(
                &query,
                request.native.trace,
                query.context(),
                request.native.scope,
                EvaluationBudget::default(),
                case.view,
            )
            .unwrap();
        let temporal = PointRequest {
            native: NativePoint {
                store: &store,
                id,
                query: &query,
                trace: request.native.trace,
                scope: request.native.scope,
            },
            ..request
        };
        // Eventually(p) happens to agree at this one point, but the registered
        // fragment is Boolean-only, not arbitrary finite-trace equivalence.
        assert!(matches!(
            evaluate_point(temporal, case.view, PointBudget::checked(10_000).unwrap()),
            Err(PointError::Unsupported(UnsupportedPoint::TemporalOperator))
        ));
        let clock = request.native.trace.steps()[0].instant().clock();
        let trace = FiniteTrace::checked(
            vec![
                TraceStep::new(
                    Instant::absolute(clock, 0, TimeUnit::Second).unwrap(),
                    vec![Truth::True],
                ),
                TraceStep::new(
                    Instant::absolute(clock, 1, TimeUnit::Second).unwrap(),
                    vec![Truth::True],
                ),
            ],
            TraceClosure::Closed,
        )
        .unwrap();
        let mut multi_store = FiniteResultStore::with_resources(1, case.view).unwrap();
        let id = multi_store
            .evaluate_temporal(
                request.native.query,
                &trace,
                request.native.query.context(),
                request.native.scope,
                EvaluationBudget::default(),
                case.view,
            )
            .unwrap();
        let multiple = PointRequest {
            native: NativePoint {
                store: &multi_store,
                id,
                query: request.native.query,
                trace: &trace,
                scope: request.native.scope,
            },
            ..request
        };
        assert!(matches!(
            evaluate_point(multiple, case.view, PointBudget::checked(10_000).unwrap()),
            Err(PointError::Unsupported(UnsupportedPoint::MultipleSteps))
        ));
    });
}

#[test]
fn tlcx26_point_equal_content_different_actual_store_occurrence_is_not_the_owned_link() {
    with_case(false, false, Truth::True, |case| {
        let request = case.request;
        let point =
            evaluate_point(request, case.view, PointBudget::checked(10_000).unwrap()).unwrap();
        let prepared = PreparedTemporalWorld::with_resources(
            request.native.query,
            request.native.trace,
            case.view,
        )
        .unwrap();
        let mut worlds = MaterializedWorlds::new(
            case.graph,
            MaterializedLimits {
                max_live: 1,
                max_attempts: 1,
                max_payload_bytes: 1024,
                max_referenced_bytes: 4096,
            },
        )
        .unwrap();
        let key = worlds.admit(prepared.world_query()).unwrap().key();
        let mut other_store = FiniteResultStore::with_resources(1, case.view).unwrap();
        let id = other_store
            .evaluate_temporal(
                request.native.query,
                request.native.trace,
                request.native.query.context(),
                request.native.scope,
                EvaluationBudget::default(),
                case.view,
            )
            .unwrap();
        let observed = observe_temporal_world_current(
            &worlds,
            key,
            &prepared,
            &other_store,
            id,
            request.native.scope,
            case.view,
        )
        .unwrap();
        assert_eq!(
            observed.report_current(case.view).unwrap().truth(),
            Truth::True
        );
        assert!(matches!(
            point.check_world_current(&observed, request.plan, request.candidate, case.view),
            Err(PointError::ForeignObservation)
        ));
    });
}

#[test]
fn tlcx26_point_identical_declared_plan_or_candidate_cannot_replace_exact_owners() {
    with_case(false, false, Truth::True, |case| {
        let request = case.request;
        let point =
            evaluate_point(request, case.view, PointBudget::checked(10_000).unwrap()).unwrap();
        let prepared = PreparedTemporalWorld::with_resources(
            request.native.query,
            request.native.trace,
            case.view,
        )
        .unwrap();
        let mut worlds = MaterializedWorlds::new(
            case.graph,
            MaterializedLimits {
                max_live: 1,
                max_attempts: 1,
                max_payload_bytes: 1024,
                max_referenced_bytes: 4096,
            },
        )
        .unwrap();
        let key = worlds.admit(prepared.world_query()).unwrap().key();
        let observed = observe_temporal_world_current(
            &worlds,
            key,
            &prepared,
            request.native.store,
            request.native.id,
            request.native.scope,
            case.view,
        )
        .unwrap();
        let work = m::WorkBudget {
            max_work: 100_000,
            max_payload_bytes: 1_000_000,
        };
        let other_plan = m::ProbePlan::checked(
            request.plan.scope(),
            request.plan.fixed_probes()[0].template.source,
            request.plan.fixed_probes(),
            work,
        )
        .unwrap();
        assert!(matches!(
            point.check_world_current(&observed, &other_plan, request.candidate, case.view),
            Err(PointError::ForeignPlan)
        ));
        let other_candidate = m::Candidate::given(
            request.candidate.id(),
            request.plan.scope().comparison,
            request.candidate.target_query(),
            request.candidate.revision(),
            request.candidate.context(),
        )
        .unwrap();
        assert!(matches!(
            point.check_world_current(&observed, request.plan, &other_candidate, case.view),
            Err(PointError::ForeignCandidate)
        ));
    });
}
