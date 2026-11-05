//! Reported-selector policy/cap tests, separate from actual formal owner tests.
use lc631_core::context::{ContextRevision, ContextScope, SourceRevision};
use lc631_core::measurement_numeric::RationalInterval as Interval;
use lc631_core::CandidateId;
use lc631_runtime::joint_selection::*;
use lc631_substrate::Digest32;

fn d(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}
fn pins() -> SelectionPins {
    SelectionPins {
        source: SourceRevision::of("fixed reported boundary suite"),
        scope: ContextScope::declared(d(1), d(2), d(3), d(4)),
        baseline: d(5),
        hard_invariants: d(6),
        policy: d(7),
        goal_profile: Some(d(8)),
        evidence_revision: d(9),
    }
}
fn policy(axes: &[AxisKind]) -> SelectionPolicy<'_> {
    SelectionPolicy {
        pins: pins(),
        axes,
        representative_order: None,
        declared_goal_solve_overlap: false,
        max_candidates: 2,
        max_canonical_bytes: 64,
    }
}
fn row<'a>(id: u64, values: &'a [AxisValue<'a>], loss: &'a Interval) -> JointCandidate<'a> {
    JointCandidate {
        id: CandidateId(id),
        context: ContextRevision::original(pins().source),
        pins: pins(),
        hard_gate: ReportedGate::Pass,
        map_back: ReportedGate::Pass,
        axes: values,
        total_loss: AxisValue::Exact(loss),
        canonical_key: if id == 1 { b"a" } else { b"b" },
        measurement_binding: d(id as u8),
    }
}
fn q(n: i64, den: i64) -> Interval {
    Interval::exact_ratio(n, den).unwrap()
}

#[test]
fn tlcx23_limits_fail_closed_before_frontier_publication() {
    let zero = q(0, 1);
    let axes = [AxisKind::Goal(d(40))];
    let values = [AxisValue::Exact(&zero)];
    let rows = [row(1, &values, &zero), row(2, &values, &zero)];
    for limit in [0, 2049] {
        let mut p = policy(&axes);
        p.max_candidates = limit;
        assert!(matches!(
            select_joint_reported(&p, &rows, SearchCoverage::EvaluatedSet),
            Err(SelectionError::InvalidLimits)
        ));
    }
    let mut p = policy(&axes);
    p.max_candidates = 1;
    assert!(matches!(
        select_joint_reported(&p, &rows, SearchCoverage::EvaluatedSet),
        Err(SelectionError::ResourceExhausted)
    ));
    for limit in [0, 128 * 1024 * 1024 + 1] {
        let mut p = policy(&axes);
        p.max_canonical_bytes = limit;
        assert!(matches!(
            select_joint_reported(&p, &rows, SearchCoverage::EvaluatedSet),
            Err(SelectionError::InvalidLimits)
        ));
    }
    let mut p = policy(&axes);
    p.max_canonical_bytes = 1;
    assert!(matches!(
        select_joint_reported(&p, &rows, SearchCoverage::EvaluatedSet),
        Err(SelectionError::ResourceExhausted)
    ));
    p.max_canonical_bytes = 2;
    assert!(select_joint_reported(&p, &rows, SearchCoverage::EvaluatedSet).is_ok());
    let duplicate_ids = [row(1, &values, &zero), row(1, &values, &zero)];
    assert!(matches!(
        select_joint_reported(&p, &duplicate_ids, SearchCoverage::EvaluatedSet),
        Err(SelectionError::DuplicateCandidateId)
    ));
}

#[test]
fn tlcx23_axis_and_representative_order_caps_preserve_invalid_states() {
    let zero = q(0, 1);
    for count in [0, 65] {
        let axes: Vec<_> = (0..count).map(|i| AxisKind::Goal(d(i as u8))).collect();
        let values: Vec<_> = (0..count).map(|_| AxisValue::Exact(&zero)).collect();
        let rows = [row(1, &values, &zero)];
        assert!(matches!(
            select_joint_reported(&policy(&axes), &rows, SearchCoverage::EvaluatedSet),
            Err(SelectionError::InvalidAxes)
        ));
    }
    let axes = [AxisKind::Goal(d(40)), AxisKind::MigrationCost];
    let values = [AxisValue::Exact(&zero), AxisValue::Exact(&zero)];
    let rows = [row(1, &values, &zero)];
    for order in [&[0][..], &[0, 0][..], &[0, 2][..]] {
        let mut p = policy(&axes);
        p.representative_order = Some(order);
        assert!(matches!(
            select_joint_reported(&p, &rows, SearchCoverage::EvaluatedSet),
            Err(SelectionError::InvalidRepresentativeOrder)
        ));
    }
    let duplicate_axes = [AxisKind::Goal(d(40)), AxisKind::Goal(d(40))];
    assert!(matches!(
        select_joint_reported(
            &policy(&duplicate_axes),
            &rows,
            SearchCoverage::EvaluatedSet
        ),
        Err(SelectionError::InvalidAxes)
    ));
}

#[test]
fn tlcx23_total_loss_representative_is_consumed_without_changing_pareto_axes() {
    let zero = q(0, 1);
    let half = q(1, 2);
    let axes = [AxisKind::Goal(d(40))];
    let values = [AxisValue::Exact(&half)];
    let rows = [row(1, &values, &half), row(2, &values, &zero)];
    let p = policy(&axes);
    let before = select_joint_reported(&p, &rows, SearchCoverage::EvaluatedSet).unwrap();
    assert_eq!(before.frontier, vec![0, 1]);
    assert_eq!(before.recommendation, Some(1));
    let [a, b] = rows;
    let changed = [
        JointCandidate {
            total_loss: AxisValue::Exact(&zero),
            ..a
        },
        JointCandidate {
            total_loss: AxisValue::Exact(&half),
            ..b
        },
    ];
    let after = select_joint_reported(&p, &changed, SearchCoverage::EvaluatedSet).unwrap();
    assert_eq!(after.frontier, vec![0, 1]);
    assert_eq!(after.recommendation, Some(0));
    let [a, b] = changed;
    let cut = [
        JointCandidate {
            total_loss: AxisValue::Unavailable(MissingAxis::Unknown),
            ..a
        },
        b,
    ];
    let after_cut = select_joint_reported(&p, &cut, SearchCoverage::EvaluatedSet).unwrap();
    assert_eq!(after_cut.recommendation, Some(1));
    assert!(after_cut.alternatives.contains(&0));
}

#[test]
fn tlcx23_explicit_multi_goal_representative_order_changes_only_declared_choice() {
    let zero = q(0, 1);
    let one = q(1, 1);
    let axes = [AxisKind::Goal(d(40)), AxisKind::Goal(d(41))];
    let a = [AxisValue::Exact(&one), AxisValue::Exact(&zero)];
    let b = [AxisValue::Exact(&zero), AxisValue::Exact(&one)];
    let rows = [row(1, &a, &zero), row(2, &b, &zero)];
    let mut p = policy(&axes);
    p.representative_order = Some(&[0, 1]);
    let first = select_joint_reported(&p, &rows, SearchCoverage::EvaluatedSet).unwrap();
    assert_eq!(first.frontier, vec![0, 1]);
    assert_eq!(first.recommendation, Some(0));
    p.representative_order = Some(&[1, 0]);
    let second = select_joint_reported(&p, &rows, SearchCoverage::EvaluatedSet).unwrap();
    assert_eq!(second.frontier, vec![0, 1]);
    assert_eq!(second.recommendation, Some(1));
    let mut relabeled = rows;
    relabeled[0].id = CandidateId(99);
    relabeled[1].id = CandidateId(77);
    let third = select_joint_reported(&p, &relabeled, SearchCoverage::EvaluatedSet).unwrap();
    assert_eq!(third.recommendation, Some(1));
}
