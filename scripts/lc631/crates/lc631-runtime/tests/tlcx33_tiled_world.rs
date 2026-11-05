#![cfg(feature = "semantic-gpu")]
#[path = "tlcx33_world_support/mod.rs"]
mod support;
use lc631_accelerator::semantic_pareto::{GridAxis, GridDirection, ScopedSemanticGpuBudget};
use lc631_accelerator::{AcceleratorBroker, CubeClWgpuBackend};
use lc631_core::measurement_numeric::RationalInterval as Interval;
use lc631_core::{context::*, AuthorityRevision, CandidateId};
use lc631_runtime::{
    finite_results::FiniteResultStore,
    formal_measurement::{self as m, owned},
    joint_selection::formal::reduce_formal_owned,
    joint_selection::*,
    materialized_world_bridge::*,
    semantic_accelerator::selection::{tiled::world::*, *},
};
use lc631_solvers::temporal::*;
use lc631_substrate::resource::Cancellation;
use lc631_world::{context_graph::ContextGraph, materialized_worlds::*};
use std::sync::Arc;
use support::*;
fn gpu_budget() -> ScopedSemanticGpuBudget {
    ScopedSemanticGpuBudget {
        max_buffers: 4,
        max_requested_bytes: 128 * 1024,
        max_inflight_requested_bytes: 128 * 1024,
        max_buffer_bytes: 64 * 1024,
    }
}
fn scenario(n: usize, hardware: bool) {
    let source = SourceRevision::of("given source interpreted into two target theories");
    let scope = ContextScope::declared(d(1), d(20), d(3), d(21));
    let graph = ContextGraph::new(
        OriginalContext::checked(
            source,
            scope,
            AuthorityRevision(1),
            vec![ContextPremise::reported(
                d(80),
                Arc::from("explicit given finite model"),
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
    cases_in_context(graph.states()[0].revision(), |c| {
        let b = broker();
        let cancellation = Cancellation::default();
        let run = run(&b, c.plan.scope().source.digest(), cancellation.clone());
        let reservation = reserve(&run, 48 * 1024 * 1024, 8 * 1024 * 1024);
        let view = reservation.lease();
        let sound = owned::measure_owned(
            c.plan,
            c.good,
            m::Direction::Soundness,
            c.good_obs,
            work(),
            &view,
        )
        .unwrap();
        let complete = owned::measure_owned(
            c.plan,
            c.good,
            m::Direction::Completeness,
            c.good_obs,
            work(),
            &view,
        )
        .unwrap();
        let one = Interval::exact_ratio(1, 1).unwrap();
        let zero = Interval::exact_ratio(0, 1).unwrap();
        let reduction = policy(pins(c.plan), one.lower(), one.lower(), &view);
        let p = reduction.pins();
        let semantic =
            reduce_formal_owned(&reduction, &sound, &complete, c.plan, c.good, &view).unwrap();
        let loss = match semantic
            .project_current(&reduction, p, c.plan, c.good, &view)
            .unwrap()
            .1
        {
            AxisValue::Exact(v) => v,
            _ => panic!("exact owned semantic loss required"),
        };
        let context = graph.states()[0].revision();
        assert_eq!(context, c.good.context());
        let query = TemporalQuery::checked(
            context,
            vec![TemporalNode::Atom(0)],
            0,
            TemporalProfile::ClosedFiniteKleeneV1,
        )
        .unwrap();
        let clock = ClockSpec::exact(d(70), TimeScale::Monotonic, d(71), d(72));
        let traces: Vec<_> = (0..n)
            .map(|i| {
                FiniteTrace::checked(
                    vec![TraceStep::new(
                        Instant::absolute(clock, i as i64, TimeUnit::Second).unwrap(),
                        vec![if i % 2 == 0 {
                            Truth::True
                        } else {
                            Truth::False
                        }],
                    )],
                    TraceClosure::Closed,
                )
                .unwrap()
            })
            .collect();
        let prepared: Vec<_> = traces
            .iter()
            .map(|trace| PreparedTemporalWorld::with_resources(&query, trace, &view).unwrap())
            .collect();
        let mut worlds = MaterializedWorlds::new(
            &graph,
            MaterializedLimits {
                max_live: n,
                max_attempts: n as u64,
                max_payload_bytes: 1024,
                max_referenced_bytes: 1024 * n,
            },
        )
        .unwrap();
        let keys: Vec<_> = prepared
            .iter()
            .map(|v| worlds.admit(v.world_query()).unwrap().key())
            .collect();
        let mut store = FiniteResultStore::with_resources(n, &view).unwrap();
        let ids: Vec<_> = traces
            .iter()
            .map(|trace| {
                store
                    .evaluate_temporal(
                        &query,
                        trace,
                        context,
                        p.scope,
                        EvaluationBudget::default(),
                        &view,
                    )
                    .unwrap()
            })
            .collect();
        let axes = [AxisKind::Goal(d(40)), AxisKind::SemanticLoss];
        let policy = SelectionPolicy {
            pins: p,
            axes: &axes,
            representative_order: None,
            declared_goal_solve_overlap: false,
            max_candidates: n,
            max_canonical_bytes: 1024 * 1024,
        };
        let values: Vec<_> = (0..n)
            .map(|i| {
                [
                    AxisValue::Exact(if i % 2 == 0 { &one } else { &zero }),
                    AxisValue::Exact(loss),
                ]
            })
            .collect();
        let rows: Vec<_> = (0..n)
            .map(|i| JointCandidate {
                id: CandidateId(i as u64 + 1),
                context,
                pins: p,
                hard_gate: ReportedGate::Pass,
                map_back: ReportedGate::Pass,
                axes: &values[i],
                total_loss: AxisValue::Exact(loss),
                canonical_key: prepared[i].world_query().query_bytes(),
                measurement_binding: d(55),
            })
            .collect();
        let world_rows: Vec<_> = (0..n)
            .map(|i| WorldRow {
                key: keys[i],
                prepared: &prepared[i],
                result: ids[i],
                semantic: &semantic,
                reduction: &reduction,
                plan: c.plan,
                candidate: c.good,
                expected_measurement_binding: d(55),
            })
            .collect();
        let inputs = WorldInputSet {
            worlds: &worlds,
            store: &store,
            rows: &world_rows,
            goal: DeclaredTemporalGoal {
                metric: d(40),
                profile: p.goal_profile.unwrap(),
                scope: p.scope,
                map: TruthGoalMap::TrueOneFalseZeroV1,
                total: TotalLossProfile::SingleSemanticAxisTotal,
            },
            budget: WorldWorkBudget {
                max_rows: n,
                max_payload_bytes: 1024 * 1024,
                max_pairs: 2048 * 2048,
                max_comparison_bytes: 1024 * 1024 * 1024,
            },
        };
        let stamp = EpochStamp {
            epoch: 1,
            generation: 1,
            fence: 1,
        };
        let comparisons = n * n * 2;
        let batch = if hardware {
            // The same actual rows must still fail the legacy accounting cap.
            assert!(matches!(
                prepare_numeric_batch(
                    &policy,
                    &rows,
                    SearchCoverage::EvaluatedSet,
                    stamp,
                    &view,
                    comparisons,
                ),
                Err(ConsumerError::PayloadLimit)
            ));
            prepare_numeric_batch_with_profile(
                &policy,
                &rows,
                SearchCoverage::EvaluatedSet,
                stamp,
                &view,
                comparisons,
                NumericPayloadProfile::TiledWorld2048V1,
            )
            .unwrap()
        } else {
            prepare_numeric_batch(
                &policy,
                &rows,
                SearchCoverage::EvaluatedSet,
                stamp,
                &view,
                comparisons,
            )
            .unwrap()
        };
        let count = inputs.check_rows_current(&batch, &view).unwrap();
        assert_eq!(count.evaluated_rows, n);
        assert_eq!(count.unique_full_inputs, n);
        assert_eq!(count.logical_distinct_unconfirmed, n);
        // Equal copied endpoints cannot stand in for the private owned formal axis.
        let copy = Interval::exact_ratio(0, 1).unwrap();
        let copied = [AxisValue::Exact(&one), AxisValue::Exact(&copy)];
        let bad = JointCandidate {
            id: CandidateId(1),
            context,
            pins: p,
            hard_gate: ReportedGate::Pass,
            map_back: ReportedGate::Pass,
            axes: &copied,
            total_loss: AxisValue::Exact(&copy),
            canonical_key: rows[0].canonical_key,
            measurement_binding: d(55),
        };
        if n == 2 {
            let swapped = [
                WorldRow {
                    key: keys[1],
                    prepared: &prepared[1],
                    result: ids[1],
                    semantic: &semantic,
                    reduction: &reduction,
                    plan: c.plan,
                    candidate: c.good,
                    expected_measurement_binding: d(55),
                },
                WorldRow {
                    key: keys[0],
                    prepared: &prepared[0],
                    result: ids[0],
                    semantic: &semantic,
                    reduction: &reduction,
                    plan: c.plan,
                    candidate: c.good,
                    expected_measurement_binding: d(55),
                },
            ];
            let altered = WorldInputSet {
                worlds: &worlds,
                store: &store,
                rows: &swapped,
                goal: inputs.goal,
                budget: inputs.budget,
            };
            assert!(altered.check_rows_current(&batch, &view).is_err());
            let foreign = reserve(&run, 8 * 1024 * 1024, 1024 * 1024);
            assert!(inputs.check_rows_current(&batch, &foreign.lease()).is_err());
            let badrows = [bad];
            let badpolicy = SelectionPolicy {
                max_candidates: 1,
                ..policy
            };
            let badbatch = prepare_numeric_batch(
                &badpolicy,
                &badrows,
                SearchCoverage::EvaluatedSet,
                stamp,
                &view,
                2,
            )
            .unwrap();
            let single = WorldInputSet {
                worlds: &worlds,
                store: &store,
                rows: &world_rows[..1],
                goal: inputs.goal,
                budget: inputs.budget,
            };
            assert!(matches!(
                single.check_rows_current(&badbatch, &view),
                Err(WorldRunError::Formal(_))
            ));
        }
        if !hardware {
            // Both producers are freshly executed and genuinely owned, but no
            // registered Temporal->formal content bridge connects them.
            // An intentionally unsupported direction forces fresh CPU numeric
            // work without native effects. It must still NEVER recommend a world.
            let grid = [
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
            let mut native = AcceleratorBroker::new(CubeClWgpuBackend::profiled());
            let outcome =
                run_world_tiled(&inputs, &batch, &grid, &mut native, gpu_budget(), &view).unwrap();
            let projection = outcome.project_current(&batch, &native, &view).unwrap();
            assert_eq!(projection.world_recommendation(), None);
            assert_eq!(
                projection.correspondence(),
                WorldCorrespondence::NeedsEvidence
            );
            assert_eq!(projection.numeric_frontier(), [0]);
            assert_eq!(projection.actual_ordered_gpu_words(), None);
        }
        if hardware {
            assert_eq!(
                std::env::var("EPISTESYS_TEST_SEMANTIC_GPU").as_deref(),
                Ok("1")
            );
            let grid = [
                GridAxis {
                    denominator: 1,
                    minimum: 0,
                    maximum: 1,
                    direction: GridDirection::Maximize,
                },
                GridAxis {
                    denominator: 1,
                    minimum: 0,
                    maximum: 1,
                    direction: GridDirection::Minimize,
                },
            ];
            let mut native = AcceleratorBroker::new(CubeClWgpuBackend::profiled());
            let result =
                run_world_tiled(&inputs, &batch, &grid, &mut native, gpu_budget(), &view).unwrap();
            let projected = result.project_current(&batch, &native, &view).unwrap();
            assert_eq!(projected.world_recommendation(), None);
            assert_eq!(
                projected.correspondence(),
                WorldCorrespondence::NeedsEvidence
            );
            assert_eq!(
                result.eligibility_basis(),
                EligibilityBasis::ReportedConditional
            );
            assert_eq!(result.source_meaning(), SourceMeaning::NeedsEvidence);
            let full = projected
                .actual_ordered_gpu_words()
                .expect("actual native world numeric frontier, no fallback");
            assert_eq!(full.len(), n);
            for (i, v) in full.iter().enumerate() {
                assert_eq!(*v, u32::from(i % 2 == 0));
            }
            assert_eq!(projected.numeric_frontier().len(), n / 2);
            eprintln!("actual world numeric rows={} unique_inputs={} logical_distinct_unconfirmed={} world_recommendation=None correspondence=NeedsEvidence",n,count.unique_full_inputs,count.logical_distinct_unconfirmed);
            // A completed native observation remains historical after the run
            // is cancelled, while its current world projection is rejected.
            cancellation.cancel();
            assert_eq!(result.historical_gpu_words(), Some(full));
            assert!(result.project_current(&batch, &native, &view).is_err());
        }
    });
}
#[test]
fn tlcx33_world_actual_current_owner_bijection_copy_and_foreign_cuts() {
    scenario(2, false);
}
#[test]
#[ignore = "explicit bounded actual world producer+GPU2048 optin"]
fn tlcx33_world_actual_2048_producer_gpu_cpu_conditional_selection() {
    scenario(2048, true);
}
