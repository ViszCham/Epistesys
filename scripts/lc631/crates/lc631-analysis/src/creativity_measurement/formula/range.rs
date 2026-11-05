//! Coefficient-derived closed outer ranges from finite-formal-v1.4.
//! These ranges describe algebraic possibilities, not attainable candidates,
//! observations, calibration, ranking eligibility, semantic truth or authority.
use super::{DerivedComponents, FormulaError, FormulaProfile, OriginalFormula};
use crate::creativity_measurement::{ComponentMetric, Rational, RationalInterval};
use lc631_substrate::{
    measurement_numeric::{
        checked_add, checked_compare, checked_multiply, checked_subtract, BigInt,
    },
    Digest32,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclaredFormulaRange {
    pub formula: OriginalFormula,
    pub profile_digest: Digest32,
    pub raw: RationalInterval,
    pub structural_gain: Option<RationalInterval>,
    pub reorganization_gain: Option<RationalInterval>,
    pub utility_gain: Option<RationalInterval>,
}

impl FormulaProfile {
    /// No coefficient normalization/defaults: all 27 admitted values retain
    /// their original groups and signs. A zero range does not permit ranking.
    pub fn declared_range(
        &self,
        formula: OriginalFormula,
    ) -> Result<DeclaredFormulaRange, FormulaError> {
        let zero = Rational::from_integer(BigInt::from(0));
        let (raw, structural_gain, reorganization_gain, utility_gain) = match formula {
            OriginalFormula::F1V1 => {
                let c = &self.coefficients.f1;
                let lower = checked_subtract(&zero, &checked_add(&c[2], &c[5])?)?;
                let upper = sum(&[&c[0], &c[1], &c[3], &c[4]])?;
                (RationalInterval::new(lower, upper)?, None, None, None)
            }
            OriginalFormula::F2V1 => {
                let a = &self.coefficients.structural;
                let b = &self.coefficients.reorganization;
                let c = &self.coefficients.utility;
                let g_upper = sum(&[&a[0], &a[1], &a[2], &a[3]])?;
                let g_lower = checked_subtract(&zero, &checked_add(&g_upper, &a[4])?)?;
                let structural = RationalInterval::new(g_lower, g_upper)?;
                let reorganization = RationalInterval::new(
                    checked_subtract(&zero, &b[3])?,
                    sum(&[&b[0], &b[1], &b[2]])?,
                )?;
                let u_upper = sum(&[&c[0], &c[1], &c[2], &c[3]])?;
                let utility = RationalInterval::new(checked_subtract(&zero, &u_upper)?, u_upper)?;
                let w = &self.coefficients.f2;
                let lower_gain =
                    weighted_endpoint(w, &structural, &reorganization, &utility, false)?;
                let upper_gain =
                    weighted_endpoint(w, &structural, &reorganization, &utility, true)?;
                let lower = checked_subtract(&lower_gain, &sum(&[&w[5], &w[6], &w[7]])?)?;
                let upper = checked_add(&upper_gain, &checked_add(&w[0], &w[4])?)?;
                (
                    RationalInterval::new(lower, upper)?,
                    Some(structural),
                    Some(reorganization),
                    Some(utility),
                )
            }
        };
        Ok(DeclaredFormulaRange {
            formula,
            profile_digest: self.digest(),
            raw,
            structural_gain,
            reorganization_gain,
            utility_gain,
        })
    }
}

impl DeclaredFormulaRange {
    /// Arithmetic containment only. No evidence or eligibility is generated.
    pub fn contains_raw(&self, value: &RationalInterval) -> Result<bool, FormulaError> {
        contains(&self.raw, value)
    }

    pub(super) fn check_evaluation(
        &self,
        raw: &RationalInterval,
        derived: &DerivedComponents,
    ) -> Result<(), FormulaError> {
        if !self.contains_raw(raw)? {
            return Err(FormulaError::DeclaredRangeViolation(match self.formula {
                OriginalFormula::F1V1 => ComponentMetric::OriginalV1,
                OriginalFormula::F2V1 => ComponentMetric::OriginalV2,
            }));
        }
        for (metric, declared, value) in [
            (
                ComponentMetric::StructuralGain,
                &self.structural_gain,
                &derived.structural_gain,
            ),
            (
                ComponentMetric::ReorganizationGain,
                &self.reorganization_gain,
                &derived.reorganization_gain,
            ),
            (
                ComponentMetric::UtilityGain,
                &self.utility_gain,
                &derived.utility_gain,
            ),
        ] {
            match (declared, value) {
                (Some(range), Some(value)) if contains(range, value)? => {}
                (None, None) => {}
                (Some(_), Some(_)) | (Some(_), None) | (None, Some(_)) => {
                    return Err(FormulaError::DeclaredRangeViolation(metric));
                }
            }
        }
        Ok(())
    }
}

fn contains(range: &RationalInterval, value: &RationalInterval) -> Result<bool, FormulaError> {
    Ok(!checked_compare(value.lower(), range.lower())?.is_lt()
        && !checked_compare(value.upper(), range.upper())?.is_gt())
}

fn sum(values: &[&Rational]) -> Result<Rational, FormulaError> {
    let mut total = Rational::from_integer(BigInt::from(0));
    for value in values {
        total = checked_add(&total, value)?;
    }
    Ok(total)
}

fn weighted_endpoint(
    w: &[Rational; 8],
    g: &RationalInterval,
    r: &RationalInterval,
    u: &RationalInterval,
    upper: bool,
) -> Result<Rational, FormulaError> {
    let endpoint = |value: &RationalInterval| {
        if upper {
            value.upper().clone()
        } else {
            value.lower().clone()
        }
    };
    sum(&[
        &checked_multiply(&w[1], &endpoint(g))?,
        &checked_multiply(&w[2], &endpoint(r))?,
        &checked_multiply(&w[3], &endpoint(u))?,
    ])
}
