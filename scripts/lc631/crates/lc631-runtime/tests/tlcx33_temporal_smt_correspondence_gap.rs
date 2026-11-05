//! 日本語: 同source/contextでも独立したtraceとSMT理論は内容対応を証明しない。
//! English: Equal source/context IDs do not connect independent traces and SMT content.
//! This test requires the compiled semantic-gpu feature but intentionally takes
//! the unsupported-grid fresh CPU path. No GPU or external SMT process is run.
#![cfg(feature = "semantic-gpu")]
#[path = "tlcx33_world_support/mod.rs"]
mod support;
use lc631_accelerator::{
    semantic_pareto::{GridAxis, GridDirection, ScopedSemanticGpuBudget},
    AcceleratorBroker, CubeClWgpuBackend,
};
use lc631_core::measurement_numeric::RationalInterval as Interval;
use lc631_core::{context::*, AuthorityRevision, CandidateId};
use lc631_runtime::{
    finite_results::FiniteResultStore,
    formal_boolean as f,
    formal_measurement::{self as m, owned, produced},
    joint_selection::{formal::reduce_formal_owned, *},
    materialized_world_bridge::*,
    semantic_accelerator::selection::{tiled::world::*, *},
};
use lc631_solvers::{
    protocol::sexpr::{self, SExprLimits},
    smt::{validate_model, ModelValidationError},
    temporal::*,
    typed_query::{BoolExpr, BoolSymbolId, SmtEmitLimits, SmtStatusQuery},
};
use lc631_substrate::resource::Cancellation;
use lc631_world::{context_graph::ContextGraph, materialized_worlds::*};
use std::sync::Arc;
use support::*;

fn incompatible_instance(theory_negated: bool) {
    let source =
        SourceRevision::of("Given same identity; no registered Temporal-to-SMT content bridge");
    let scope = ContextScope::declared(d(1), d(20), d(3), d(21));
    let graph = ContextGraph::new(
        OriginalContext::checked(
            source,
            scope,
            AuthorityRevision(1),
            vec![ContextPremise::reported(
                d(80),
                Arc::from("separate Given interpretations"),
                PremiseBasis::Given,
                false,
            )],
            ContextLimits {
                max_contexts: 2,
                max_premises: 4,
                max_edits: 4,
                max_statement_bytes: 1024,
            },
        )
        .unwrap(),
        2,
    )
    .unwrap();
    let context = graph.states()[0].revision();
    let symbol = BoolSymbolId::new(0);
    let predicate = BoolExpr::Variable(symbol);
    let assertion = if theory_negated {
        BoolExpr::Not(Box::new(predicate.clone()))
    } else {
        predicate.clone()
    };
    let formal_source = SmtStatusQuery::checked(
        vec![symbol],
        vec![],
        vec![assertion.clone()],
        SmtEmitLimits::default(),
    )
    .unwrap();
    let formal_target = SmtStatusQuery::checked(
        vec![symbol],
        vec![],
        vec![assertion],
        SmtEmitLimits::default(),
    )
    .unwrap();
    let symbols = [(symbol, symbol)];
    let mut request = f::Request {
        source: &formal_source,
        target: &formal_target,
        source_predicate: &predicate,
        target_predicate: &predicate,
        symbols: &symbols,
        unconstrained: f::UnconstrainedPolicy::ExplicitlyGiven,
        binding: f::Binding {
            source,
            target: source,
            source_context: context,
            target_context: context,
            probe_set: d(3),
            probe_map: d(0),
        },
    };
    request.binding.probe_map = f::probe_map_digest(&request, 100_000).unwrap();
    let one = Interval::exact_ratio(1, 1).unwrap();
    let fixed = [m::FixedProbe {
        identity: b"protected-p",
        weight: one.lower(),
        template: &request,
    }];
    let plan = m::ProbePlan::checked(
        m::PlanScope {
            comparison: d(1),
            definition: d(2),
            probe_set: d(3),
            source,
            context,
        },
        &formal_source,
        &fixed,
        work(),
    )
    .unwrap();
    let candidate = m::Candidate::given(d(4), d(1), &formal_target, source, context).unwrap();
    let broker = broker();
    let run = run(&broker, source.digest(), Cancellation::default());
    let reserved = reserve(&run, 48 * 1024 * 1024, 8 * 1024 * 1024);
    let view = reserved.lease();
    // Actual private formal production: identical consistent theories preserve
    // their protected p outcome. These are not supplied pass/fail reports.
    let production = produced::execute(
        &plan,
        &candidate,
        &[produced::ScheduledProbe {
            identity: b"protected-p",
            request: &request,
            max_work: 10_000,
        }],
        work(),
        &view,
    )
    .unwrap();
    let observations = production.observations_current(&view).unwrap();
    let sound = owned::measure_owned(
        &plan,
        &candidate,
        m::Direction::Soundness,
        observations.as_slice(),
        work(),
        &view,
    )
    .unwrap();
    let complete = owned::measure_owned(
        &plan,
        &candidate,
        m::Direction::Completeness,
        observations.as_slice(),
        work(),
        &view,
    )
    .unwrap();
    let reduction = policy(pins(&plan), one.lower(), one.lower(), &view);
    let pins = reduction.pins();
    let semantic =
        reduce_formal_owned(&reduction, &sound, &complete, &plan, &candidate, &view).unwrap();
    let loss = match semantic
        .project_current(&reduction, pins, &plan, &candidate, &view)
        .unwrap()
        .1
    {
        AxisValue::Exact(loss) => loss,
        _ => panic!("actual exact formal identity loss required"),
    };
    assert_eq!(loss, &Interval::exact_ratio(0, 1).unwrap());

    // Independent handwritten conflict: theory ¬p versus trace p=true, and
    // theory p versus trace p=false. Same ID0 is NOT a semantic correspondence.
    let temporal_truth = if theory_negated {
        Truth::True
    } else {
        Truth::False
    };
    let temporal_query = TemporalQuery::checked(
        context,
        vec![TemporalNode::Atom(0)],
        0,
        TemporalProfile::ClosedFiniteKleeneV1,
    )
    .unwrap();
    let clock = ClockSpec::exact(d(70), TimeScale::Monotonic, d(71), d(72));
    let trace = FiniteTrace::checked(
        vec![TraceStep::new(
            Instant::absolute(clock, 0, TimeUnit::Second).unwrap(),
            vec![temporal_truth],
        )],
        TraceClosure::Closed,
    )
    .unwrap();
    let conflicting_model = sexpr::parse(
        if theory_negated {
            b"((define-fun b0 () Bool true))"
        } else {
            b"((define-fun b0 () Bool false))"
        },
        SExprLimits::default(),
    )
    .unwrap();
    assert_eq!(
        validate_model(&formal_target, &conflicting_model).err(),
        Some(ModelValidationError::AssertionFalse { index: 0 })
    );
    let feasible_model = sexpr::parse(
        if theory_negated {
            b"((define-fun b0 () Bool false))"
        } else {
            b"((define-fun b0 () Bool true))"
        },
        SExprLimits::default(),
    )
    .unwrap();
    assert_eq!(
        validate_model(&formal_target, &feasible_model)
            .unwrap()
            .boolean(symbol),
        Some(!theory_negated)
    );
    // Local validate_model is not a native SMT process receipt or source truth.
    let prepared = PreparedTemporalWorld::with_resources(&temporal_query, &trace, &view).unwrap();
    let mut worlds = MaterializedWorlds::new(
        &graph,
        MaterializedLimits {
            max_live: 1,
            max_attempts: 1,
            max_payload_bytes: 1024,
            max_referenced_bytes: 4096,
        },
    )
    .unwrap();
    let key = worlds.admit(prepared.world_query()).unwrap().key();
    let mut store = FiniteResultStore::with_resources(1, &view).unwrap();
    let result = store
        .evaluate_temporal(
            &temporal_query,
            &trace,
            context,
            scope,
            EvaluationBudget::default(),
            &view,
        )
        .unwrap();
    let native = store
        .lookup_temporal_current(result, &temporal_query, &trace, context, scope, &view)
        .unwrap();
    assert_eq!(native.truth(), temporal_truth);
    let goal = Interval::exact_ratio(if native.truth() == Truth::True { 1 } else { 0 }, 1).unwrap();
    let axes = [AxisKind::Goal(d(40)), AxisKind::SemanticLoss];
    let policy = SelectionPolicy {
        pins,
        axes: &axes,
        representative_order: None,
        declared_goal_solve_overlap: false,
        max_candidates: 1,
        max_canonical_bytes: 8192,
    };
    let values = [AxisValue::Exact(&goal), AxisValue::Exact(loss)];
    let rows = [JointCandidate {
        id: CandidateId(1),
        context,
        pins,
        hard_gate: ReportedGate::Pass,
        map_back: ReportedGate::Pass,
        axes: &values,
        total_loss: AxisValue::Exact(loss),
        canonical_key: prepared.world_query().query_bytes(),
        measurement_binding: d(55),
    }];
    let descriptions = [WorldRow {
        key,
        prepared: &prepared,
        result,
        semantic: &semantic,
        reduction: &reduction,
        plan: &plan,
        candidate: &candidate,
        expected_measurement_binding: d(55),
    }];
    let inputs = WorldInputSet {
        worlds: &worlds,
        store: &store,
        rows: &descriptions,
        goal: DeclaredTemporalGoal {
            metric: d(40),
            profile: pins.goal_profile.unwrap(),
            scope,
            map: TruthGoalMap::TrueOneFalseZeroV1,
            total: TotalLossProfile::SingleSemanticAxisTotal,
        },
        budget: WorldWorkBudget {
            max_rows: 1,
            max_payload_bytes: 8192,
            max_pairs: 1,
            max_comparison_bytes: 8192,
        },
    };
    let batch = prepare_numeric_batch(
        &policy,
        &rows,
        SearchCoverage::EvaluatedSet,
        EpochStamp {
            epoch: 1,
            generation: 1,
            fence: 1,
        },
        &view,
        2,
    )
    .unwrap();
    // Current owner/binding checks succeed independently; they do not establish
    // that the Temporal trace is a model of this unrelated formal theory.
    assert_eq!(
        inputs
            .check_rows_current(&batch, &view)
            .unwrap()
            .evaluated_rows,
        1
    );
    let unsupported_grid = [
        GridAxis {
            denominator: 1,
            minimum: 0,
            maximum: 1,
            direction: GridDirection::Minimize,
        },
        GridAxis {
            denominator: 1,
            minimum: 0,
            maximum: 1,
            direction: GridDirection::Minimize,
        },
    ];
    let mut accelerator = AcceleratorBroker::new(CubeClWgpuBackend::profiled());
    let outcome = run_world_tiled(
        &inputs,
        &batch,
        &unsupported_grid,
        &mut accelerator,
        ScopedSemanticGpuBudget {
            max_buffers: 4,
            max_requested_bytes: 128 * 1024,
            max_inflight_requested_bytes: 128 * 1024,
            max_buffer_bytes: 64 * 1024,
        },
        &view,
    )
    .unwrap();
    let projected = outcome
        .project_current(&batch, &accelerator, &view)
        .unwrap();
    assert_eq!(projected.numeric_frontier(), [0]);
    assert_eq!(projected.actual_ordered_gpu_words(), None);
    assert_eq!(projected.world_recommendation(), None);
    assert_eq!(
        projected.correspondence(),
        WorldCorrespondence::NeedsEvidence
    );
    assert_eq!(outcome.source_meaning(), SourceMeaning::NeedsEvidence);
}

#[test]
fn tlcx33_unrelated_negated_smt_theory_and_native_true_trace_never_recommend_world() {
    incompatible_instance(true);
}
#[test]
fn tlcx33_unrelated_positive_smt_theory_and_native_false_trace_never_recommend_world() {
    incompatible_instance(false);
}
