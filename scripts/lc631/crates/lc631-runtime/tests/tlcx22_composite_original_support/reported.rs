//! Synthetic reported inputs only; never actual metric observations or source validation.
use lc631_analysis::creativity_measurement::{formula::*, *};
use lc631_substrate::Digest32;
pub fn d(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}
pub fn binding() -> MeasurementBinding {
    MeasurementBinding {
        scope: d(1),
        candidate: d(2),
        source: d(3),
        target: d(4),
        context: d(5),
        baseline: d(6),
        producer: d(7),
        transform: d(8),
        result: d(9),
    }
}
pub fn reports() -> Vec<ComponentReport> {
    [
        ComponentMetric::DescriptionBits,
        ComponentMetric::ContextDistance,
        ComponentMetric::SemanticConfigurationDistance,
        ComponentMetric::Compatibility,
        ComponentMetric::Knownness,
    ]
    .into_iter()
    .map(|metric| ComponentReport {
        metric,
        definition: d(10),
        binding: binding(),
        probe_set: d(11),
        oracle: d(12),
        dependency_group: d(13),
        evidence_payload: d(14),
        basis: InputBasis::Given,
        validation: SemanticValidation::Unassessed,
        independence: IndependenceStatus::SharedDependency,
        state: ComponentState::Measured(
            RationalInterval::exact_ratio(
                match metric {
                    ComponentMetric::DescriptionBits => 80,
                    ComponentMetric::Compatibility | ComponentMetric::Knownness => 1,
                    _ => 0,
                },
                1,
            )
            .unwrap(),
        ),
    })
    .collect()
}
pub fn profile(rows: &[ComponentReport], zero_knownness_coefficient: bool) -> FormulaProfile {
    let q = |n: i64, d: i64| Rational::new(n.into(), d.into());
    let mut f1 = std::array::from_fn(|_| q(1_i64, 6_i64));
    if zero_knownness_coefficient {
        f1[5] = q(0, 1);
    }
    FormulaProfile::checked(
        FormulaBinding {
            comparison_scope: d(1),
            baseline_revision: d(6),
            baseline_candidate: d(2),
            encoding_revision: d(15),
            tl_reduction_policy: d(16),
        },
        CoefficientSet {
            f1,
            f2: std::array::from_fn(|_| q(1, 8)),
            structural: std::array::from_fn(|_| q(1, 5)),
            reorganization: std::array::from_fn(|_| q(1, 4)),
            utility: std::array::from_fn(|_| q(1, 4)),
        },
        ScaleParameters {
            baseline_bits: q(80, 1),
            sigma_bits: q(80, 1),
            match_scale_bits: q(80, 1),
        },
        &rows.iter().map(|r| (r.metric, d(10))).collect::<Vec<_>>(),
        64,
    )
    .unwrap()
}
