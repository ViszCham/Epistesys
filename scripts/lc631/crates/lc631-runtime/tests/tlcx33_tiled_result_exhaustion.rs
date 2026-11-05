//! Opt-in actual numeric GPU observation with deliberately insufficient result RAM.
//! The input here is caller reported; it establishes no world correspondence.
#![cfg(feature = "semantic-gpu")]
#[path = "tlcx23_formal_support/mod.rs"]
mod support;

use lc631_accelerator::semantic_pareto::{GridAxis, GridDirection, ScopedSemanticGpuBudget};
use lc631_accelerator::{AcceleratorBroker, CubeClWgpuBackend};
use lc631_core::measurement_numeric::RationalInterval as Interval;
use lc631_core::CandidateId;
use lc631_runtime::joint_selection::*;
use lc631_runtime::semantic_accelerator::selection::{tiled::*, *};
use lc631_substrate::resource::{Cancellation, ResourceError, ResourceKind};
use support::*;

#[test]
#[ignore = "explicit actual GPU result-accounting rejection observation"]
fn tlcx33_tiled_actual_gpu_result_exhaustion_preserves_observation() {
    assert_eq!(
        std::env::var("EPISTESYS_TEST_SEMANTIC_GPU").as_deref(),
        Ok("1")
    );
    cases(|c| {
        let b = broker();
        let run = run(&b, c.plan.scope().source.digest(), Cancellation::default());
        // Preparation peak: 2 MiB scratch + 4.25 MiB batch. Native run:
        // batch 4.25 MiB + transport 0.25 MiB + native receipt 0.25 MiB.
        // Another 4.25 MiB selection claim cannot fit in this reservation.
        let reservation = reserve(
            &run,
            2 * 1024 * 1024 + 256 * 1024 + 4 * 1024 * 1024,
            1024 * 1024,
        );
        let view = reservation.lease();
        let pins = pins(c.plan);
        let one = Interval::exact_ratio(1, 1).unwrap();
        let kinds = [AxisKind::Goal(d(40))];
        let values = [AxisValue::Exact(&one)];
        let policy = SelectionPolicy {
            pins,
            axes: &kinds,
            representative_order: None,
            declared_goal_solve_overlap: false,
            max_candidates: 1,
            max_canonical_bytes: 1024,
        };
        let rows = [JointCandidate {
            id: CandidateId(1),
            context: c.good.context(),
            pins,
            hard_gate: ReportedGate::Pass,
            map_back: ReportedGate::Pass,
            axes: &values,
            total_loss: AxisValue::Exact(&one),
            canonical_key: b"actual-gpu-resource-cut",
            measurement_binding: d(1),
        }];
        let batch = prepare_numeric_batch_with_profile(
            &policy,
            &rows,
            SearchCoverage::EvaluatedSet,
            EpochStamp {
                epoch: 1,
                generation: 1,
                fence: 1,
            },
            &view,
            1,
            NumericPayloadProfile::TiledWorld2048V1,
        )
        .unwrap();
        let axes = [GridAxis {
            denominator: 1,
            minimum: 0,
            maximum: 1,
            direction: GridDirection::Maximize,
        }];
        let budget = ScopedSemanticGpuBudget {
            max_buffers: 4,
            max_requested_bytes: 128 * 1024,
            max_inflight_requested_bytes: 128 * 1024,
            max_buffer_bytes: 64 * 1024,
        };
        let mut native = AcceleratorBroker::new(CubeClWgpuBackend::profiled());
        let result = run_semantic_tiled(&batch, &axes, &mut native, budget, &view).unwrap();
        let TiledOutcome::ObservedRejected {
            reason: ConsumerError::Resource(ResourceError::Exhausted(ResourceKind::Ram)),
            observation,
        } = result
        else {
            panic!("actual GPU observation with result RAM exhaustion must be retained");
        };
        assert!(matches!(
            observation.adapter().device_class(),
            lc631_accelerator::semantic_pareto::SemanticDeviceClass::DiscreteGpu
                | lc631_accelerator::semantic_pareto::SemanticDeviceClass::IntegratedGpu
        ));
        assert_eq!(observation.historical_frontier_words(), [1]);
        assert_eq!(
            observation
                .check_current(&native, &view)
                .unwrap()
                .frontier_words(),
            [1]
        );
    });
}
