use super::{
    check, checked_add, checked_compare, checked_divide, checked_multiply, checked_subtract,
    NumericError, Rational,
};
use num_bigint::BigInt;
/// Arithmetic precision only; equal endpoints do not certify observed semantics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RationalInterval {
    lower: Rational,
    upper: Rational,
}
impl RationalInterval {
    pub fn new(lower: Rational, upper: Rational) -> Result<Self, NumericError> {
        check(&lower)?;
        check(&upper)?;
        if checked_compare(&lower, &upper)?.is_gt() {
            return Err(NumericError::ReversedInterval);
        }
        Ok(Self { lower, upper })
    }
    pub fn from_ratio(ln: i64, ld: i64, un: i64, ud: i64) -> Result<Self, NumericError> {
        if ld <= 0 || ud <= 0 {
            return Err(NumericError::DivisionByZero);
        }
        Self::new(
            Rational::new(BigInt::from(ln), BigInt::from(ld)),
            Rational::new(BigInt::from(un), BigInt::from(ud)),
        )
    }
    pub fn exact_ratio(n: i64, d: i64) -> Result<Self, NumericError> {
        Self::from_ratio(n, d, n, d)
    }
    pub fn lower(&self) -> &Rational {
        &self.lower
    }
    pub fn upper(&self) -> &Rational {
        &self.upper
    }
    pub fn is_exact(&self) -> bool {
        self.lower == self.upper
    }
    pub fn add(&self, rhs: &Self) -> Result<Self, NumericError> {
        Self::new(
            checked_add(&self.lower, &rhs.lower)?,
            checked_add(&self.upper, &rhs.upper)?,
        )
    }
    pub fn subtract(&self, rhs: &Self) -> Result<Self, NumericError> {
        Self::new(
            checked_subtract(&self.lower, &rhs.upper)?,
            checked_subtract(&self.upper, &rhs.lower)?,
        )
    }
    pub fn multiply(&self, rhs: &Self) -> Result<Self, NumericError> {
        let products = [
            checked_multiply(&self.lower, &rhs.lower)?,
            checked_multiply(&self.lower, &rhs.upper)?,
            checked_multiply(&self.upper, &rhs.lower)?,
            checked_multiply(&self.upper, &rhs.upper)?,
        ];
        for p in &products {
            check(p)?;
        }
        let mut lo = products[0].clone();
        let mut hi = lo.clone();
        for p in &products[1..] {
            if checked_compare(p, &lo)?.is_lt() {
                lo = p.clone();
            }
            if checked_compare(p, &hi)?.is_gt() {
                hi = p.clone();
            }
        }
        Self::new(lo, hi)
    }
    pub fn square(&self) -> Result<Self, NumericError> {
        let a = checked_multiply(&self.lower, &self.lower)?;
        let b = checked_multiply(&self.upper, &self.upper)?;
        let zero = Rational::from_integer(BigInt::from(0));
        let a_le_b = !checked_compare(&a, &b)?.is_gt();
        let lo = if self.lower <= zero && self.upper >= zero {
            zero
        } else if a_le_b {
            a.clone()
        } else {
            b.clone()
        };
        let hi = if a_le_b { b } else { a };
        Self::new(lo, hi)
    }
    pub fn divide(&self, rhs: &Self) -> Result<Self, NumericError> {
        let zero = Rational::from_integer(BigInt::from(0));
        if rhs.lower <= zero && rhs.upper >= zero {
            return Err(NumericError::DivisionByZero);
        }
        let one = Rational::from_integer(BigInt::from(1));
        self.multiply(&Self::new(
            checked_divide(&one, &rhs.upper)?,
            checked_divide(&one, &rhs.lower)?,
        )?)
    }
    pub fn abs(&self) -> Result<Self, NumericError> {
        let zero = Rational::from_integer(BigInt::from(0));
        if self.lower >= zero {
            return Ok(self.clone());
        }
        if self.upper <= zero {
            return Self::new(-&self.upper, -&self.lower);
        }
        let left = -&self.lower;
        let hi = if checked_compare(&left, &self.upper)?.is_gt() {
            left
        } else {
            self.upper.clone()
        };
        Self::new(zero, hi)
    }
    pub fn negate(&self) -> Result<Self, NumericError> {
        Self::new(-&self.upper, -&self.lower)
    }
    pub fn scale_positive(&self, weight: &Rational) -> Result<Self, NumericError> {
        check(weight)?;
        if weight <= &Rational::from_integer(BigInt::from(0)) {
            return Err(NumericError::InvalidInput);
        }
        Self::new(
            checked_multiply(&self.lower, weight)?,
            checked_multiply(&self.upper, weight)?,
        )
    }
    pub fn divide_positive(&self, weight: &Rational) -> Result<Self, NumericError> {
        check(weight)?;
        if weight <= &Rational::from_integer(BigInt::from(0)) {
            return Err(NumericError::DivisionByZero);
        }
        Self::new(
            checked_divide(&self.lower, weight)?,
            checked_divide(&self.upper, weight)?,
        )
    }
    pub fn require_unit(&self) -> Result<(), NumericError> {
        let zero = Rational::from_integer(BigInt::from(0));
        let one = Rational::from_integer(BigInt::from(1));
        if self.lower < zero || self.upper > one {
            return Err(NumericError::InvalidInput);
        }
        Ok(())
    }
}

pub fn min_enclosure(values: &[RationalInterval]) -> Result<RationalInterval, NumericError> {
    let first = values.first().ok_or(NumericError::NoApplicable)?;
    if values.len() > 100_000 {
        return Err(NumericError::NumericBudget);
    }
    let mut lo = first.lower.clone();
    let mut hi = first.upper.clone();
    for v in &values[1..] {
        if checked_compare(&v.lower, &lo)?.is_lt() {
            lo = v.lower.clone();
        }
        if checked_compare(&v.upper, &hi)?.is_lt() {
            hi = v.upper.clone();
        }
    }
    RationalInterval::new(lo, hi)
}

pub fn max_enclosure(values: &[RationalInterval]) -> Result<RationalInterval, NumericError> {
    let first = values.first().ok_or(NumericError::NoApplicable)?;
    if values.len() > 100_000 {
        return Err(NumericError::NumericBudget);
    }
    let mut lo = first.lower.clone();
    let mut hi = first.upper.clone();
    for v in &values[1..] {
        if checked_compare(&v.lower, &lo)?.is_gt() {
            lo = v.lower.clone();
        }
        if checked_compare(&v.upper, &hi)?.is_gt() {
            hi = v.upper.clone();
        }
    }
    RationalInterval::new(lo, hi)
}
