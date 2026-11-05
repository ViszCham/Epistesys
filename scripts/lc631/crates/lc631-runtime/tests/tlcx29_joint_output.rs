//! Hand-written reported arithmetic inputs; not actual-solver or semantic gold.
use lc631_analysis::creativity_measurement::RationalInterval;
use lc631_core::context::*;
use lc631_core::{AuthorityRevision, CandidateId};
use lc631_runtime::joint_selection::*;
use lc631_runtime::recommendation_output::*;
use lc631_substrate::Digest32;
use std::sync::Arc;

fn d(byte: u8) -> Digest32 {
    Digest32::from_bytes([byte; 32])
}

fn fixture(
    test: impl FnOnce(
        &mut SelectionPolicy<'_>,
        &mut [JointCandidate<'_>],
        &[CandidateExplanation<'_>],
        &mut [ExplanationBinding],
    ),
) {
    let source = SourceRevision::of("question");
    let scope = ContextScope::declared(d(1), d(2), d(3), d(4));
    let original = OriginalContext::checked(
        source,
        scope,
        AuthorityRevision(1),
        vec![],
        ContextLimits {
            max_contexts: 4,
            max_premises: 4,
            max_edits: 4,
            max_statement_bytes: 1024,
        },
    )
    .unwrap();
    let mut contexts = ContextConsumer::new(original).unwrap();
    let delta = ContextDelta::proposed(
        contexts.original().state().revision(),
        scope,
        AuthorityRevision(1),
        ContextApplicability::Unconditional,
        vec![PremiseEdit::Add(ContextPremise::reported(
            d(5),
            Arc::from("Reconstructed premise"),
            PremiseBasis::Proposed,
            true,
        ))],
    );
    let rebuilt = match contexts.admit(delta).unwrap() {
        ContextAdmission::Inserted(value) => value,
        ContextAdmission::Duplicate(_) => panic!("fixture changes premise"),
    };
    let history = [contexts.original().state()];
    let pins = SelectionPins {
        source,
        scope,
        baseline: d(6),
        hard_invariants: d(7),
        policy: d(8),
        goal_profile: Some(d(9)),
        evidence_revision: d(10),
    };
    let axes = [AxisKind::Goal(d(11))];
    let mut policy = SelectionPolicy {
        pins,
        axes: &axes,
        representative_order: None,
        declared_goal_solve_overlap: false,
        max_candidates: 32,
        max_canonical_bytes: 1024,
    };
    let low = RationalInterval::exact_ratio(1, 4).unwrap();
    let high = RationalInterval::exact_ratio(3, 4).unwrap();
    let zero = RationalInterval::exact_ratio(0, 1).unwrap();
    let a = [AxisValue::Exact(&low)];
    let b = [AxisValue::Exact(&high)];
    let root = contexts.original().state();
    let rebuilt = contexts.get(rebuilt).unwrap();
    let mut rows = [
        JointCandidate {
            id: CandidateId(1),
            context: root.revision(),
            pins,
            hard_gate: ReportedGate::Pass,
            map_back: ReportedGate::Pass,
            axes: &a,
            total_loss: AxisValue::Exact(&zero),
            canonical_key: b"original",
            measurement_binding: d(12),
        },
        JointCandidate {
            id: CandidateId(2),
            context: rebuilt.revision(),
            pins,
            hard_gate: ReportedGate::Pass,
            map_back: ReportedGate::Pass,
            axes: &b,
            total_loss: AxisValue::Exact(&zero),
            canonical_key: b"rebuilt",
            measurement_binding: d(13),
        },
    ];
    let facets = FourFacetRefs {
        defects: None,
        delta: None,
        uncertainty: None,
        gain: None,
    };
    let explanations = [
        CandidateExplanation {
            context: root,
            context_history: &[],
            result: d(14),
            explanation: "Original answer",
            dependencies: &[],
            facets,
        },
        CandidateExplanation {
            context: rebuilt,
            context_history: &history,
            result: d(15),
            explanation: "Reconstructed answer",
            dependencies: &[],
            facets,
        },
    ];
    let mut bindings = [
        ExplanationBinding {
            candidate: CandidateId(1),
            measurement: d(12),
            result: d(14),
        },
        ExplanationBinding {
            candidate: CandidateId(2),
            measurement: d(13),
            result: d(15),
        },
    ];
    test(&mut policy, &mut rows, &explanations, &mut bindings);
}

#[test]
fn tlcx29_connector_rejects_swapped_and_cut_lineage() {
    fixture(|policy, rows, explanations, bindings| {
        bindings.swap(0, 1);
        assert!(matches!(
            connect_joint_reported(
                "question",
                policy,
                rows,
                SearchCoverage::EvaluatedSet,
                explanations,
                bindings
            ),
            Err(ConnectionError::LineageMismatch)
        ));
        bindings.swap(0, 1);
        assert!(matches!(
            connect_joint_reported(
                "question",
                policy,
                rows,
                SearchCoverage::EvaluatedSet,
                explanations,
                &bindings[..1]
            ),
            Err(ConnectionError::CountMismatch)
        ));
        bindings[1].measurement = d(99);
        assert!(matches!(
            connect_joint_reported(
                "question",
                policy,
                rows,
                SearchCoverage::EvaluatedSet,
                explanations,
                bindings
            ),
            Err(ConnectionError::LineageMismatch)
        ));
    });
}

#[test]
fn tlcx29_actual_selector_consumption_changes_exact_output_after_gate_cut() {
    fixture(|policy, rows, explanations, bindings| {
        let selected = connect_joint_reported(
            "question",
            policy,
            rows,
            SearchCoverage::BudgetExhausted,
            explanations,
            bindings,
        )
        .unwrap();
        assert_eq!(selected.recommendation(), Some(1));
        let first = selected.render(16384).unwrap();
        assert!(first.text().contains("BudgetLimited"));
        assert!(first.text().contains("Reconstructed premise"));
        assert!(first.text().contains("Unknown/NeedsEvidence"));
        drop(selected);
        rows[1].hard_gate = ReportedGate::NeedsEvidence;
        let cut = connect_joint_reported(
            "question",
            policy,
            rows,
            SearchCoverage::BudgetExhausted,
            explanations,
            bindings,
        )
        .unwrap();
        assert_eq!(cut.recommendation(), Some(0));
        let second = cut.render(16384).unwrap();
        assert_ne!(first.binding(), second.binding());
        assert!(second.text().contains("NeedsEvidence"));
        assert!(!first.matches_exact("question", second.text().as_bytes()));
        assert!(!first.matches_exact("different question", first.text().as_bytes()));
        assert_eq!(second.explanation_tl(), ExplanationTl::NeedsEvidence);
    });
}

#[test]
fn tlcx29_goal_cut_keeps_alternatives_without_cached_recommendation() {
    fixture(|policy, rows, explanations, bindings| {
        policy.pins.goal_profile = None;
        for row in rows.iter_mut() {
            row.pins = policy.pins;
        }
        let cut = connect_joint_reported(
            "question",
            policy,
            rows,
            SearchCoverage::EvaluatedSet,
            explanations,
            bindings,
        )
        .unwrap();
        assert_eq!(cut.recommendation(), None);
        let text = cut.render(16384).unwrap();
        assert!(text.text().contains("NoGoalProfile"));
        assert!(text.text().contains("Original answer"));
        assert!(text.text().contains("Reconstructed answer"));
        assert!(matches!(cut.render(64), Err(OutputError::OutputBudget)));
    });
}

#[test]
fn tlcx29_result_and_source_swaps_do_not_render() {
    fixture(|policy, rows, explanations, bindings| {
        assert!(matches!(
            connect_joint_reported(
                "other",
                policy,
                rows,
                SearchCoverage::EvaluatedSet,
                explanations,
                bindings
            ),
            Err(ConnectionError::SourceMismatch)
        ));
        bindings[0].result = d(99);
        assert!(matches!(
            connect_joint_reported(
                "question",
                policy,
                rows,
                SearchCoverage::EvaluatedSet,
                explanations,
                bindings
            ),
            Err(ConnectionError::LineageMismatch)
        ));
    });
}
