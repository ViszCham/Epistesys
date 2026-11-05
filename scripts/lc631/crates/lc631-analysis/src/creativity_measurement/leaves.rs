//! Fixed-set K1/K2 arithmetic over reported probe results.
//! Probe independence and oracle authenticity require external validation.
use super::{numeric::check, MeasurementError, Rational, RationalInterval};
use lc631_substrate::measurement_numeric::{checked_add, BigInt};
use lc631_substrate::Digest32;

pub struct WeightedProbe<'a> {
    pub canonical_payload: &'a [u8],
    pub weight: &'a Rational,
    pub value: &'a RationalInterval,
}

pub fn weighted_assessment(
    probes: &[WeightedProbe<'_>],
) -> Result<RationalInterval, MeasurementError> {
    if probes.is_empty() {
        return Err(MeasurementError::EmptyDenominator);
    }
    if probes.len() > 100_000 {
        return Err(MeasurementError::NumericBudget);
    }
    let mut sum = RationalInterval::exact_ratio(0, 1)?;
    let mut denominator = Rational::from_integer(BigInt::from(0));
    let mut seen = std::collections::BTreeSet::new();
    for probe in probes {
        if probe.canonical_payload.is_empty() || probe.canonical_payload.len() > 4096 {
            return Err(MeasurementError::InvalidInput);
        }
        if !seen.insert(probe.canonical_payload) {
            return Err(MeasurementError::DuplicateProbe);
        }
        probe.value.require_unit()?;
        sum = sum.add(&probe.value.scale_positive(probe.weight)?)?;
        denominator = checked_add(&denominator, probe.weight)?;
        check(&denominator)?;
    }
    sum.divide_positive(&denominator)
        .map_err(MeasurementError::from)
}

pub struct ProbePair<'a> {
    pub canonical_payload: &'a [u8],
    pub weight: &'a Rational,
    pub baseline: &'a RationalInterval,
    pub candidate: &'a RationalInterval,
    /// ValueNode keys include metric/definition/binding/probes, not merely an observation ID.
    pub baseline_node: Digest32,
    pub candidate_node: Digest32,
}

pub fn paired_gain(probes: &[ProbePair<'_>]) -> Result<RationalInterval, MeasurementError> {
    if probes.is_empty() {
        return Err(MeasurementError::EmptyDenominator);
    }
    if probes.len() > 100_000 {
        return Err(MeasurementError::NumericBudget);
    }
    let mut sum = RationalInterval::exact_ratio(0, 1)?;
    let mut denominator = Rational::from_integer(BigInt::from(0));
    let mut seen = std::collections::BTreeSet::new();
    for pair in probes {
        if pair.canonical_payload.is_empty() || pair.canonical_payload.len() > 4096 {
            return Err(MeasurementError::InvalidInput);
        }
        if !seen.insert(pair.canonical_payload) {
            return Err(MeasurementError::DuplicateProbe);
        }
        pair.baseline.require_unit()?;
        pair.candidate.require_unit()?;
        let difference = if pair.baseline_node == pair.candidate_node {
            if pair.baseline != pair.candidate {
                return Err(MeasurementError::InvalidInput);
            }
            RationalInterval::exact_ratio(0, 1)?
        } else {
            pair.candidate.subtract(pair.baseline)?
        };
        sum = sum.add(&difference.scale_positive(pair.weight)?)?;
        denominator = checked_add(&denominator, pair.weight)?;
        check(&denominator)?;
    }
    sum.divide_positive(&denominator)
        .map_err(MeasurementError::from)
}
