use lc631_substrate::reported::{Measurement, ObservationGap};

#[test]
fn tlcx_01_unobserved_measurement_is_not_a_known_zero() {
    let unknown: Measurement<u64> = Measurement::Unobserved(ObservationGap::MissingEvaluator);
    assert_eq!(unknown.known_value(), None);
    assert_eq!(Measurement::Known(0_u64).known_value(), Some(&0));
    assert_eq!(Measurement::<u64>::Incomparable.known_value(), None);
    assert_eq!(Measurement::<u64>::NotApplicable.known_value(), None);
}
