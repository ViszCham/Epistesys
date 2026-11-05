//! Structural retention tests; reported premises are not independent truth.
use super::*;
use lc631_core::context::{
    ContextAdmission, ContextConsumer, ContextDelta, ContextLimits, ContextPremise, ContextScope,
    OriginalContext, PremiseEdit,
};
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
            max_premises: 8,
            max_edits: 8,
            max_statement_bytes: 128 * 1024,
        },
    )
    .unwrap()
}

fn candidate<'a>(
    context: &'a ContextState,
    history: &'a [&'a ContextState],
    dependencies: &'a [PremiseDependency],
) -> CandidateExplanation<'a> {
    CandidateExplanation {
        context,
        context_history: history,
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
fn tlcx29_original_premise_statement_is_retained_in_existing_render() {
    let context = original(vec![ContextPremise::reported(
        d(5),
        Arc::from("Original input premise must remain visible"),
        PremiseBasis::Given,
        true,
    )]);
    let dependencies = [PremiseDependency {
        premise: d(5),
        state: DependencyState::Given,
    }];
    let candidates = [candidate(context.state(), &[], &dependencies)];
    let prepared = PreparedDecision::prepare("seed", &candidates, None, None).unwrap();
    let rendered = prepared.render(8192).unwrap();
    assert!(rendered
        .text()
        .contains("Original input premise must remain visible"));
    assert_eq!(rendered.explanation_tl(), ExplanationTl::NeedsEvidence);
}

#[test]
fn tlcx29_original_replaced_and_retracted_statements_are_not_erased() {
    let original = original(vec![
        ContextPremise::reported(
            d(5),
            Arc::from("Original before replacement"),
            PremiseBasis::Given,
            true,
        ),
        ContextPremise::reported(
            d(6),
            Arc::from("Original before retraction"),
            PremiseBasis::Given,
            true,
        ),
    ]);
    let mut contexts = ContextConsumer::new(original).unwrap();
    let delta = ContextDelta::proposed(
        contexts.original().state().revision(),
        contexts.original().state().scope(),
        AuthorityRevision(1),
        ContextApplicability::Unconditional,
        vec![
            PremiseEdit::Replace {
                id: d(5),
                statement: Arc::from("Proposed replacement"),
                basis: PremiseBasis::Proposed,
            },
            PremiseEdit::Retract { id: d(6) },
        ],
    );
    let revision = match contexts.admit(delta).unwrap() {
        ContextAdmission::Inserted(revision) => revision,
        ContextAdmission::Duplicate(_) => panic!("fixture changes the original context"),
    };
    let history = [contexts.original().state()];
    let candidates = [candidate(contexts.get(revision).unwrap(), &history, &[])];
    let prepared = PreparedDecision::prepare("seed", &candidates, None, None).unwrap();
    let rendered = prepared.render(16384).unwrap();
    for retained in [
        "Original before replacement",
        "Original before retraction",
        "Proposed replacement",
        "context change/retract:",
    ] {
        assert!(rendered.text().contains(retained), "missing {retained}");
    }
}
