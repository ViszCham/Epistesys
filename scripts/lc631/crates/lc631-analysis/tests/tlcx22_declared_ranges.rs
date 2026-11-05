use lc631_analysis::creativity_measurement::formula::{
    evaluate_original, CoefficientSet, FormulaBinding, FormulaError, FormulaInput, FormulaProfile,
    OriginalEvaluation, OriginalFormula as F, ScaleParameters, TranslationLossInput,
};
use lc631_analysis::creativity_measurement::{
    AssessmentPolicy, ComponentMetric as M, ComponentReport, ComponentState, CreativeAssessment,
    IndependenceStatus, InputBasis, MeasurementBinding, MeasurementGap, Rational, RationalInterval,
    SemanticValidation,
};
use lc631_substrate::{
    measurement_numeric::{BigInt, NumericError},
    Digest32,
};

fn pin(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}
fn q(n: i64) -> Rational {
    Rational::from_integer(BigInt::from(n))
}
fn interval(lo: i64, hi: i64) -> RationalInterval {
    RationalInterval::new(q(lo), q(hi)).unwrap()
}
fn coefficients() -> CoefficientSet {
    CoefficientSet {
        f1: [2, 3, 5, 7, 11, 13].map(q),
        f2: [2, 3, 5, 7, 11, 13, 17, 19].map(q),
        structural: [2, 3, 5, 7, 11].map(q),
        reorganization: [13, 17, 19, 23].map(q),
        utility: [29, 31, 37, 41].map(q),
    }
}
fn binding() -> MeasurementBinding {
    MeasurementBinding {
        scope: pin(1),
        candidate: pin(2),
        source: pin(3),
        target: pin(4),
        context: pin(5),
        baseline: pin(6),
        producer: pin(7),
        transform: pin(8),
        result: pin(9),
    }
}
fn metrics() -> [M; 21] {
    [
        M::ContextDistance,
        M::FrameworkDistance,
        M::SemanticConfigurationDistance,
        M::Compatibility,
        M::Knownness,
        M::Coverage,
        M::Compression,
        M::Bridge,
        M::Prediction,
        M::Redundancy,
        M::Preserve,
        M::Rebind,
        M::Reframe,
        M::BreakCost,
        M::Explain,
        M::Design,
        M::Transfer,
        M::Solve,
        M::DescriptionBits,
        M::CounterexampleRisk,
        M::BridgeCost,
    ]
}
fn rows() -> Vec<ComponentReport> {
    metrics()
        .into_iter()
        .map(|metric| ComponentReport {
            metric,
            definition: pin(10),
            binding: binding(),
            probe_set: pin(11),
            oracle: pin(12),
            dependency_group: pin(13),
            evidence_payload: pin(14),
            basis: InputBasis::Assumed,
            validation: SemanticValidation::Unassessed,
            independence: IndependenceStatus::SharedDependency,
            state: ComponentState::Measured(if metric == M::DescriptionBits {
                interval(80, 80)
            } else {
                interval(0, 0)
            }),
        })
        .collect()
}
fn profile(c: CoefficientSet) -> FormulaProfile {
    FormulaProfile::checked(
        FormulaBinding {
            comparison_scope: pin(1),
            baseline_revision: pin(6),
            baseline_candidate: pin(2),
            encoding_revision: pin(15),
            tl_reduction_policy: pin(16),
        },
        c,
        ScaleParameters {
            baseline_bits: q(80),
            sigma_bits: q(80),
            match_scale_bits: q(80),
        },
        &metrics().map(|m| (m, pin(10))),
        64,
    )
    .unwrap()
}
#[allow(clippy::result_large_err)]
fn evaluate(
    p: &FormulaProfile,
    f: F,
    rows: &[ComponentReport],
) -> Result<OriginalEvaluation, FormulaError> {
    let required = [M::DescriptionBits];
    let assessment = CreativeAssessment::from_reports(
        &AssessmentPolicy {
            scope: pin(1),
            baseline: pin(6),
            required: &required,
        },
        rows,
    )
    .unwrap();
    let loss = ComponentState::Measured(interval(0, 1));
    evaluate_original(
        p,
        f,
        &FormulaInput {
            assessment: &assessment,
            translation_loss: TranslationLossInput {
                binding: assessment.binding(),
                reduction_policy: pin(16),
                state: &loss,
            },
        },
    )
}

#[test]
fn tlcx22_declared_nonuniform_f1_and_f2_ranges_match_hand_calculation() {
    let p = profile(coefficients());
    let f1 = p.declared_range(F::F1V1).unwrap();
    assert_eq!(f1.raw, interval(-18, 23));
    assert_eq!(f1.profile_digest, p.digest());
    assert_eq!(f1.structural_gain, None);
    let f2 = p.declared_range(F::F2V1).unwrap();
    // G=[-28,17], R=[-23,49], U=[-138,138].
    // lower=-84-115-966-13-17-19=-1214; upper=2+51+245+966+11=1275.
    assert_eq!(f2.structural_gain, Some(interval(-28, 17)));
    assert_eq!(f2.reorganization_gain, Some(interval(-23, 49)));
    assert_eq!(f2.utility_gain, Some(interval(-138, 138)));
    assert_eq!(f2.raw, interval(-1214, 1275));
    assert!(!f2.contains_raw(&interval(-1215, 0)).unwrap());
    assert!(!f2.contains_raw(&interval(0, 1276)).unwrap());
}

#[test]
fn tlcx22_declared_actual_public_evaluation_retains_signed_gain_and_bounds() {
    let p = profile(coefficients());
    let mut rows = rows();
    for row in &mut rows {
        row.state = ComponentState::Measured(match row.metric {
            M::DescriptionBits => interval(80, 80),
            M::Coverage
            | M::Compression
            | M::Bridge
            | M::Prediction
            | M::Explain
            | M::Design
            | M::Transfer
            | M::Solve => interval(-1, 1),
            _ => interval(0, 1),
        });
    }
    for formula in [F::F1V1, F::F2V1] {
        let result = evaluate(&p, formula, &rows).unwrap();
        assert_eq!(result.declared_range, p.declared_range(formula).unwrap());
        assert!(result.declared_range.contains_raw(&result.raw).unwrap());
        assert!(!result.raw.is_exact());
        if formula == F::F2V1 {
            assert_eq!(result.derived.structural_gain, Some(interval(-28, 17)));
            assert_eq!(result.derived.reorganization_gain, Some(interval(-23, 49)));
            assert_eq!(result.derived.utility_gain, Some(interval(-138, 138)));
        }
    }
}

#[test]
fn tlcx22_declared_sampled_valid_leaf_endpoints_all_remain_inside_ranges() {
    let p = profile(coefficients());
    for metric in metrics().into_iter().filter(|m| *m != M::DescriptionBits) {
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
        for value in if signed { [-1, 0, 1] } else { [0, 0, 1] } {
            let mut rows = rows();
            rows.iter_mut().find(|r| r.metric == metric).unwrap().state =
                ComponentState::Measured(interval(value, value));
            for formula in [F::F1V1, F::F2V1] {
                let result = evaluate(&p, formula, &rows).unwrap();
                assert!(result.declared_range.contains_raw(&result.raw).unwrap());
            }
        }
    }
}

#[test]
fn tlcx22_declared_zero_range_does_not_rescue_unknown_or_allow_ranking() {
    let zero = || q(0);
    let p = profile(CoefficientSet {
        f1: std::array::from_fn(|_| zero()),
        f2: std::array::from_fn(|_| zero()),
        structural: std::array::from_fn(|_| zero()),
        reorganization: std::array::from_fn(|_| zero()),
        utility: std::array::from_fn(|_| zero()),
    });
    for f in [F::F1V1, F::F2V1] {
        assert_eq!(p.declared_range(f).unwrap().raw, interval(0, 0));
        assert_eq!(p.ranking_allowed(f), Err(FormulaError::DegenerateWeights));
    }
    let mut rows = rows();
    rows.iter_mut()
        .find(|r| r.metric == M::Knownness)
        .unwrap()
        .state = ComponentState::Unknown(MeasurementGap::MissingEvaluator);
    assert!(matches!(
        evaluate(&p, F::F1V1, &rows),
        Err(FormulaError::NoScalar {
            metric: Some(M::Knownness),
            retained: ComponentState::Unknown(MeasurementGap::MissingEvaluator),
        })
    ));
}

#[test]
fn tlcx22_declared_numeric_budget_is_not_clipped_into_success() {
    let mut c = coefficients();
    let huge = Rational::from_integer(BigInt::from(1) << 65_534_usize);
    c.f1[2] = huge;
    let p = profile(c);
    assert!(matches!(
        p.declared_range(F::F1V1),
        Err(FormulaError::Numeric(NumericError::NumericBudget))
    ));
}
