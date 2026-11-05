use lc631_substrate::source::{SourceError, SourceSpan, SourceStore};
use std::sync::Arc;

#[test]
fn tlcx_02_source_rejects_oversize_before_admission() {
    assert_eq!(
        SourceStore::new(Arc::from("four"), 3).unwrap_err(),
        SourceError::LimitExceeded,
    );
    assert!(SourceStore::new(Arc::from(""), 0).is_err());
}

#[test]
fn tlcx_02_empty_source_with_a_positive_limit_is_valid() {
    let source = SourceStore::new(Arc::from(""), 1).unwrap();
    let span = SourceSpan::checked(&source, 0, 0).unwrap();
    assert_eq!(source.text(), "");
    assert_eq!(source.slice(span), Ok(""));
}

#[test]
fn tlcx_02_spans_reject_invalid_order_bounds_and_utf8_boundaries() {
    let source = SourceStore::new(Arc::from("aあ🦀z"), 64).unwrap();
    assert_eq!(
        SourceSpan::checked(&source, 4, 1),
        Err(SourceError::ReversedSpan),
    );
    for (start, end) in [(0, 10), (usize::MAX, usize::MAX)] {
        assert_eq!(
            SourceSpan::checked(&source, start, end),
            Err(SourceError::OutOfBounds),
        );
    }
    for (start, end) in [(2, 4), (1, 3), (5, 5)] {
        assert_eq!(
            SourceSpan::checked(&source, start, end),
            Err(SourceError::InvalidUtf8Boundary),
        );
    }
}

#[test]
fn tlcx_02_source_rejects_a_span_from_different_bytes() {
    let original = SourceStore::new(Arc::from("alpha"), 32).unwrap();
    let other = SourceStore::new(Arc::from("omega"), 32).unwrap();
    let span = SourceSpan::checked(&original, 0, 5).unwrap();
    assert_eq!(other.slice(span), Err(SourceError::SourceMismatch));
}

#[test]
fn tlcx_02_source_digest_binds_actual_utf8_bytes() {
    let source = SourceStore::new(Arc::from("abc"), 32).unwrap();
    let expected = [
        0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae, 0x22,
        0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61, 0xf2, 0x00,
        0x15, 0xad,
    ];
    assert_eq!(source.id().digest().as_bytes(), &expected);
    let changed = SourceStore::new(Arc::from("abd"), 32).unwrap();
    assert_ne!(source.id(), changed.id());
}

#[test]
fn tlcx_02_source_views_share_one_arc_allocation() {
    let text: Arc<str> = Arc::from("sharedあsource");
    let source = SourceStore::new(Arc::clone(&text), 64).unwrap();
    let view = source.clone();
    assert_eq!(Arc::strong_count(&text), 3);
    assert_eq!(source.text().as_ptr(), text.as_ptr());
    assert_eq!(view.text().as_ptr(), text.as_ptr());
    let span = SourceSpan::checked(&source, 6, 9).unwrap();
    assert_eq!(span.source_id(), source.id());
    assert_eq!((span.start(), span.end()), (6, 9));
    assert_eq!(view.slice(span).unwrap(), "あ");
}

#[test]
fn tlcx_02_empty_spans_are_allowed_only_at_valid_utf8_boundaries() {
    let source = SourceStore::new(Arc::from("aあ🦀z"), 64).unwrap();
    for position in [0, 1, 4, 8, 9] {
        let span = SourceSpan::checked(&source, position, position).unwrap();
        assert_eq!(source.slice(span).unwrap(), "");
    }
    for position in [2, 3, 5, 6, 7] {
        assert!(SourceSpan::checked(&source, position, position).is_err());
    }
}

#[test]
fn tlcx_02_all_small_utf8_ranges_match_checked_string_slicing() {
    let text = "aéあ🦀z";
    let source = SourceStore::new(Arc::from(text), 64).unwrap();
    let mut accepted = 0;
    let mut rejected = 0;
    for start in 0..=text.len() + 1 {
        for end in 0..=text.len() + 1 {
            match (
                text.get(start..end),
                SourceSpan::checked(&source, start, end),
            ) {
                (Some(expected), Ok(span)) => {
                    assert_eq!(source.slice(span).unwrap(), expected);
                    accepted += 1;
                }
                (None, Err(_)) => rejected += 1,
                inconsistent => panic!("range {start}..{end} disagrees: {inconsistent:?}"),
            }
        }
    }
    assert!(accepted > 0);
    assert!(rejected > accepted);
}
