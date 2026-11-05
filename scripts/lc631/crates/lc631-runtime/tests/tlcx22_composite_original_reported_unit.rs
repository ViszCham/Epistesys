//! Runnable independently of missing composite APIs; synthetic arithmetic/control only.
#[path = "tlcx22_composite_original_support/reported.rs"]
mod reported;
use lc631_analysis::creativity_measurement::{formula::*, *};
use reported::*;

#[test]
fn r01_legacy_strict_subject_rejects_heterogeneous_native_targets() {
    let mut rows = reports();
    rows[1].binding.target = d(99);
    let refs: Vec<_> = rows.iter().collect();
    let required = [
        ComponentMetric::DescriptionBits,
        ComponentMetric::ContextDistance,
    ];
    let policy = AssessmentPolicy {
        scope: d(1),
        baseline: d(6),
        required: &required,
    };
    assert!(matches!(
        ComponentAssessment::from_reports(
            &policy,
            CandidateSubject::from_binding(binding()),
            &refs
        ),
        Err(MeasurementError::ScopeMismatch)
    ));
}
#[test]
fn r02_retagging_can_satisfy_reported_shape_but_does_not_validate_source() {
    let mut rows = reports();
    rows[1].binding.target = d(99);
    let original = rows[1].clone();
    let mut retagged = rows[1].clone();
    retagged.binding = binding();
    let refs = [&rows[0], &retagged];
    let required = [
        ComponentMetric::DescriptionBits,
        ComponentMetric::ContextDistance,
    ];
    let assessment = ComponentAssessment::from_reports(
        &AssessmentPolicy {
            scope: d(1),
            baseline: d(6),
            required: &required,
        },
        CandidateSubject::from_binding(binding()),
        &refs,
    )
    .unwrap();
    assert_eq!(
        assessment
            .get(ComponentMetric::ContextDistance)
            .unwrap()
            .validation,
        SemanticValidation::Unassessed
    );
    assert_eq!(retagged.basis, InputBasis::Given);
    assert_eq!(retagged.independence, IndependenceStatus::SharedDependency);
    assert_eq!(rows[1], original);
    // This deliberate acceptance control is why a runtime actual-owner gate is mandatory.
}
#[test]
fn r03_f1_synthetic_identity_is_exact_one_sixth_only_as_reported_arithmetic() {
    let rows = reports();
    let refs: Vec<_> = rows.iter().collect();
    let required = [ComponentMetric::DescriptionBits];
    let assessment = ComponentAssessment::from_reports(
        &AssessmentPolicy {
            scope: d(1),
            baseline: d(6),
            required: &required,
        },
        CandidateSubject::from_binding(binding()),
        &refs,
    )
    .unwrap();
    let loss = ComponentState::Measured(RationalInterval::exact_ratio(0, 1).unwrap());
    let input = ComponentFormulaInput {
        assessment: &assessment,
        translation_loss: TranslationLossInput {
            binding: binding(),
            reduction_policy: d(16),
            state: &loss,
        },
    };
    let p = profile(&rows, false);
    let result = evaluate_components(&p, OriginalFormula::F1V1, &input).unwrap();
    assert_eq!(
        result.arithmetic.raw,
        RationalInterval::exact_ratio(1, 6).unwrap()
    );
    assert_eq!(
        result.arithmetic.preference_basis,
        PreferenceBasis::DeclaredUncalibrated
    );
    assert!(std::ptr::eq(result.input, &input));
}
#[test]
fn r04_zero_knownness_coefficient_does_not_rescue_unknown_leaf() {
    let mut rows = reports();
    rows.iter_mut()
        .find(|r| r.metric == ComponentMetric::Knownness)
        .unwrap()
        .state = ComponentState::Unknown(MeasurementGap::MissingObservation);
    let refs: Vec<_> = rows.iter().collect();
    let required = [ComponentMetric::DescriptionBits];
    let assessment = ComponentAssessment::from_reports(
        &AssessmentPolicy {
            scope: d(1),
            baseline: d(6),
            required: &required,
        },
        CandidateSubject::from_binding(binding()),
        &refs,
    )
    .unwrap();
    let loss = ComponentState::Measured(RationalInterval::exact_ratio(0, 1).unwrap());
    let input = ComponentFormulaInput {
        assessment: &assessment,
        translation_loss: TranslationLossInput {
            binding: binding(),
            reduction_policy: d(16),
            state: &loss,
        },
    };
    let p = profile(&rows, true);
    assert!(matches!(
        evaluate_components(&p, OriginalFormula::F1V1, &input),
        Err(FormulaError::NoScalar {
            metric: Some(ComponentMetric::Knownness),
            ..
        })
    ));
}
