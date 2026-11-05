//! Fixed decimal reference brackets are independent of the production rational algorithm.
use lc631_world::curved_exploration;

use curved_exploration::arithmetic::{Interval, Rational};
use curved_exploration::k8::{evaluate, K8Budget};
use curved_exploration::{prepare_poincare, BallPolicy};
use lc631_core::measurement_numeric::{BigInt, NumericError, NumericEvaluation};

fn q(n: i64, d: i64) -> Rational {
    Interval::exact_ratio(n, d).unwrap().lower().clone()
}
fn point(n: i64, d: i64) -> Interval {
    Interval::exact_ratio(n, d).unwrap()
}
fn policy() -> BallPolicy {
    BallPolicy::new(q(1, 1), q(1, 4), q(1, 1), 8).unwrap()
}
fn decimal(digits: &str, scale: u32) -> Rational {
    Rational::new(
        digits.parse::<BigInt>().unwrap(),
        BigInt::from(10).pow(scale),
    )
}
fn completed(evaluation: NumericEvaluation) -> Interval {
    match evaluation {
        NumericEvaluation::Complete(bounds) => bounds,
        NumericEvaluation::ResourceExhausted { partial } => {
            panic!("unexpected exhaustion: {partial:?}")
        }
    }
}
fn reference(bounds: &Interval, lower: &str, upper: &str, scale: u32) {
    assert!(bounds.lower() <= &decimal(lower, scale));
    assert!(bounds.upper() >= &decimal(upper, scale));
}

#[test]
fn tlcx24_k8_g21_log_three_outward_decimal_reference() {
    let prepared = prepare_poincare(&policy(), &[point(0, 1)], &[point(1, 2)]).unwrap();
    let bounds = completed(prepared.distance().unwrap());
    reference(
        &bounds,
        "1098612288668109691395245236922",
        "1098612288668109691395245236923",
        30,
    );
    let tolerance = Rational::new(BigInt::from(1), BigInt::from(1) << 64);
    assert!(bounds.upper() - bounds.lower() <= tolerance);
}

#[test]
fn tlcx24_k8_g32_irrational_q_endpoint_slack_is_independent_of_input_width() {
    let prepared = prepare_poincare(
        &policy(),
        &[point(1, 2), point(0, 1)],
        &[point(0, 1), point(1, 2)],
    )
    .unwrap();
    assert_eq!(prepared.t_enclosure(), &point(8, 9));
    // t/(1+t)=8/17; unlike G21, q is irrational.
    assert_eq!(q(8, 9) / (q(1, 1) + q(8, 9)), q(8, 17));
    let report = evaluate(prepared.t_enclosure(), &q(1, 1), K8Budget::default()).unwrap();
    assert!(report.lower_endpoint_precision >= 80 && report.upper_endpoint_precision >= 80);
    let tolerance = Rational::new(BigInt::from(1), BigInt::from(1) << 64);
    assert!(report.lower_endpoint_slack.unwrap() <= tolerance);
    assert!(report.upper_endpoint_slack.unwrap() <= tolerance);
    reference(
        &completed(report.evaluation),
        "1680699772428003564463509249407",
        "1680699772428003564463509249408",
        30,
    );
}

#[test]
fn tlcx24_k8_g33_wide_input_is_not_a_numeric_convergence_failure() {
    let input = [Interval::from_ratio(0, 1, 1, 10).unwrap()];
    let prepared = prepare_poincare(&policy(), &input, &[point(1, 2)]).unwrap();
    let report = evaluate(
        prepared.t_enclosure(),
        prepared.curvature(),
        K8Budget::default(),
    )
    .unwrap();
    let tolerance = Rational::new(BigInt::from(1), BigInt::from(1) << 64);
    assert!(report.lower_endpoint_slack.unwrap() <= tolerance);
    assert!(report.upper_endpoint_slack.unwrap() <= tolerance);
    let bounds = completed(report.evaluation);
    // Exact coordinate-box distance spans ln(27/11)..ln3. K4 may enclose more widely.
    assert!(bounds.lower() <= &decimal("897941593205958530123792132802", 30));
    assert!(bounds.upper() >= &decimal("1098612288668109691395245236923", 30));
    assert!(bounds.upper() - bounds.lower() > q(1, 5));
}

#[test]
fn tlcx24_k8_budget_exhaustion_preserves_a_sound_finite_outer_bound() {
    let report = evaluate(&point(8, 9), &q(1, 1), K8Budget::new(80, 0).unwrap()).unwrap();
    match report.evaluation {
        NumericEvaluation::ResourceExhausted {
            partial: Some(bounds),
        } => {
            reference(
                &bounds,
                "1680699772428003564463509249407",
                "1680699772428003564463509249408",
                30,
            );
        }
        other => panic!("expected retained exhaustion, got {other:?}"),
    }
}

#[test]
fn tlcx24_k8_q_upper_one_and_tiny_curvature_never_become_infinity_or_zero_success() {
    let huge = Rational::from_integer(BigInt::from(1) << 600);
    let interval = Interval::new(huge.clone(), huge).unwrap();
    assert_eq!(
        evaluate(&interval, &q(1, 1), K8Budget::default())
            .unwrap()
            .evaluation,
        NumericEvaluation::ResourceExhausted { partial: None }
    );
    let tiny = Rational::new(BigInt::from(1), BigInt::from(3) << 600);
    assert_eq!(
        evaluate(&point(1, 3), &tiny, K8Budget::default())
            .unwrap()
            .evaluation,
        NumericEvaluation::ResourceExhausted { partial: None }
    );
}

#[test]
fn tlcx24_k8_symmetry_self_and_negative_inputs() {
    let a = [point(1, 4), point(0, 1)];
    let b = [point(0, 1), point(1, 2)];
    let ab = prepare_poincare(&policy(), &a, &b)
        .unwrap()
        .distance()
        .unwrap();
    let ba = prepare_poincare(&policy(), &b, &a)
        .unwrap()
        .distance()
        .unwrap();
    assert_eq!(ab, ba);
    assert_eq!(
        completed(
            prepare_poincare(&policy(), &a, &a)
                .unwrap()
                .distance()
                .unwrap()
        ),
        point(0, 1)
    );
    assert_eq!(
        evaluate(&point(-1, 1), &q(1, 1), K8Budget::default()),
        Err(NumericError::InvalidInput)
    );
    assert_eq!(
        evaluate(&point(1, 1), &q(0, 1), K8Budget::default()),
        Err(NumericError::InvalidInput)
    );
    assert_eq!(K8Budget::new(64, 256), Err(NumericError::InvalidPrecision));
}
