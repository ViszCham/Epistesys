use lc631_analysis::creativity_measurement::{
    complexity_match, compression_gain, knownness, set_distance, Feature, PreservationPrerequisite,
};
use lc631_analysis::creativity_measurement::{
    paired_gain, weighted_assessment, AssessmentPolicy, ComponentMetric, ComponentReport,
    ComponentState, CreativeAssessment, IndependenceStatus, InputBasis, MeasurementBinding,
    MeasurementError, MeasurementGap, ProbePair, RationalInterval, SemanticValidation,
    WeightedProbe,
};
use lc631_substrate::Digest32;

fn pin(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}
fn interval(a: i64, b: i64, c: i64, d: i64) -> RationalInterval {
    RationalInterval::from_ratio(a, b, c, d).unwrap()
}
fn report(metric: ComponentMetric) -> ComponentReport {
    ComponentReport {
        metric,
        definition: pin(1),
        binding: MeasurementBinding {
            scope: pin(2),
            candidate: pin(3),
            source: pin(4),
            target: pin(5),
            context: pin(6),
            baseline: pin(7),
            producer: pin(8),
            transform: pin(9),
            result: pin(10),
        },
        probe_set: pin(11),
        oracle: pin(12),
        dependency_group: pin(13),
        evidence_payload: pin(14),
        basis: InputBasis::Assumed,
        validation: SemanticValidation::SolverReported,
        independence: IndependenceStatus::SharedDependency,
        state: ComponentState::Unknown(MeasurementGap::MissingObservation),
    }
}

#[test]
fn tlcx22_k1_g01_g02_fixed_denominator_hand_goldens() {
    let z = interval(0, 1, 0, 1);
    let o = interval(1, 1, 1, 1);
    let u = interval(0, 1, 1, 1);
    let w1 = o.lower();
    let three = interval(3, 1, 3, 1);
    let w3 = three.lower();
    let probes = [
        WeightedProbe {
            canonical_payload: b"a",
            weight: w1,
            value: &z,
        },
        WeightedProbe {
            canonical_payload: b"b",
            weight: w3,
            value: &o,
        },
    ];
    assert_eq!(weighted_assessment(&probes).unwrap(), interval(3, 4, 3, 4));
    let probes = [
        WeightedProbe {
            canonical_payload: b"a",
            weight: w1,
            value: &z,
        },
        WeightedProbe {
            canonical_payload: b"b",
            weight: w3,
            value: &u,
        },
    ];
    assert_eq!(weighted_assessment(&probes).unwrap(), interval(0, 1, 3, 4));
}

#[test]
fn tlcx22_k2_g04_paired_gain_hand_golden() {
    let z = interval(0, 1, 0, 1);
    let o = interval(1, 1, 1, 1);
    let probes = [
        ProbePair {
            canonical_payload: b"a",
            weight: o.lower(),
            baseline: &o,
            candidate: &o,
            baseline_node: pin(1),
            candidate_node: pin(2),
        },
        ProbePair {
            canonical_payload: b"b",
            weight: o.lower(),
            baseline: &z,
            candidate: &o,
            baseline_node: pin(3),
            candidate_node: pin(4),
        },
    ];
    assert_eq!(paired_gain(&probes).unwrap(), interval(1, 2, 1, 2));
}

#[test]
fn tlcx22_k2_same_valid_node_preserves_dependence_separate_nodes_do_not() {
    let u = interval(0, 1, 1, 1);
    let w = interval(1, 1, 1, 1);
    let mut probes = [ProbePair {
        canonical_payload: b"p",
        weight: w.lower(),
        baseline: &u,
        candidate: &u,
        baseline_node: pin(1),
        candidate_node: pin(1),
    }];
    assert_eq!(paired_gain(&probes).unwrap(), interval(0, 1, 0, 1));
    probes[0].candidate_node = pin(2);
    assert_eq!(paired_gain(&probes).unwrap(), interval(-1, 1, 1, 1));
}

#[test]
fn tlcx22_negative_empty_duplicate_nonpositive_out_of_range() {
    assert_eq!(
        weighted_assessment(&[]),
        Err(MeasurementError::EmptyDenominator)
    );
    let z = interval(0, 1, 0, 1);
    let o = interval(1, 1, 1, 1);
    let duplicate = [
        WeightedProbe {
            canonical_payload: b"a",
            weight: o.lower(),
            value: &z,
        },
        WeightedProbe {
            canonical_payload: b"a",
            weight: o.lower(),
            value: &o,
        },
    ];
    assert_eq!(
        weighted_assessment(&duplicate),
        Err(MeasurementError::DuplicateProbe)
    );
    let zero_weight = [WeightedProbe {
        canonical_payload: b"a",
        weight: z.lower(),
        value: &o,
    }];
    assert_eq!(
        weighted_assessment(&zero_weight),
        Err(MeasurementError::InvalidInput)
    );
    let outside = interval(2, 1, 2, 1);
    let bad = [WeightedProbe {
        canonical_payload: b"a",
        weight: o.lower(),
        value: &outside,
    }];
    assert_eq!(
        weighted_assessment(&bad),
        Err(MeasurementError::InvalidInput)
    );
}

#[test]
fn tlcx22_assessment_retains_unknown_and_shared_unvalidated_basis() {
    let reports = [report(ComponentMetric::Coverage)];
    let required = [ComponentMetric::Coverage];
    let policy = AssessmentPolicy {
        scope: pin(2),
        baseline: pin(7),
        required: &required,
    };
    let assessment = CreativeAssessment::from_reports(&policy, &reports).unwrap();
    assert_eq!(assessment.reports(), reports.as_slice());
    let consumed = assessment.get(ComponentMetric::Coverage).unwrap();
    assert_eq!(
        consumed.state,
        ComponentState::Unknown(MeasurementGap::MissingObservation)
    );
    assert_eq!(consumed.validation, SemanticValidation::SolverReported);
    assert_eq!(consumed.basis, InputBasis::Assumed);
    assert_eq!(consumed.independence, IndependenceStatus::SharedDependency);
}

#[test]
fn tlcx22_assessment_cut_missing_metric_and_scope_swap_are_errors() {
    let mut reports = [report(ComponentMetric::Coverage)];
    let required = [ComponentMetric::Coverage, ComponentMetric::Compression];
    let policy = AssessmentPolicy {
        scope: pin(2),
        baseline: pin(7),
        required: &required,
    };
    assert!(matches!(
        CreativeAssessment::from_reports(&policy, &reports),
        Err(MeasurementError::MissingMetric)
    ));
    reports[0].binding.scope = pin(99);
    assert!(matches!(
        CreativeAssessment::from_reports(&policy, &reports),
        Err(MeasurementError::ScopeMismatch)
    ));
    assert!(CreativeAssessment::from_reports(&policy, &[]).is_err());
}

#[test]
fn tlcx22_k3_g03_g36_and_empty_archive_are_distinct() {
    let one = interval(1, 1, 1, 1);
    let universe = [
        Feature {
            canonical_payload: b"a",
            weight: one.lower(),
        },
        Feature {
            canonical_payload: b"b",
            weight: one.lower(),
        },
        Feature {
            canonical_payload: b"c",
            weight: one.lower(),
        },
    ];
    let left: &[&[u8]] = &[b"a", b"b"];
    let right: &[&[u8]] = &[b"b", b"c"];
    assert_eq!(
        set_distance(&universe, left, right).unwrap(),
        interval(2, 3, 2, 3)
    );
    assert_eq!(
        set_distance(&universe, &[b"outside"], right),
        Err(MeasurementError::UniverseMismatch)
    );
    assert_eq!(
        knownness(&universe, left, &[]),
        Err(MeasurementError::EmptyCorpus)
    );
    assert_eq!(knownness(&universe, left, &[left]).unwrap(), one);
}

#[test]
fn tlcx22_compression_g15_and_symmetric_cmatch_g06() {
    assert_eq!(
        compression_gain(80, 40, PreservationPrerequisite::ReportedChecked).unwrap(),
        interval(1, 2, 1, 2)
    );
    assert_eq!(
        compression_gain(80, 40, PreservationPrerequisite::Pending),
        Err(MeasurementError::PrerequisitePending)
    );
    assert_eq!(
        compression_gain(80, 40, PreservationPrerequisite::Failed),
        Err(MeasurementError::PrerequisiteFailed)
    );
    assert_eq!(complexity_match(80, 160, 80).unwrap(), interval(1, 2, 1, 2));
    assert_eq!(complexity_match(160, 80, 80).unwrap(), interval(1, 2, 1, 2));
}
