//! Explicit hardware cuts for late cancellation, lease expiry and injected callback panic.
//! These numeric inputs do not establish world correspondence or actual device loss.
#![cfg(feature = "semantic-gpu")]
#[path = "tlcx23_formal_support/mod.rs"]
mod support;

use lc631_accelerator::semantic_pareto::{tiled::*, *};
use lc631_accelerator::{AcceleratorBroker, CubeClWgpuBackend, NumericMask};
use lc631_core::measurement_numeric::RationalInterval as Interval;
use lc631_core::{CandidateId, EpochId};
use lc631_runtime::joint_selection::*;
use lc631_runtime::resource::ResourceLimits;
use lc631_runtime::semantic_accelerator::selection::{tiled::*, *};
use lc631_substrate::resource::{Cancellation, Deadline, ResourceError};
use std::time::Duration;
use support::*;

fn enabled() {
    assert_eq!(
        std::env::var("EPISTESYS_TEST_SEMANTIC_GPU").as_deref(),
        Ok("1")
    );
}

fn packet(rows: usize) -> CheckedTiledBatch {
    let ids = (0..rows)
        .map(|i| {
            let mut id = [0; 32];
            id[..8].copy_from_slice(&(i as u64).to_le_bytes());
            id
        })
        .collect();
    CheckedTiledBatch::checked_numeric(
        [7; 32],
        EpochId(1),
        vec![
            GridAxis {
                denominator: 1,
                minimum: 0,
                maximum: 1,
                direction: GridDirection::Minimize,
            };
            2
        ],
        ids,
        vec![0; rows * 2],
        vec![NumericMask::Known; rows],
        MAX_PAIR_AXES,
    )
    .unwrap()
}

fn budget(max_requested_bytes: u64) -> ScopedSemanticGpuBudget {
    ScopedSemanticGpuBudget {
        max_buffers: 4,
        max_requested_bytes,
        max_inflight_requested_bytes: max_requested_bytes,
        max_buffer_bytes: max_requested_bytes,
    }
}

fn warm(broker: &mut AcceleratorBroker<CubeClWgpuBackend>) {
    let small = packet(2);
    let observation = broker
        .execute_semantic_tiled(&small, budget(128 * 1024), &mut || true)
        .unwrap();
    assert!(matches!(
        observation.adapter().device_class(),
        SemanticDeviceClass::DiscreteGpu | SemanticDeviceClass::IntegratedGpu
    ));
    assert_eq!(observation.historical_frontier_words(), [1, 1]);
    assert_eq!(observation.requested_buffer_bytes(), 40);
}

#[test]
#[ignore = "explicit hardware late-cancellation observation"]
fn tlcx33_late_cancel_fifteen_synced_tiles_and_known_charge_release() {
    enabled();
    let mut gpu = AcceleratorBroker::new(CubeClWgpuBackend::profiled());
    warm(&mut gpu);
    let resource_broker = broker();
    let cancel = Cancellation::default();
    let run = run(&resource_broker, d(3), cancel.clone());
    let reservation = reserve(&run, 16 * 1024 * 1024, 1024 * 1024);
    let view = reservation.lease();
    let big = packet(2048);
    assert_eq!(big.tile_count(), 16);
    assert_eq!(big.requested_buffer_bytes().unwrap(), 32776);
    let mut calls = 0;
    let mut check = || {
        calls += 1;
        if calls == 17 {
            cancel.cancel();
        }
        view.check().is_ok()
    };
    assert!(matches!(
        gpu.execute_semantic_tiled(&big, budget(128 * 1024), &mut check),
        Err(TiledError::Incomplete {
            completed_tiles: 15,
            reason: SemanticGpuError::StaleObservation
        })
    ));
    assert_eq!(calls, 17);
    assert!(matches!(view.check(), Err(ResourceError::Cancelled)));
    assert!(matches!(
        gpu.execute_semantic_tiled(&big, budget(128 * 1024), &mut || true),
        Err(TiledError::Input(SemanticGpuError::Quarantined))
    ));

    // New logical broker, same process-global GPU client and requested ledger.
    // A 64-byte cap cannot admit the old 32776-byte charge if it is retained.
    let fresh_run = resource_broker
        .begin_run(
            2,
            d(3),
            ResourceLimits::uniform(64 * 1024 * 1024).unwrap(),
            Deadline::after(Duration::from_secs(60)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let fresh_reservation = reserve(&fresh_run, 16 * 1024 * 1024, 1024 * 1024);
    let fresh_view = fresh_reservation.lease();
    let mut fresh_gpu = AcceleratorBroker::new(CubeClWgpuBackend::profiled());
    let small = packet(2);
    let recovered = fresh_gpu
        .execute_semantic_tiled(&small, budget(64), &mut || fresh_view.check().is_ok())
        .unwrap();
    assert_eq!(recovered.requested_buffer_bytes(), 40);
    assert_eq!(recovered.historical_frontier_words(), [1, 1]);
    assert_eq!(
        fresh_gpu
            .check_semantic_tiled_current(&recovered)
            .unwrap()
            .frontier_words(),
        [1, 1]
    );
}

#[test]
#[ignore = "explicit hardware completed-observation deadline expiry"]
fn tlcx33_actual_gpu_observation_expires_with_real_reservation_deadline() {
    enabled();
    let mut gpu = AcceleratorBroker::new(CubeClWgpuBackend::profiled());
    warm(&mut gpu); // Initialize the actual device before starting the short lease.
    cases(|c| {
        let resource_broker = broker();
        let run = resource_broker
            .begin_run(
                1,
                c.plan.scope().source.digest(),
                ResourceLimits::uniform(64 * 1024 * 1024).unwrap(),
                Deadline::after(Duration::from_secs(2)).unwrap(),
                Cancellation::default(),
            )
            .unwrap();
        let reservation = reserve(&run, 16 * 1024 * 1024, 1024 * 1024);
        let view = reservation.lease();
        let pins = pins(c.plan);
        let one = Interval::exact_ratio(1, 1).unwrap();
        let zero = Interval::exact_ratio(0, 1).unwrap();
        let kinds = [AxisKind::Goal(d(40)), AxisKind::SemanticLoss];
        let first = [AxisValue::Exact(&one), AxisValue::Exact(&zero)];
        let second = [AxisValue::Exact(&zero), AxisValue::Exact(&one)];
        let policy = SelectionPolicy {
            pins,
            axes: &kinds,
            representative_order: None,
            declared_goal_solve_overlap: false,
            max_candidates: 2,
            max_canonical_bytes: 1024,
        };
        let rows = [
            JointCandidate {
                id: CandidateId(1),
                context: c.good.context(),
                pins,
                hard_gate: ReportedGate::Pass,
                map_back: ReportedGate::Pass,
                axes: &first,
                total_loss: AxisValue::Exact(&zero),
                canonical_key: b"first",
                measurement_binding: d(1),
            },
            JointCandidate {
                id: CandidateId(2),
                context: c.good.context(),
                pins,
                hard_gate: ReportedGate::Pass,
                map_back: ReportedGate::Pass,
                axes: &second,
                total_loss: AxisValue::Exact(&one),
                canonical_key: b"second",
                measurement_binding: d(2),
            },
        ];
        let stamp = EpochStamp {
            epoch: 1,
            generation: 1,
            fence: 1,
        };
        let batch = prepare_numeric_batch(
            &policy,
            &rows,
            SearchCoverage::EvaluatedSet,
            stamp,
            &view,
            8,
        )
        .unwrap();
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
        let outcome =
            run_semantic_tiled(&batch, &grid, &mut gpu, budget(128 * 1024), &view).unwrap();
        let TiledOutcome::ObservedCurrent {
            selection,
            observation,
        } = outcome
        else {
            panic!("actual completed GPU and current selection required before expiry");
        };
        assert_eq!(
            observation
                .check_current(&gpu, &view)
                .unwrap()
                .frontier_words(),
            [1, 0]
        );
        assert_eq!(selection.project_current(&view).unwrap().frontier, [0]);
        let remaining = view
            .remaining()
            .expect("deadline must still be current after GPU");
        std::thread::sleep(remaining + Duration::from_millis(20));
        assert!(matches!(view.check(), Err(ResourceError::DeadlineExceeded)));
        assert_eq!(observation.historical_frontier_words(), [1, 0]);
        assert!(matches!(
            observation.check_current(&gpu, &view),
            Err(TiledRunError::Consumer(ConsumerError::Resource(
                ResourceError::DeadlineExceeded
            )))
        ));
        assert!(matches!(
            selection.project_current(&view),
            Err(ConsumerError::Resource(ResourceError::DeadlineExceeded))
        ));
    });
}

#[test]
#[ignore = "explicit injected callback panic; run in a separate process"]
fn tlcx33_injected_callback_panic_after_fifteen_tiles_reports_known_progress() {
    enabled();
    let mut gpu = AcceleratorBroker::new(CubeClWgpuBackend::profiled());
    warm(&mut gpu);
    let big = packet(2048);
    let mut calls = 0;
    let mut check = || {
        calls += 1;
        assert_ne!(calls, 17, "deliberately injected callback panic");
        true
    };
    // Expected failure before a local catch exists: the current outer catch
    // reports zero, losing the fifteen known synchronized tiles.
    let actual = gpu.execute_semantic_tiled(&big, budget(128 * 1024), &mut check);
    if let Err(error) = &actual {
        eprintln!("injected callback panic result: {error:?}");
    }
    assert!(matches!(
        actual,
        Err(TiledError::Incomplete {
            completed_tiles: 15,
            ..
        })
    ));
    assert_eq!(calls, 17);
    assert!(matches!(
        gpu.execute_semantic_tiled(&big, budget(128 * 1024), &mut || true),
        Err(TiledError::Input(SemanticGpuError::Quarantined))
    ));
    // Unknown completion must also keep the shared requested ledger closed,
    // even for a fresh logical broker and a much smaller allocation.
    let mut fresh_gpu = AcceleratorBroker::new(CubeClWgpuBackend::profiled());
    assert!(matches!(
        fresh_gpu.execute_semantic_tiled(&packet(2), budget(64), &mut || true),
        Err(TiledError::Input(SemanticGpuError::Quarantined))
    ));
}
