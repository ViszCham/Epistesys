//! 日本語: point ownerを限定production portで消費しても世界推薦へ昇格しない。
//! English: Consuming point owners never promotes them to a world recommendation.
#![cfg(feature = "semantic-gpu")]
#[path = "tlcx33_world_support/mod.rs"]
mod support;
use lc631_accelerator::{
    semantic_pareto::{GridAxis, GridDirection, ScopedSemanticGpuBudget},
    AcceleratorBroker, CubeClWgpuBackend,
};
use lc631_core::{
    context::*, measurement_numeric::RationalInterval as Interval, AuthorityRevision, CandidateId,
};
use lc631_runtime::{
    finite_results::FiniteResultStore,
    formal_measurement::{self as m, owned},
    joint_selection::{formal::reduce_formal_owned, *},
    materialized_world_bridge::*,
    semantic_accelerator::selection::{tiled::world::*, *},
    temporal_smt_point::{world_port::*, *},
};
use lc631_solvers::{temporal::*, typed_query::BoolSymbolId};
use lc631_substrate::{
    resource::Cancellation,
    source::{SourceSpan, SourceStore},
};
use lc631_world::{context_graph::ContextGraph, materialized_worlds::*};
use std::sync::Arc;
use support::*;

fn budget() -> ScopedSemanticGpuBudget {
    ScopedSemanticGpuBudget {
        max_buffers: 4,
        max_requested_bytes: 128 * 1024,
        max_inflight_requested_bytes: 128 * 1024,
        max_buffer_bytes: 64 * 1024,
    }
}
fn scenario(hardware: bool, consume: bool, reverse: bool) {
    let source = SourceStore::new(
        Arc::from("given source interpreted into two target theories"),
        1024,
    )
    .unwrap();
    let revision = SourceRevision::of(source.text());
    let scope = ContextScope::declared(d(1), d(20), d(3), d(21));
    let graph = ContextGraph::new(
        OriginalContext::checked(
            revision,
            scope,
            AuthorityRevision(1),
            vec![ContextPremise::reported(
                d(80),
                Arc::from("Given finite point interpretations"),
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
        let cancel = Cancellation::default();
        let owner = broker();
        let run = run(&owner, revision.digest(), cancel.clone());
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
        let pins = reduction.pins();
        let semantic =
            reduce_formal_owned(&reduction, &sound, &complete, c.plan, c.good, &view).unwrap();
        let loss = match semantic
            .project_current(&reduction, pins, c.plan, c.good, &view)
            .unwrap()
            .1
        {
            AxisValue::Exact(v) => v,
            _ => panic!("exact actual formal axis required"),
        };
        let context = graph.states()[0].revision();
        let query = TemporalQuery::checked(
            context,
            vec![TemporalNode::Atom(0)],
            0,
            TemporalProfile::ClosedFiniteKleeneV1,
        )
        .unwrap();
        let clock = ClockSpec::exact(d(70), TimeScale::Monotonic, d(71), d(72));
        let truths = if reverse {
            [Truth::False, Truth::True]
        } else {
            [Truth::True, Truth::False]
        };
        let traces = truths.map(|truth| {
            FiniteTrace::checked(
                vec![TraceStep::new(
                    Instant::absolute(clock, 0, TimeUnit::Second).unwrap(),
                    vec![truth],
                )],
                TraceClosure::Closed,
            )
            .unwrap()
        });
        let prepared = [
            PreparedTemporalWorld::with_resources(&query, &traces[0], &view).unwrap(),
            PreparedTemporalWorld::with_resources(&query, &traces[1], &view).unwrap(),
        ];
        let mut worlds = MaterializedWorlds::new(
            &graph,
            MaterializedLimits {
                max_live: 2,
                max_attempts: 2,
                max_payload_bytes: 1024,
                max_referenced_bytes: 8192,
            },
        )
        .unwrap();
        let keys = [
            worlds.admit(prepared[0].world_query()).unwrap().key(),
            worlds.admit(prepared[1].world_query()).unwrap().key(),
        ];
        let mut store = FiniteResultStore::with_resources(2, &view).unwrap();
        let ids = [
            store
                .evaluate_temporal(
                    &query,
                    &traces[0],
                    context,
                    scope,
                    EvaluationBudget::default(),
                    &view,
                )
                .unwrap(),
            store
                .evaluate_temporal(
                    &query,
                    &traces[1],
                    context,
                    scope,
                    EvaluationBudget::default(),
                    &view,
                )
                .unwrap(),
        ];
        let map = [AtomPair {
            temporal: 0,
            formal: BoolSymbolId::new(0),
        }];
        let span = SourceSpan::checked(&source, 0, source.text().len()).unwrap();
        let point0 = evaluate_point(
            PointRequest {
                source: &source,
                span,
                plan: c.plan,
                candidate: c.good,
                probe: 0,
                native: NativePoint {
                    store: &store,
                    id: ids[0],
                    query: &query,
                    trace: &traces[0],
                    scope,
                },
                map: &map,
            },
            &view,
            PointBudget::checked(10_000).unwrap(),
        )
        .unwrap();
        let point1 = evaluate_point(
            PointRequest {
                source: &source,
                span,
                plan: c.plan,
                candidate: c.good,
                probe: 0,
                native: NativePoint {
                    store: &store,
                    id: ids[1],
                    query: &query,
                    trace: &traces[1],
                    scope,
                },
                map: &map,
            },
            &view,
            PointBudget::checked(10_000).unwrap(),
        )
        .unwrap();
        let kinds = [AxisKind::Goal(d(40)), AxisKind::SemanticLoss];
        let policy = SelectionPolicy {
            pins,
            axes: &kinds,
            representative_order: None,
            declared_goal_solve_overlap: false,
            max_candidates: 2,
            max_canonical_bytes: 8192,
        };
        let values = truths.map(|truth| {
            [
                AxisValue::Exact(if truth == Truth::True { &one } else { &zero }),
                AxisValue::Exact(loss),
            ]
        });
        let rows = [
            JointCandidate {
                id: CandidateId(1),
                context,
                pins,
                hard_gate: ReportedGate::Pass,
                map_back: ReportedGate::Pass,
                axes: &values[0],
                total_loss: AxisValue::Exact(loss),
                canonical_key: prepared[0].world_query().query_bytes(),
                measurement_binding: d(55),
            },
            JointCandidate {
                id: CandidateId(2),
                context,
                pins,
                hard_gate: ReportedGate::Pass,
                map_back: ReportedGate::Pass,
                axes: &values[1],
                total_loss: AxisValue::Exact(loss),
                canonical_key: prepared[1].world_query().query_bytes(),
                measurement_binding: d(55),
            },
        ];
        let descriptors = [
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
        ];
        let inputs = WorldInputSet {
            worlds: &worlds,
            store: &store,
            rows: &descriptors,
            goal: DeclaredTemporalGoal {
                metric: d(40),
                profile: pins.goal_profile.unwrap(),
                scope,
                map: TruthGoalMap::TrueOneFalseZeroV1,
                total: TotalLossProfile::SingleSemanticAxisTotal,
            },
            budget: WorldWorkBudget {
                max_rows: 2,
                max_payload_bytes: 8192,
                max_pairs: 4,
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
            8,
        )
        .unwrap();
        let points = [
            PointWorldRow {
                row: 0,
                owner: &point0,
            },
            PointWorldRow {
                row: 1,
                owner: &point1,
            },
        ];
        let checked = PointWorldInputs {
            inputs: &inputs,
            points: &points,
        };
        let grid = [
            GridAxis {
                denominator: 1,
                minimum: 0,
                maximum: 1,
                direction: if hardware {
                    GridDirection::Maximize
                } else {
                    GridDirection::Minimize
                },
            },
            GridAxis {
                denominator: 1,
                minimum: 0,
                maximum: 1,
                direction: GridDirection::Minimize,
            },
        ];
        let mut native = AcceleratorBroker::new(CubeClWgpuBackend::profiled());
        if !hardware {
            let missing = PointWorldInputs {
                inputs: &inputs,
                points: &points[..1],
            };
            assert!(
                run_point_world_tiled(&missing, &batch, &grid, &mut native, budget(), &view)
                    .is_err()
            );
            let duplicate = [
                PointWorldRow {
                    row: 0,
                    owner: &point0,
                },
                PointWorldRow {
                    row: 0,
                    owner: &point1,
                },
            ];
            let duplicated = PointWorldInputs {
                inputs: &inputs,
                points: &duplicate,
            };
            assert!(run_point_world_tiled(
                &duplicated,
                &batch,
                &grid,
                &mut native,
                budget(),
                &view
            )
            .is_err());
            let disguised = [
                PointWorldRow {
                    row: 0,
                    owner: &point0,
                },
                PointWorldRow {
                    row: 1,
                    owner: &point0,
                },
            ];
            let masquerade = PointWorldInputs {
                inputs: &inputs,
                points: &disguised,
            };
            assert!(run_point_world_tiled(
                &masquerade,
                &batch,
                &grid,
                &mut native,
                budget(),
                &view
            )
            .is_err());
            if consume {
                use lc631_runtime::semantic_accelerator::selection::given_points::*;
                assert!(select_given_points_current(
                    &missing,
                    &batch,
                    &grid,
                    &mut native,
                    budget(),
                    &view,
                )
                .is_err());
                assert!(select_given_points_current(
                    &duplicated,
                    &batch,
                    &grid,
                    &mut native,
                    budget(),
                    &view,
                )
                .is_err());
                assert!(select_given_points_current(
                    &masquerade,
                    &batch,
                    &grid,
                    &mut native,
                    budget(),
                    &view,
                )
                .is_err());
            }
        } else {
            assert_eq!(
                std::env::var("EPISTESYS_TEST_SEMANTIC_GPU").as_deref(),
                Ok("1")
            );
        }
        // Restoration consumes actual private point owners and the existing
        // world production path, while preserving the public None boundary.
        let outcome =
            run_point_world_tiled(&checked, &batch, &grid, &mut native, budget(), &view).unwrap();
        let projected = outcome.project_current(&batch, &native, &view).unwrap();
        assert_eq!(projected.checked_point_instances(), 2);
        assert_eq!(
            projected.point_scope(),
            PointPortScope::ProvidedGivenInstancesOnly
        );
        let winner = usize::from(reverse);
        assert_eq!(projected.numeric_frontier(), [winner]);
        assert_eq!(projected.world_recommendation(), None);
        assert_eq!(
            projected.correspondence(),
            WorldCorrespondence::NeedsEvidence
        );
        assert_eq!(
            projected.eligibility_basis(),
            EligibilityBasis::ReportedConditional
        );
        if hardware {
            assert_eq!(
                projected.actual_ordered_gpu_words().unwrap(),
                if reverse { [0, 1] } else { [1, 0] }
            );
        } else {
            assert_eq!(projected.actual_ordered_gpu_words(), None);
        }
        let consumed = if consume {
            use lc631_runtime::semantic_accelerator::selection::given_points::*;
            let selected =
                select_given_points_current(&checked, &batch, &grid, &mut native, budget(), &view)
                    .unwrap();
            let report = selected.project_current(&batch, &native, &view).unwrap();
            assert_eq!(report.provided_point_recommendation(), Some(winner));
            assert_eq!(
                report.eligibility_basis(),
                EligibilityBasis::ReportedConditional
            );
            assert_eq!(report.world_recommendation(), None);
            assert_eq!(report.numeric_frontier(), [winner]);
            assert_eq!(report.checked_point_instances(), 2);
            if hardware {
                assert_eq!(
                    report.actual_ordered_gpu_words().unwrap(),
                    if reverse { [0, 1] } else { [1, 0] }
                );
            } else {
                assert_eq!(report.actual_ordered_gpu_words(), None);
            }
            Some(selected)
        } else {
            None
        };
        let before = outcome.historical_gpu_words().map(|v| v.to_vec());
        let consumed_words = consumed
            .as_ref()
            .and_then(|v| v.historical_gpu_words())
            .map(|v| v.to_vec());
        cancel.cancel();
        if let Some(selected) = consumed.as_ref() {
            assert!(selected.project_current(&batch, &native, &view).is_err());
            assert_eq!(selected.historical_gpu_words(), consumed_words.as_deref());
        }
        assert!(outcome.project_current(&batch, &native, &view).is_err());
        assert_eq!(outcome.historical_checked_point_instances(), 2);
        assert_eq!(outcome.historical_gpu_words(), before.as_deref());
        assert_eq!(point0.historical_report().native_truth(), truths[0]);
        assert_eq!(point1.historical_report().native_truth(), truths[1]);
    });
}
#[test]
fn tlcx33_point_port_cut_restore_masquerade_duplicates_and_current_cancel_keep_no_recommendation() {
    scenario(false, false, false);
}
#[test]
#[ignore = "explicit real GPU point-port observation; post-completion cancellation, not a mid-kernel timing claim"]
fn tlcx33_point_port_actual_gpu_words_remain_historical_after_cancellation() {
    scenario(true, false, false);
}

#[test]
fn tlcx33_given_point_consumer_actual_observation_reversal_changes_conditional_selection() {
    scenario(false, true, false);
    scenario(false, true, true);
}

#[test]
#[ignore = "explicit actual GPU -> CPU final -> scoped conditional point selection"]
fn tlcx33_given_point_consumer_actual_gpu_to_cpu_selection_and_cancel() {
    scenario(true, true, false);
}
