use lc631_analysis::creativity_measurement::formula::{
    evaluate_original, evaluate_original_pair, CoefficientSet, FormulaBinding, FormulaError,
    FormulaInput, FormulaProfile, OriginalEvaluation, OriginalFormula as F, PreferenceBasis,
    ScaleParameters, TranslationLossInput,
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
fn q(n: i64, d: i64) -> Rational {
    Rational::new(n.into(), d.into())
}
fn point(n: i64, d: i64) -> RationalInterval {
    RationalInterval::exact_ratio(n, d).unwrap()
}
fn coefficients() -> CoefficientSet {
    CoefficientSet {
        f1: std::array::from_fn(|_| q(1, 6)),
        f2: std::array::from_fn(|_| q(1, 8)),
        structural: std::array::from_fn(|_| q(1, 5)),
        reorganization: std::array::from_fn(|_| q(1, 4)),
        utility: std::array::from_fn(|_| q(1, 4)),
    }
}
fn binding() -> MeasurementBinding {
    MeasurementBinding {
        scope: pin(2),
        candidate: pin(3),
        source: pin(4),
        target: pin(5),
        context: pin(6),
        baseline: pin(7),
        producer: pin(8),
        transform: pin(9),
        result: pin(10),
    }
}
fn sample() -> Vec<ComponentReport> {
    let metrics = [
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
    ];
    metrics
        .into_iter()
        .map(|metric| ComponentReport {
            metric,
            definition: pin(11),
            binding: binding(),
            probe_set: pin(12),
            oracle: pin(13),
            dependency_group: pin(14),
            evidence_payload: pin(15),
            basis: InputBasis::Assumed,
            validation: SemanticValidation::Unassessed,
            independence: IndependenceStatus::SharedDependency,
            state: ComponentState::Measured(match metric {
                M::Compatibility | M::Knownness | M::Preserve => point(1, 1),
                M::DescriptionBits => point(80, 1),
                _ => point(0, 1),
            }),
        })
        .collect()
}
fn set(rows: &mut [ComponentReport], metric: M, value: RationalInterval) {
    rows.iter_mut().find(|r| r.metric == metric).unwrap().state = ComponentState::Measured(value);
}
fn profile(rows: &[ComponentReport], coeff: CoefficientSet, k0: i64, sigma: i64) -> FormulaProfile {
    FormulaProfile::checked(
        FormulaBinding {
            comparison_scope: pin(2),
            baseline_revision: pin(7),
            baseline_candidate: pin(3),
            encoding_revision: pin(16),
            tl_reduction_policy: pin(17),
        },
        coeff,
        ScaleParameters {
            baseline_bits: q(k0, 1),
            sigma_bits: q(sigma, 1),
            match_scale_bits: q(80, 1),
        },
        &rows.iter().map(|r| (r.metric, pin(11))).collect::<Vec<_>>(),
        64,
    )
    .unwrap()
}
// Preserve the same inline failure-state contract as the formula API.
#[allow(clippy::result_large_err)]
fn evaluate(
    p: &FormulaProfile,
    f: F,
    rows: &[ComponentReport],
    loss: ComponentState,
) -> Result<OriginalEvaluation, FormulaError> {
    let required = [M::DescriptionBits];
    let a = CreativeAssessment::from_reports(
        &AssessmentPolicy {
            scope: pin(2),
            baseline: pin(7),
            required: &required,
        },
        rows,
    )
    .unwrap();
    evaluate_original(
        p,
        f,
        &FormulaInput {
            assessment: &a,
            translation_loss: TranslationLossInput {
                binding: a.binding(),
                reduction_policy: pin(17),
                state: &loss,
            },
        },
    )
}
fn loss(n: i64, d: i64) -> ComponentState {
    ComponentState::Measured(point(n, d))
}

#[test]
fn tlcx22_original_g09_g10_g11_identity_raw_is_nonzero() {
    let mut rows = sample();
    let p = profile(&rows, coefficients(), 80, 80);
    let f1 = evaluate(&p, F::F1V1, &rows, loss(0, 1)).unwrap();
    assert_eq!(f1.raw, point(1, 6));
    assert_eq!(f1.preference_basis, PreferenceBasis::DeclaredUncalibrated);
    assert_eq!(f1.derived.complexity_match, Some(point(1, 1)));
    let f2 = evaluate(&p, F::F2V1, &rows, loss(0, 1)).unwrap();
    assert_eq!(f2.raw, point(5, 32));
    set(&mut rows, M::Redundancy, point(1, 4));
    let f2 = evaluate(&p, F::F2V1, &rows, loss(0, 1)).unwrap();
    assert_eq!(f2.derived.structural_gain, Some(point(-1, 20)));
    assert_eq!(f2.raw, point(3, 20));
}

#[test]
fn tlcx22_original_g05_tnc_uses_dpar_not_dctx() {
    let mut rows = sample();
    set(&mut rows, M::FrameworkDistance, point(1, 4));
    set(&mut rows, M::SemanticConfigurationDistance, point(1, 1));
    set(&mut rows, M::Compatibility, point(1, 2));
    let p = profile(&rows, coefficients(), 80, 80);
    assert_eq!(
        evaluate(&p, F::F2V1, &rows, loss(0, 1))
            .unwrap()
            .derived
            .novelty_compatibility,
        Some(point(1, 4))
    );
    set(&mut rows, M::ContextDistance, point(1, 1));
    assert_eq!(
        evaluate(&p, F::F2V1, &rows, loss(0, 1))
            .unwrap()
            .derived
            .novelty_compatibility,
        Some(point(1, 4))
    );
}

#[test]
fn tlcx22_original_all_27_coefficients_have_original_signed_sensitivity() {
    let mut rows = sample();
    for r in &mut rows {
        if r.metric != M::DescriptionBits {
            r.state = loss(1, 2);
        }
    }
    let p = profile(&rows, coefficients(), 80, 80);
    let reference1 = evaluate(&p, F::F1V1, &rows, loss(1, 2)).unwrap().raw;
    let reference2 = evaluate(&p, F::F2V1, &rows, loss(1, 2)).unwrap().raw;
    let expected1 = [q(1, 2), q(1, 2), q(-1, 2), q(1, 2), q(1, 1), q(-1, 2)];
    let expected2 = [
        q(1, 4),
        q(3, 10),
        q(1, 4),
        q(1, 2),
        q(1, 1),
        q(-1, 2),
        q(-1, 2),
        q(-1, 2),
    ];
    for index in 0..27 {
        let mut c = coefficients();
        let (f, expected) = if index < 6 {
            c.f1[index] += q(1, 1);
            (F::F1V1, expected1[index].clone())
        } else if index < 14 {
            c.f2[index - 6] += q(1, 1);
            (F::F2V1, expected2[index - 6].clone())
        } else if index < 19 {
            let i = index - 14;
            c.structural[i] += q(1, 1);
            (F::F2V1, q(if i == 4 { -1 } else { 1 }, 16))
        } else if index < 23 {
            let i = index - 19;
            c.reorganization[i] += q(1, 1);
            (F::F2V1, q(if i == 3 { -1 } else { 1 }, 16))
        } else {
            c.utility[index - 23] += q(1, 1);
            (F::F2V1, q(1, 16))
        };
        let altered = profile(&rows, c, 80, 80);
        assert_ne!(p.digest(), altered.digest());
        let actual = evaluate(&altered, f, &rows, loss(1, 2)).unwrap().raw;
        let reference = if f == F::F1V1 {
            &reference1
        } else {
            &reference2
        };
        assert_eq!(
            actual.subtract(reference).unwrap(),
            RationalInterval::new(expected.clone(), expected).unwrap(),
            "coefficient{index}"
        );
    }
}

#[test]
fn tlcx22_original_unknown_missing_na_and_zero_weight_never_zero_fill() {
    let mut rows = sample();
    let mut c = coefficients();
    c.utility[3] = q(0, 1);
    let p = profile(&rows, c, 80, 80);
    let solve = rows.iter_mut().find(|r| r.metric == M::Solve).unwrap();
    solve.state = ComponentState::Unknown(MeasurementGap::MissingEvaluator);
    assert!(matches!(
        evaluate(&p, F::F2V1, &rows, loss(0, 1)),
        Err(FormulaError::NoScalar {
            metric: Some(M::Solve),
            ..
        })
    ));
    rows.iter_mut()
        .find(|r| r.metric == M::Solve)
        .unwrap()
        .state = ComponentState::NotApplicable { basis: pin(40) };
    assert!(matches!(
        evaluate(&p, F::F2V1, &rows, loss(0, 1)),
        Err(FormulaError::NoScalar {
            retained: ComponentState::NotApplicable { .. },
            ..
        })
    ));
    rows.retain(|r| r.metric != M::Solve);
    assert_eq!(
        evaluate(&p, F::F2V1, &rows, loss(0, 1)),
        Err(FormulaError::MissingMetric(M::Solve))
    );
}

#[test]
fn tlcx22_original_all_zero_weights_return_algebraic_zero_but_reject_ranking() {
    let rows = sample();
    let c = CoefficientSet {
        f1: std::array::from_fn(|_| q(0, 1)),
        f2: std::array::from_fn(|_| q(0, 1)),
        structural: std::array::from_fn(|_| q(0, 1)),
        reorganization: std::array::from_fn(|_| q(0, 1)),
        utility: std::array::from_fn(|_| q(0, 1)),
    };
    let p = profile(&rows, c, 80, 80);
    for f in [F::F1V1, F::F2V1] {
        let result = evaluate(&p, f, &rows, loss(0, 1)).unwrap();
        assert_eq!(result.raw, point(0, 1));
        assert!(result.degenerate_weights);
        assert_eq!(p.ranking_allowed(f), Err(FormulaError::DegenerateWeights));
    }
}

#[test]
fn tlcx22_original_g07_g08_g20_q_preserves_symmetry_and_crossing_bounds() {
    let mut rows = sample();
    let p = profile(&rows, coefficients(), 160, 80);
    set(&mut rows, M::DescriptionBits, point(80, 1));
    let left = evaluate(&p, F::F2V1, &rows, loss(0, 1))
        .unwrap()
        .derived
        .complexity_parity
        .unwrap();
    set(&mut rows, M::DescriptionBits, point(240, 1));
    let right = evaluate(&p, F::F2V1, &rows, loss(0, 1))
        .unwrap()
        .derived
        .complexity_parity
        .unwrap();
    assert_eq!(left, right);
    assert!(!left.is_exact());
    set(
        &mut rows,
        M::DescriptionBits,
        RationalInterval::from_ratio(80, 1, 240, 1).unwrap(),
    );
    let cross = evaluate(&p, F::F2V1, &rows, loss(0, 1))
        .unwrap()
        .derived
        .complexity_parity
        .unwrap();
    assert_eq!(cross.lower(), left.lower());
    assert_eq!(cross.upper(), &q(1, 1));
    assert!(!cross.is_exact());
    set(
        &mut rows,
        M::DescriptionBits,
        RationalInterval::from_ratio(200, 1, 240, 1).unwrap(),
    );
    let side = evaluate(&p, F::F2V1, &rows, loss(0, 1))
        .unwrap()
        .derived
        .complexity_parity
        .unwrap();
    assert_eq!(side.lower(), left.lower());
    assert!(side.upper() < &q(1, 1));
}

#[test]
fn tlcx22_original_g12_pair_retains_raw_and_nonzero_candidate_delta() {
    let rows = sample();
    let mut candidate_rows = sample();
    for r in &mut candidate_rows {
        r.binding.candidate = pin(30);
    }
    set(&mut candidate_rows, M::Redundancy, point(1, 4));
    let required = [M::DescriptionBits];
    let policy = AssessmentPolicy {
        scope: pin(2),
        baseline: pin(7),
        required: &required,
    };
    let base = CreativeAssessment::from_reports(&policy, &rows).unwrap();
    let candidate = CreativeAssessment::from_reports(&policy, &candidate_rows).unwrap();
    let p = profile(&rows, coefficients(), 80, 80);
    let zero = loss(0, 1);
    let input = FormulaInput {
        assessment: &base,
        translation_loss: TranslationLossInput {
            binding: base.binding(),
            reduction_policy: pin(17),
            state: &zero,
        },
    };
    let c = FormulaInput {
        assessment: &candidate,
        translation_loss: TranslationLossInput {
            binding: candidate.binding(),
            reduction_policy: pin(17),
            state: &zero,
        },
    };
    let identity = evaluate_original_pair(&p, F::F2V1, &input, &input).unwrap();
    assert_eq!(identity.baseline.raw, point(5, 32));
    assert_eq!(identity.delta, point(0, 1));
    assert!(identity.same_input_nodes);
    let changed = evaluate_original_pair(&p, F::F2V1, &c, &input).unwrap();
    assert_eq!(changed.candidate.raw, point(3, 20));
    assert_eq!(changed.delta, point(-1, 160));
    assert!(!changed.same_input_nodes);
}

#[test]
fn tlcx22_original_scope_definition_estimates_and_invalid_coefficients_remain_separate() {
    let mut rows = sample();
    let p = profile(&rows, coefficients(), 80, 80);
    rows.iter_mut()
        .find(|r| r.metric == M::Preserve)
        .unwrap()
        .definition = pin(90);
    assert_eq!(
        evaluate(&p, F::F2V1, &rows, loss(0, 1)),
        Err(FormulaError::DefinitionMismatch(M::Preserve))
    );
    rows = sample();
    rows.iter_mut()
        .find(|r| r.metric == M::Compatibility)
        .unwrap()
        .state = ComponentState::Estimated {
        bounds: point(1, 1),
        method: pin(91),
    };
    let e = evaluate(&p, F::F1V1, &rows, loss(0, 1)).unwrap();
    assert!(e.includes_estimates);
    let binding = FormulaBinding {
        comparison_scope: pin(2),
        baseline_revision: pin(7),
        baseline_candidate: pin(3),
        encoding_revision: pin(16),
        tl_reduction_policy: pin(17),
    };
    let scales = || ScaleParameters {
        baseline_bits: q(80, 1),
        sigma_bits: q(80, 1),
        match_scale_bits: q(80, 1),
    };
    let mut bad = coefficients();
    bad.f1[0] = q(-1, 1);
    assert!(matches!(
        FormulaProfile::checked(binding, bad, scales(), &[(M::DescriptionBits, pin(11))], 64),
        Err(FormulaError::InvalidParameter)
    ));
    let mut raw = coefficients();
    raw.f1[0] = Rational::new_raw(BigInt::from(1), BigInt::from(0));
    assert!(matches!(
        FormulaProfile::checked(binding, raw, scales(), &[(M::DescriptionBits, pin(11))], 64),
        Err(FormulaError::Numeric(NumericError::DivisionByZero))
    ));
}

#[test]
fn tlcx22_original_tl_scope_cut_and_all_nonnumeric_states_return_no_scalar() {
    use lc631_analysis::creativity_measurement::MeasurementError;
    let rows = sample();
    let p = profile(&rows, coefficients(), 80, 80);
    let required = [M::DescriptionBits];
    let a = CreativeAssessment::from_reports(
        &AssessmentPolicy {
            scope: pin(2),
            baseline: pin(7),
            required: &required,
        },
        &rows,
    )
    .unwrap();
    let zero = loss(0, 1);
    let input = FormulaInput {
        assessment: &a,
        translation_loss: TranslationLossInput {
            binding: a.binding(),
            reduction_policy: pin(99),
            state: &zero,
        },
    };
    assert_eq!(
        evaluate_original(&p, F::F1V1, &input),
        Err(FormulaError::ScopeMismatch)
    );
    let states = [
        ComponentState::Unknown(MeasurementGap::Timeout),
        ComponentState::NotApplicable { basis: pin(91) },
        ComponentState::Incomparable { scope: pin(92) },
        ComponentState::Undefined(MeasurementError::PrerequisiteFailed),
        ComponentState::InvalidConfig(MeasurementError::InvalidInput),
        ComponentState::ResourceExhausted {
            partial: Some(point(0, 1)),
        },
    ];
    for state in states {
        let error = evaluate(&p, F::F1V1, &rows, state.clone()).unwrap_err();
        assert_eq!(
            error,
            FormulaError::NoScalar {
                metric: None,
                retained: state
            }
        );
    }
}

#[test]
fn tlcx22_original_shared_uncertain_nodes_self_difference_survives_reordering() {
    let mut rows = sample();
    set(
        &mut rows,
        M::Knownness,
        RationalInterval::from_ratio(0, 1, 1, 1).unwrap(),
    );
    let mut reordered = rows.clone();
    reordered.reverse();
    let mut different = rows.clone();
    for r in &mut different {
        r.binding.candidate = pin(30);
    }
    let required = [M::DescriptionBits];
    let policy = AssessmentPolicy {
        scope: pin(2),
        baseline: pin(7),
        required: &required,
    };
    let base = CreativeAssessment::from_reports(&policy, &rows).unwrap();
    let reordered = CreativeAssessment::from_reports(&policy, &reordered).unwrap();
    let other = CreativeAssessment::from_reports(&policy, &different).unwrap();
    let p = profile(&rows, coefficients(), 80, 80);
    let zero = loss(0, 1);
    let b = FormulaInput {
        assessment: &base,
        translation_loss: TranslationLossInput {
            binding: base.binding(),
            reduction_policy: pin(17),
            state: &zero,
        },
    };
    let r = FormulaInput {
        assessment: &reordered,
        translation_loss: TranslationLossInput {
            binding: reordered.binding(),
            reduction_policy: pin(17),
            state: &zero,
        },
    };
    let o = FormulaInput {
        assessment: &other,
        translation_loss: TranslationLossInput {
            binding: other.binding(),
            reduction_policy: pin(17),
            state: &zero,
        },
    };
    let shared = evaluate_original_pair(&p, F::F1V1, &r, &b).unwrap();
    assert!(shared.same_input_nodes);
    assert!(!shared.baseline.raw.is_exact());
    assert_eq!(shared.delta, point(0, 1));
    let separate = evaluate_original_pair(&p, F::F1V1, &o, &b).unwrap();
    assert!(!separate.same_input_nodes);
    assert_eq!(
        separate.delta,
        RationalInterval::from_ratio(-1, 6, 1, 6).unwrap()
    );
}
