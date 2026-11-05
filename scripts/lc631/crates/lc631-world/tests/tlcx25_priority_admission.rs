//! Safe raw Rational values must be rejected before any comparison/arithmetic.
use lc631_core::measurement_numeric::{BigInt, Rational};
use lc631_world::exploration_schedule::priority::{checked_compare, UnitInterval};

fn raw(numerator: i64, denominator: i64) -> Rational {
    Rational::new_raw(BigInt::from(numerator), BigInt::from(denominator))
}

#[test]
fn tlcx25_zero_denominator_is_an_error_not_a_scheduler_panic() {
    assert!(UnitInterval::checked(raw(1, 0), raw(1, 1)).is_err());
}

#[test]
fn tlcx25_negative_denominator_and_unreduced_values_are_not_canonical_inputs() {
    for value in [raw(1, -2), raw(2, 4), raw(0, 2)] {
        assert!(UnitInterval::checked(value, raw(1, 1)).is_err());
    }
}

#[test]
fn tlcx25_public_comparator_rejects_raw_invalid_rationals_in_either_position() {
    for value in [raw(1, 0), raw(1, -2), raw(2, 4)] {
        assert!(checked_compare(&value, &raw(1, 1)).is_err());
        assert!(checked_compare(&raw(0, 1), &value).is_err());
    }
}

#[test]
fn tlcx25_numeric_budget_is_checked_before_large_fraction_comparison() {
    let large = Rational::new_raw(BigInt::from(1) << 65_536, BigInt::from(1));
    assert!(UnitInterval::checked(large.clone(), raw(1, 1)).is_err());
    assert!(checked_compare(&raw(0, 1), &large).is_err());
    let half = Rational::new(BigInt::from(2), BigInt::from(4));
    assert!(UnitInterval::checked(half.clone(), raw(1, 1)).is_ok());
    assert_eq!(
        checked_compare(&half, &raw(1, 1)).unwrap(),
        std::cmp::Ordering::Less
    );
}
