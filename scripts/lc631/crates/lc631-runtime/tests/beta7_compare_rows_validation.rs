//! 未知値より後の不正値も拒否する、公開比較APIの合成数値fixture。
//! 意味的正しさ、solver実行、GPU性能やoccupancyの証拠にはしない。
//! Synthetic numeric fixtures for public comparison after unavailable values.
//! They establish no semantic truth, solver execution, GPU performance or occupancy.

use lc631_analysis::creativity_measurement::RationalInterval;
use lc631_runtime::joint_selection::{
    compare_rows, AxisKind, AxisValue, MissingAxis, PairRelation, SelectionError,
};
use lc631_substrate::Digest32;

fn value(numerator: i64, denominator: i64) -> RationalInterval {
    RationalInterval::exact_ratio(numerator, denominator).unwrap()
}

fn goal_value(missing: bool, known: &RationalInterval) -> AxisValue<'_> {
    if missing {
        AxisValue::Unavailable(MissingAxis::Unknown)
    } else {
        AxisValue::Exact(known)
    }
}

fn compare_unknown_pair(
    unknown_first: bool,
    reverse_rows: bool,
    missing: (bool, bool),
    left_loss: &RationalInterval,
    right_loss: &RationalInterval,
) -> Result<PairRelation, SelectionError> {
    let zero = value(0, 1);
    let one = value(1, 1);
    let goal = AxisKind::Goal(Digest32::from_bytes([1; 32]));
    let axes = if unknown_first {
        [goal, AxisKind::SemanticLoss]
    } else {
        [AxisKind::SemanticLoss, goal]
    };
    let left_goal = goal_value(missing.0, &one);
    let right_goal = goal_value(missing.1, &zero);
    let left_loss = AxisValue::Exact(left_loss);
    let right_loss = AxisValue::Exact(right_loss);
    let left = if unknown_first {
        [left_goal, left_loss]
    } else {
        [left_loss, left_goal]
    };
    let right = if unknown_first {
        [right_goal, right_loss]
    } else {
        [right_loss, right_goal]
    };
    if reverse_rows {
        compare_rows(&axes, &right, &left)
    } else {
        compare_rows(&axes, &left, &right)
    }
}

fn check_invalid_tail(invalid: &RationalInterval, expected: SelectionError) {
    let zero = value(0, 1);
    for reverse_rows in [false, true] {
        for missing in [(true, true), (true, false), (false, true)] {
            for unknown_first in [false, true] {
                assert_eq!(
                    compare_unknown_pair(unknown_first, reverse_rows, missing, invalid, &zero),
                    Err(expected),
                    "unknown_first={unknown_first}, reverse_rows={reverse_rows}, missing={missing:?}"
                );
            }
        }
    }
}

#[test]
fn unknown_prefix_does_not_hide_out_of_range_axis() {
    let invalid = value(2, 1);
    check_invalid_tail(&invalid, SelectionError::OutOfRange);
}

#[test]
fn unknown_prefix_does_not_hide_inexact_exact_label() {
    let nonpoint = RationalInterval::from_ratio(0, 1, 1, 1).unwrap();
    check_invalid_tail(&nonpoint, SelectionError::InexactExactLabel);
}

#[test]
fn valid_unknown_axes_stay_unresolved_in_both_orders() {
    let zero = value(0, 1);
    let one = value(1, 1);
    for reverse_rows in [false, true] {
        for missing in [(true, true), (true, false), (false, true)] {
            for unknown_first in [false, true] {
                assert_eq!(
                    compare_unknown_pair(unknown_first, reverse_rows, missing, &zero, &one),
                    Ok(PairRelation::Unresolved),
                    "unknown_first={unknown_first}, reverse_rows={reverse_rows}, missing={missing:?}"
                );
            }
        }
    }

    let goal = AxisKind::Goal(Digest32::from_bytes([1; 32]));
    for goal_first in [false, true] {
        let axes = if goal_first {
            [goal, AxisKind::SemanticLoss]
        } else {
            [AxisKind::SemanticLoss, goal]
        };
        let preferred = if goal_first {
            [AxisValue::Exact(&one), AxisValue::Exact(&zero)]
        } else {
            [AxisValue::Exact(&zero), AxisValue::Exact(&one)]
        };
        let other = if goal_first {
            [AxisValue::Exact(&zero), AxisValue::Exact(&one)]
        } else {
            [AxisValue::Exact(&one), AxisValue::Exact(&zero)]
        };
        assert_eq!(
            compare_rows(&axes, &preferred, &other),
            Ok(PairRelation::LeftDominates)
        );
        assert_eq!(
            compare_rows(&axes, &other, &preferred),
            Ok(PairRelation::RightDominates)
        );
        assert_eq!(
            compare_rows(&axes, &preferred, &preferred),
            Ok(PairRelation::EquivalentInScope)
        );
    }

    let bounds = RationalInterval::from_ratio(1, 4, 3, 4).unwrap();
    let half = value(1, 2);
    let axes = [goal];
    let bounded = [AxisValue::Bounded(&bounds)];
    let estimated = [AxisValue::Estimated(&bounds)];
    let measured = [AxisValue::Exact(&half)];
    assert_eq!(
        compare_rows(&axes, &bounded, &measured),
        Ok(PairRelation::Unresolved)
    );
    assert_eq!(
        compare_rows(&axes, &bounded, &bounded),
        Ok(PairRelation::SameBounds)
    );
    assert_eq!(
        compare_rows(&axes, &estimated, &estimated),
        Ok(PairRelation::SameBounds)
    );
    assert_eq!(
        compare_rows(&axes, &measured, &measured),
        Ok(PairRelation::EquivalentInScope)
    );

    let negative_one = value(-1, 1);
    let gains = [AxisKind::CoverageGain];
    let negative = [AxisValue::Exact(&negative_one)];
    let neutral = [AxisValue::Exact(&zero)];
    assert_eq!(
        compare_rows(&gains, &negative, &neutral),
        Ok(PairRelation::RightDominates)
    );
    assert_eq!(
        compare_rows(&gains, &neutral, &negative),
        Ok(PairRelation::LeftDominates)
    );
}
