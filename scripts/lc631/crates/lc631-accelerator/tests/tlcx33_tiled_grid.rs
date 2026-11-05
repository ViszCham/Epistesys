use lc631_accelerator::semantic_pareto::{tiled::*, *};
use lc631_accelerator::NumericMask;
use lc631_core::EpochId;
fn packet(
    n: usize,
    m: usize,
    values: Vec<i32>,
    masks: Vec<NumericMask>,
    budget: usize,
) -> Result<CheckedTiledBatch, TiledError> {
    let rows = (0..n)
        .map(|i| {
            let mut id = [0; 32];
            id[..8].copy_from_slice(&(i as u64).to_le_bytes());
            id
        })
        .collect();
    CheckedTiledBatch::checked_numeric(
        [1; 32],
        EpochId(1),
        vec![
            GridAxis {
                denominator: 1,
                minimum: -10,
                maximum: 10,
                direction: GridDirection::Minimize
            };
            m
        ],
        rows,
        values,
        masks,
        budget,
    )
}
#[test]
fn tlcx33_tiled_reject_unknown_shape_work_and_cross_block_alias() {
    assert!(matches!(
        packet(2, 2, vec![0; 4], vec![NumericMask::Unknown; 2], 8),
        Err(TiledError::Input(SemanticGpuError::UnsupportedMask))
    ));
    assert!(packet(
        2049,
        1,
        vec![0; 2049],
        vec![NumericMask::Known; 2049],
        MAX_PAIR_AXES
    )
    .is_err());
    assert!(matches!(
        packet(
            2048,
            3,
            vec![0; 6144],
            vec![NumericMask::Known; 2048],
            MAX_PAIR_AXES
        ),
        Err(TiledError::WorkLimit)
    ));
    assert!(matches!(
        packet(2, 1, vec![0; 2], vec![NumericMask::Known; 2], usize::MAX),
        Err(TiledError::WorkLimit)
    ));
    let mut rows = vec![[1; 32]; 513];
    for (i, id) in rows.iter_mut().enumerate() {
        id[..8].copy_from_slice(&(i as u64).to_le_bytes());
    }
    rows[512] = rows[0];
    assert!(matches!(
        CheckedTiledBatch::checked_numeric(
            [1; 32],
            EpochId(1),
            vec![GridAxis {
                denominator: 1,
                minimum: 0,
                maximum: 1,
                direction: GridDirection::Minimize
            }],
            rows,
            vec![0; 513],
            vec![NumericMask::Known; 513],
            MAX_PAIR_AXES
        ),
        Err(TiledError::Input(SemanticGpuError::DuplicateRow))
    ));
}
#[test]
fn tlcx33_tiled_full_cross_block_non_neighbor_and_equal_rows() {
    let mut v = vec![3; 1026];
    v[1024] = 1;
    v[1025] = 1;
    let p = packet(513, 2, v, vec![NumericMask::Known; 513], MAX_PAIR_AXES).unwrap();
    let out = cpu_tiled_frontier(&p);
    assert_eq!(out.len(), 513);
    assert!(out[..512].iter().all(|v| *v == 0));
    assert_eq!(out[512], 1);
    assert_eq!(p.tile_count(), 4);
    let tie = packet(2, 2, vec![1; 4], vec![NumericMask::Known; 2], 8).unwrap();
    assert_eq!(cpu_tiled_frontier(&tie), [1, 1]);
}

#[cfg(feature = "gpu-cubecl-wgpu")]
mod hardware {
    use super::*;
    use lc631_accelerator::{AcceleratorBroker, CubeClWgpuBackend, FaultKind};
    use std::time::{Duration, Instant};
    fn budget() -> ScopedSemanticGpuBudget {
        ScopedSemanticGpuBudget {
            max_buffers: 4,
            max_requested_bytes: 128 * 1024,
            max_inflight_requested_bytes: 128 * 1024,
            max_buffer_bytes: 64 * 1024,
        }
    }
    fn enabled() {
        assert_eq!(
            std::env::var("EPISTESYS_TEST_SEMANTIC_GPU").as_deref(),
            Ok("1"),
            "explicit optin required"
        );
    }
    #[test]
    #[ignore = "requires explicit actual GPU optin and bounded 2048-row work"]
    fn tlcx33_tiled_actual_2048_cross_block_full_vector_and_physical_adapter() {
        enabled();
        let mut values = Vec::with_capacity(4096);
        for i in 0..2048 {
            values.extend_from_slice(if i % 2 == 0 { &[1, 3] } else { &[3, 1] });
        }
        values[0] = 0;
        values[1] = 2; // first-tile-only dominator for (1,3)
        values[4094] = 2;
        values[4095] = 0; // last-tile-only dominator for (3,1)
        let p = packet(
            2048,
            2,
            values,
            vec![NumericMask::Known; 2048],
            MAX_PAIR_AXES,
        )
        .unwrap();
        assert_eq!(p.tile_count(), 16);
        assert_eq!(p.pair_axis_work(), MAX_PAIR_AXES);
        let expected = cpu_tiled_frontier(&p);
        assert_eq!(expected[0], 1);
        assert!(expected[1..2047].iter().all(|v| *v == 0));
        assert_eq!(expected[2047], 1);
        let started = Instant::now();
        let mut current = || started.elapsed() < Duration::from_secs(30);
        let mut broker = AcceleratorBroker::new(CubeClWgpuBackend::profiled());
        let observation = broker
            .execute_semantic_tiled(&p, budget(), &mut current)
            .unwrap();
        eprintln!("actual tiled2048 adapter={} backend={} class={:?} completed_tiles={} requested_bytes={}",observation.adapter().name(),observation.adapter().backend(),observation.adapter().device_class(),observation.completed_tiles(),observation.requested_buffer_bytes());
        assert!(matches!(
            observation.adapter().device_class(),
            SemanticDeviceClass::DiscreteGpu | SemanticDeviceClass::IntegratedGpu
        ));
        assert_eq!(observation.completed_tiles(), 16);
        assert_eq!(observation.buffer_count(), 4);
        assert_eq!(
            broker
                .check_semantic_tiled_current(&observation)
                .unwrap()
                .frontier_words(),
            expected
        );
        let other = AcceleratorBroker::new(CubeClWgpuBackend::profiled());
        assert!(matches!(
            other.check_semantic_tiled_current(&observation),
            Err(SemanticGpuError::ForeignObservation)
        ));
        broker.quarantine(FaultKind::DeviceLost);
        assert!(matches!(
            broker.check_semantic_tiled_current(&observation),
            Err(SemanticGpuError::StaleObservation)
        ));
    }
    #[test]
    #[ignore = "requires explicit actual GPU optin"]
    fn tlcx33_tiled_actual_two_row_tie_and_partial_cancel_is_incomplete() {
        enabled();
        let mut broker = AcceleratorBroker::new(CubeClWgpuBackend::profiled());
        let p = packet(2, 2, vec![1; 4], vec![NumericMask::Known; 2], 8).unwrap();
        let o = broker
            .execute_semantic_tiled(&p, budget(), &mut || true)
            .unwrap();
        assert_eq!(o.historical_frontier_words(), [1, 1]);
        let mixed = CheckedTiledBatch::checked_numeric(
            [1; 32],
            EpochId(1),
            vec![
                GridAxis {
                    denominator: 1,
                    minimum: -10,
                    maximum: 10,
                    direction: GridDirection::Minimize,
                },
                GridAxis {
                    denominator: 1,
                    minimum: -10,
                    maximum: 10,
                    direction: GridDirection::Maximize,
                },
            ],
            vec![[1; 32], [2; 32]],
            vec![0, 0, 1, 1],
            vec![NumericMask::Known; 2],
            8,
        )
        .unwrap();
        let mixed_observation = broker
            .execute_semantic_tiled(&mixed, budget(), &mut || true)
            .unwrap();
        assert_eq!(mixed_observation.historical_frontier_words(), [1, 1]);
        let p = packet(
            513,
            2,
            vec![1; 1026],
            vec![NumericMask::Known; 513],
            MAX_PAIR_AXES,
        )
        .unwrap();
        let mut calls = 0;
        let mut cut = || {
            calls += 1;
            calls <= 2
        };
        assert!(matches!(
            broker.execute_semantic_tiled(&p, budget(), &mut cut),
            Err(TiledError::Incomplete {
                completed_tiles: 1,
                reason: SemanticGpuError::StaleObservation
            })
        ));
        assert!(matches!(
            broker.execute_semantic_tiled(&p, budget(), &mut || true),
            Err(TiledError::Input(SemanticGpuError::Quarantined))
        ));
    }
}
