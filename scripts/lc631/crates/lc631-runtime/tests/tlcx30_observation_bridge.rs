//! Preparatory contract tests; synthetic reports are never backend observations.
use lc631_runtime::observation_bridge;

use lc631_analysis::{DependencySentence, DependencyToken};
use lc631_core::context::{
    ContextAdmission, ContextConsumer, ContextLimits, ContextPremise, ContextScope,
    OriginalContext, PremiseBasis, SourceRevision,
};
use lc631_core::AuthorityRevision;
use lc631_substrate::source::{SourceSpan, SourceStore};
use lc631_substrate::Digest32;
use observation_bridge::*;
use std::sync::Arc;

fn digest(byte: u8) -> Digest32 {
    Digest32::from_bytes([byte; 32])
}

fn registration() -> LanguageRegistration<'static> {
    LanguageRegistration {
        language: "en",
        backend: "stanza-1.14.0",
        model: digest(1),
        runtime: digest(2),
        requirements: digest(3),
    }
}

fn limits() -> ObservationBridgeLimits {
    ObservationBridgeLimits {
        max_sentences: 4,
        max_tokens: 4,
        max_fragment_bytes: 64,
    }
}

fn fixture(text: &str) -> (SourceStore, ContextConsumer) {
    let source = SourceStore::new(Arc::from(text), 256).unwrap();
    let span = SourceSpan::checked(&source, 0, text.len()).unwrap();
    let hard =
        ContextPremise::from_source(digest(7), &source, span, PremiseBasis::Given, false).unwrap();
    let scope = ContextScope::declared(digest(8), digest(9), digest(10), digest(11));
    let original = OriginalContext::checked(
        SourceRevision::of(text),
        scope,
        AuthorityRevision(1),
        vec![hard],
        ContextLimits {
            max_contexts: 4,
            max_premises: 8,
            max_edits: 4,
            max_statement_bytes: 256,
        },
    )
    .unwrap();
    (source, ContextConsumer::new(original).unwrap())
}

fn sentence(text: &str, start: usize, end: usize) -> DependencySentence {
    DependencySentence {
        tokens: vec![DependencyToken {
            text: text.into(),
            start_byte: start,
            end_byte: end,
            words: vec![],
        }],
    }
}

fn report<'a>(
    source: &SourceStore,
    sentences: &'a [DependencySentence],
) -> ReportedLanguageInput<'a> {
    ReportedLanguageInput {
        source: source.id().digest(),
        registration: registration(),
        payload: digest(12),
        sentences,
    }
}

#[test]
fn tlcx30_actual_context_consumer_keeps_proposals_and_original() {
    let (source, mut consumer) = fixture("alpha beta");
    let parent = consumer.original().state().clone();
    let sentences = [sentence("alpha", 0, 5), sentence("beta", 6, 10)];
    let batch = prepare_language_candidates(
        &source,
        &parent,
        AuthorityRevision(1),
        registration(),
        report(&source, &sentences),
        limits(),
    )
    .unwrap();
    assert_eq!(batch.delta().parent(), parent.revision());
    let admitted = batch.admit(&mut consumer).unwrap();
    let ContextAdmission::Inserted(revision) = admitted.admission else {
        panic!("expected new branch")
    };
    let branch = consumer.get(revision).unwrap();
    assert_eq!(branch.premises().len(), 3);
    assert!(branch.premises()[1..]
        .iter()
        .all(|p| p.basis() == PremiseBasis::Proposed));
    assert_eq!(consumer.original().state().premises().len(), 1);
    assert_eq!(admitted.candidate_count, 2);
    assert_eq!(admitted.report.payload, digest(12));
}

#[test]
fn tlcx30_source_pin_model_pin_and_empty_cut_do_not_admit() {
    let (source, consumer) = fixture("alpha");
    let sentences = [sentence("alpha", 0, 5)];
    for case in 0..3 {
        let mut input = report(&source, &sentences);
        let expected = match case {
            0 => {
                input.source = digest(99);
                ObservationBridgeError::SourceMismatch
            }
            1 => {
                input.registration.model = digest(99);
                ObservationBridgeError::RegistrationMismatch
            }
            _ => {
                input.sentences = &[];
                ObservationBridgeError::EmptyObservation
            }
        };
        let result = prepare_language_candidates(
            &source,
            consumer.original().state(),
            AuthorityRevision(1),
            registration(),
            input,
            limits(),
        );
        assert_eq!(result.err(), Some(expected));
        assert_eq!(consumer.states().len(), 1);
    }
}

#[test]
fn tlcx30_utf8_text_swap_reverse_and_overlap_are_rejected() {
    let (source, consumer) = fixture("猫 beta");
    // Independent byte facts: 猫 occupies [0,3), space [3,4), beta [4,8).
    let cases = [
        vec![sentence("猫", 1, 3)],
        vec![sentence("dog", 0, 3)],
        vec![sentence("beta", 8, 4)],
        vec![sentence("猫", 0, 3), sentence("猫", 0, 3)],
    ];
    for sentences in cases {
        let result = prepare_language_candidates(
            &source,
            consumer.original().state(),
            AuthorityRevision(1),
            registration(),
            report(&source, &sentences),
            limits(),
        );
        assert_eq!(result.err(), Some(ObservationBridgeError::InvalidToken));
    }
}

#[test]
fn tlcx30_sentence_token_and_byte_budgets_fail_without_partial_state() {
    let (source, consumer) = fixture("alpha beta");
    let sentences = [sentence("alpha", 0, 5), sentence("beta", 6, 10)];
    for budget in [
        ObservationBridgeLimits {
            max_sentences: 1,
            ..limits()
        },
        ObservationBridgeLimits {
            max_tokens: 1,
            ..limits()
        },
        ObservationBridgeLimits {
            max_fragment_bytes: 8,
            ..limits()
        },
    ] {
        let result = prepare_language_candidates(
            &source,
            consumer.original().state(),
            AuthorityRevision(1),
            registration(),
            report(&source, &sentences),
            budget,
        );
        assert_eq!(result.err(), Some(ObservationBridgeError::BudgetExceeded));
        assert_eq!(consumer.states().len(), 1);
    }
}

#[test]
fn tlcx30_wrong_authority_revision_fails_actual_consumer() {
    let (source, mut consumer) = fixture("alpha");
    let parent = consumer.original().state().clone();
    let sentences = [sentence("alpha", 0, 5)];
    let batch = prepare_language_candidates(
        &source,
        &parent,
        AuthorityRevision(2),
        registration(),
        report(&source, &sentences),
        limits(),
    )
    .unwrap();
    assert!(matches!(
        batch.admit(&mut consumer),
        Err(ObservationBridgeError::Context(
            lc631_core::context::ContextError::AuthorityMismatch
        ))
    ));
    assert_eq!(consumer.states().len(), 1);
}

#[test]
fn tlcx30_same_source_fragment_under_changed_payload_retains_report_lineage() {
    let (source, consumer) = fixture("alpha");
    let sentences = [sentence("alpha", 0, 5)];
    let input = report(&source, &sentences);
    let changed = ReportedLanguageInput {
        payload: digest(13),
        ..input
    };
    let a = prepare_language_candidates(
        &source,
        consumer.original().state(),
        AuthorityRevision(1),
        registration(),
        input,
        limits(),
    )
    .unwrap();
    let b = prepare_language_candidates(
        &source,
        consumer.original().state(),
        AuthorityRevision(1),
        registration(),
        changed,
        limits(),
    )
    .unwrap();
    assert_ne!(a.delta().edits(), b.delta().edits());
}
