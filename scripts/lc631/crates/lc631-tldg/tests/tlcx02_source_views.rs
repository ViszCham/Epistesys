use lc631_core::SourceSpan;
use lc631_substrate::source::{SourceError, SourceSpan as CheckedSourceSpan, SourceStore};
use lc631_tldg::{
    analyze, BoundaryCell, BoundaryClass, BoundaryLedger, CanonicalBoundaryLedger,
    ConstraintBindingError, SourceRevision, SourceTextView, Token, TokenKind,
};
use std::sync::Arc;

#[test]
fn tlcx_02_legacy_boundary_rejects_changed_surface_under_original_revision() {
    let mut ledger = BoundaryLedger::build("A");
    ledger.cells[0].surface = "B".to_owned();
    assert!(!ledger.covers_source());
}

#[test]
fn tlcx_02_legacy_boundary_rejects_utf8_interior_span_partition() {
    let ledger = BoundaryLedger {
        source: SourceRevision::from_source("日"),
        cells: vec![
            BoundaryCell {
                span: SourceSpan { start: 0, end: 1 },
                class: BoundaryClass::Alphanumeric,
                surface: "日".to_owned(),
            },
            BoundaryCell {
                span: SourceSpan { start: 1, end: 3 },
                class: BoundaryClass::Symbol,
                surface: String::new(),
            },
        ],
    };
    assert!(!ledger.covers_source());
}

#[test]
fn tlcx_02_canonical_cells_borrow_one_source_allocation() {
    let text: Arc<str> = Arc::from("日🙂\r\n");
    let source = SourceStore::new(Arc::clone(&text), 64).unwrap();
    let ledger = CanonicalBoundaryLedger::from_store(&source, 4).unwrap();
    assert!(std::ptr::eq(ledger.source(), &source));
    assert_eq!(ledger.roundtrip().as_ptr(), text.as_ptr());
    assert!(ledger.covers_source());
    assert_eq!(ledger.cells().len(), 4);
    for cell in ledger.cells() {
        let span = cell.span();
        let surface = ledger.surface(span).unwrap();
        assert_eq!(surface.as_ptr(), text[span.start()..span.end()].as_ptr());
        assert_eq!(surface.chars().count(), 1);
    }
    assert_eq!(ledger.cells()[0].class(), BoundaryClass::Alphanumeric);
    assert_eq!(ledger.cells()[1].class(), BoundaryClass::Symbol);
}

#[test]
fn tlcx_02_borrowed_boundary_json_preserves_the_legacy_shape_and_bytes() {
    for text in ["", "A", "日🙂\r\n", "e\u{301}\0", "引用>文"] {
        let source = SourceStore::new(Arc::from(text), 64).unwrap();
        let ledger = CanonicalBoundaryLedger::from_store(&source, text.len()).unwrap();
        let legacy = BoundaryLedger::build(text);
        assert_eq!(SourceRevision::from_store(&source), legacy.source);
        assert_eq!(
            serde_json::to_string(&ledger).unwrap(),
            serde_json::to_string(&legacy).unwrap()
        );
    }
    let empty = SourceStore::new(Arc::from(""), 1).unwrap();
    let ledger = CanonicalBoundaryLedger::from_store(&empty, 0).unwrap();
    assert_eq!(
        serde_json::to_string(&ledger).unwrap(),
        concat!(
            "{\"source\":{\"revision\":\"sha256:",
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            "\",\"byte_len\":0},\"cells\":[]}"
        )
    );
}

#[test]
fn tlcx_02_source_text_view_rejects_foreign_and_invalid_spans() {
    let source = SourceStore::new(Arc::from("日"), 64).unwrap();
    let other = SourceStore::new(Arc::from("A"), 64).unwrap();
    let foreign = CheckedSourceSpan::checked(&other, 0, 1).unwrap();
    assert_eq!(
        SourceTextView::from_span(&source, foreign).unwrap_err(),
        SourceError::SourceMismatch
    );
    assert_eq!(
        SourceTextView::checked(&source, 1, 3).unwrap_err(),
        SourceError::InvalidUtf8Boundary
    );
    assert_eq!(
        SourceTextView::checked(&source, 3, 0).unwrap_err(),
        SourceError::ReversedSpan
    );
}

#[test]
fn tlcx_02_source_text_view_clone_and_string_serialization_share_bytes() {
    let source = SourceStore::new(Arc::from("prefix 日本語\r\n"), 64).unwrap();
    let view = SourceTextView::checked(&source, 7, source.text().len()).unwrap();
    let clone = view.clone();
    assert_eq!(view.as_str(), "日本語\r\n");
    assert_eq!(view.as_ptr(), source.text()[7..].as_ptr());
    assert_eq!(clone.as_ptr(), view.as_ptr());
    assert_eq!(view.source().text().as_ptr(), source.text().as_ptr());
    assert_eq!(serde_json::to_string(&view).unwrap(), "\"日本語\\r\\n\"");
    assert_eq!(SourceTextView::full(&source).unwrap(), source.text());
    assert_eq!(SourceTextView::default(), "");
}

#[test]
fn tlcx_02_token_view_preserves_exact_legacy_json_field_order() {
    let source = SourceStore::new(Arc::from("alpha"), 64).unwrap();
    let token = Token {
        id: 0,
        span: SourceSpan { start: 0, end: 5 },
        kind: TokenKind::Word,
        surface: SourceTextView::full(&source).unwrap(),
    };
    assert_eq!(
        serde_json::to_string(&token).unwrap(),
        "{\"id\":0,\"span\":{\"start\":0,\"end\":5},\"kind\":\"word\",\"surface\":\"alpha\"}"
    );
}

#[test]
fn tlcx_02_constraint_binding_rejects_foreign_source_with_identical_token_text() {
    let mut artifact = analyze("alpha beta").unwrap();
    let foreign = SourceStore::new(Arc::from("alpha"), 64).unwrap();
    artifact.tokens.tokens[0].surface = SourceTextView::full(&foreign).unwrap();
    assert_eq!(
        artifact.constraints.validate_source_bindings(
            "alpha beta",
            &artifact.tokens,
            &artifact.syntax
        ),
        Err(ConstraintBindingError::InvalidSourceSpan)
    );
}

#[test]
fn tlcx_02_constraint_binding_rejects_same_text_at_a_different_source_offset() {
    let mut artifact = analyze("x x").unwrap();
    let source = artifact.boundary_roundtrip.source().clone();
    artifact.tokens.tokens[0].surface = SourceTextView::checked(&source, 2, 3).unwrap();
    assert_eq!(
        artifact
            .constraints
            .validate_source_bindings("x x", &artifact.tokens, &artifact.syntax),
        Err(ConstraintBindingError::InvalidSourceSpan)
    );
}
