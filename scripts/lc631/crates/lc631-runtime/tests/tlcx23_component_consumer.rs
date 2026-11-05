use lc631_analysis::creativity_measurement::*;
use lc631_core::context::{ContextRevision, ContextScope, SourceRevision};
use lc631_core::CandidateId;
use lc631_runtime::joint_selection::*;
use lc631_substrate::Digest32;

fn d(byte: u8) -> Digest32 {
    Digest32::from_bytes([byte; 32])
}
fn component(n: i64, state: Option<MeasurementGap>) -> ComponentReport {
    ComponentReport {
        metric: ComponentMetric::Coverage,
        definition: d(1),
        binding: MeasurementBinding {
            scope: d(2),
            candidate: d(3),
            source: d(4),
            target: d(5),
            context: d(6),
            baseline: d(7),
            producer: d(8),
            transform: d(9),
            result: d(10),
        },
        probe_set: d(11),
        oracle: d(12),
        dependency_group: d(13),
        evidence_payload: d(14),
        basis: InputBasis::Assumed,
        validation: SemanticValidation::SolverReported,
        independence: IndependenceStatus::SharedDependency,
        state: match state {
            Some(gap) => ComponentState::Unknown(gap),
            None => ComponentState::Measured(RationalInterval::exact_ratio(n, 4).unwrap()),
        },
    }
}

fn make_row<'a>(
    id: u64,
    values: &'a [AxisValue<'a>],
    key: &'a [u8],
    pins: SelectionPins,
    zero: &'a RationalInterval,
) -> JointCandidate<'a> {
    JointCandidate {
        id: CandidateId(id),
        context: ContextRevision::original(pins.source),
        pins,
        hard_gate: ReportedGate::Pass,
        map_back: ReportedGate::Pass,
        axes: values,
        total_loss: AxisValue::Exact(zero),
        canonical_key: key,
        measurement_binding: d(28),
    }
}

#[test]
fn tlcx23_component_measurement_changes_frontier_and_cut_retains_gap_metadata() {
    let low = component(1, None);
    let high = component(3, None);
    let gap = component(0, Some(MeasurementGap::MissingObservation));
    let low_axis = project_component_reported(&low).unwrap();
    let high_axis = project_component_reported(&high).unwrap();
    let cut_axis = project_component_reported(&gap).unwrap();
    assert!(std::ptr::eq(low_axis.report, &low));
    assert_eq!(low_axis.report.basis, InputBasis::Assumed);
    assert_eq!(
        low_axis.report.validation,
        SemanticValidation::SolverReported
    );
    assert_eq!(
        low_axis.report.independence,
        IndependenceStatus::SharedDependency
    );
    let axes = [AxisKind::Goal(d(20)), low_axis.kind];
    let zero = RationalInterval::exact_ratio(0, 1).unwrap();
    let a = [AxisValue::Exact(&zero), low_axis.value];
    let b = [AxisValue::Exact(&zero), high_axis.value];
    let cut = [AxisValue::Exact(&zero), cut_axis.value];
    let pins = SelectionPins {
        source: SourceRevision::of("question"),
        scope: ContextScope::declared(d(2), d(21), d(22), d(23)),
        baseline: d(7),
        hard_invariants: d(24),
        policy: d(25),
        goal_profile: Some(d(26)),
        evidence_revision: d(27),
    };
    let policy = SelectionPolicy {
        pins,
        axes: &axes,
        representative_order: None,
        declared_goal_solve_overlap: false,
        max_candidates: 2,
        max_canonical_bytes: 100,
    };
    let rows = [
        make_row(1, &a, b"a", pins, &zero),
        make_row(2, &b, b"b", pins, &zero),
    ];
    let report = select_joint_reported(&policy, &rows, SearchCoverage::EvaluatedSet).unwrap();
    assert_eq!(report.recommendation, Some(1));
    let cut_rows = [
        make_row(1, &a, b"a", pins, &zero),
        make_row(2, &cut, b"b", pins, &zero),
    ];
    let report = select_joint_reported(&policy, &cut_rows, SearchCoverage::EvaluatedSet).unwrap();
    assert_eq!(report.recommendation, Some(0));
    assert_eq!(report.frontier.len(), 2);
    assert!(matches!(
        gap.state,
        ComponentState::Unknown(MeasurementGap::MissingObservation)
    ));
}

#[test]
fn tlcx23_original_scores_and_aggregates_are_not_added_to_jointpareto() {
    let mut report = component(1, None);
    for metric in [
        ComponentMetric::OriginalV1,
        ComponentMetric::OriginalV2,
        ComponentMetric::StructuralGain,
        ComponentMetric::ReorganizationGain,
        ComponentMetric::UtilityGain,
        ComponentMetric::NoveltyCompatibility,
    ] {
        report.metric = metric;
        assert!(matches!(
            project_component_reported(&report),
            Err(SelectionError::InvalidAxes)
        ));
    }
}

#[test]
fn tlcx23_estimated_and_invalid_config_are_not_observed_or_unknown_zero() {
    let mut report = component(1, None);
    report.state = ComponentState::Estimated {
        bounds: RationalInterval::exact_ratio(1, 4).unwrap(),
        method: d(99),
    };
    assert!(matches!(
        project_component_reported(&report).unwrap().value,
        AxisValue::Estimated(_)
    ));
    report.state = ComponentState::InvalidConfig(MeasurementError::InvalidInput);
    assert!(matches!(
        project_component_reported(&report).unwrap().value,
        AxisValue::Unavailable(MissingAxis::InvalidConfig)
    ));
}
