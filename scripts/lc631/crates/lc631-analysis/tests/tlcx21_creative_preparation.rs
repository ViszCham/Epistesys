//! 日本語: 実core consumerで構造境界を試験。独立意味検証ではない。
//! English: exercises structural boundaries using the real core consumer only.
use lc631_analysis::creative_candidates;

use creative_candidates::*;
use lc631_core::context::{
    ContextLimits, ContextPremise, ContextScope, OriginalContext, PremiseBasis, SourceRevision,
};
use lc631_core::AuthorityRevision;
use lc631_substrate::source::{SourceSpan, SourceStore};
use lc631_substrate::Digest32;
use std::sync::Arc;

fn id(byte: u8) -> Digest32 {
    Digest32::from_bytes([byte; 32])
}

fn fixture(max_proposals: usize) -> (CreativePreparation, SourceStore, SourceSpan) {
    let source = SourceStore::new(Arc::from("fixed requirement; mutable premise"), 128).unwrap();
    let anchor = SourceSpan::checked(&source, 0, source.text().len()).unwrap();
    let original = OriginalContext::checked(
        SourceRevision::of(source.text()),
        ContextScope::declared(id(1), id(2), id(3), id(4)),
        AuthorityRevision(7),
        vec![
            ContextPremise::reported(id(5), Arc::from("fixed"), PremiseBasis::Given, false),
            ContextPremise::reported(id(6), Arc::from("mutable"), PremiseBasis::Observed, true),
        ],
        ContextLimits {
            max_contexts: 8,
            max_premises: 8,
            max_edits: 2,
            max_statement_bytes: 64,
        },
    )
    .unwrap();
    (
        CreativePreparation::new(original, max_proposals).unwrap(),
        source,
        anchor,
    )
}

fn revise(text: &str) -> CreativeOperator {
    CreativeOperator::RevisePremise {
        target: id(6),
        replacement: Arc::from(text),
    }
}

#[test]
fn tlcx21_replacement_is_new_structure_but_never_independent_validation() {
    let (mut prep, source, anchor) = fixture(4);
    let parent = prep.contexts().original().state().revision();
    let result = prep
        .prepare(
            &source,
            anchor,
            parent,
            ProposalProvider::DeterministicOperator,
            revise("proposed alternative"),
        )
        .unwrap();
    assert!(!result.is_duplicate());
    assert_ne!(parent, result.target());
    assert_eq!(
        result.validation(),
        CandidateValidation::NeedsIndependentValidation
    );
    assert_eq!(result.parent(), parent);
    assert_eq!(result.source_anchor(), anchor);
    assert_eq!(result.provider(), ProposalProvider::DeterministicOperator);
    assert_eq!(result.operator(), OperatorKind::RevisePremise);
    let candidate = prep.contexts().get(result.target()).unwrap();
    let changed = candidate
        .premises()
        .iter()
        .find(|p| p.id() == id(6))
        .unwrap();
    assert_eq!(changed.statement(), "proposed alternative");
    assert_eq!(changed.basis(), PremiseBasis::Proposed);
    assert_eq!(
        prep.contexts().original().state().premises()[1].basis(),
        PremiseBasis::Observed
    );
    assert_eq!(
        candidate.scope(),
        prep.contexts().original().state().scope()
    );
    assert_eq!(
        candidate.declared_delta().unwrap().authority(),
        AuthorityRevision(7)
    );
}

#[test]
fn tlcx21_renamed_provider_duplicates_preserve_provenance_without_new_context() {
    let (mut prep, source, anchor) = fixture(4);
    let parent = prep.contexts().original().state().revision();
    let first = prep
        .prepare(
            &source,
            anchor,
            parent,
            ProposalProvider::DeterministicOperator,
            revise("alternative"),
        )
        .unwrap();
    let alias = prep
        .prepare(
            &source,
            anchor,
            parent,
            ProposalProvider::ReportedLlm {
                provider_revision: id(90),
            },
            revise("alternative"),
        )
        .unwrap();
    assert_eq!(first.target(), alias.target());
    assert!(alias.is_duplicate());
    assert_eq!(prep.contexts().states().len(), 2);
    assert_eq!(prep.proposals().len(), 2);
    assert_eq!(
        alias.validation(),
        CandidateValidation::NeedsIndependentValidation
    );
}

#[test]
fn tlcx21_hard_edit_rejected_without_publishing_state_or_provenance() {
    let (mut prep, source, anchor) = fixture(4);
    let parent = prep.contexts().original().state().revision();
    let result = prep.prepare(
        &source,
        anchor,
        parent,
        ProposalProvider::DeterministicOperator,
        CreativeOperator::RevisePremise {
            target: id(5),
            replacement: Arc::from("relaxed"),
        },
    );
    assert_eq!(result, Err(PreparationError::HardInvariant));
    assert_eq!(prep.contexts().states().len(), 1);
    assert!(prep.proposals().is_empty());
}

#[test]
fn tlcx21_label_only_revision_is_rejected_and_preservation_is_duplicate() {
    let (mut prep, source, anchor) = fixture(4);
    let parent = prep.contexts().original().state().revision();
    assert_eq!(
        prep.prepare(
            &source,
            anchor,
            parent,
            ProposalProvider::DeterministicOperator,
            revise("mutable")
        ),
        Err(PreparationError::NoStructuralChange)
    );
    let preserved = prep
        .prepare(
            &source,
            anchor,
            parent,
            ProposalProvider::DeterministicOperator,
            CreativeOperator::Preserve,
        )
        .unwrap();
    assert!(preserved.is_duplicate());
    assert_eq!(preserved.target(), parent);
    assert_eq!(prep.contexts().states().len(), 1);
}

#[test]
fn tlcx21_foreign_source_stale_parent_and_empty_anchor_rejected() {
    let (mut prep, source, anchor) = fixture(4);
    let parent = prep.contexts().original().state().revision();
    let other = SourceStore::new(Arc::from("other source"), 128).unwrap();
    assert_eq!(
        prep.prepare(
            &other,
            anchor,
            parent,
            ProposalProvider::DeterministicOperator,
            revise("alternative")
        ),
        Err(PreparationError::ForeignSource)
    );
    let stale = parent.reconstructed([99; 32]);
    assert_eq!(
        prep.prepare(
            &source,
            anchor,
            stale,
            ProposalProvider::DeterministicOperator,
            revise("alternative")
        ),
        Err(PreparationError::UnknownParent)
    );
    let empty = SourceSpan::checked(&source, 0, 0).unwrap();
    assert_eq!(
        prep.prepare(
            &source,
            empty,
            parent,
            ProposalProvider::DeterministicOperator,
            revise("alternative")
        ),
        Err(PreparationError::EmptySourceAnchor)
    );
    assert_eq!(prep.contexts().states().len(), 1);
}

#[test]
fn tlcx21_all_unimplemented_operators_retain_unsupported_state() {
    let (mut prep, source, anchor) = fixture(8);
    let parent = prep.contexts().original().state().revision();
    let unsupported = [
        CreativeOperator::Recombine,
        CreativeOperator::Rebind,
        CreativeOperator::Reframe,
        CreativeOperator::Abstract,
        CreativeOperator::CounterexampleRevision,
    ];
    for operator in unsupported {
        let kind = operator.kind();
        assert_eq!(
            prep.prepare(
                &source,
                anchor,
                parent,
                ProposalProvider::DeterministicOperator,
                operator
            ),
            Err(PreparationError::UnsupportedOperator(kind))
        );
    }
    assert_eq!(prep.contexts().states().len(), 1);
    assert!(prep.proposals().is_empty());
}

#[test]
fn tlcx21_proposal_budget_precedes_core_publication_even_for_duplicates() {
    let (mut prep, source, anchor) = fixture(1);
    let parent = prep.contexts().original().state().revision();
    prep.prepare(
        &source,
        anchor,
        parent,
        ProposalProvider::DeterministicOperator,
        revise("first"),
    )
    .unwrap();
    assert_eq!(
        prep.prepare(
            &source,
            anchor,
            parent,
            ProposalProvider::DeterministicOperator,
            revise("second")
        ),
        Err(PreparationError::ProposalLimit)
    );
    assert_eq!(prep.contexts().states().len(), 2);
    assert_eq!(prep.proposals().len(), 1);
}

#[test]
fn tlcx21_empty_oversize_and_unknown_target_rejected_without_state_change() {
    let (mut prep, source, anchor) = fixture(4);
    let parent = prep.contexts().original().state().revision();
    for (operator, error) in [
        (revise(""), PreparationError::EmptyReplacement),
        (revise(&"x".repeat(65)), PreparationError::ReplacementLimit),
        (
            CreativeOperator::RevisePremise {
                target: id(99),
                replacement: Arc::from("x"),
            },
            PreparationError::MissingPremise,
        ),
    ] {
        assert_eq!(
            prep.prepare(
                &source,
                anchor,
                parent,
                ProposalProvider::DeterministicOperator,
                operator
            ),
            Err(error)
        );
    }
    assert_eq!(prep.contexts().states().len(), 1);
    assert!(prep.proposals().is_empty());
}
