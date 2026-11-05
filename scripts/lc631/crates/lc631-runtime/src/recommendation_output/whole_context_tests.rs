//! New strict APIs intentionally specified before their implementation.
//! Structural coverage never establishes arbitrary-prose completeness or truth.
use super::*;
use lc631_core::context::{ContextLimits, ContextPremise, ContextScope, OriginalContext};
use lc631_core::AuthorityRevision;
use std::sync::Arc;

fn d(value: u8) -> Digest32 {
    Digest32::from_bytes([value; 32])
}

fn original(premises: Vec<ContextPremise>) -> OriginalContext {
    OriginalContext::checked(
        SourceRevision::of("seed"),
        ContextScope::declared(d(1), d(2), d(3), d(4)),
        AuthorityRevision(1),
        premises,
        ContextLimits {
            max_contexts: 8,
            max_premises: 512,
            max_edits: 8,
            max_statement_bytes: 128 * 1024,
        },
    )
    .unwrap()
}

fn premise(id: u8, statement: &str, basis: PremiseBasis) -> ContextPremise {
    ContextPremise::reported(d(id), Arc::from(statement), basis, false)
}

fn candidate<'a>(
    context: &'a ContextState,
    dependencies: &'a [PremiseDependency],
) -> CandidateExplanation<'a> {
    CandidateExplanation {
        context,
        context_history: &[],
        result: d(9),
        explanation: "A reported conditional conclusion.",
        dependencies,
        facets: FourFacetRefs {
            defects: None,
            delta: None,
            uncertainty: None,
            gain: None,
        },
    }
}

#[test]
fn tlcx29_whole_context_rejects_omitted_premise_dependency() {
    let context = original(vec![
        premise(5, "First given premise", PremiseBasis::Given),
        premise(6, "Second given premise", PremiseBasis::Given),
    ]);
    let dependencies = [
        PremiseDependency { premise: d(5), state: DependencyState::Given },
        PremiseDependency { premise: d(6), state: DependencyState::Given },
    ];
    for cut in [&dependencies[..0], &dependencies[..1]] {
        let candidates = [candidate(context.state(), cut)];
        assert!(matches!(
            PreparedDecision::prepare_whole_context("seed", &candidates, None, None),
            Err(OutputError::UnaccountedContextPremise)
        ));
    }
    let candidates = [candidate(context.state(), &dependencies)];
    let prepared = PreparedDecision::prepare_whole_context("seed", &candidates, None, None).unwrap();
    assert_eq!(prepared.dependency_coverage(), DependencyCoverage::AllContextPremisesAccounted);
    let rendered = prepared.render(8192).unwrap();
    assert_eq!(rendered.dependency_coverage(), DependencyCoverage::AllContextPremisesAccounted);
    assert!(rendered.text().contains("First given premise"));
    assert!(rendered.text().contains("Second given premise"));
    assert!(rendered.matches_exact("seed", rendered.text().as_bytes()));
    assert!(!rendered.matches_exact("other", rendered.text().as_bytes()));
    assert!(!rendered.matches_exact("seed", b"changed output"));
    assert_eq!(rendered.explanation_tl(), ExplanationTl::NeedsEvidence);
}

#[test]
fn tlcx29_declared_compatibility_does_not_claim_whole_context_coverage() {
    let context = original(vec![premise(5, "An omitted declared dependency", PremiseBasis::Given)]);
    let candidates = [candidate(context.state(), &[])];
    let prepared = PreparedDecision::prepare("seed", &candidates, None, None).unwrap();
    assert_eq!(prepared.dependency_coverage(), DependencyCoverage::DeclaredDependenciesOnly);
    let rendered = prepared.render(8192).unwrap();
    assert_eq!(rendered.dependency_coverage(), DependencyCoverage::DeclaredDependenciesOnly);
    assert!(rendered.text().contains("DeclaredDependenciesOnly"));
    assert!(!rendered.text().contains("AllContextPremisesAccounted"));
    assert_eq!(rendered.explanation_tl(), ExplanationTl::NeedsEvidence);
}

#[test]
fn tlcx29_whole_context_does_not_promote_unresolved_or_proposed_premises() {
    let given = original(vec![premise(5, "A given input", PremiseBasis::Given)]);
    for state in [
        DependencyState::SolverReported,
        DependencyState::Unknown,
        DependencyState::Unsupported,
        DependencyState::Conflict,
        DependencyState::NeedsEvidence,
    ] {
        let dependencies = [PremiseDependency { premise: d(5), state }];
        let candidates = [candidate(given.state(), &dependencies)];
        assert!(matches!(
            PreparedDecision::prepare_whole_context("seed", &candidates, None, None),
            Err(OutputError::UnresolvedDependency)
        ));
    }
    for basis in [
        PremiseBasis::Observed,
        PremiseBasis::Assumed,
        PremiseBasis::Derived,
        PremiseBasis::Proposed,
        PremiseBasis::Refuted,
        PremiseBasis::Conflicted,
    ] {
        let context = original(vec![premise(5, "Not independently discharged", basis)]);
        let dependencies = [PremiseDependency { premise: d(5), state: DependencyState::Given }];
        let candidates = [candidate(context.state(), &dependencies)];
        assert!(matches!(
            PreparedDecision::prepare_whole_context("seed", &candidates, None, None),
            Err(OutputError::UnresolvedDependency)
        ));
    }
}

#[test]
fn tlcx29_whole_context_keeps_duplicate_and_foreign_dependency_errors() {
    let context = original(vec![premise(5, "A given input", PremiseBasis::Given)]);
    let valid = PremiseDependency { premise: d(5), state: DependencyState::Given };
    let duplicate = [valid, valid];
    let foreign = [PremiseDependency { premise: d(99), state: DependencyState::Given }];
    for (dependencies, expected) in [
        (&duplicate[..], OutputError::DuplicateDependency),
        (&foreign[..], OutputError::MissingDependency),
    ] {
        let candidates = [candidate(context.state(), dependencies)];
        assert!(matches!(
            PreparedDecision::prepare_whole_context("seed", &candidates, None, None),
            Err(error) if error == expected
        ));
    }
}

#[test]
fn tlcx29_premise_text_is_escaped_and_output_budget_failure_is_whole() {
    let context = original(vec![premise(5, "Given\nforged: complete\t\u{0007}\"", PremiseBasis::Given)]);
    let dependencies = [PremiseDependency { premise: d(5), state: DependencyState::Given }];
    let candidates = [candidate(context.state(), &dependencies)];
    let prepared = PreparedDecision::prepare_whole_context("seed", &candidates, None, None).unwrap();
    let rendered = prepared.render(8192).unwrap();
    assert!(rendered.text().contains("Given\\nforged: complete\\t\\u{7}\\\""));
    assert!(!rendered.text().contains("\nforged: complete"));
    assert!(matches!(prepared.render(64), Err(OutputError::OutputBudget)));
    let long = "x".repeat(64 * 1024);
    let context = original(vec![premise(5, &long, PremiseBasis::Given)]);
    let candidates = [candidate(context.state(), &dependencies)];
    let prepared = PreparedDecision::prepare_whole_context("seed", &candidates, None, None).unwrap();
    assert!(matches!(prepared.render(64 * 1024), Err(OutputError::OutputBudget)));
}

#[test]
fn tlcx29_strict_connector_cut_rejects_output_and_restoration_binds_exact_bytes() {
    use crate::joint_selection::*;
    use lc631_analysis::creativity_measurement::RationalInterval;
    use lc631_core::CandidateId;

    let context = original(vec![premise(5, "Actual context dependency", PremiseBasis::Given)]);
    let pins = SelectionPins {
        source: SourceRevision::of("seed"),
        scope: context.state().scope(),
        baseline: d(10),
        hard_invariants: d(11),
        policy: d(12),
        goal_profile: Some(d(2)),
        evidence_revision: d(13),
    };
    let axes = [AxisKind::Goal(d(2))];
    let policy = SelectionPolicy {
        pins,
        axes: &axes,
        representative_order: None,
        declared_goal_solve_overlap: false,
        max_candidates: 1,
        max_canonical_bytes: 1024,
    };
    let one = RationalInterval::exact_ratio(1, 1).unwrap();
    let zero = RationalInterval::exact_ratio(0, 1).unwrap();
    let values = [AxisValue::Exact(&one)];
    let rows = [JointCandidate {
        id: CandidateId(1),
        context: context.state().revision(),
        pins,
        hard_gate: ReportedGate::Pass,
        map_back: ReportedGate::Pass,
        axes: &values,
        total_loss: AxisValue::Exact(&zero),
        canonical_key: b"declared structural fixture",
        measurement_binding: d(14),
    }];
    let bindings = [ExplanationBinding {
        candidate: CandidateId(1),
        measurement: d(14),
        result: d(9),
    }];
    let cut = [candidate(context.state(), &[])];
    assert!(matches!(
        connect_joint_whole_context_reported(
            "seed", &policy, &rows, SearchCoverage::EvaluatedSet, &cut, &bindings,
        ),
        Err(ConnectionError::Output(OutputError::UnaccountedContextPremise))
    ));
    let declared = connect_joint_reported(
        "seed", &policy, &rows, SearchCoverage::EvaluatedSet, &cut, &bindings,
    )
    .unwrap();
    assert_eq!(declared.dependency_coverage(), DependencyCoverage::DeclaredDependenciesOnly);
    let dependencies = [PremiseDependency { premise: d(5), state: DependencyState::Given }];
    let explanations = [candidate(context.state(), &dependencies)];
    let connected = connect_joint_whole_context_reported(
        "seed", &policy, &rows, SearchCoverage::EvaluatedSet, &explanations, &bindings,
    )
    .unwrap();
    assert_eq!(connected.recommendation(), Some(0));
    assert_eq!(connected.report().candidates.len(), 1);
    assert_eq!(connected.dependency_coverage(), DependencyCoverage::AllContextPremisesAccounted);
    let rendered = connected.render(16384).unwrap();
    assert!(rendered.text().contains("Actual context dependency"));
    assert!(rendered.text().contains("AllContextPremisesAccounted"));
    assert!(rendered.matches_exact("seed", rendered.text().as_bytes()));
    assert_eq!(rendered.explanation_tl(), ExplanationTl::NeedsEvidence);
}

#[test]
fn tlcx29_source_view_premise_retains_checked_span_without_truth_promotion() {
    use lc631_substrate::source::{SourceSpan, SourceStore};

    let source = SourceStore::new(Arc::from("seed"), 16).unwrap();
    let span = SourceSpan::checked(&source, 0, 4).unwrap();
    let premise = ContextPremise::from_source(d(5), &source, span, PremiseBasis::Given, false).unwrap();
    let context = original(vec![premise]);
    let dependencies = [PremiseDependency { premise: d(5), state: DependencyState::Given }];
    let candidates = [candidate(context.state(), &dependencies)];
    let prepared = PreparedDecision::prepare_whole_context("seed", &candidates, None, None).unwrap();
    let rendered = prepared.render(8192).unwrap();
    assert!(rendered.text().contains("source span: [0,4)"));
    assert!(rendered.text().contains("basis: Given"));
    assert!(rendered.text().contains("statement: \"seed\""));
    assert_eq!(rendered.explanation_tl(), ExplanationTl::NeedsEvidence);
}

#[test]
fn tlcx29_whole_context_limit_is_not_bypassed_by_omitting_all_dependencies() {
    let premises = (0u16..257)
        .map(|index| {
            let mut bytes = [0; 32];
            bytes[..2].copy_from_slice(&index.to_le_bytes());
            ContextPremise::reported(
                Digest32::from_bytes(bytes),
                Arc::from("A bounded structural premise"),
                PremiseBasis::Given,
                false,
            )
        })
        .collect();
    let context = original(premises);
    let candidates = [candidate(context.state(), &[])];
    assert!(matches!(
        PreparedDecision::prepare_whole_context("seed", &candidates, None, None),
        Err(OutputError::InputBudget)
    ));
}
