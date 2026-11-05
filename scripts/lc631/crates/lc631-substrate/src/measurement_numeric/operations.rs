//! Conservative pre-allocation bit admission. Algebraic cancellation may fit
//! after reduction, but is never assumed to justify an oversized intermediate.
use super::{check, NumericError, Rational, MAX_RATIONAL_BITS};
use std::cmp::Ordering;

fn sum_bits(a: u64, b: u64) -> Result<u64, NumericError> {
    let bits = a.checked_add(b).ok_or(NumericError::NumericBudget)?;
    if bits > MAX_RATIONAL_BITS {
        return Err(NumericError::NumericBudget);
    }
    Ok(bits)
}
pub fn checked_compare(a: &Rational, b: &Rational) -> Result<Ordering, NumericError> {
    check(a)?;
    check(b)?;
    sum_bits(a.numer().bits(), b.denom().bits())?;
    sum_bits(b.numer().bits(), a.denom().bits())?;
    Ok(a.cmp(b))
}
fn admit_sum(a: &Rational, b: &Rational) -> Result<(), NumericError> {
    check(a)?;
    check(b)?;
    let l = sum_bits(a.numer().bits(), b.denom().bits())?;
    let r = sum_bits(b.numer().bits(), a.denom().bits())?;
    sum_bits(l.max(r), 1)?;
    sum_bits(a.denom().bits(), b.denom().bits())?;
    Ok(())
}
pub fn checked_add(a: &Rational, b: &Rational) -> Result<Rational, NumericError> {
    admit_sum(a, b)?;
    let result = a + b;
    check(&result)?;
    Ok(result)
}
pub fn checked_subtract(a: &Rational, b: &Rational) -> Result<Rational, NumericError> {
    admit_sum(a, b)?;
    let result = a - b;
    check(&result)?;
    Ok(result)
}
pub fn checked_multiply(a: &Rational, b: &Rational) -> Result<Rational, NumericError> {
    check(a)?;
    check(b)?;
    sum_bits(a.numer().bits(), b.numer().bits())?;
    sum_bits(a.denom().bits(), b.denom().bits())?;
    let result = a * b;
    check(&result)?;
    Ok(result)
}
pub fn checked_divide(a: &Rational, b: &Rational) -> Result<Rational, NumericError> {
    check(a)?;
    check(b)?;
    if b.numer().bits() == 0 {
        return Err(NumericError::DivisionByZero);
    }
    sum_bits(a.numer().bits(), b.denom().bits())?;
    sum_bits(a.denom().bits(), b.numer().bits())?;
    let result = a / b;
    check(&result)?;
    Ok(result)
}
