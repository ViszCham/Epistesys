//! Preserve the existing core facade while moving pure identity ownership down.
use lc631_core::context::{ContextRevision, SourceRevision};

#[test]
fn tlcx_identity_core_and_substrate_are_the_same_type_and_keep_source_goldens() {
    let source = SourceRevision::of("abc");
    let bytes = source.digest();
    let hex = bytes
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(
        hex,
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    let low: lc631_substrate::context_identity::ContextRevision = ContextRevision::original(source);
    let high: ContextRevision = low.reconstructed([7; 32]);
    assert_eq!(high.source(), source);
    assert_eq!(high.interpretation_digest().as_bytes(), &[7; 32]);
    assert_eq!(low.interpretation_digest(), source.digest());
}
