#![forbid(unsafe_code)]

use lc631_tldg::{
    build_tldg_pipeline, evaluate_geometry_execution, run_adversarial_evaluation,
    CalibrationPartition, GeometryFault, GeometryParityState,
};

#[test]
fn tldg_21_calibration_partitions_are_distinct_and_advisory() {
    let partition =
        CalibrationPartition::checked("train-r1", "calibration-r1", "holdout-r1", "adversarial-r1")
            .unwrap();
    let report = build_tldg_pipeline("answer both parts", None, &partition).unwrap();
    assert!(report.empirical_risk.advisory_only);
    assert!(!report.empirical_risk.can_authorize());
    assert!(!report.empirical_risk.can_replace_defect_graph());
}

#[test]
fn tldg_22_program_output_closure_remains_unbound_without_host_output() {
    let partition = CalibrationPartition::reference();
    let report = build_tldg_pipeline("implement parser", None, &partition).unwrap();
    assert!(!report.output.output_commit_allowed);
    assert!(!report.output.authority_created);
    assert_eq!(report.output.binding_state, "host_output_unbound");
}

#[test]
fn tldg_23_device_loss_quarantines_without_partial_stitching() {
    let receipt = evaluate_geometry_execution(None, Some(GeometryFault::DeviceLost));
    assert_eq!(receipt.parity, GeometryParityState::Unavailable);
    assert!(receipt.quarantined);
    assert!(!receipt.partial_stitching_allowed);
    assert!(!receipt.creates_authority);
}

#[test]
fn tldg_23_adversarial_evaluation_keeps_real_gpu_unobserved() {
    let report = run_adversarial_evaluation().unwrap();
    assert!(report.source_roundtrip_pass);
    assert!(report.geometry_non_authority_pass);
    assert!(report.unknown_retention_pass);
    assert!(!report.real_gpu_geometry_observed);
    assert!(!report.general_performance_claim_supported);
}
