//! Pure arithmetic over explicitly reported canonical construction features.
//! This module does not discover equivalence or verify an encoder/checker.
use super::{numeric::check, MeasurementError, Rational, RationalInterval};
use lc631_substrate::measurement_numeric::{
    checked_add, checked_compare, checked_divide, checked_subtract, BigInt,
};

pub struct Feature<'a> {
    /// Logic-canonical meaning payload, never an opaque feature ID.
    pub canonical_payload: &'a [u8],
    pub weight: &'a Rational,
}

/// Fixed-universe K3. Unextracted sets must not enter as empty slices.
pub fn set_distance(
    universe: &[Feature<'_>],
    left: &[&[u8]],
    right: &[&[u8]],
) -> Result<RationalInterval, MeasurementError> {
    validate_sorted(universe, left, right)?;
    let zero = Rational::from_integer(BigInt::from(0));
    let mut numerator = zero.clone();
    let mut denominator = zero;
    let (mut l, mut r) = (0, 0);
    for feature in universe {
        let in_left = left
            .get(l)
            .is_some_and(|key| *key == feature.canonical_payload);
        let in_right = right
            .get(r)
            .is_some_and(|key| *key == feature.canonical_payload);
        if in_left {
            l += 1;
        }
        if in_right {
            r += 1;
        }
        if in_left || in_right {
            denominator = checked_add(&denominator, feature.weight)?;
        }
        if in_left != in_right {
            numerator = checked_add(&numerator, feature.weight)?;
        }
    }
    if l != left.len() || r != right.len() {
        return Err(MeasurementError::UniverseMismatch);
    }
    if left.is_empty() && right.is_empty() {
        return RationalInterval::exact_ratio(0, 1).map_err(MeasurementError::from);
    }
    let result = checked_divide(&numerator, &denominator)?;
    RationalInterval::new(result.clone(), result).map_err(MeasurementError::from)
}

fn validate_sorted(
    universe: &[Feature<'_>],
    left: &[&[u8]],
    right: &[&[u8]],
) -> Result<(), MeasurementError> {
    if universe.len() > 100_000 || left.len() > 100_000 || right.len() > 100_000 {
        return Err(MeasurementError::NumericBudget);
    }
    let zero = Rational::from_integer(BigInt::from(0));
    let mut total = 0_usize;
    for (i, feature) in universe.iter().enumerate() {
        total = total
            .checked_add(feature.canonical_payload.len())
            .ok_or(MeasurementError::NumericBudget)?;
        if feature.canonical_payload.is_empty() || total > 1_048_576 {
            return Err(MeasurementError::NumericBudget);
        }
        if i > 0 && universe[i - 1].canonical_payload >= feature.canonical_payload {
            return Err(MeasurementError::InvalidInput);
        }
        check(feature.weight)?;
        if feature.weight <= &zero {
            return Err(MeasurementError::InvalidInput);
        }
    }
    for keys in [left, right] {
        if keys.windows(2).any(|p| p[0] >= p[1]) {
            return Err(MeasurementError::InvalidInput);
        }
    }
    Ok(())
}

pub enum PreservationPrerequisite {
    Pending,
    Failed,
    ReportedChecked,
}

pub fn compression_gain(
    baseline_bits: u64,
    candidate_bits: u64,
    preservation: PreservationPrerequisite,
) -> Result<RationalInterval, MeasurementError> {
    match preservation {
        PreservationPrerequisite::Pending => return Err(MeasurementError::PrerequisitePending),
        PreservationPrerequisite::Failed => return Err(MeasurementError::PrerequisiteFailed),
        PreservationPrerequisite::ReportedChecked => {}
    }
    if baseline_bits == 0 || candidate_bits == 0 {
        return Err(MeasurementError::InvalidInput);
    }
    let n = BigInt::from(baseline_bits) - BigInt::from(candidate_bits);
    let d = BigInt::from(baseline_bits.max(candidate_bits));
    let result = Rational::new(n, d);
    RationalInterval::new(result.clone(), result).map_err(MeasurementError::from)
}

pub fn complexity_match(
    baseline_bits: u64,
    candidate_bits: u64,
    scale_bits: u64,
) -> Result<RationalInterval, MeasurementError> {
    if baseline_bits == 0 || candidate_bits == 0 || scale_bits == 0 {
        return Err(MeasurementError::InvalidInput);
    }
    let scale = BigInt::from(scale_bits);
    let d = &scale + BigInt::from(baseline_bits.abs_diff(candidate_bits));
    let result = Rational::new(scale, d);
    RationalInterval::new(result.clone(), result).map_err(MeasurementError::from)
}

/// Complete archive search; missing corpora retain Unknown in ComponentState.
pub fn knownness(
    universe: &[Feature<'_>],
    candidate: &[&[u8]],
    archive: &[&[&[u8]]],
) -> Result<RationalInterval, MeasurementError> {
    if archive.is_empty() {
        return Err(MeasurementError::EmptyCorpus);
    }
    if archive.len() > 10_000 {
        return Err(MeasurementError::NumericBudget);
    }
    let one = Rational::from_integer(BigInt::from(1));
    let mut maximum = Rational::from_integer(BigInt::from(0));
    for construction in archive {
        let distance = set_distance(universe, candidate, construction)?;
        let similarity = checked_subtract(&one, distance.lower())?;
        if checked_compare(&similarity, &maximum)?.is_gt() {
            maximum = similarity;
        }
    }
    RationalInterval::new(maximum.clone(), maximum).map_err(MeasurementError::from)
}
