//! Synthetic arithmetic/report-consumer fixtures, not actual solver exit evidence.
use lc631_analysis::creativity_measurement::RationalInterval;
use lc631_core::context::{ContextRevision, ContextScope, SourceRevision};
use lc631_core::CandidateId;
use lc631_runtime::joint_selection::*;
use lc631_substrate::Digest32;

fn digest(byte: u8) -> Digest32 {
    Digest32::from_bytes([byte; 32])
}
fn pins() -> SelectionPins {
    SelectionPins {
        source: SourceRevision::of("fixed question"),
        scope: ContextScope::declared(digest(1), digest(2), digest(3), digest(4)),
        baseline: digest(5),
        hard_invariants: digest(6),
        policy: digest(7),
        goal_profile: Some(digest(8)),
        evidence_revision: digest(9),
    }
}
fn policy(axes: &[AxisKind]) -> SelectionPolicy<'_> {
    SelectionPolicy {
        pins: pins(),
        axes,
        representative_order: None,
        declared_goal_solve_overlap: false,
        max_candidates: 2_048,
        max_canonical_bytes: 131_072,
    }
}
fn value(n: i64, d: i64) -> RationalInterval {
    RationalInterval::exact_ratio(n, d).unwrap()
}
fn row<'a>(
    id: u64,
    axes: &'a [AxisValue<'a>],
    loss: &'a RationalInterval,
    key: &'a [u8],
) -> JointCandidate<'a> {
    JointCandidate {
        id: CandidateId(id),
        context: ContextRevision::original(pins().source),
        pins: pins(),
        hard_gate: ReportedGate::Pass,
        map_back: ReportedGate::Pass,
        axes,
        total_loss: AxisValue::Exact(loss),
        canonical_key: key,
        measurement_binding: digest(10),
    }
}

#[test]
fn tlcx23_reconstructed_context_can_be_recommended_and_cuts_change_selection() {
    let (low, high, zero) = (value(1, 4), value(3, 4), value(0, 1));
    let axis = [AxisKind::Goal(digest(20)), AxisKind::CoverageGain];
    let base = [AxisValue::Exact(&low), AxisValue::Exact(&zero)];
    let rebuilt = [AxisValue::Exact(&high), AxisValue::Exact(&high)];
    let mut rows = [
        row(1, &base, &zero, b"original"),
        row(2, &rebuilt, &zero, b"reconstructed"),
    ];
    rows[1].context = rows[1].context.reconstructed([21; 32]);
    let report =
        select_joint_reported(&policy(&axis), &rows, SearchCoverage::EvaluatedSet).unwrap();
    assert_eq!(report.recommendation, Some(1));
    assert_eq!(report.dispositions[0], Disposition::Dominated { by: 1 });
    rows[1].hard_gate = ReportedGate::NeedsEvidence;
    let cut = select_joint_reported(&policy(&axis), &rows, SearchCoverage::EvaluatedSet).unwrap();
    assert_eq!(cut.recommendation, Some(0));
    assert_eq!(cut.dispositions[1], Disposition::NeedsEvidence);
    assert_eq!(cut.alternatives, vec![1]);
}

#[test]
fn tlcx23_hard_failure_cannot_be_bought_by_gain() {
    let (low, high, zero) = (value(0, 1), value(1, 1), value(0, 1));
    let axis = [AxisKind::Goal(digest(20)), AxisKind::CoverageGain];
    let a = [AxisValue::Exact(&low), AxisValue::Exact(&low)];
    let b = [AxisValue::Exact(&high), AxisValue::Exact(&high)];
    let mut rows = [row(1, &a, &zero, b"a"), row(2, &b, &zero, b"b")];
    rows[1].hard_gate = ReportedGate::Fail;
    let report =
        select_joint_reported(&policy(&axis), &rows, SearchCoverage::EvaluatedSet).unwrap();
    assert_eq!(report.recommendation, Some(0));
    assert_eq!(report.dispositions[1], Disposition::HardFailure);
}

#[test]
fn tlcx23_interval_overlap_is_unresolved_and_same_bounds_is_not_equivalence() {
    let bounds = RationalInterval::from_ratio(1, 4, 3, 4).unwrap();
    let exact = value(1, 2);
    let axes = [AxisKind::Goal(digest(1))];
    let bounded = [AxisValue::Bounded(&bounds)];
    let measured = [AxisValue::Exact(&exact)];
    assert_eq!(
        compare_rows(&axes, &bounded, &measured),
        Ok(PairRelation::Unresolved)
    );
    assert_eq!(
        compare_rows(&axes, &bounded, &bounded),
        Ok(PairRelation::SameBounds)
    );
    assert_eq!(
        compare_rows(&axes, &measured, &measured),
        Ok(PairRelation::EquivalentInScope)
    );
}

#[test]
fn tlcx23_conservative_dominance_uses_all_endpoint_directions() {
    let (zero, one) = (value(0, 1), value(1, 1));
    let a = [AxisValue::Exact(&one), AxisValue::Exact(&zero)];
    let b = [AxisValue::Exact(&zero), AxisValue::Exact(&one)];
    let axes = [AxisKind::Goal(digest(1)), AxisKind::SemanticLoss];
    assert_eq!(compare_rows(&axes, &a, &b), Ok(PairRelation::LeftDominates));
    assert_eq!(
        compare_rows(&axes, &b, &a),
        Ok(PairRelation::RightDominates)
    );
}

#[test]
fn tlcx23_missing_goal_and_multi_goal_order_do_not_invent_recommendations() {
    let zero = value(0, 1);
    let axes = [AxisKind::Goal(digest(1)), AxisKind::Goal(digest(2))];
    let values = [AxisValue::Exact(&zero), AxisValue::Exact(&zero)];
    let rows = [row(1, &values, &zero, b"a")];
    let mut p = policy(&axes);
    let report = select_joint_reported(&p, &rows, SearchCoverage::EvaluatedSet).unwrap();
    assert_eq!(
        report.recommendation_state,
        RecommendationState::MissingRepresentativePolicy
    );
    p.pins.goal_profile = None;
    let mut no_goal_rows = [row(1, &values, &zero, b"a")];
    no_goal_rows[0].pins = p.pins;
    let report = select_joint_reported(&p, &no_goal_rows, SearchCoverage::EvaluatedSet).unwrap();
    assert_eq!(report.recommendation, None);
    assert_eq!(
        report.recommendation_state,
        RecommendationState::NoGoalProfile
    );
    assert_eq!(report.alternatives, vec![0]);
}

#[test]
fn tlcx23_scope_mapback_and_budget_boundaries_remain_visible() {
    let zero = value(0, 1);
    let axes = [AxisKind::Goal(digest(1))];
    let values = [AxisValue::Exact(&zero)];
    let mut rows = [
        row(1, &values, &zero, b"a"),
        row(2, &values, &zero, b"b"),
        row(3, &values, &zero, b"c"),
    ];
    rows[1].pins.hard_invariants = digest(50);
    rows[2].map_back = ReportedGate::Fail;
    let report =
        select_joint_reported(&policy(&axes), &rows, SearchCoverage::BudgetExhausted).unwrap();
    assert_eq!(report.dispositions[1], Disposition::OtherScope);
    assert_eq!(report.dispositions[2], Disposition::MapBackFailure);
    assert_eq!(report.optimality, SelectionOptimality::BudgetLimited);
    assert_eq!(report.alternatives.len(), 2);
}

#[test]
fn tlcx23_no_top16_truncation_and_id_relabeling_does_not_change_content_winner() {
    let zero = value(0, 1);
    let axes = [AxisKind::Goal(digest(1))];
    let values = [AxisValue::Exact(&zero)];
    let keys: Vec<Vec<u8>> = (0..32).map(|i| vec![b'z' - i / 2, b'a' + i % 2]).collect();
    let rows: Vec<_> = keys
        .iter()
        .enumerate()
        .map(|(i, key)| row(i as u64, &values, &zero, key))
        .collect();
    let p = policy(&axes);
    let report = select_joint_reported(&p, &rows, SearchCoverage::EvaluatedSet).unwrap();
    assert_eq!(report.candidates.len(), 32);
    assert_eq!(report.frontier.len(), 32);
    assert_eq!(report.alternatives.len(), 31);
    let chosen_key = report.candidates[report.recommendation.unwrap()].canonical_key;
    assert_eq!(chosen_key, keys[30].as_slice());
    let relabeled: Vec<_> = keys
        .iter()
        .enumerate()
        .rev()
        .map(|(i, key)| row((100 - i) as u64, &values, &zero, key))
        .collect();
    let report = select_joint_reported(&p, &relabeled, SearchCoverage::EvaluatedSet).unwrap();
    assert_eq!(
        report.candidates[report.recommendation.unwrap()].canonical_key,
        chosen_key
    );
}

#[test]
fn tlcx23_unknown_is_retained_without_becoming_zero() {
    let zero = value(0, 1);
    let axes = [AxisKind::Goal(digest(1))];
    let values = [AxisValue::Unavailable(MissingAxis::Unknown)];
    let rows = [row(1, &values, &zero, b"a")];
    let report =
        select_joint_reported(&policy(&axes), &rows, SearchCoverage::EvaluatedSet).unwrap();
    assert_eq!(report.recommendation, None);
    assert_eq!(report.frontier, vec![0]);
    assert_eq!(report.alternatives, vec![0]);
    assert_eq!(
        report.recommendation_state,
        RecommendationState::NeedsEvidence
    );
}

#[test]
fn tlcx23_aliases_retained_and_ambiguous_lineage_does_not_use_arrival_order() {
    let zero = value(0, 1);
    let axes = [AxisKind::Goal(digest(1))];
    let values = [AxisValue::Exact(&zero)];
    let mut rows = [
        row(1, &values, &zero, b"same"),
        row(2, &values, &zero, b"same"),
    ];
    let report =
        select_joint_reported(&policy(&axes), &rows, SearchCoverage::EvaluatedSet).unwrap();
    assert_eq!(
        report.dispositions[1],
        Disposition::Duplicate { representative: 0 }
    );
    assert_eq!(report.candidates.len(), 2);
    rows[1].measurement_binding = digest(99);
    let report =
        select_joint_reported(&policy(&axes), &rows, SearchCoverage::EvaluatedSet).unwrap();
    assert_eq!(report.recommendation, None);
    assert_eq!(
        report.recommendation_state,
        RecommendationState::NeedsEvidence
    );
}

#[test]
fn tlcx23_invalid_reported_inputs_are_rejected_before_comparison() {
    let bounds = RationalInterval::from_ratio(0, 1, 1, 1).unwrap();
    let axes = [AxisKind::Goal(digest(1))];
    let values = [AxisValue::Exact(&bounds)];
    assert_eq!(
        compare_rows(&axes, &values, &values),
        Err(SelectionError::InexactExactLabel)
    );
    let out = value(2, 1);
    let invalid = [AxisValue::Exact(&out)];
    assert_eq!(
        compare_rows(&axes, &invalid, &invalid),
        Err(SelectionError::OutOfRange)
    );
    let p = policy(&axes);
    assert!(matches!(
        select_joint_reported(&p, &[], SearchCoverage::EvaluatedSet),
        Err(SelectionError::EmptyCandidates)
    ));
}
