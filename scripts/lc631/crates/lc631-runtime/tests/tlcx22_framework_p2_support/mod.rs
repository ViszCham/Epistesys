//! Given cross-positive archive inputs and literal fractions, no metric implementation.
#![allow(dead_code)]
pub use lc631_runtime::context_novelty_measurement::framework as f;
pub use lc631_runtime::context_novelty_measurement::framework::novelty as m;
#[path = "../tlcx22_framework_p1_support/mod.rs"]
mod p1;
pub use lc631_analysis::creativity_measurement::formula::{
    evaluate_components, CoefficientSet, ComponentFormulaInput, FormulaBinding, FormulaError,
    FormulaProfile, OriginalFormula, ScaleParameters, TranslationLossInput,
};
pub use lc631_analysis::creativity_measurement::{
    AssessmentPolicy, CandidateSubject, ComponentAssessment, ComponentReport, ComponentState,
    MeasurementError, MeasurementGap, RationalInterval,
};
pub use p1::*;

pub fn limits() -> m::Limits {
    m::Limits {
        max_features: 256,
        max_entries: 64,
        max_comparisons: 64,
        max_archive_dispatches: 4096,
        max_assessments: 4,
        max_current_checks: 16,
        max_feature_bytes: 4096,
        max_payload_bytes: 1_048_576,
        max_weight_bits: 1024,
        max_intermediate_bits: 4096,
        plan_ram: 2 * 1024 * 1024,
        assessment_ram: 4 * 1024 * 1024,
        scratch_ram: 8 * 1024 * 1024,
        output_bytes: 65536,
    }
}
pub fn registration<'a>(
    features: &'a [n::ConstructionFeature<'a>],
    archive: m::ArchiveInput<'a>,
) -> m::Registration<'a> {
    m::Registration {
        constructions: features,
        archive,
        corpus_revision: d(90),
        definition_revision: d(91),
        oracle_revision: d(92),
        policy_revision: d(93),
        limits: limits(),
    }
}
pub fn archive<'a>(
    h: &Inputs<'a>,
    query: &'a SmtStatusQuery,
    kernel: f::Kernel,
) -> m::ArchiveEntry<'a> {
    m::ArchiveEntry {
        source: h.source,
        query,
        atoms: h.atoms,
        kernel,
    }
}
pub fn query(h: &Inputs<'_>, expressions: Vec<BoolExpr>) -> SmtStatusQuery {
    SmtStatusQuery::checked(
        h.theory.boolean_symbols().to_vec(),
        vec![],
        expressions,
        SmtEmitLimits::default(),
    )
    .unwrap()
}
pub fn p(h: &Inputs<'_>) -> u32 {
    h.theory.boolean_symbols()[0].get()
}
pub fn q_id(h: &Inputs<'_>) -> u32 {
    h.theory.boolean_symbols()[1].get()
}
pub fn exact(h: &Inputs<'_>) -> SmtStatusQuery {
    query(h, vec![var(p(h)), weak(p(h), q_id(h))])
}
pub fn baseline(h: &Inputs<'_>) -> SmtStatusQuery {
    query(
        h,
        vec![BoolExpr::And(vec![var(p(h)), var(q_id(h))]), var(p(h))],
    )
}
pub fn point(report: &ComponentReport, n: i64, d: i64) {
    assert_eq!(
        report.state,
        ComponentState::Measured(RationalInterval::exact_ratio(n, d).unwrap())
    );
    assert_eq!(report.basis, InputBasis::Given);
    assert_eq!(report.validation, SemanticValidation::Unassessed);
    assert_eq!(report.independence, IndependenceStatus::SharedDependency);
}
pub fn pair<'s, 'a, 'v>(
    root: &'s m::Root<'a, 'v>,
    left: &'s f::OwnedFrameworkCase<'s, 'a, 'v>,
    le: f::ExpectedInputs<'s, 'a, 'v>,
    right: &'s f::OwnedFrameworkCase<'s, 'a, 'v>,
    re: f::ExpectedInputs<'s, 'a, 'v>,
    view: &ResourceReservationView<'_>,
) -> m::ExpectedPair<'s, 'a, 'v> {
    root.expected_pair_current(
        m::CaseRef {
            owner: left,
            expected: le,
        },
        m::CaseRef {
            owner: right,
            expected: re,
        },
        view,
    )
    .unwrap()
}
// Use the REAL existing formula consumer with explicitly missing TL; never synthesize
// a measured TL/K/Dsem/A merely to make a strict original score computable.
pub fn strict_missing(current: m::Current<'_>) {
    let dpar = current.report(ComponentMetric::FrameworkDistance).unwrap();
    let reports = [dpar, current.report(ComponentMetric::Knownness).unwrap()];
    let subject = CandidateSubject::from_binding(dpar.binding);
    let strict = AssessmentPolicy {
        scope: subject.scope,
        baseline: subject.baseline,
        required: &[
            ComponentMetric::FrameworkDistance,
            ComponentMetric::SemanticConfigurationDistance,
            ComponentMetric::Compatibility,
        ],
    };
    assert!(matches!(
        ComponentAssessment::from_reports(&strict, subject, &reports),
        Err(MeasurementError::MissingMetric)
    ));
    let policy = AssessmentPolicy {
        scope: subject.scope,
        baseline: subject.baseline,
        required: &[
            ComponentMetric::FrameworkDistance,
            ComponentMetric::Knownness,
        ],
    };
    let assessment = ComponentAssessment::from_reports(&policy, subject, &reports).unwrap();
    let profile = FormulaProfile::checked(
        FormulaBinding {
            comparison_scope: subject.scope,
            baseline_revision: subject.baseline,
            baseline_candidate: subject.candidate,
            encoding_revision: d(94),
            tl_reduction_policy: d(95),
        },
        CoefficientSet {
            f1: std::array::from_fn(|_| q(0, 1)),
            f2: std::array::from_fn(|_| q(0, 1)),
            structural: std::array::from_fn(|_| q(0, 1)),
            reorganization: std::array::from_fn(|_| q(0, 1)),
            utility: std::array::from_fn(|_| q(0, 1)),
        },
        ScaleParameters {
            baseline_bits: q(8, 1),
            sigma_bits: q(8, 1),
            match_scale_bits: q(8, 1),
        },
        &[
            (dpar.metric, dpar.definition),
            (reports[1].metric, reports[1].definition),
        ],
        64,
    )
    .unwrap();
    assert!(matches!(
        profile.ranking_allowed(OriginalFormula::F2V1),
        Err(FormulaError::DegenerateWeights)
    ));
    let missing = ComponentState::Unknown(MeasurementGap::MissingObservation);
    let mut tl_binding = dpar.binding;
    tl_binding.producer = d(96);
    let input = ComponentFormulaInput {
        assessment: &assessment,
        translation_loss: TranslationLossInput {
            binding: tl_binding,
            reduction_policy: d(95),
            state: &missing,
        },
    };
    assert!(matches!(
        evaluate_components(&profile, OriginalFormula::F2V1, &input),
        Err(FormulaError::NoScalar { metric: None, .. })
    ));
    assert!(current
        .report(ComponentMetric::SemanticConfigurationDistance)
        .is_none());
    assert!(current.report(ComponentMetric::Compatibility).is_none());
    assert!(current
        .report(ComponentMetric::NoveltyCompatibility)
        .is_none());
}

// Independent literal signature bytes; never generated by a production encoder.
pub const COMMON_SIGNATURE: &[u8] = &[
    192, 1, 63, 0, 8, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 1, 0, 0, 1, 0, 1, 0, 1, 1, 0, 1,
    0, 1, 1, 0, 0, 0, 1, 0, 1, 1, 0, 1, 0, 1, 0, 1, 1, 1, 0, 0, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1,
];
pub const CLASSICAL_CLOSURE: &[u8] = &[
    193, 1, 192, 3, 0, 4, 0, 1, 2, 1, 1, 1, 1, 0, 1, 0, 1, 1, 2, 1, 1, 2, 0, 1, 0, 1, 0, 1, 0, 0,
    1, 1, 2, 1, 2, 1, 1, 4, 0, 0, 1, 0, 1, 0, 0, 1, 1, 0, 1, 0, 1, 0, 0, 0, 1, 1, 1, 1, 0, 0, 1, 1,
    3, 1, 2, 1, 1, 8, 0, 0, 0, 1, 0, 1, 0, 0, 0, 1, 1, 0, 1, 0, 0, 1, 0, 1, 0, 1, 0, 0, 1, 1, 1, 0,
    1, 0, 1, 0, 0, 0, 0, 1, 1, 1, 0, 1, 0, 0, 1, 1, 1, 1, 0, 0, 0, 1, 1, 1, 1, 1, 0, 0, 1, 1,
];
pub const PERSISTENT_CLOSURE: &[u8] = &[
    193, 1, 192, 3, 0, 4, 0, 2, 3, 2, 3, 2, 1, 3, 0, 3, 0, 1, 2, 3, 2, 3, 2, 3, 0, 3, 0, 3, 0, 1,
    0, 0, 2, 3, 2, 0, 0, 3, 3, 2, 2, 3, 2, 3, 2, 9, 0, 0, 3, 0, 3, 0, 0, 1, 3, 0, 3, 0, 0, 2, 3, 0,
    3, 0, 1, 0, 0, 0, 2, 3, 1, 1, 0, 0, 2, 3, 1, 2, 0, 0, 2, 3, 2, 0, 0, 0, 3, 3, 2, 1, 0, 0, 3, 3,
    2, 2, 0, 0, 3, 3, 3, 2, 3, 2, 3, 2, 27, 0, 0, 0, 3, 0, 3, 0, 0, 0, 1, 3, 0, 3, 0, 0, 0, 2, 3,
    0, 3, 0, 0, 1, 0, 3, 0, 3, 0, 0, 1, 1, 3, 0, 3, 0, 0, 1, 2, 3, 0, 3, 0, 0, 2, 0, 3, 0, 3, 0, 0,
    2, 1, 3, 0, 3, 0, 0, 2, 2, 3, 0, 3, 0, 1, 0, 0, 0, 0, 2, 3, 1, 0, 1, 0, 0, 2, 3, 1, 0, 2, 0, 0,
    2, 3, 1, 1, 0, 0, 0, 2, 3, 1, 1, 1, 0, 0, 2, 3, 1, 1, 2, 0, 0, 2, 3, 1, 2, 0, 0, 0, 2, 3, 1, 2,
    1, 0, 0, 2, 3, 1, 2, 2, 0, 0, 2, 3, 2, 0, 0, 0, 0, 3, 3, 2, 0, 1, 0, 0, 3, 3, 2, 0, 2, 0, 0, 3,
    3, 2, 1, 0, 0, 0, 3, 3, 2, 1, 1, 0, 0, 3, 3, 2, 1, 2, 0, 0, 3, 3, 2, 2, 0, 0, 0, 3, 3, 2, 2, 1,
    0, 0, 3, 3, 2, 2, 2, 0, 0, 3, 3,
];

#[macro_export]
macro_rules! p2_pair {
    ($root:ident,$generated:ident,$view:ident,$left:ident,$le:ident,$right:ident,$re:ident) => {
        let $le = $root
            .expected_case_current(f::Subject::Original, f::Kernel::ClassicalPointV1, $view)
            .unwrap();
        let $left = $root.execute_case_current($le, $view).unwrap();
        let $re = $root
            .expected_case_current(
                f::Subject::Generated(&$generated),
                f::Kernel::PersistentChain2V1,
                $view,
            )
            .unwrap();
        let $right = $root.execute_case_current($re, $view).unwrap();
    };
}
