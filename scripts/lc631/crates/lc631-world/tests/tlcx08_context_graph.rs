use lc631_core::{context::*, AuthorityRevision};
use lc631_world::context_graph::*;
use std::sync::Arc;

fn original() -> OriginalContext {
    let pin = SourceRevision::of("source").digest();
    OriginalContext::checked(
        SourceRevision::of("source"),
        ContextScope::declared(pin, pin, pin, pin),
        AuthorityRevision(2),
        vec![ContextPremise::reported(
            pin,
            Arc::from("changeable premise"),
            PremiseBasis::Assumed,
            true,
        )],
        ContextLimits {
            max_contexts: 4,
            max_premises: 4,
            max_edits: 4,
            max_statement_bytes: 1024,
        },
    )
    .unwrap()
}

#[test]
fn tlcx08_graph_records_duplicate_proposals_without_inventing_worlds() {
    let original = original();
    let root = original.state().revision();
    let scope = original.state().scope();
    let mut graph = ContextGraph::new(original, 2).unwrap();
    let delta = || {
        ContextDelta::proposed(
            root,
            scope,
            AuthorityRevision(2),
            ContextApplicability::Unconditional,
            vec![],
        )
    };
    assert_eq!(
        graph.propose(delta()).unwrap(),
        ContextAdmission::Duplicate(root)
    );
    assert_eq!(graph.states().len(), 1);
    assert_eq!(graph.transitions().len(), 1);
    assert!(graph.transitions()[0].is_duplicate());
    assert_eq!(graph.transitions()[0].delta().parent(), root);
    graph.propose(delta()).unwrap();
    assert!(matches!(
        graph.propose(delta()),
        Err(ContextGraphError::TransitionBudget)
    ));
    assert_eq!(graph.transitions().len(), 2);
}

#[test]
fn tlcx08_graph_does_not_record_rejected_authority_or_promotions() {
    let original = original();
    let root = original.state().revision();
    let scope = original.state().scope();
    let id = original.state().premises()[0].id();
    let mut graph = ContextGraph::new(original, 4).unwrap();
    assert!(matches!(
        graph.propose(ContextDelta::proposed(
            root,
            scope,
            AuthorityRevision(3),
            ContextApplicability::Unconditional,
            vec![]
        )),
        Err(ContextGraphError::Context(ContextError::AuthorityMismatch))
    ));
    assert!(matches!(
        graph.propose(ContextDelta::proposed(
            root,
            scope,
            AuthorityRevision(2),
            ContextApplicability::Unconditional,
            vec![PremiseEdit::Replace {
                id,
                statement: Arc::from("unverified observation"),
                basis: PremiseBasis::Observed
            }]
        )),
        Err(ContextGraphError::Context(ContextError::BasisPromotion))
    ));
    assert!(graph.transitions().is_empty());
    assert_eq!(graph.states().len(), 1);
}

#[test]
fn tlcx08_world_graph_materializes_reconstruction_and_keeps_unknown_applicability() {
    let original = original();
    let root = original.state().revision();
    let scope = original.state().scope();
    let premise_id = original.state().premises()[0].id();
    let mut graph = ContextGraph::new(original.clone(), 4).unwrap();
    let applicability = ContextApplicability::Conditional {
        scope,
        predicate: SourceRevision::of("if the proposed context applies").digest(),
        state: ApplicabilityState::Unknown,
    };
    let admission = graph
        .propose(ContextDelta::proposed(
            root,
            scope,
            AuthorityRevision(2),
            applicability,
            vec![PremiseEdit::Replace {
                id: premise_id,
                statement: Arc::from("changeable premise under a proposal"),
                basis: PremiseBasis::Proposed,
            }],
        ))
        .unwrap();
    let ContextAdmission::Inserted(revision) = admission else {
        panic!("a changed proposal must materialize one distinct state");
    };
    let child = graph.get(revision).unwrap();
    assert_eq!(child.parent(), Some(root));
    assert_eq!(child.applicability(), applicability);
    assert_eq!(
        child.premises()[0].statement(),
        "changeable premise under a proposal"
    );
    assert_eq!(child.premises()[0].basis(), PremiseBasis::Proposed);
    assert_eq!(
        graph.original().state().premises()[0].statement(),
        "changeable premise"
    );
    assert_eq!(graph.transitions().len(), 1);
    assert!(!graph.transitions()[0].is_duplicate());
}
