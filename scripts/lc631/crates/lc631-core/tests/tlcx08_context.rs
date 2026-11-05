use lc631_core::context::{
    ApplicabilityState, ContextAdmission, ContextApplicability, ContextConsumer, ContextDelta,
    ContextError, ContextLimits, ContextPremise, ContextScope, OriginalContext, PremiseBasis,
    PremiseEdit, SourceRevision,
};
use lc631_core::AuthorityRevision;
use lc631_substrate::Digest32;
use std::sync::Arc;

#[test]
fn tlcx08_source_premise_shares_checked_view_across_child_contexts() {
    use lc631_substrate::source::{SourceSpan, SourceStore};
    let source = SourceStore::new(Arc::from("fixed α; premise β"), 1024).unwrap();
    let span = SourceSpan::checked(&source, 0, "fixed α".len()).unwrap();
    let premise =
        ContextPremise::from_source(id(40), &source, span, PremiseBasis::Proposed, false).unwrap();
    assert_eq!(premise.statement().as_ptr(), source.text().as_ptr());
    assert_eq!(premise.source_span(), Some(span));
    let original = OriginalContext::checked(
        SourceRevision::of(source.text()),
        scope(),
        AuthorityRevision(7),
        vec![premise],
        ContextLimits {
            max_contexts: 8,
            max_premises: 8,
            max_edits: 8,
            max_statement_bytes: 1024,
        },
    )
    .unwrap();
    let mut consumer = ContextConsumer::new(original.clone()).unwrap();
    let admission = consumer
        .admit(delta(
            &original,
            vec![PremiseEdit::Add(ContextPremise::reported(
                id(41),
                Arc::from("new proposed premise"),
                PremiseBasis::Assumed,
                true,
            ))],
        ))
        .unwrap();
    let ContextAdmission::Inserted(revision) = admission else {
        panic!("new proposal must change canonical content")
    };
    let child = consumer.get(revision).unwrap();
    assert_eq!(
        child.premises()[0].statement().as_ptr(),
        source.text().as_ptr()
    );
    assert_eq!(child.premises()[0].source_span(), Some(span));
    assert_eq!(child.premises()[1].source_span(), None);
}

#[test]
fn tlcx08_source_premise_rejects_foreign_store_and_original_source() {
    use lc631_substrate::source::{SourceError, SourceSpan, SourceStore};
    let source = SourceStore::new(Arc::from("α"), 1024).unwrap();
    let other = SourceStore::new(Arc::from("β"), 1024).unwrap();
    let span = SourceSpan::checked(&source, 0, source.text().len()).unwrap();
    assert_eq!(
        ContextPremise::from_source(id(40), &other, span, PremiseBasis::Proposed, true)
            .unwrap_err(),
        SourceError::SourceMismatch
    );
    let premise =
        ContextPremise::from_source(id(40), &source, span, PremiseBasis::Proposed, true).unwrap();
    assert!(matches!(
        OriginalContext::checked(
            SourceRevision::of("other root"),
            scope(),
            AuthorityRevision(7),
            vec![premise],
            ContextLimits {
                max_contexts: 8,
                max_premises: 8,
                max_edits: 8,
                max_statement_bytes: 1024
            }
        ),
        Err(ContextError::ForeignSource)
    ));
}

fn id(value: u8) -> Digest32 {
    Digest32::from_bytes([value; 32])
}

fn scope() -> ContextScope {
    ContextScope::declared(id(10), id(11), id(12), id(13))
}

fn original() -> OriginalContext {
    OriginalContext::checked(
        SourceRevision::of("Preserve q; reconsider premise p."),
        scope(),
        AuthorityRevision(7),
        vec![
            ContextPremise::reported(id(1), Arc::from("q"), PremiseBasis::Given, false),
            ContextPremise::reported(id(2), Arc::from("p"), PremiseBasis::Assumed, true),
        ],
        ContextLimits {
            max_contexts: 8,
            max_premises: 8,
            max_edits: 8,
            max_statement_bytes: 128,
        },
    )
    .unwrap()
}

fn delta(original: &OriginalContext, edits: Vec<PremiseEdit>) -> ContextDelta {
    ContextDelta::proposed(
        original.state().revision(),
        scope(),
        AuthorityRevision(7),
        ContextApplicability::Unconditional,
        edits,
    )
}

#[test]
fn tlcx_08_hard_premise_cannot_be_retracted_or_replaced() {
    for edit in [
        PremiseEdit::Retract { id: id(1) },
        PremiseEdit::Replace {
            id: id(1),
            statement: Arc::from("not q"),
            basis: PremiseBasis::Proposed,
        },
    ] {
        let original = original();
        let mut consumer = ContextConsumer::new(original.clone()).unwrap();
        assert_eq!(
            consumer.admit(delta(&original, vec![edit])),
            Err(ContextError::HardInvariant)
        );
        assert_eq!(consumer.states().len(), 1);
        assert_eq!(original.state().premises()[0].statement(), "q");
    }
}

#[test]
fn tlcx_08_reconstruction_cannot_change_authority_or_comparison_scope() {
    let original = original();
    let mut consumer = ContextConsumer::new(original.clone()).unwrap();
    let foreign_authority = ContextDelta::proposed(
        original.state().revision(),
        scope(),
        AuthorityRevision(8),
        ContextApplicability::Unconditional,
        vec![],
    );
    assert_eq!(
        consumer.admit(foreign_authority),
        Err(ContextError::AuthorityMismatch)
    );
    let foreign_scope = ContextDelta::proposed(
        original.state().revision(),
        ContextScope::declared(id(99), id(11), id(12), id(13)),
        AuthorityRevision(7),
        ContextApplicability::Unconditional,
        vec![],
    );
    assert_eq!(
        consumer.admit(foreign_scope),
        Err(ContextError::ScopeMismatch)
    );
}

#[test]
fn tlcx_08_unknown_or_foreign_parent_is_not_admitted() {
    let original = original();
    let mut consumer = ContextConsumer::new(original.clone()).unwrap();
    for (parent, expected) in [
        (
            original.state().revision().reconstructed([31; 32]),
            ContextError::UnknownParent,
        ),
        (
            lc631_core::context::ContextRevision::original(SourceRevision::of("foreign")),
            ContextError::ForeignSource,
        ),
    ] {
        let proposal = ContextDelta::proposed(
            parent,
            scope(),
            AuthorityRevision(7),
            ContextApplicability::Unconditional,
            vec![],
        );
        assert_eq!(consumer.admit(proposal), Err(expected));
    }
}

#[test]
fn tlcx_08_proposed_premise_cannot_be_promoted_to_observed() {
    let original = original();
    let mut consumer = ContextConsumer::new(original.clone()).unwrap();
    let proposal = delta(
        &original,
        vec![PremiseEdit::Replace {
            id: id(2),
            statement: Arc::from("r"),
            basis: PremiseBasis::Observed,
        }],
    );
    assert_eq!(consumer.admit(proposal), Err(ContextError::BasisPromotion));
    assert_eq!(
        original.state().premises()[1].basis(),
        PremiseBasis::Assumed
    );
}

#[test]
fn tlcx_08_unknown_applicability_remains_unknown_in_the_consumer() {
    let original = original();
    let applicability = ContextApplicability::Conditional {
        scope: scope(),
        predicate: id(41),
        state: ApplicabilityState::Unknown,
    };
    let proposal = ContextDelta::proposed(
        original.state().revision(),
        scope(),
        AuthorityRevision(7),
        applicability,
        vec![PremiseEdit::Replace {
            id: id(2),
            statement: Arc::from("r"),
            basis: PremiseBasis::Proposed,
        }],
    );
    let mut consumer = ContextConsumer::new(original.clone()).unwrap();
    let ContextAdmission::Inserted(revision) = consumer.admit(proposal).unwrap() else {
        panic!("nonempty reconstruction must be consumed");
    };
    let child = consumer.get(revision).unwrap();
    assert_eq!(child.applicability(), applicability);
    assert_eq!(child.parent(), Some(original.state().revision()));
    assert_eq!(child.premises()[1].statement(), "r");
    assert_eq!(original.state().premises()[1].statement(), "p");
}

#[test]
fn tlcx_08_id_only_reconstruction_is_a_duplicate_world() {
    let original = original();
    let mut consumer = ContextConsumer::new(original.clone()).unwrap();
    let proposal = delta(
        &original,
        vec![
            PremiseEdit::Retract { id: id(2) },
            PremiseEdit::Add(ContextPremise::reported(
                id(77),
                Arc::from("p"),
                PremiseBasis::Assumed,
                true,
            )),
        ],
    );
    assert_eq!(
        consumer.admit(proposal),
        Ok(ContextAdmission::Duplicate(original.state().revision()))
    );
    assert_eq!(consumer.states().len(), 1);
}

#[test]
fn tlcx_08_repeated_edit_targets_and_new_hard_premises_are_rejected() {
    let original = original();
    let mut consumer = ContextConsumer::new(original.clone()).unwrap();
    assert_eq!(
        consumer.admit(delta(
            &original,
            vec![
                PremiseEdit::Retract { id: id(2) },
                PremiseEdit::Retract { id: id(2) },
            ]
        )),
        Err(ContextError::DuplicateEdit)
    );
    assert_eq!(
        consumer.admit(delta(
            &original,
            vec![PremiseEdit::Add(ContextPremise::reported(
                id(9),
                Arc::from("new hard"),
                PremiseBasis::Proposed,
                false
            ),)]
        )),
        Err(ContextError::HardInvariant)
    );
}

#[test]
fn tlcx_08_partial_delta_failure_does_not_publish_a_context() {
    let original = original();
    let mut consumer = ContextConsumer::new(original.clone()).unwrap();
    let proposal = delta(
        &original,
        vec![
            PremiseEdit::Replace {
                id: id(2),
                statement: Arc::from("r"),
                basis: PremiseBasis::Proposed,
            },
            PremiseEdit::Retract { id: id(1) },
        ],
    );
    assert_eq!(consumer.admit(proposal), Err(ContextError::HardInvariant));
    assert_eq!(consumer.states().len(), 1);
    assert_eq!(consumer.states()[0].premises()[1].statement(), "p");
}

#[test]
fn tlcx_08_duplicate_claim_id_padding_does_not_create_a_distinct_world() {
    let original = original();
    let mut consumer = ContextConsumer::new(original.clone()).unwrap();
    let proposal = delta(
        &original,
        vec![PremiseEdit::Add(ContextPremise::reported(
            id(88),
            Arc::from("p"),
            PremiseBasis::Assumed,
            true,
        ))],
    );
    assert_eq!(
        consumer.admit(proposal),
        Ok(ContextAdmission::Duplicate(original.state().revision()))
    );
    assert_eq!(consumer.states().len(), 1);
}

#[test]
fn tlcx_08_applicability_cannot_borrow_a_foreign_comparison_scope() {
    let original = original();
    let mut consumer = ContextConsumer::new(original.clone()).unwrap();
    let applicability = ContextApplicability::Conditional {
        scope: ContextScope::declared(id(99), id(11), id(12), id(13)),
        predicate: id(41),
        state: ApplicabilityState::Supported,
    };
    let proposal = ContextDelta::proposed(
        original.state().revision(),
        scope(),
        AuthorityRevision(7),
        applicability,
        vec![],
    );
    assert_eq!(consumer.admit(proposal), Err(ContextError::ScopeMismatch));
    assert_eq!(consumer.states().len(), 1);
}

#[test]
fn tlcx_08_state_and_text_budgets_fail_without_partial_admission() {
    let original = original();
    let source = original.state().revision().source();
    let mut limits = original.limits();
    limits.max_contexts = 1;
    let bounded = OriginalContext::checked(
        source,
        scope(),
        AuthorityRevision(7),
        original.state().premises().to_vec(),
        limits,
    )
    .unwrap();
    let mut consumer = ContextConsumer::new(bounded.clone()).unwrap();
    let proposal = delta(
        &bounded,
        vec![PremiseEdit::Replace {
            id: id(2),
            statement: Arc::from("r"),
            basis: PremiseBasis::Proposed,
        }],
    );
    assert_eq!(consumer.admit(proposal), Err(ContextError::BudgetExceeded));
    assert_eq!(consumer.states().len(), 1);
    let mut consumer = ContextConsumer::new(original.clone()).unwrap();
    let proposal = delta(
        &original,
        vec![PremiseEdit::Replace {
            id: id(2),
            statement: Arc::from("x".repeat(129)),
            basis: PremiseBasis::Proposed,
        }],
    );
    assert_eq!(consumer.admit(proposal), Err(ContextError::BudgetExceeded));
    assert_eq!(consumer.states().len(), 1);
}

#[test]
fn tlcx_08_declared_root_rejects_duplicate_premise_identity() {
    let original = original();
    let premise = original.state().premises()[0].clone();
    assert!(matches!(
        OriginalContext::checked(
            original.state().revision().source(),
            scope(),
            AuthorityRevision(7),
            vec![premise.clone(), premise],
            original.limits()
        ),
        Err(ContextError::DuplicatePremise)
    ));
}

#[test]
fn tlcx_08_child_reclassification_retains_the_reported_original_observation() {
    let template = original();
    let observed = OriginalContext::checked(
        template.state().revision().source(),
        scope(),
        AuthorityRevision(7),
        vec![ContextPremise::reported(
            id(2),
            Arc::from("p"),
            PremiseBasis::Observed,
            true,
        )],
        template.limits(),
    )
    .unwrap();
    let proposal = delta(
        &observed,
        vec![PremiseEdit::Replace {
            id: id(2),
            statement: Arc::from("r"),
            basis: PremiseBasis::Proposed,
        }],
    );
    let mut consumer = ContextConsumer::new(observed.clone()).unwrap();
    let ContextAdmission::Inserted(revision) = consumer.admit(proposal).unwrap() else {
        panic!("a changed proposal must reach the structural consumer");
    };
    assert_eq!(consumer.original().state().premises()[0].statement(), "p");
    assert_eq!(
        consumer.original().state().premises()[0].basis(),
        PremiseBasis::Observed
    );
    assert_eq!(
        consumer.get(revision).unwrap().premises()[0].basis(),
        PremiseBasis::Proposed
    );
    assert_eq!(consumer.original().authority(), AuthorityRevision(7));
}
