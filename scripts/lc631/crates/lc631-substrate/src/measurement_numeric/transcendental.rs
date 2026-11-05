//! Rational K7 enclosures. Resource exhaustion is separate from completed arithmetic.
use super::{
    check, checked_add, checked_compare, checked_divide, checked_multiply, checked_subtract,
    NumericBudget, NumericError, NumericEvaluation, Rational, RationalInterval, MAX_RATIONAL_BITS,
};
use num_bigint::BigInt;

fn integer(n: i64) -> Rational {
    Rational::from_integer(BigInt::from(n))
}
fn pow2(p: u32) -> BigInt {
    BigInt::from(1) << p
}
fn isqrt(n: &BigInt) -> BigInt {
    if n <= &BigInt::from(1) {
        return n.clone();
    }
    let mut x = BigInt::from(1) << n.bits().div_ceil(2);
    loop {
        let next = (&x + n / &x) >> 1;
        if next >= x {
            return x;
        }
        x = next;
    }
}

pub fn sqrt_enclosure(
    value: &Rational,
    budget: NumericBudget,
) -> Result<NumericEvaluation, NumericError> {
    check(value)?;
    if value < &integer(0) {
        return Err(NumericError::InvalidInput);
    }
    let nr = isqrt(value.numer());
    let dr = isqrt(value.denom());
    if &nr * &nr == *value.numer() && &dr * &dr == *value.denom() {
        let exact = Rational::new(nr, dr);
        return Ok(NumericEvaluation::Complete(RationalInterval::new(
            exact.clone(),
            exact,
        )?));
    }
    let p = budget.precision();
    if value.numer().bits() + u64::from(2 * p) > MAX_RATIONAL_BITS {
        return Err(NumericError::NumericBudget);
    }
    let scaled = (value.numer() << (2 * p)) / value.denom();
    let q = isqrt(&scaled);
    let grid = pow2(p);
    let lo = Rational::new(q.clone(), grid.clone());
    let hi = Rational::new(q + 1, grid);
    Ok(NumericEvaluation::Complete(RationalInterval::new(lo, hi)?))
}

fn floor_grid(value: &Rational, p: u32) -> Result<Rational, NumericError> {
    if value < &integer(0) {
        return Err(NumericError::InvalidInput);
    }
    if (value.numer().bits() + u64::from(p)).max(value.denom().bits()) + 1 > MAX_RATIONAL_BITS {
        return Err(NumericError::NumericBudget);
    }
    let grid = pow2(p);
    let q = (value.numer() * &grid) / value.denom();
    let result = Rational::new(q, grid);
    check(&result)?;
    Ok(result)
}
fn ceil_grid(value: &Rational, p: u32) -> Result<Rational, NumericError> {
    if value < &integer(0) {
        return Err(NumericError::InvalidInput);
    }
    if (value.numer().bits() + u64::from(p)).max(value.denom().bits()) + 1 > MAX_RATIONAL_BITS {
        return Err(NumericError::NumericBudget);
    }
    let grid = pow2(p);
    let q = (value.numer() * &grid + value.denom() - 1) / value.denom();
    let result = Rational::new(q, grid);
    check(&result)?;
    Ok(result)
}

fn square_grid(
    lo: &Rational,
    hi: &Rational,
    p: u32,
    r: u32,
) -> Result<RationalInterval, NumericError> {
    let mut lower = floor_grid(lo, p)?;
    let mut upper = ceil_grid(hi, p)?;
    for _ in 0..r {
        let l2 = checked_multiply(&lower, &lower)?;
        let h2 = checked_multiply(&upper, &upper)?;
        check(&l2)?;
        check(&h2)?;
        lower = floor_grid(&l2, p)?;
        upper = ceil_grid(&h2, p)?;
    }
    RationalInterval::new(lower, upper)
}

pub fn exp_neg_enclosure(
    value: &Rational,
    budget: NumericBudget,
) -> Result<NumericEvaluation, NumericError> {
    check(value)?;
    if value < &integer(0) {
        return Err(NumericError::InvalidInput);
    }
    if value == &integer(0) {
        return Ok(NumericEvaluation::Complete(RationalInterval::exact_ratio(
            1, 1,
        )?));
    }
    let mut y = value.clone();
    let mut r = 0_u32;
    let half = Rational::new(BigInt::from(1), BigInt::from(2));
    while y > half {
        if r == 64 {
            return Ok(NumericEvaluation::ResourceExhausted {
                partial: Some(RationalInterval::from_ratio(0, 1, 1, 1)?),
            });
        }
        y = checked_divide(&y, &integer(2))?;
        r += 1;
    }
    let p = budget.precision();
    let internal = p + r + 16;
    let target = Rational::new(BigInt::from(1), pow2(internal));
    let final_target = Rational::new(BigInt::from(1), pow2(p));
    let mut sum = integer(1);
    let mut term = integer(1);
    let mut lower = integer(0);
    let mut upper = integer(1);
    let mut converged = false;
    for k in 1..=256_i64 {
        term = checked_divide(&checked_multiply(&term, &y)?, &integer(k))?;
        if k % 2 == 1 {
            sum = checked_subtract(&sum, &term)?;
            lower = sum.clone();
        } else {
            sum = checked_add(&sum, &term)?;
            upper = sum.clone();
        }
        check(&sum)?;
        if !checked_compare(&checked_subtract(&upper, &lower)?, &target)?.is_gt() {
            converged = true;
            break;
        }
    }
    let mut partial = None;
    for retry in 0..=3 {
        let bound = square_grid(&lower, &upper, internal + retry * 32, r)?;
        if converged
            && !checked_compare(
                &checked_subtract(bound.upper(), bound.lower())?,
                &final_target,
            )?
            .is_gt()
        {
            return Ok(NumericEvaluation::Complete(bound));
        }
        partial = Some(bound);
    }
    Ok(NumericEvaluation::ResourceExhausted { partial })
}
