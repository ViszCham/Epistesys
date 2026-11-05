//! Synthetic reported-axis integration; not current-owner or native-solver proof.
use lc631_analysis::creativity_measurement::{
    AssessmentPolicy, CandidateSubject, ComponentAssessment, ComponentMetric as M, ComponentReport,
    ComponentState, IndependenceStatus, InputBasis, MeasurementBinding, MeasurementGap,
    RationalInterval, SemanticValidation,
};
use lc631_core::{
    context::{ContextRevision, ContextScope, SourceRevision},
    CandidateId,
};
use lc631_runtime::joint_selection::*;
use lc631_substrate::Digest32;

fn d(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}
fn q(n: i64, den: i64) -> RationalInterval {
    RationalInterval::exact_ratio(n, den).unwrap()
}
fn pins() -> SelectionPins {
    SelectionPins {
        source: SourceRevision::of("component lineage declared question"),
        scope: ContextScope::declared(d(1), d(2), d(3), d(4)),
        baseline: d(5),
        hard_invariants: d(6),
        policy: d(7),
        goal_profile: Some(d(8)),
        evidence_revision: d(9),
    }
}
fn reports(candidate: u8, design: ComponentState) -> [ComponentReport; 2] {
    [M::Design, M::Prediction].map(|metric| {
        let producer = if metric == M::Design { 20 } else { 40 };
        ComponentReport {
            metric,
            definition: d(10),
            binding: MeasurementBinding {
                scope: pins().scope.comparison(),
                candidate: d(candidate),
                source: pins().source.digest(),
                target: d(candidate + 10),
                context: ContextRevision::original(pins().source).interpretation_digest(),
                baseline: pins().baseline,
                producer: d(producer),
                transform: d(producer + 1),
                result: d(producer + candidate),
            },
            probe_set: d(producer + 2),
            oracle: d(producer + 3),
            dependency_group: d(11),
            evidence_payload: d(producer + 4),
            basis: InputBasis::Given,
            validation: SemanticValidation::Unassessed,
            independence: IndependenceStatus::SharedDependency,
            state: if metric == M::Design {
                design.clone()
            } else {
                ComponentState::Measured(q(0, 1))
            },
        }
    })
}
fn selection(
    a: &[ComponentReport; 2],
    b: &[ComponentReport; 2],
) -> (Option<CandidateId>, Vec<usize>) {
    let policy = AssessmentPolicy {
        scope: pins().scope.comparison(),
        baseline: pins().baseline,
        required: &[M::Design, M::Prediction],
    };
    let refs_a = [&a[0], &a[1]];
    let refs_b = [&b[0], &b[1]];
    let assess_a = ComponentAssessment::from_reports(
        &policy,
        CandidateSubject::from_binding(a[0].binding),
        &refs_a,
    )
    .unwrap();
    let assess_b = ComponentAssessment::from_reports(
        &policy,
        CandidateSubject::from_binding(b[0].binding),
        &refs_b,
    )
    .unwrap();
    let projected_a = project_assessment_reported(&assess_a)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let projected_b = project_assessment_reported(&assess_b)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    for (axis, original) in projected_a.iter().zip(a) {
        assert!(std::ptr::eq(axis.report, original));
        assert_eq!(
            axis.report.independence,
            IndependenceStatus::SharedDependency
        );
    }
    let goal = q(1, 2);
    let total = q(0, 1);
    let axes_a = [
        AxisValue::Exact(&goal),
        project_component_reported(&a[0]).unwrap().value,
        project_component_reported(&a[1]).unwrap().value,
    ];
    let axes_b = [
        AxisValue::Exact(&goal),
        project_component_reported(&b[0]).unwrap().value,
        project_component_reported(&b[1]).unwrap().value,
    ];
    let kinds = [
        AxisKind::Goal(d(8)),
        projected_a[0].kind,
        projected_b[1].kind,
    ];
    let row = |id, key, axes| JointCandidate {
        id: CandidateId(id),
        context: ContextRevision::original(pins().source),
        pins: pins(),
        hard_gate: ReportedGate::Pass,
        map_back: ReportedGate::Pass,
        axes,
        total_loss: AxisValue::Exact(&total),
        canonical_key: key,
        measurement_binding: d(id as u8),
    };
    let rows = [row(1, b"base", &axes_a), row(2, b"candidate", &axes_b)];
    let select_policy = SelectionPolicy {
        pins: pins(),
        axes: &kinds,
        representative_order: None,
        declared_goal_solve_overlap: false,
        max_candidates: 2,
        max_canonical_bytes: 1024,
    };
    let result =
        select_joint_reported(&select_policy, &rows, SearchCoverage::EvaluatedSet).unwrap();
    (
        result.recommendation.map(|i| result.candidates[i].id),
        result.frontier,
    )
}

#[test]
fn tlcx23_distinct_producer_axes_reach_frontier_and_cut_changes_recommendation() {
    let base = reports(1, ComponentState::Measured(q(1, 4)));
    let candidate = reports(2, ComponentState::Measured(q(3, 4)));
    assert_eq!(
        selection(&base, &candidate),
        (Some(CandidateId(2)), vec![1])
    );
    let cut = reports(2, ComponentState::Unknown(MeasurementGap::MissingEvaluator));
    let pending = selection(&base, &cut);
    assert_ne!(pending.0, Some(CandidateId(2)));
    assert!(pending.1.contains(&1));
    assert_eq!(
        selection(&base, &candidate),
        (Some(CandidateId(2)), vec![1])
    );
}

#[test]
fn tlcx23_component_collection_does_not_smuggle_aggregates_into_pareto() {
    let mut rows = reports(1, ComponentState::Measured(q(1, 4)));
    rows[0].metric = M::StructuralGain;
    let refs = [&rows[0], &rows[1]];
    let policy = AssessmentPolicy {
        scope: pins().scope.comparison(),
        baseline: pins().baseline,
        required: &[M::StructuralGain, M::Prediction],
    };
    let a = ComponentAssessment::from_reports(
        &policy,
        CandidateSubject::from_binding(rows[0].binding),
        &refs,
    )
    .unwrap();
    assert!(project_assessment_reported(&a).any(|r| matches!(r, Err(SelectionError::InvalidAxes))));
}
