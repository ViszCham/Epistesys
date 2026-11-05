use lc631_analysis::creativity_measurement::formula::{
    evaluate_components, CoefficientSet, ComponentFormulaInput, FormulaBinding, FormulaError,
    FormulaProfile, OriginalFormula, ScaleParameters, TranslationLossInput,
};
use lc631_analysis::creativity_measurement::{
    AssessmentPolicy, CandidateSubject, ComponentAssessment, ComponentMetric as M, ComponentReport,
    ComponentState, CreativeAssessment, IndependenceStatus, InputBasis, MeasurementBinding,
    MeasurementError, MeasurementGap, Rational, RationalInterval, SemanticValidation,
};
use lc631_substrate::Digest32;

fn pin(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}

fn report(metric: M, producer: u8) -> ComponentReport {
    ComponentReport {
        metric,
        definition: pin(10),
        binding: MeasurementBinding {
            scope: pin(1),
            candidate: pin(2),
            source: pin(3),
            target: pin(4),
            context: pin(5),
            baseline: pin(6),
            producer: pin(producer),
            transform: pin(producer + 1),
            result: pin(producer + 2),
        },
        probe_set: pin(producer + 3),
        oracle: pin(producer + 4),
        dependency_group: pin(11),
        evidence_payload: pin(producer + 5),
        basis: InputBasis::Given,
        validation: SemanticValidation::Unassessed,
        independence: IndependenceStatus::SharedDependency,
        state: ComponentState::Measured(RationalInterval::exact_ratio(1, 2).unwrap()),
    }
}

#[test]
fn tlcx22_same_candidate_does_not_require_identical_metric_producers() {
    let reports = [report(M::ContextDistance, 20), report(M::Knownness, 40)];
    let policy = AssessmentPolicy {
        scope: pin(1),
        baseline: pin(6),
        required: &[M::ContextDistance, M::Knownness],
    };
    // Preserve the legacy single-producer API; the additive view is explicit.
    assert!(CreativeAssessment::from_reports(&policy, &reports).is_err());
    let subject = CandidateSubject::from_binding(reports[0].binding);
    let refs = [&reports[0], &reports[1]];
    let assessment = ComponentAssessment::from_reports(&policy, subject, &refs).unwrap();
    assert!(std::ptr::eq(assessment.reports(), refs.as_slice()));
    assert!(std::ptr::eq(
        assessment.get(M::Knownness).unwrap(),
        &reports[1]
    ));
    assert_ne!(reports[0].binding.producer, reports[1].binding.producer);
    assert_ne!(reports[0].binding.result, reports[1].binding.result);
}

#[test]
fn tlcx22_component_subject_swaps_are_not_cross_representation_bridges() {
    let original = report(M::ContextDistance, 20);
    let subject = CandidateSubject::from_binding(original.binding);
    let policy = AssessmentPolicy {
        scope: pin(1),
        baseline: pin(6),
        required: &[M::ContextDistance, M::Knownness],
    };
    for field in 0..6 {
        let mut other = report(M::Knownness, 40);
        match field {
            0 => other.binding.scope = pin(99),
            1 => other.binding.candidate = pin(99),
            2 => other.binding.source = pin(99),
            3 => other.binding.target = pin(99),
            4 => other.binding.context = pin(99),
            5 => other.binding.baseline = pin(99),
            _ => unreachable!(),
        }
        assert!(matches!(
            ComponentAssessment::from_reports(&policy, subject, &[&original, &other]),
            Err(MeasurementError::ScopeMismatch)
        ));
    }
    let mut foreign = subject;
    foreign.candidate = pin(99);
    assert!(matches!(
        ComponentAssessment::from_reports(&policy, foreign, &[&original]),
        Err(MeasurementError::ScopeMismatch)
    ));
}

#[test]
fn tlcx22_component_admission_preserves_missing_duplicate_and_bounded_states() {
    let rows = [report(M::ContextDistance, 20), report(M::Knownness, 40)];
    let subject = CandidateSubject::from_binding(rows[0].binding);
    let required = [M::ContextDistance, M::Knownness];
    let refs = [&rows[0], &rows[1]];
    let policy = AssessmentPolicy {
        scope: pin(1),
        baseline: pin(6),
        required: &required,
    };
    assert!(matches!(
        ComponentAssessment::from_reports(&policy, subject, &[]),
        Err(MeasurementError::InvalidInput)
    ));
    assert!(matches!(
        ComponentAssessment::from_reports(&policy, subject, &refs[..1]),
        Err(MeasurementError::MissingMetric)
    ));
    assert!(matches!(
        ComponentAssessment::from_reports(&policy, subject, &[&rows[0], &rows[0]]),
        Err(MeasurementError::DuplicateMetric)
    ));
    let oversized = vec![M::ContextDistance; 30];
    let policy = AssessmentPolicy {
        required: &oversized,
        ..policy
    };
    assert!(matches!(
        ComponentAssessment::from_reports(&policy, subject, &refs),
        Err(MeasurementError::InvalidInput)
    ));
    let policy = AssessmentPolicy {
        required: &[M::ContextDistance, M::ContextDistance],
        ..policy
    };
    assert!(matches!(
        ComponentAssessment::from_reports(&policy, subject, &refs),
        Err(MeasurementError::DuplicateMetric)
    ));
}

fn identity_reports() -> Vec<ComponentReport> {
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
    .into_iter()
    .enumerate()
    .map(|(index, metric)| {
        let mut value = report(metric, 20 + u8::try_from(index).unwrap() * 7);
        let raw = match metric {
            M::Compatibility | M::Knownness | M::Preserve => 1,
            M::DescriptionBits => 80,
            _ => 0,
        };
        value.state = ComponentState::Measured(RationalInterval::exact_ratio(raw, 1).unwrap());
        value
    })
    .collect()
}

fn profile(rows: &[ComponentReport]) -> FormulaProfile {
    FormulaProfile::checked(
        FormulaBinding {
            comparison_scope: pin(1),
            baseline_revision: pin(6),
            baseline_candidate: pin(2),
            encoding_revision: pin(8),
            tl_reduction_policy: pin(9),
        },
        CoefficientSet {
            f1: std::array::from_fn(|_| Rational::new(1.into(), 6.into())),
            f2: std::array::from_fn(|_| Rational::new(1.into(), 8.into())),
            structural: std::array::from_fn(|_| Rational::new(1.into(), 5.into())),
            reorganization: std::array::from_fn(|_| Rational::new(1.into(), 4.into())),
            utility: std::array::from_fn(|_| Rational::new(1.into(), 4.into())),
        },
        ScaleParameters {
            baseline_bits: Rational::from_integer(80.into()),
            sigma_bits: Rational::from_integer(80.into()),
            match_scale_bits: Rational::from_integer(80.into()),
        },
        &rows
            .iter()
            .map(|r| (r.metric, r.definition))
            .collect::<Vec<_>>(),
        64,
    )
    .unwrap()
}

#[test]
fn tlcx22_distinct_component_lineage_reaches_unmodified_f1_and_f2_arithmetic() {
    // Synthetic arithmetic vectors, not observed real-world creativity/evidence.
    let rows = identity_reports();
    let p = profile(&rows);
    let policy = AssessmentPolicy {
        scope: pin(1),
        baseline: pin(6),
        required: &[M::DescriptionBits],
    };
    let subject = CandidateSubject::from_binding(rows[0].binding);
    let refs: Vec<_> = rows.iter().collect();
    let a = ComponentAssessment::from_reports(&policy, subject, &refs).unwrap();
    let loss = ComponentState::Measured(RationalInterval::exact_ratio(0, 1).unwrap());
    let tl_binding = report(M::DescriptionBits, 200).binding;
    let input = ComponentFormulaInput {
        assessment: &a,
        translation_loss: TranslationLossInput {
            binding: tl_binding,
            reduction_policy: pin(9),
            state: &loss,
        },
    };
    for (f, expected) in [
        (OriginalFormula::F1V1, (1, 6)),
        (OriginalFormula::F2V1, (5, 32)),
    ] {
        let result = evaluate_components(&p, f, &input).unwrap();
        assert_eq!(
            result.arithmetic.raw,
            RationalInterval::exact_ratio(expected.0, expected.1).unwrap()
        );
        assert_eq!(result.subject, subject);
        assert_eq!(result.input.translation_loss.binding, tl_binding);
        for (observed, original) in result.input.assessment.reports().iter().zip(&rows) {
            assert!(std::ptr::eq(*observed, original));
            assert_eq!(observed.basis, InputBasis::Given);
            assert_eq!(observed.validation, SemanticValidation::Unassessed);
            assert_eq!(observed.independence, IndependenceStatus::SharedDependency);
        }
    }
}

#[test]
fn tlcx22_component_formula_cut_and_foreign_tl_never_rescue_a_score() {
    let mut rows = identity_reports();
    let p = profile(&rows);
    let policy = AssessmentPolicy {
        scope: pin(1),
        baseline: pin(6),
        required: &[M::DescriptionBits],
    };
    let subject = CandidateSubject::from_binding(rows[0].binding);
    let index = rows.iter().position(|r| r.metric == M::Knownness).unwrap();
    rows[index].state = ComponentState::Unknown(MeasurementGap::MissingEvaluator);
    let refs: Vec<_> = rows.iter().collect();
    let a = ComponentAssessment::from_reports(&policy, subject, &refs).unwrap();
    let loss = ComponentState::Measured(RationalInterval::exact_ratio(0, 1).unwrap());
    let mut input = ComponentFormulaInput {
        assessment: &a,
        translation_loss: TranslationLossInput {
            binding: report(M::DescriptionBits, 200).binding,
            reduction_policy: pin(9),
            state: &loss,
        },
    };
    assert!(matches!(
        evaluate_components(&p, OriginalFormula::F1V1, &input),
        Err(FormulaError::NoScalar {
            metric: Some(M::Knownness),
            retained: ComponentState::Unknown(MeasurementGap::MissingEvaluator)
        })
    ));
    input.translation_loss.binding.context = pin(99);
    assert!(matches!(
        evaluate_components(&p, OriginalFormula::F2V1, &input),
        Err(FormulaError::ScopeMismatch)
    ));
}
