//! Preflight the SMALLER root bit cap BEFORE existing K3 arithmetic/clone.
//! Values below are resource admission, never an alternative score calculator.
use super::*;
use lc631_substrate::measurement_numeric as n;
fn fail() -> Error {
    Error::Arithmetic(MeasurementError::NumericBudget)
}
fn bits(v: &Rational) -> u64 {
    v.numer().bits().max(v.denom().bits())
}
fn sum(a: u64, b: u64, cap: u32) -> Result<u64, Error> {
    let n = a.checked_add(b).ok_or_else(fail)?;
    if n > u64::from(cap) {
        Err(fail())
    } else {
        Ok(n)
    }
}
pub(super) fn weight(v: &Rational, l: Limits, w: &mut Work<'_>) -> Result<(), Error> {
    if bits(v) > u64::from(l.max_weight_bits) {
        return Err(fail());
    }
    w.charge(
        bits(v)
            .checked_mul(8)
            .ok_or_else(fail)?
            .checked_add(64)
            .ok_or_else(fail)?,
    )?;
    // Check canonical rational invariants before clone/BigRational comparison.
    n::check(v).map_err(arithmetic)?;
    if v.numer() <= &0.into() {
        return Err(Error::InvalidWeight);
    }
    Ok(())
}
fn add(a: &Rational, b: &Rational, l: Limits, w: &mut Work<'_>) -> Result<Rational, Error> {
    let x = sum(a.numer().bits(), b.denom().bits(), l.max_intermediate_bits)?;
    let y = sum(b.numer().bits(), a.denom().bits(), l.max_intermediate_bits)?;
    sum(x.max(y), 1, l.max_intermediate_bits)?;
    sum(a.denom().bits(), b.denom().bits(), l.max_intermediate_bits)?;
    w.charge((bits(a) + bits(b) + 1).checked_mul(16).ok_or_else(fail)?)?;
    n::checked_add(a, b).map_err(arithmetic)
}
pub(super) fn pair(
    u: &Universe,
    left: &FeatureSet,
    right: &FeatureSet,
    l: Limits,
    w: &mut Work<'_>,
) -> Result<Rational, Error> {
    // Upper-bound kernel validation, merge, rational checks and comparisons
    // before entering the existing kernel (which has a larger global bit cap).
    let mut denominator = number(0);
    let mut numerator = number(0);
    for (i, f) in u.features.iter().enumerate() {
        w.charge((left.len() + right.len() + f.payload.len() + 1) as u64)?;
        let a = left.contains(&i);
        let b = right.contains(&i);
        if a || b {
            denominator = add(&denominator, &f.weight, l, w)?;
        }
        if a != b {
            numerator = add(&numerator, &f.weight, l, w)?;
        }
    }
    if !left.is_empty() || !right.is_empty() {
        sum(
            numerator.numer().bits(),
            denominator.denom().bits(),
            l.max_intermediate_bits,
        )?;
        sum(
            numerator.denom().bits(),
            denominator.numer().bits(),
            l.max_intermediate_bits,
        )?;
    }
    // Similarity subtraction and cross-product max comparison are admitted
    // conservatively from the quotient operand envelope, prior to knownness.
    let quotient = bits(&numerator)
        .checked_add(bits(&denominator))
        .ok_or_else(fail)?;
    sum(quotient, 2, l.max_intermediate_bits)?;
    sum(quotient, quotient, l.max_intermediate_bits)?;
    let numeric = u
        .features
        .iter()
        .try_fold(0u64, |a, f| a.checked_add(bits(&f.weight)).ok_or_else(fail))?;
    w.charge(
        numeric
            .checked_mul(64)
            .and_then(|n| n.checked_add(4096))
            .ok_or_else(fail)?,
    )?;
    Ok(denominator)
}
