//! Future PRIVATE actual-helper parity packet; unregistered until helper GO.
use super::*;
#[test]
fn cached_source_revision_preserves_exact_legacy_spelling() {
    let larger = "a".repeat(16 * 1024);
    for text in ["", "日本語 Σ", larger.as_str()] {
        let store =
            lc631_substrate::source::SourceStore::new(std::sync::Arc::from(text), 16 * 1024)
                .unwrap();
        assert_eq!(
            cached_legacy_source_revision(&store).unwrap(),
            lc631_core::stable_sha256(text)
        );
    }
    // Parity only; no source-size scan or timing observation is claimed.
}
