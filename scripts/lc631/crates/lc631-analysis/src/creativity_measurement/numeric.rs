//! Reuse canonical substrate arithmetic; no analysis-local numeric implementation.
use super::MeasurementError;
use lc631_substrate::measurement_numeric::NumericError;
pub use lc631_substrate::measurement_numeric::{Rational, RationalInterval, MAX_RATIONAL_BITS};
pub fn check(value: &Rational) -> Result<(), MeasurementError> {
    lc631_substrate::measurement_numeric::check(value).map_err(MeasurementError::from)
}
impl From<NumericError> for MeasurementError {
    fn from(error: NumericError) -> Self {
        match error {
            NumericError::DivisionByZero => Self::DivisionByZero,
            NumericError::ReversedInterval => Self::ReversedInterval,
            NumericError::NumericBudget | NumericError::ResourceExhausted => Self::NumericBudget,
            NumericError::InvalidInput | NumericError::InvalidPrecision => Self::InvalidInput,
            NumericError::NoApplicable => Self::EmptyDenominator,
        }
    }
}
