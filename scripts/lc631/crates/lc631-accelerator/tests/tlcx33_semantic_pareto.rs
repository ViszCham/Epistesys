use lc631_accelerator::semantic_pareto::*;
use lc631_accelerator::NumericMask;
use lc631_core::EpochId;

fn axes() -> [GridAxis; 2] {
    [GridAxis {
        denominator: 2,
        minimum: -20,
        maximum: 20,
        direction: GridDirection::Minimize,
    }; 2]
}
#[test]
fn tlcx33_full_pairs_non_neighbor_dominator() {
    let batch = CheckedSemanticGridBatch::checked_numeric(
        [1; 32],
        EpochId(1),
        axes().to_vec(),
        vec![[0; 32], [1; 32], [2; 32], [3; 32]],
        vec![3, 3, 9, 0, 1, 1, 0, 9],
        vec![NumericMask::Known; 4],
    )
    .unwrap();
    assert_eq!(cpu_grid_frontier(&batch), [0, 1, 1, 1]);
    assert_eq!(batch.requested_buffer_bytes().unwrap(), 32 + 8 + 16 + 16);
}
#[test]
fn tlcx33_grid_rejects_unknown_shape_and_limits() {
    assert!(CheckedSemanticGridBatch::checked_numeric(
        [1; 32],
        EpochId(1),
        axes().to_vec(),
        vec![[1; 32]],
        vec![0, 0],
        vec![NumericMask::Unknown]
    )
    .is_err());
    assert!(CheckedSemanticGridBatch::checked_numeric(
        [1; 32],
        EpochId(1),
        axes().to_vec(),
        vec![[1; 32]],
        vec![0],
        vec![NumericMask::Known]
    )
    .is_err());
    let mut a = axes();
    a[0].denominator = 0;
    assert!(CheckedSemanticGridBatch::checked_numeric(
        [1; 32],
        EpochId(1),
        a.to_vec(),
        vec![[1; 32]],
        vec![0, 0],
        vec![NumericMask::Known]
    )
    .is_err());
    assert!(ScopedSemanticGpuBudget {
        max_buffers: 3,
        max_requested_bytes: 4096,
        max_inflight_requested_bytes: 4096,
        max_buffer_bytes: 4096
    }
    .validate_requested(64, 32)
    .is_err());
}
