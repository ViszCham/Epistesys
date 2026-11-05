use lc631_substrate::measurement_numeric::{
    exp_neg_enclosure, sqrt_enclosure, NumericBudget, NumericError, NumericEvaluation, Rational,
    RationalInterval,
};
fn interval(a: i64, b: i64, c: i64, d: i64) -> RationalInterval {
    RationalInterval::from_ratio(a, b, c, d).unwrap()
}
fn complete(value: NumericEvaluation) -> RationalInterval {
    match value {
        NumericEvaluation::Complete(v) => v,
        NumericEvaluation::ResourceExhausted { .. } => panic!("numeric endpoint did not converge"),
    }
}
#[test]
fn tlcx22_k4_g17_product_and_zero_division() {
    let a = interval(-2, 1, 3, 1);
    let b = interval(-4, 1, 5, 1);
    assert_eq!(a.multiply(&b).unwrap(), interval(-12, 1, 15, 1));
    assert_eq!(a.divide(&b), Err(NumericError::DivisionByZero));
}
#[test]
fn tlcx22_k4_g34_dependent_square_has_nonnegative_lower_bound() {
    let a = interval(-1, 10, 1, 10);
    assert_eq!(a.square().unwrap(), interval(0, 1, 1, 100));
    assert_eq!(a.multiply(&a).unwrap(), interval(-1, 100, 1, 100));
}
#[test]
fn tlcx22_k7_square_root_exact_and_squared_outer_bounds() {
    let budget = NumericBudget::new(64).unwrap();
    let exact = complete(sqrt_enclosure(interval(9, 16, 9, 16).lower(), budget).unwrap());
    assert_eq!(exact, interval(3, 4, 3, 4));
    let half = interval(1, 2, 1, 2);
    let bounds = complete(sqrt_enclosure(half.lower(), budget).unwrap());
    assert!(!bounds.is_exact());
    assert!(bounds.lower() * bounds.lower() <= *half.lower());
    assert!(bounds.upper() * bounds.upper() >= *half.upper());
    assert!(sqrt_enclosure(interval(-1, 1, -1, 1).lower(), budget).is_err());
}
#[test]
fn tlcx22_k7_exp_one_encloses_independent_decimal_golden() {
    let budget = NumericBudget::new(64).unwrap();
    let bounds = complete(exp_neg_enclosure(interval(1, 1, 1, 1).lower(), budget).unwrap());
    // Decimal digits from the fixed contract G07, not from the production evaluator.
    let lo = Rational::new(
        "3678794411714423215955237701614608674458".parse().unwrap(),
        "10000000000000000000000000000000000000000".parse().unwrap(),
    );
    let hi = Rational::new(
        "3678794411714423215955237701614608674459".parse().unwrap(),
        "10000000000000000000000000000000000000000".parse().unwrap(),
    );
    assert!(bounds.lower() <= &lo && bounds.upper() >= &hi);
    let width = bounds.upper() - bounds.lower();
    let p = Rational::new(1.into(), (num_bigint::BigInt::from(1)) << 64);
    assert!(width <= p);
    assert!(!bounds.is_exact());
}
#[test]
fn tlcx22_numeric_precision_and_invalid_domain_remain_errors() {
    assert_eq!(NumericBudget::new(0), Err(NumericError::InvalidPrecision));
    assert_eq!(NumericBudget::new(257), Err(NumericError::InvalidPrecision));
    assert!(RationalInterval::from_ratio(0, 0, 1, 1).is_err());
    assert!(RationalInterval::from_ratio(2, 1, 1, 1).is_err());
    assert!(interval(-1, 1, 0, 1).require_unit().is_err());
}
#[test]
fn tlcx22_exp_range_reduction_cap_is_not_underflow_zero() {
    let huge = Rational::from_integer(num_bigint::BigInt::from(1) << 65);
    assert!(
        matches!(exp_neg_enclosure(&huge, NumericBudget::new(64).unwrap()).unwrap(),
        NumericEvaluation::ResourceExhausted { partial: Some(bound) } if bound == interval(0,1,1,1))
    );
}

#[test]
fn tlcx22_k4_g28_family_bounds_and_empty_are_distinct() {
    use lc631_substrate::measurement_numeric::{max_enclosure, min_enclosure};
    let family = [interval(0, 1, 10, 1), interval(5, 1, 6, 1)];
    assert_eq!(min_enclosure(&family).unwrap(), interval(0, 1, 6, 1));
    assert_eq!(max_enclosure(&family).unwrap(), interval(5, 1, 10, 1));
    assert_eq!(min_enclosure(&[]), Err(NumericError::NoApplicable));
}

#[test]
fn tlcx22_large_product_is_rejected_by_preallocation_bit_admission() {
    let large = Rational::from_integer(num_bigint::BigInt::from(1) << 33_000);
    let value = RationalInterval::new(large.clone(), large).unwrap();
    assert_eq!(value.square(), Err(NumericError::NumericBudget));
}

#[test]
fn tlcx22_raw_rational_zero_negative_and_unreduced_denominators_are_rejected() {
    use lc631_substrate::measurement_numeric::check;
    let zero_denominator = Rational::new_raw(1.into(), 0.into());
    let negative_denominator = Rational::new_raw(1.into(), (-2).into());
    let unreduced = Rational::new_raw(2.into(), 4.into());
    assert_eq!(check(&zero_denominator), Err(NumericError::DivisionByZero));
    assert_eq!(
        check(&negative_denominator),
        Err(NumericError::InvalidInput)
    );
    assert_eq!(check(&unreduced), Err(NumericError::InvalidInput));
    let budget = NumericBudget::new(64).unwrap();
    assert_eq!(
        sqrt_enclosure(&zero_denominator, budget),
        Err(NumericError::DivisionByZero)
    );
    assert_eq!(
        exp_neg_enclosure(&negative_denominator, budget),
        Err(NumericError::InvalidInput)
    );
    assert_eq!(
        RationalInterval::new(unreduced.clone(), unreduced),
        Err(NumericError::InvalidInput)
    );
}
