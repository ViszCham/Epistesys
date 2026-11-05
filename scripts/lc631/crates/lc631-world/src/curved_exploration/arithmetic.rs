//! Re-export the canonical substrate arithmetic; no independent numeric core.
#![forbid(unsafe_code)]

pub use lc631_core::measurement_numeric::{NumericError, Rational, RationalInterval as Interval};

pub fn zero() -> Rational {
    Rational::from_integer(0.into())
}
pub fn one() -> Rational {
    Rational::from_integer(1.into())
}
pub fn point(value: Rational) -> Result<Interval, NumericError> {
    Interval::new(value.clone(), value)
}
