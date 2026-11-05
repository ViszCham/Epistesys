use lc631_core::context::{ContextLimits, ContextScope, OriginalContext, SourceRevision};
use lc631_core::AuthorityRevision;
use lc631_runtime::recommendation_output::*;
use lc631_substrate::Digest32;

fn original(source: &str) -> OriginalContext {
    let d = Digest32::from_bytes([1; 32]);
    OriginalContext::checked(
        SourceRevision::of(source),
        ContextScope::declared(d, d, d, d),
        AuthorityRevision(1),
        vec![],
        ContextLimits {
            max_contexts: 8,
            max_premises: 8,
            max_edits: 8,
            max_statement_bytes: 1024,
        },
    )
    .unwrap()
}

fn candidate<'a>(context: &'a lc631_core::context::ContextState) -> CandidateExplanation<'a> {
    CandidateExplanation {
        context,
        context_history: &[],
        result: Digest32::from_bytes([2; 32]),
        explanation: "A conditional candidate.",
        dependencies: &[],
        facets: FourFacetRefs {
            defects: None,
            delta: None,
            uncertainty: None,
            gain: None,
        },
    }
}

#[test]
fn tlcx29_empty_candidates_cannot_render_a_recommendation() {
    assert!(matches!(
        PreparedDecision::prepare("seed", &[], Some(0), None),
        Err(OutputError::EmptyCandidates)
    ));
}

#[test]
fn tlcx29_source_swap_is_rejected() {
    let context = original("other");
    let candidates = [candidate(context.state())];
    assert!(matches!(
        PreparedDecision::prepare("seed", &candidates, None, None),
        Err(OutputError::SourceMismatch)
    ));
}

#[test]
fn tlcx29_goal_absence_prevents_recommendation_but_retains_alternatives() {
    let context = original("seed");
    let candidates = [candidate(context.state())];
    let decision = PreparedDecision::prepare("seed", &candidates, Some(0), None).unwrap();
    assert_eq!(decision.recommendation(), None);
    assert_eq!(decision.candidates().len(), 1);
    assert_eq!(decision.state(), PreparationState::NoRecommendation);
}

#[test]
fn tlcx29_unresolved_dependency_cannot_be_presented_as_partial_answer() {
    let context = original("seed");
    let dependencies = [PremiseDependency {
        premise: Digest32::from_bytes([3; 32]),
        state: DependencyState::Unknown,
    }];
    let mut value = candidate(context.state());
    value.dependencies = &dependencies;
    let candidates = [value];
    assert!(matches!(
        PreparedDecision::prepare("seed", &candidates, Some(0), None),
        Err(OutputError::UnresolvedDependency)
    ));
}

#[test]
fn tlcx29_raw_json_is_not_a_final_explanation() {
    let context = original("seed");
    let mut value = candidate(context.state());
    value.explanation = " {\"status\": \"complete\"}";
    let candidates = [value];
    assert!(matches!(
        PreparedDecision::prepare("seed", &candidates, None, None),
        Err(OutputError::RawDump)
    ));
}

#[test]
fn tlcx29_exact_output_rejects_changed_source_and_post_render_bytes() {
    let context = original("seed");
    let candidates = [candidate(context.state())];
    let decision = PreparedDecision::prepare("seed", &candidates, None, None).unwrap();
    let rendered = decision.render(4096).unwrap();
    assert!(rendered.matches_exact("seed", rendered.text().as_bytes()));
    assert!(!rendered.matches_exact("other", rendered.text().as_bytes()));
    assert!(!rendered.matches_exact("seed", b"changed"));
    assert_eq!(rendered.explanation_tl(), ExplanationTl::NeedsEvidence);
}

#[test]
fn tlcx29_budget_failure_returns_no_truncated_success() {
    let context = original("seed");
    let candidates = [candidate(context.state())];
    let decision = PreparedDecision::prepare("seed", &candidates, None, None).unwrap();
    assert!(matches!(
        decision.render(32),
        Err(OutputError::OutputBudget)
    ));
}

#[test]
fn tlcx29_whitespace_cannot_hide_unbounded_or_control_payload() {
    let context = original("seed");
    let mut value = candidate(context.state());
    value.explanation = "\nClaim without a visible qualifier";
    let candidates = [value];
    assert!(matches!(
        PreparedDecision::prepare("seed", &candidates, None, None),
        Err(OutputError::ControlCharacter)
    ));
    let padded = format!("{}Claim", " ".repeat(8192));
    let mut value = candidate(context.state());
    value.explanation = &padded;
    let candidates = [value];
    assert!(matches!(
        PreparedDecision::prepare("seed", &candidates, None, None),
        Err(OutputError::InputBudget)
    ));
}

#[test]
fn tlcx29_solver_report_does_not_discharge_partial_answer_dependencies() {
    let context = original("seed");
    for state in [
        DependencyState::SolverReported,
        DependencyState::Unsupported,
        DependencyState::Conflict,
        DependencyState::NeedsEvidence,
    ] {
        let dependencies = [PremiseDependency {
            premise: Digest32::from_bytes([3; 32]),
            state,
        }];
        let mut value = candidate(context.state());
        value.dependencies = &dependencies;
        let candidates = [value];
        assert!(matches!(
            PreparedDecision::prepare("seed", &candidates, None, None),
            Err(OutputError::UnresolvedDependency)
        ));
    }
}

#[test]
fn tlcx29_declared_selection_keeps_every_alternative_and_unknown_facet() {
    let context = original("seed");
    let first = candidate(context.state());
    let mut second = candidate(context.state());
    second.result = Digest32::from_bytes([4; 32]);
    second.explanation = "Another candidate retained for comparison.";
    let candidates = [first, second];
    let decision = PreparedDecision::prepare(
        "seed",
        &candidates,
        Some(0),
        Some(Digest32::from_bytes([7; 32])),
    )
    .unwrap();
    assert_eq!(decision.state(), PreparationState::DeclaredRecommendation);
    let output = decision.render(8192).unwrap();
    assert!(output.text().contains("Another candidate retained"));
    assert!(output.text().contains("Unknown/NeedsEvidence"));
    assert!(output
        .text()
        .contains("optimality and explanation TL: NeedsEvidence"));
    assert!(output.text().find("日本語").unwrap() < output.text().find("English").unwrap());
}

#[test]
fn tlcx29_missing_context_history_cannot_hide_premise_changes() {
    use lc631_core::context::{
        ContextAdmission, ContextApplicability, ContextConsumer, ContextDelta, ContextPremise,
        PremiseBasis, PremiseEdit,
    };
    use std::sync::Arc;
    let original = original("seed");
    let mut consumer = ContextConsumer::new(original).unwrap();
    let root = consumer.original().state().revision();
    let delta = ContextDelta::proposed(
        root,
        consumer.original().state().scope(),
        AuthorityRevision(1),
        ContextApplicability::Unconditional,
        vec![PremiseEdit::Add(ContextPremise::reported(
            Digest32::from_bytes([5; 32]),
            Arc::from("Proposed premise"),
            PremiseBasis::Proposed,
            true,
        ))],
    );
    let revision = match consumer.admit(delta).unwrap() {
        ContextAdmission::Inserted(value) => value,
        ContextAdmission::Duplicate(_) => panic!("fixture adds an actual premise"),
    };
    let value = candidate(consumer.get(revision).unwrap());
    let candidates = [value];
    assert!(matches!(
        PreparedDecision::prepare("seed", &candidates, None, None),
        Err(OutputError::InvalidContextHistory)
    ));
    let history = [consumer.original().state()];
    let mut value = candidate(consumer.get(revision).unwrap());
    value.context_history = &history;
    let candidates = [value];
    let decision = PreparedDecision::prepare("seed", &candidates, None, None).unwrap();
    let output = decision.render(8192).unwrap();
    assert!(output.text().contains("context change/add:"));
    assert!(output.text().contains("Proposed premise"));
}
