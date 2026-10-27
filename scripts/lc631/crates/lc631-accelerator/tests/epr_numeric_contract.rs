use lc631_accelerator::{fixture_input, AcceleratorError, CpuBackend, NumericBackend, NumericMask};

#[test]
fn epr_06_every_csr_offset_is_checked_before_any_indexing() {
    let mut malformed = fixture_input(2);
    malformed.jacobi.offsets = vec![0, 2, 1];
    malformed.jacobi.indices = vec![0];
    malformed.jacobi.weights_x100 = vec![100];
    let result = std::panic::catch_unwind(|| malformed.validate());
    assert!(result.is_ok(), "shape validation must never panic");
    assert!(result.unwrap().is_err(), "nonmonotone CSR must be rejected");

    for offsets in [vec![1, 1, 2], vec![0, 3, 2], vec![0, 0, 3]] {
        let mut invalid = fixture_input(2);
        invalid.jacobi.offsets = offsets;
        assert!(invalid.validate().is_err());
    }
    let mut orphaned_relation = fixture_input(2);
    orphaned_relation.relation_offsets[0] = 1;
    assert!(orphaned_relation.validate().is_err());
}

#[test]
fn epr_06_unsafe_arithmetic_is_rejected_before_backend_execution() {
    let mut minimum = fixture_input(2);
    minimum.qubo.linear = vec![i32::MIN, 0];
    minimum.qubo.selected = vec![true, false];
    minimum.qubo.interactions.clear();
    assert!(matches!(
        minimum.validate(),
        Err(AcceleratorError::ArithmeticOverflow(_))
    ));
    let result = std::panic::catch_unwind(|| CpuBackend.execute(&minimum));
    assert!(result.is_ok(), "rejected arithmetic must not panic");
    assert!(result.unwrap().is_err());

    let mut overflow = fixture_input(2);
    overflow.jacobi.initial_x100.fill(i16::MAX);
    overflow.jacobi.offsets = vec![0, 202, 202];
    overflow.jacobi.indices = vec![0; 202];
    overflow.jacobi.weights_x100 = vec![i16::MAX; 202];
    overflow.jacobi.rounds = 1;
    assert!(matches!(
        overflow.validate(),
        Err(AcceleratorError::ArithmeticOverflow(_))
    ));
}

#[test]
fn epr_06_round_and_work_budgets_precede_cpu_or_gpu_submit() {
    let mut input = fixture_input(2);
    input.jacobi.rounds = 1_025;
    assert!(matches!(
        input.validate(),
        Err(AcceleratorError::BudgetExceeded(_))
    ));
    input.jacobi.rounds = usize::MAX;
    assert!(input.validate().is_err());
}

#[test]
fn epr_06_empty_and_duplicate_edges_have_defined_safe_behavior() {
    let mut input = fixture_input(2);
    input.relation_offsets = vec![0, 0, 0];
    input.relation_left.clear();
    input.relation_right.clear();
    input.jacobi.offsets = vec![0, 0, 0];
    input.jacobi.indices.clear();
    input.jacobi.weights_x100.clear();
    input.qubo.interactions.clear();
    assert!(input.validate().is_ok());
    assert!(CpuBackend.execute(&input).is_ok());

    input.jacobi.offsets = vec![0, 2, 2];
    input.jacobi.indices = vec![0, 0];
    input.jacobi.weights_x100 = vec![10, 10];
    input.jacobi.masks[1] = NumericMask::Unknown;
    assert!(input.validate().is_ok());
    assert!(CpuBackend.execute(&input).is_ok());
}

#[test]
fn epr_06_small_csr_shape_space_and_mask_boundaries_are_total() {
    for first in 0..=5 {
        for middle in 0..=5 {
            for last in 0..=5 {
                let mut input = fixture_input(2);
                input.jacobi.offsets = vec![first, middle, last];
                let expected = first == 0 && middle <= 4 && last == 4;
                let observed = std::panic::catch_unwind(|| input.validate());
                assert!(observed.is_ok());
                assert_eq!(observed.unwrap().is_ok(), expected);
                if expected {
                    assert!(CpuBackend.execute(&input).is_ok());
                }
            }
        }
    }
    for mask in [
        NumericMask::Known,
        NumericMask::Unknown,
        NumericMask::NotApplicable,
        NumericMask::Incomparable,
    ] {
        let mut input = fixture_input(2);
        input.masks.fill(mask);
        input.jacobi.masks.fill(mask);
        input.jacobi.rounds = 1_024;
        assert!(input.validate().is_ok());
        assert!(CpuBackend.execute(&input).is_ok());
    }
    let mut oversized = fixture_input(2);
    oversized
        .axes_x100
        .resize(lc631_accelerator::MAX_NUMERIC_ROWS + 1, [0; 3]);
    assert!(matches!(
        oversized.validate(),
        Err(AcceleratorError::BudgetExceeded("projection_rows"))
    ));
}

#[cfg(feature = "gpu-cubecl-wgpu")]
#[test]
#[ignore = "requires an actual GPU; run explicitly for the EPR-06 device gate"]
fn epr_06_actual_gpu_parity_including_empty_edges() {
    use lc631_accelerator::CubeClWgpuBackend;
    let mut gpu = CubeClWgpuBackend::default();
    let mut invalid = fixture_input(2);
    invalid.jacobi.offsets = vec![0, 2, 1];
    assert!(matches!(
        gpu.execute(&invalid),
        Err(AcceleratorError::ShapeMismatch("jacobi"))
    ));
    let normal = fixture_input(2_048);
    let mut empty = fixture_input(2);
    empty.relation_offsets = vec![0, 0, 0];
    empty.relation_left.clear();
    empty.relation_right.clear();
    empty.jacobi.offsets = vec![0, 0, 0];
    empty.jacobi.indices.clear();
    empty.jacobi.weights_x100.clear();
    empty.qubo.interactions.clear();
    let mut diagonal = fixture_input(2);
    diagonal
        .qubo
        .interactions
        .push(lc631_accelerator::QuboInteraction {
            left: 1,
            right: 1,
            coefficient: 13,
        });
    let mut masks = fixture_input(8);
    masks.masks = [
        NumericMask::Known,
        NumericMask::Unknown,
        NumericMask::NotApplicable,
        NumericMask::Incomparable,
    ]
    .repeat(2);
    masks.jacobi.masks = masks.masks.clone();
    let boundary = fixture_input(lc631_accelerator::MAX_NUMERIC_ROWS);
    for input in [normal, empty, diagonal, masks, boundary] {
        let cpu = CpuBackend.execute(&input).unwrap();
        let device = gpu.execute(&input).unwrap();
        assert!(device.native_completion_observed);
        assert_eq!(device.backend, lc631_accelerator::BackendKind::CubeClWgpu);
        eprintln!(
            "EPR-06 observed device={} rows={} native_completion={} full_readback={}",
            device.device_identity,
            input.rows(),
            device.native_completion_observed,
            device.full_readback
        );
        assert_eq!(cpu.lanes.len(), device.lanes.len());
        for (reference, actual) in cpu.lanes.iter().zip(&device.lanes) {
            assert_eq!(reference.lane, actual.lane);
            assert_eq!(reference.summary_words, actual.summary_words);
            assert_eq!(reference.mask_digest, actual.mask_digest);
            assert_eq!(reference.finalist_rows, actual.finalist_rows);
            assert_eq!(reference.finalist_values, actual.finalist_values);
            assert_eq!(reference.decision_digest, actual.decision_digest);
        }
    }
}
