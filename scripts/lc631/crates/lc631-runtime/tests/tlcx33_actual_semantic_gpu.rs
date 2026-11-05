#![cfg(feature = "semantic-gpu")]
#[path = "tlcx23_formal_support/mod.rs"]
mod support;
use lc631_accelerator::semantic_pareto::*;
use lc631_accelerator::{AcceleratorBroker, CubeClWgpuBackend, FaultKind, NumericMask};
use lc631_core::measurement_numeric::RationalInterval as Interval;
use lc631_core::{CandidateId, EpochId};
use lc631_runtime::formal_measurement::owned;
use lc631_runtime::joint_selection::formal::reduce_formal_owned;
use lc631_runtime::semantic_accelerator::selection::gpu::*;
use lc631_runtime::semantic_accelerator::selection::*;
use lc631_runtime::{formal_measurement as m, joint_selection::*};
use lc631_substrate::resource::Cancellation;
use support::*;

fn budget() -> ScopedSemanticGpuBudget {
    ScopedSemanticGpuBudget {
        max_buffers: 4,
        max_requested_bytes: 128 * 1024,
        max_inflight_requested_bytes: 128 * 1024,
        max_buffer_bytes: 64 * 1024,
    }
}
fn stamp() -> EpochStamp {
    EpochStamp {
        epoch: 1,
        generation: 1,
        fence: 1,
    }
}
fn opt_in() {
    assert_eq!(
        std::env::var("EPISTESYS_TEST_SEMANTIC_GPU").as_deref(),
        Ok("1")
    );
}

#[test]
fn tlcx33_grid_fraction_interval_and_unknown_never_launch() {
    cases(|c| {
        let (_b, _run, r) = setup(&c, Cancellation::default());
        let view = r.lease();
        let pins = pins(c.plan);
        let fraction = Interval::exact_ratio(1, 3).unwrap();
        let bounds = Interval::from_ratio(0, 1, 1, 1).unwrap();
        let values = [AxisValue::Exact(&fraction)];
        let axes = [AxisKind::Goal(d(40))];
        let policy = SelectionPolicy {
            pins,
            axes: &axes,
            representative_order: None,
            declared_goal_solve_overlap: false,
            max_candidates: 1,
            max_canonical_bytes: 4096,
        };
        let row = |axes| JointCandidate {
            id: CandidateId(1),
            context: c.good.context(),
            pins,
            hard_gate: ReportedGate::Pass,
            map_back: ReportedGate::Pass,
            axes,
            total_loss: AxisValue::Exact(&fraction),
            canonical_key: b"fraction-negative",
            measurement_binding: d(1),
        };
        let rows = [row(&values)];
        let batch = prepare_numeric_batch(
            &policy,
            &rows,
            SearchCoverage::EvaluatedSet,
            stamp(),
            &view,
            1,
        )
        .unwrap();
        let grid = [GridAxis {
            denominator: 2,
            minimum: 0,
            maximum: 2,
            direction: GridDirection::Maximize,
        }];
        let mut broker = AcceleratorBroker::new(CubeClWgpuBackend::profiled());
        assert!(matches!(
            run_semantic_grid(&batch, &grid, &mut broker, budget(), &view).unwrap(),
            SemanticGridOutcome::FreshCpu {
                reason: FreshCpuReason::Unsupported(GridUnsupported::Fraction),
                ..
            }
        ));
        let bv = [AxisValue::Bounded(&bounds)];
        let rows = [row(&bv)];
        let batch = prepare_numeric_batch(
            &policy,
            &rows,
            SearchCoverage::EvaluatedSet,
            stamp(),
            &view,
            1,
        )
        .unwrap();
        assert!(matches!(
            run_semantic_grid(&batch, &grid, &mut broker, budget(), &view).unwrap(),
            SemanticGridOutcome::FreshCpu {
                reason: FreshCpuReason::Unsupported(GridUnsupported::NonExact),
                ..
            }
        ));
        let uv = [AxisValue::Unavailable(MissingAxis::Unknown)];
        let rows = [row(&uv)];
        assert!(matches!(
            prepare_numeric_batch(
                &policy,
                &rows,
                SearchCoverage::EvaluatedSet,
                stamp(),
                &view,
                1
            ),
            Err(ConsumerError::MissingNumeric {
                reason: MissingAxis::Unknown,
                ..
            })
        ));
    });
}

#[test]
#[ignore = "Explicit actual hardware observation: set EPISTESYS_TEST_SEMANTIC_GPU=1"]
fn tlcx33_actual_gpu_owned_measurement_parity_and_foreign_stale_cuts() {
    opt_in();
    cases(|c| {
        let b = broker();
        let run = run(&b, c.plan.scope().source.digest(), Cancellation::default());
        // Two simultaneous actual outcomes, each retains native RAM/output and
        // CPU-selection claims, plus four owned losses and two reduced axes.
        let r = reserve(&run, 32 * 1024 * 1024, 4 * 1024 * 1024);
        let view = r.lease();
        let gs = owned::measure_owned(
            c.plan,
            c.good,
            m::Direction::Soundness,
            c.good_obs,
            work(),
            &view,
        )
        .unwrap();
        let gc = owned::measure_owned(
            c.plan,
            c.good,
            m::Direction::Completeness,
            c.good_obs,
            work(),
            &view,
        )
        .unwrap();
        let bs = owned::measure_owned(
            c.plan,
            c.bad,
            m::Direction::Soundness,
            c.bad_obs,
            work(),
            &view,
        )
        .unwrap();
        let bc = owned::measure_owned(
            c.plan,
            c.bad,
            m::Direction::Completeness,
            c.bad_obs,
            work(),
            &view,
        )
        .unwrap();
        let one = Interval::exact_ratio(1, 1).unwrap();
        let zero = Interval::exact_ratio(0, 1).unwrap();
        let rp = policy(pins(c.plan), one.lower(), one.lower(), &view);
        let p = rp.pins();
        let good = reduce_formal_owned(&rp, &gs, &gc, c.plan, c.good, &view).unwrap();
        let bad = reduce_formal_owned(&rp, &bs, &bc, c.plan, c.bad, &view).unwrap();
        let (_, gv) = good.project_current(&rp, p, c.plan, c.good, &view).unwrap();
        let (_, bv) = bad.project_current(&rp, p, c.plan, c.bad, &view).unwrap();
        let ga = [AxisValue::Exact(&one), gv];
        let ba = [AxisValue::Exact(&one), bv];
        let axes = [AxisKind::Goal(d(40)), AxisKind::SemanticLoss];
        let policy = SelectionPolicy {
            pins: p,
            axes: &axes,
            representative_order: None,
            declared_goal_solve_overlap: false,
            max_candidates: 2,
            max_canonical_bytes: 4096,
        };
        let rows = [
            JointCandidate {
                id: CandidateId(4),
                context: c.good.context(),
                pins: p,
                hard_gate: ReportedGate::Pass,
                map_back: ReportedGate::Pass,
                axes: &ga,
                total_loss: AxisValue::Exact(&zero),
                canonical_key: b"good",
                measurement_binding: d(4),
            },
            JointCandidate {
                id: CandidateId(5),
                context: c.bad.context(),
                pins: p,
                hard_gate: ReportedGate::Pass,
                map_back: ReportedGate::Pass,
                axes: &ba,
                total_loss: AxisValue::Exact(&zero),
                canonical_key: b"bad",
                measurement_binding: d(5),
            },
        ];
        let batch = prepare_numeric_batch(
            &policy,
            &rows,
            SearchCoverage::EvaluatedSet,
            stamp(),
            &view,
            8,
        )
        .unwrap();
        let grid = [
            GridAxis {
                denominator: 2,
                minimum: 0,
                maximum: 2,
                direction: GridDirection::Maximize,
            },
            GridAxis {
                denominator: 2,
                minimum: 0,
                maximum: 2,
                direction: GridDirection::Minimize,
            },
        ];
        let mut first = AcceleratorBroker::new(CubeClWgpuBackend::profiled());
        let outcome = run_semantic_grid(&batch, &grid, &mut first, budget(), &view).unwrap();
        let SemanticGridOutcome::ObservedCurrent {
            selection,
            observation,
        } = outcome
        else {
            panic!("real profile must execute; fallback is not hardware success")
        };
        assert_eq!(selection.project_current(&view).unwrap().frontier, [0]);
        assert_eq!(
            selection.project_current(&view).unwrap().recommendation,
            Some(0)
        );
        assert_eq!(
            observation
                .check_current(&first, &view)
                .unwrap()
                .frontier_words(),
            [1, 0]
        );
        let adapter = observation.historical().adapter();
        eprintln!(
            "actual semantic receipt {:?}: adapter={} backend={} class={:?} name_truncated={}",
            observation.historical().receipt_id(),
            adapter.name(),
            adapter.backend(),
            adapter.device_class(),
            adapter.name_truncated()
        );
        assert!(
            matches!(
                adapter.device_class(),
                lc631_accelerator::semantic_pareto::SemanticDeviceClass::DiscreteGpu
                    | lc631_accelerator::semantic_pareto::SemanticDeviceClass::IntegratedGpu
            ),
            "actual adapter must be a physical GPU; CPU/virtual/other classes are rejected"
        );
        assert_eq!(observation.historical().buffer_count(), 4);
        assert_eq!(observation.historical().requested_buffer_bytes(), 40);
        let mut second = AcceleratorBroker::new(CubeClWgpuBackend::profiled());
        let other = run_semantic_grid(&batch, &grid, &mut second, budget(), &view).unwrap();
        let SemanticGridOutcome::ObservedCurrent {
            observation: other, ..
        } = other
        else {
            panic!("second same-client observation required")
        };
        assert_eq!(
            observation.historical().receipt_id(),
            other.historical().receipt_id()
        );
        assert!(matches!(
            second.check_semantic_current(observation.historical()),
            Err(SemanticGpuError::ForeignObservation)
        ));
        first.quarantine(FaultKind::HostCancellation);
        assert!(matches!(
            first.check_semantic_current(observation.historical()),
            Err(SemanticGpuError::StaleObservation)
        ));
    });
}

#[test]
#[ignore = "Explicit actual hardware completion/cancellation observation"]
fn tlcx33_cancel_after_native_completion_retains_observation() {
    opt_in();
    let axes = vec![
        GridAxis {
            denominator: 1,
            minimum: 0,
            maximum: 10,
            direction: GridDirection::Minimize,
        };
        2
    ];
    let packet = CheckedSemanticGridBatch::checked_numeric(
        [9; 32],
        EpochId(3),
        axes,
        vec![[0; 32], [1; 32], [2; 32], [3; 32]],
        vec![3, 3, 9, 0, 1, 1, 0, 9],
        vec![NumericMask::Known; 4],
    )
    .unwrap();
    let mut broker = AcceleratorBroker::new(CubeClWgpuBackend::profiled());
    let mut calls = 0;
    let observation = broker
        .execute_semantic_pareto(&packet, budget(), &mut || {
            calls += 1;
            calls == 1
        })
        .unwrap();
    assert!(!observation.current_at_completion());
    // Row zero is dominated only by non-neighbor row two. This is an actual
    // numeric-kernel/cancellation vector, not a semantic measurement fixture.
    assert_eq!(observation.historical_frontier_words(), [0, 1, 1, 1]);
    assert!(matches!(
        broker.check_semantic_current(&observation),
        Err(SemanticGpuError::StaleObservation)
    ));
}
