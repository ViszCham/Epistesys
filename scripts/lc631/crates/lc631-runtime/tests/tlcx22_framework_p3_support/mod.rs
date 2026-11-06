//! Test-owned requests/literal oracles only. No parser, solver, status reducer or success stub.
#![allow(dead_code)]
#[path = "../tlcx22_framework_p2_support/mod.rs"]
mod p2;
pub use lc631_runtime::context_novelty_measurement::framework::novelty::semantic as s;
pub use p2::*;

pub fn semantic_limits() -> s::Limits {
    s::Limits {
        max_probes: 3,
        max_bridge_probes: 3,
        max_assessments: 2,
        max_current_checks: 4,
        max_native_dispatches: 78,
        max_lowered_nodes: 128,
        max_payload_bytes: 65536,
        max_weight_bits: 1024,
        max_intermediate_bits: 4096,
        plan_ram: 2 * 1024 * 1024,
        assessment_ram: 4 * 1024 * 1024,
        scratch_ram: 8 * 1024 * 1024,
        output_bytes: 65536,
        precision: 64,
    }
}
pub fn semantic_registration<'a>(
    semantic: &'a [s::SemanticProbe<'a>],
    bridges: &'a [s::BridgeProbe<'a>],
) -> s::Registration<'a> {
    s::Registration {
        semantic,
        bridges,
        status_map: Some(s::StatusMap::CanonicalForcingV1),
        bridge_map: Some(s::BridgeMap::ActualProfileMapV1),
        definition_revision: d(101),
        oracle_revision: d(102),
        policy_revision: d(103),
        limits: semantic_limits(),
    }
}
pub fn lem_probe<'a>(h: &Inputs<'a>, weight: &'a Rational) -> s::SemanticProbe<'a> {
    s::SemanticProbe {
        role: f::QueryRole::BridgeLaw,
        formula: h.probes[2].formula,
        weight,
    }
}
pub fn bridges<'a>(weight: &'a Rational) -> [s::BridgeProbe<'a>; 2] {
    [
        s::BridgeProbe {
            role: f::QueryRole::GoalEcho,
            rule: s::BridgeRule::RootGoalUseV1,
            weight,
        },
        s::BridgeProbe {
            role: f::QueryRole::BridgeLaw,
            rule: s::BridgeRule::MappedRootForcingV1,
            weight,
        },
    ]
}
pub fn semantic_pair<'s, 'a, 'v>(
    root: &'s s::Root<'a, 'v>,
    source: &'s f::OwnedFrameworkCase<'s, 'a, 'v>,
    se: f::ExpectedInputs<'s, 'a, 'v>,
    target: &'s f::OwnedFrameworkCase<'s, 'a, 'v>,
    te: f::ExpectedInputs<'s, 'a, 'v>,
    view: &ResourceReservationView<'_>,
) -> s::ExpectedPair<'s, 'a, 'v> {
    root.expected_pair_current(
        m::CaseRef {
            owner: source,
            expected: se,
        },
        m::CaseRef {
            owner: target,
            expected: te,
        },
        view,
    )
    .unwrap()
}
// The middle legal valuation is future-only. Bits are root bit0/future bit1.
// No production function generates these test-owned arrays.
pub const Q_BITS: [u64; 3] = [0, 2, 3];
pub const NOT_Q_BITS: [u64; 3] = [3, 0, 0];
pub const LEM_BITS: [u64; 3] = [3, 2, 3];
pub const NOT_LEM_BITS: [u64; 3] = [0, 0, 0];
pub const Q_STATUS: [s::CanonicalStatus; 4] = [
    s::CanonicalStatus::Supported,
    s::CanonicalStatus::Undetermined,
    s::CanonicalStatus::Supported,
    s::CanonicalStatus::Undetermined,
];
pub const LEM_STATUS: [s::CanonicalStatus; 4] = [
    s::CanonicalStatus::Supported,
    s::CanonicalStatus::Supported,
    s::CanonicalStatus::Supported,
    s::CanonicalStatus::Undetermined,
];

pub fn measured(v: &s::Current<'_>, metric: ComponentMetric, n: i64, d: i64) {
    point(v.report(metric).unwrap(), n, d);
}
pub fn no_full_f2(v: &s::Current<'_>) {
    let metrics = [
        ComponentMetric::FrameworkDistance,
        ComponentMetric::SemanticConfigurationDistance,
        ComponentMetric::Compatibility,
        ComponentMetric::Knownness,
        ComponentMetric::NoveltyCompatibility,
    ];
    let reports: Vec<_> = metrics.iter().map(|m| v.report(*m).unwrap()).collect();
    let subject = CandidateSubject::from_binding(reports[0].binding);
    let policy = AssessmentPolicy {
        scope: subject.scope,
        baseline: subject.baseline,
        required: &metrics,
    };
    let assessment = ComponentAssessment::from_reports(&policy, subject, &reports).unwrap();
    let profile = FormulaProfile::checked(
        FormulaBinding {
            comparison_scope: subject.scope,
            baseline_revision: subject.baseline,
            baseline_candidate: subject.candidate,
            encoding_revision: d(104),
            tl_reduction_policy: d(105),
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
        &reports
            .iter()
            .map(|r| (r.metric, r.definition))
            .collect::<Vec<_>>(),
        64,
    )
    .unwrap();
    assert!(matches!(
        profile.ranking_allowed(OriginalFormula::F2V1),
        Err(FormulaError::DegenerateWeights)
    ));
    let missing = ComponentState::Unknown(MeasurementGap::MissingObservation);
    let mut binding = reports[0].binding;
    binding.producer = d(106);
    let input = ComponentFormulaInput {
        assessment: &assessment,
        translation_loss: TranslationLossInput {
            binding,
            reduction_policy: d(105),
            state: &missing,
        },
    };
    assert!(matches!(
        evaluate_components(&profile, OriginalFormula::F2V1, &input),
        Err(FormulaError::NoScalar { metric: None, .. })
    ));
    assert!(v.report(ComponentMetric::OriginalV2).is_none());
}
#[macro_export]
macro_rules! p3_root {
    ($h:ident,$view:ident,$fd:ident,$nr:ident,$sr:ident,$root:ident) => {
        let $fd = $h.declaration();
        let $nr = registration($h.constructions, m::ArchiveInput::Unread);
        let native = f::FrameworkRoot::preregister($h.native($view), &$fd, $view).unwrap();
        let novelty = m::Root::preregister(native, &$nr, $view).unwrap();
        let mut $root = s::Root::preregister(novelty, &$sr, $view).unwrap();
    };
}
