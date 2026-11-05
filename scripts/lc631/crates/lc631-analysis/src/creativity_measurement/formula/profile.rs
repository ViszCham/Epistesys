use super::{FormulaError, OriginalFormula};
use crate::creativity_measurement::{ComponentMetric, Rational};
use lc631_substrate::{
    measurement_numeric::{check, BigInt, NumericBudget},
    Digest32,
};
use sha2::{Digest, Sha256};

/// The original 27 coefficients. Names/order follow the fixed contract exactly.
pub struct CoefficientSet {
    pub f1: [Rational; 6],
    pub f2: [Rational; 8],
    pub structural: [Rational; 5],
    pub reorganization: [Rational; 4],
    pub utility: [Rational; 4],
}
pub struct ScaleParameters {
    pub baseline_bits: Rational,
    pub sigma_bits: Rational,
    pub match_scale_bits: Rational,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FormulaBinding {
    pub comparison_scope: Digest32,
    pub baseline_revision: Digest32,
    pub baseline_candidate: Digest32,
    pub encoding_revision: Digest32,
    pub tl_reduction_policy: Digest32,
}

/// No default production weights. All values are owned and immutable after admission.
pub struct FormulaProfile {
    pub(super) binding: FormulaBinding,
    pub(super) coefficients: CoefficientSet,
    pub(super) scales: ScaleParameters,
    pub(super) numeric: NumericBudget,
    definitions: Vec<(ComponentMetric, Digest32)>,
    digest: Digest32,
}
impl FormulaProfile {
    pub fn checked(
        binding: FormulaBinding,
        coefficients: CoefficientSet,
        scales: ScaleParameters,
        definitions: &[(ComponentMetric, Digest32)],
        precision: u32,
    ) -> Result<Self, FormulaError> {
        let numeric = NumericBudget::new(precision)?;
        let zero = Rational::from_integer(BigInt::from(0));
        for group in [
            &coefficients.f1[..],
            &coefficients.f2[..],
            &coefficients.structural[..],
            &coefficients.reorganization[..],
            &coefficients.utility[..],
        ] {
            for coefficient in group {
                check(coefficient)?;
                if coefficient < &zero {
                    return Err(FormulaError::InvalidParameter);
                }
            }
        }
        for scale in [
            &scales.baseline_bits,
            &scales.sigma_bits,
            &scales.match_scale_bits,
        ] {
            check(scale)?;
            if scale <= &zero {
                return Err(FormulaError::InvalidParameter);
            }
        }
        if scales.baseline_bits.denom() != &BigInt::from(1) {
            return Err(FormulaError::InvalidParameter);
        }
        if definitions.is_empty() || definitions.len() > 29 {
            return Err(FormulaError::InvalidParameter);
        }
        let mut pins = Vec::new();
        pins.try_reserve_exact(definitions.len())
            .map_err(|_| FormulaError::AllocationFailed)?;
        pins.extend_from_slice(definitions);
        pins.sort_unstable_by_key(|p| p.0);
        if pins.windows(2).any(|p| p[0].0 == p[1].0) {
            return Err(FormulaError::DuplicateDefinition);
        }
        let digest = profile_digest(binding, &coefficients, &scales, &pins, precision);
        Ok(Self {
            binding,
            coefficients,
            scales,
            numeric,
            definitions: pins,
            digest,
        })
    }
    pub const fn digest(&self) -> Digest32 {
        self.digest
    }
    pub const fn binding(&self) -> FormulaBinding {
        self.binding
    }
    pub fn ranking_allowed(&self, version: OriginalFormula) -> Result<(), FormulaError> {
        if self.degenerate(version) {
            return Err(FormulaError::DegenerateWeights);
        }
        Ok(())
    }
    pub(super) fn degenerate(&self, version: OriginalFormula) -> bool {
        let group = match version {
            OriginalFormula::F1V1 => &self.coefficients.f1[..],
            OriginalFormula::F2V1 => &self.coefficients.f2[..],
        };
        group.iter().all(|q| q.numer().bits() == 0)
    }
    pub(super) fn definition(&self, metric: ComponentMetric) -> Result<Digest32, FormulaError> {
        self.definitions
            .iter()
            .find(|p| p.0 == metric)
            .map(|p| p.1)
            .ok_or(FormulaError::MissingDefinition(metric))
    }
}
fn append_rational(hash: &mut Sha256, q: &Rational) {
    let (_, n) = q.numer().to_bytes_be();
    let (_, d) = q.denom().to_bytes_be();
    hash.update((n.len() as u64).to_le_bytes());
    hash.update(n);
    hash.update((d.len() as u64).to_le_bytes());
    hash.update(d);
}
fn profile_digest(
    binding: FormulaBinding,
    coefficients: &CoefficientSet,
    scales: &ScaleParameters,
    definitions: &[(ComponentMetric, Digest32)],
    precision: u32,
) -> Digest32 {
    let mut hash = Sha256::new();
    hash.update(b"epistesys-original-f1-f2-profile/finite-formal-v1.4\0");
    for pin in [
        binding.comparison_scope,
        binding.baseline_revision,
        binding.baseline_candidate,
        binding.encoding_revision,
        binding.tl_reduction_policy,
    ] {
        hash.update(pin.as_bytes());
    }
    hash.update(precision.to_le_bytes());
    for group in [
        &coefficients.f1[..],
        &coefficients.f2[..],
        &coefficients.structural[..],
        &coefficients.reorganization[..],
        &coefficients.utility[..],
    ] {
        for q in group {
            append_rational(&mut hash, q);
        }
    }
    for q in [
        &scales.baseline_bits,
        &scales.sigma_bits,
        &scales.match_scale_bits,
    ] {
        append_rational(&mut hash, q);
    }
    hash.update((definitions.len() as u64).to_le_bytes());
    for (metric, pin) in definitions {
        hash.update((*metric as u16).to_le_bytes());
        hash.update(pin.as_bytes());
    }
    Digest32::from_bytes(hash.finalize().into())
}
