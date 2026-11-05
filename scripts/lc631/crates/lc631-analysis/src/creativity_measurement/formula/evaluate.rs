use super::{
    ComponentEvaluation, ComponentFormulaInput, DerivedComponents, FormulaError, FormulaInput,
    FormulaProfile, OriginalArithmetic, OriginalEvaluation, OriginalFormula,
    OriginalPairEvaluation, PreferenceBasis,
};
use crate::creativity_measurement::{
    CandidateSubject, ComponentMetric as M, ComponentReport, ComponentState, Rational,
    RationalInterval,
};
use lc631_substrate::measurement_numeric::{
    checked_add, checked_compare, checked_divide, exp_neg_enclosure, sqrt_enclosure, BigInt,
    NumericEvaluation,
};

fn integer(n: i64) -> Rational {
    Rational::from_integer(BigInt::from(n))
}
fn point(q: &Rational) -> Result<RationalInterval, FormulaError> {
    Ok(RationalInterval::new(q.clone(), q.clone())?)
}
fn enclosure(value: NumericEvaluation) -> Result<RationalInterval, FormulaError> {
    match value {
        NumericEvaluation::Complete(bound) => Ok(bound),
        NumericEvaluation::ResourceExhausted { partial } => {
            Err(FormulaError::ResourceExhausted { partial })
        }
    }
}
#[derive(Clone, Copy)]
enum Reports<'a> {
    Legacy(&'a [ComponentReport]),
    Components(&'a [&'a ComponentReport]),
}
impl<'a> Reports<'a> {
    fn get(self, metric: M) -> Option<&'a ComponentReport> {
        match self {
            Self::Legacy(rows) => rows.iter().find(|r| r.metric == metric),
            Self::Components(rows) => rows.iter().copied().find(|r| r.metric == metric),
        }
    }
}
struct Reader<'a, 'b> {
    profile: &'a FormulaProfile,
    reports: Reports<'b>,
    translation_loss: &'b ComponentState,
    estimates: bool,
}
impl Reader<'_, '_> {
    fn state(
        &mut self,
        metric: Option<M>,
        state: &ComponentState,
    ) -> Result<RationalInterval, FormulaError> {
        match state {
            ComponentState::Measured(bounds) => Ok(bounds.clone()),
            ComponentState::Estimated { bounds, .. } => {
                self.estimates = true;
                Ok(bounds.clone())
            }
            ComponentState::Unknown(_)
            | ComponentState::NotApplicable { .. }
            | ComponentState::Incomparable { .. }
            | ComponentState::Undefined(_)
            | ComponentState::InvalidConfig(_)
            | ComponentState::ResourceExhausted { .. } => Err(FormulaError::NoScalar {
                metric,
                retained: state.clone(),
            }),
        }
    }
    fn leaf(&mut self, metric: M) -> Result<RationalInterval, FormulaError> {
        let report = self
            .reports
            .get(metric)
            .ok_or(FormulaError::MissingMetric(metric))?;
        if report.definition != self.profile.definition(metric)? {
            return Err(FormulaError::DefinitionMismatch(metric));
        }
        let result = self.state(Some(metric), &report.state)?;
        let signed = matches!(
            metric,
            M::Coverage
                | M::Compression
                | M::Bridge
                | M::Prediction
                | M::Explain
                | M::Design
                | M::Transfer
                | M::Solve
        );
        if metric == M::DescriptionBits {
            if result.lower() <= &integer(0) {
                return Err(FormulaError::InvalidParameter);
            }
            if result.is_exact() && result.lower().denom() != &BigInt::from(1) {
                return Err(FormulaError::InvalidParameter);
            }
        } else if signed {
            if result.lower() < &integer(-1) || result.upper() > &integer(1) {
                return Err(FormulaError::InvalidParameter);
            }
        } else {
            result.require_unit()?;
        }
        Ok(result)
    }
    fn loss(&mut self) -> Result<RationalInterval, FormulaError> {
        let value = self.state(None, self.translation_loss)?;
        value.require_unit()?;
        Ok(value)
    }
    fn group<const N: usize>(
        &mut self,
        metrics: [M; N],
        weights: &[Rational; N],
        negative_last: bool,
    ) -> Result<RationalInterval, FormulaError> {
        let mut total = RationalInterval::exact_ratio(0, 1)?;
        for (index, metric) in metrics.into_iter().enumerate() {
            // Fetch even when weight=0: Undefined or missing inputs never become 0*Undefined.
            let value = self.leaf(metric)?;
            let term = value.multiply(&point(&weights[index])?)?;
            total = if negative_last && index + 1 == N {
                total.subtract(&term)?
            } else {
                total.add(&term)?
            };
        }
        Ok(total)
    }
}

fn weighted_sum<const N: usize>(
    values: [&RationalInterval; N],
    coefficients: &[Rational; N],
    negative: &[usize],
) -> Result<RationalInterval, FormulaError> {
    let mut total = RationalInterval::exact_ratio(0, 1)?;
    for (index, value) in values.into_iter().enumerate() {
        let term = value.multiply(&point(&coefficients[index])?)?;
        total = if negative.contains(&index) {
            total.subtract(&term)?
        } else {
            total.add(&term)?
        };
    }
    Ok(total)
}

fn q_parity(
    profile: &FormulaProfile,
    description: &RationalInterval,
) -> Result<RationalInterval, FormulaError> {
    let distance = description
        .subtract(&point(&profile.scales.baseline_bits)?)?
        .abs()?;
    let scaled = distance.divide_positive(&profile.scales.sigma_bits)?;
    let at_max = enclosure(exp_neg_enclosure(scaled.upper(), profile.numeric)?)?;
    let at_min = enclosure(exp_neg_enclosure(scaled.lower(), profile.numeric)?)?;
    let mut upper = at_min.upper().clone();
    if scaled.lower() > &integer(0) {
        // Strict noncrossing upper even when a tiny exp argument rounds outward to1.
        let rational_upper =
            checked_divide(&integer(1), &checked_add(&integer(1), scaled.lower())?)?;
        if checked_compare(&rational_upper, &upper)?.is_lt() {
            upper = rational_upper;
        }
    }
    Ok(RationalInterval::new(at_max.lower().clone(), upper)?)
}

pub fn evaluate_original(
    profile: &FormulaProfile,
    formula: OriginalFormula,
    input: &FormulaInput<'_>,
) -> Result<OriginalEvaluation, FormulaError> {
    let binding = input.assessment.binding();
    if binding.scope != profile.binding.comparison_scope
        || binding.baseline != profile.binding.baseline_revision
        || input.translation_loss.binding != binding
        || input.translation_loss.reduction_policy != profile.binding.tl_reduction_policy
    {
        return Err(FormulaError::ScopeMismatch);
    }
    let value = evaluate_arithmetic(
        profile,
        formula,
        Reports::Legacy(input.assessment.reports()),
        input.translation_loss.state,
    )?;
    Ok(OriginalEvaluation {
        formula: value.formula,
        profile_digest: value.profile_digest,
        binding,
        raw: value.raw,
        declared_range: value.declared_range,
        derived: value.derived,
        includes_estimates: value.includes_estimates,
        preference_basis: value.preference_basis,
        degenerate_weights: value.degenerate_weights,
    })
}

/// Consume every original metric binding without rebinding it to the TL
/// producer. Equality here means reported candidate correspondence, not trust.
pub fn evaluate_components<'a>(
    profile: &FormulaProfile,
    formula: OriginalFormula,
    input: &'a ComponentFormulaInput<'a>,
) -> Result<ComponentEvaluation<'a>, FormulaError> {
    let subject = input.assessment.subject();
    if subject.scope != profile.binding.comparison_scope
        || subject.baseline != profile.binding.baseline_revision
        || CandidateSubject::from_binding(input.translation_loss.binding) != subject
        || input.translation_loss.reduction_policy != profile.binding.tl_reduction_policy
    {
        return Err(FormulaError::ScopeMismatch);
    }
    let arithmetic = evaluate_arithmetic(
        profile,
        formula,
        Reports::Components(input.assessment.reports()),
        input.translation_loss.state,
    )?;
    Ok(ComponentEvaluation {
        subject,
        input,
        arithmetic,
    })
}

fn evaluate_arithmetic(
    profile: &FormulaProfile,
    formula: OriginalFormula,
    reports: Reports<'_>,
    translation_loss: &ComponentState,
) -> Result<OriginalArithmetic, FormulaError> {
    let mut reader = Reader {
        profile,
        reports,
        translation_loss,
        estimates: false,
    };
    let loss = reader.loss()?;
    let description = reader.leaf(M::DescriptionBits)?;
    let mut derived = DerivedComponents {
        complexity_match: None,
        novelty_compatibility: None,
        structural_gain: None,
        reorganization_gain: None,
        utility_gain: None,
        complexity_parity: None,
    };
    let raw = match formula {
        OriginalFormula::F1V1 => {
            let dctx = reader.leaf(M::ContextDistance)?;
            let dsem = reader.leaf(M::SemanticConfigurationDistance)?;
            let compatibility = reader.leaf(M::Compatibility)?;
            let knownness = reader.leaf(M::Knownness)?;
            let distance = description
                .subtract(&point(&profile.scales.baseline_bits)?)?
                .abs()?;
            let denominator = point(&integer(1))?
                .add(&distance.divide_positive(&profile.scales.match_scale_bits)?)?;
            let complexity_match = point(&integer(1))?.divide(&denominator)?;
            let raw = weighted_sum(
                [
                    &dctx,
                    &dsem,
                    &loss,
                    &compatibility,
                    &complexity_match,
                    &knownness,
                ],
                &profile.coefficients.f1,
                &[2, 5],
            )?;
            derived.complexity_match = Some(complexity_match);
            raw
        }
        OriginalFormula::F2V1 => evaluate_f2(&mut reader, &description, &loss, &mut derived)?,
    };
    // Derive metadata only after all strict inputs have been consumed. Known
    // coefficient ranges never rescue NoScalar, including zero coefficients.
    let declared_range = profile.declared_range(formula)?;
    declared_range.check_evaluation(&raw, &derived)?;
    Ok(OriginalArithmetic {
        formula,
        profile_digest: profile.digest(),
        raw,
        declared_range,
        derived,
        includes_estimates: reader.estimates,
        preference_basis: PreferenceBasis::DeclaredUncalibrated,
        degenerate_weights: profile.degenerate(formula),
    })
}

fn evaluate_f2(
    reader: &mut Reader<'_, '_>,
    description: &RationalInterval,
    loss: &RationalInterval,
    derived: &mut DerivedComponents,
) -> Result<RationalInterval, FormulaError> {
    let profile = reader.profile;
    let dpar = reader.leaf(M::FrameworkDistance)?;
    let dsem = reader.leaf(M::SemanticConfigurationDistance)?;
    let compatibility = reader.leaf(M::Compatibility)?;
    let product = dpar.multiply(&dsem)?;
    let lo = enclosure(sqrt_enclosure(product.lower(), reader.profile.numeric)?)?;
    let hi = enclosure(sqrt_enclosure(product.upper(), reader.profile.numeric)?)?;
    let tnc =
        RationalInterval::new(lo.lower().clone(), hi.upper().clone())?.multiply(&compatibility)?;
    let structural = reader.group(
        [
            M::Coverage,
            M::Compression,
            M::Bridge,
            M::Prediction,
            M::Redundancy,
        ],
        &profile.coefficients.structural,
        true,
    )?;
    let reorganization = reader.group(
        [M::Preserve, M::Rebind, M::Reframe, M::BreakCost],
        &profile.coefficients.reorganization,
        true,
    )?;
    let utility = reader.group(
        [M::Explain, M::Design, M::Transfer, M::Solve],
        &profile.coefficients.utility,
        false,
    )?;
    let parity = q_parity(reader.profile, description)?;
    let counter = reader.leaf(M::CounterexampleRisk)?;
    let bridge_cost = reader.leaf(M::BridgeCost)?;
    let raw = weighted_sum(
        [
            &tnc,
            &structural,
            &reorganization,
            &utility,
            &parity,
            loss,
            &counter,
            &bridge_cost,
        ],
        &reader.profile.coefficients.f2,
        &[5, 6, 7],
    )?;
    derived.novelty_compatibility = Some(tnc);
    derived.structural_gain = Some(structural);
    derived.reorganization_gain = Some(reorganization);
    derived.utility_gain = Some(utility);
    derived.complexity_parity = Some(parity);
    Ok(raw)
}

pub fn evaluate_original_pair(
    profile: &FormulaProfile,
    formula: OriginalFormula,
    candidate: &FormulaInput<'_>,
    baseline: &FormulaInput<'_>,
) -> Result<OriginalPairEvaluation, FormulaError> {
    if baseline.assessment.binding().candidate != profile.binding.baseline_candidate {
        return Err(FormulaError::BaselineMismatch);
    }
    let base_k = baseline
        .assessment
        .get(M::DescriptionBits)
        .ok_or(FormulaError::MissingMetric(M::DescriptionBits))?;
    match &base_k.state {
        ComponentState::Measured(k) | ComponentState::Estimated { bounds: k, .. } => {
            if !k.is_exact() || k.lower() != &profile.scales.baseline_bits {
                return Err(FormulaError::BaselineMismatch);
            }
        }
        state => {
            return Err(FormulaError::NoScalar {
                metric: Some(M::DescriptionBits),
                retained: state.clone(),
            })
        }
    }
    let candidate_value = evaluate_original(profile, formula, candidate)?;
    let baseline_value = evaluate_original(profile, formula, baseline)?;
    let same_input_nodes = candidate.assessment.reports().len()
        == baseline.assessment.reports().len()
        && candidate
            .assessment
            .reports()
            .iter()
            .all(|report| baseline.assessment.get(report.metric) == Some(report))
        && candidate.translation_loss.binding == baseline.translation_loss.binding
        && candidate.translation_loss.reduction_policy
            == baseline.translation_loss.reduction_policy
        && candidate.translation_loss.state == baseline.translation_loss.state;
    let delta = if same_input_nodes {
        RationalInterval::exact_ratio(0, 1)?
    } else {
        candidate_value.raw.subtract(&baseline_value.raw)?
    };
    Ok(OriginalPairEvaluation {
        candidate: candidate_value,
        baseline: baseline_value,
        delta,
        same_input_nodes,
    })
}
