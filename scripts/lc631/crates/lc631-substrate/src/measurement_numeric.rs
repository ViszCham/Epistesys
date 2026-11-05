//! Canonical finite-formal-v1.4 arithmetic. No semantic truth or authority is produced.
mod interval;
mod operations;
mod transcendental;
pub use interval::{max_enclosure, min_enclosure, RationalInterval};
pub use num_bigint::BigInt;
pub use num_rational::BigRational as Rational;
pub use operations::{
    checked_add, checked_compare, checked_divide, checked_multiply, checked_subtract,
};
pub use transcendental::{exp_neg_enclosure, sqrt_enclosure};
pub const MAX_RATIONAL_BITS: u64 = 65_536;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NumericError {
    DivisionByZero,
    ReversedInterval,
    NumericBudget,
    InvalidInput,
    InvalidPrecision,
    ResourceExhausted,
    NoApplicable,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NumericBudget {
    precision: u32,
}
impl NumericBudget {
    pub fn new(precision: u32) -> Result<Self, NumericError> {
        if !(1..=256).contains(&precision) {
            return Err(NumericError::InvalidPrecision);
        }
        Ok(Self { precision })
    }
    pub const fn precision(self) -> u32 {
        self.precision
    }
}
pub fn check(value: &Rational) -> Result<(), NumericError> {
    if value.numer().bits() > MAX_RATIONAL_BITS || value.denom().bits() > MAX_RATIONAL_BITS {
        return Err(NumericError::NumericBudget);
    }
    let zero = BigInt::from(0);
    let one = BigInt::from(1);
    if value.denom() == &zero {
        return Err(NumericError::DivisionByZero);
    }
    if value.denom() < &zero {
        return Err(NumericError::InvalidInput);
    }
    // Rational::new_raw is public through the alias. Do not assume its
    // denominator/reduction invariants, even before comparison or sqrt.
    let mut a = if value.numer() < &zero {
        -value.numer()
    } else {
        value.numer().clone()
    };
    let mut b = value.denom().clone();
    let mut steps = 0_u32;
    while b != zero {
        if steps == 131_072 {
            return Err(NumericError::ResourceExhausted);
        }
        let remainder = &a % &b;
        a = b;
        b = remainder;
        steps += 1;
    }
    if a != one {
        return Err(NumericError::InvalidInput);
    }
    Ok(())
}
/// A valid outer enclosure may survive exhaustion without becoming completion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NumericEvaluation {
    Complete(RationalInterval),
    ResourceExhausted { partial: Option<RationalInterval> },
}
