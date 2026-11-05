use lc631_core::context::{ContextRevision, EvidenceRevision, ReasoningRevisions, SourceRevision};
use lc631_core::AuthorityRevision;

#[test]
fn tlcx_01_reconstructed_context_cannot_replace_authority_or_source() {
    let source = SourceRevision::of("Keep the mandatory constraint.");
    let original = ContextRevision::original(source);
    let revisions = ReasoningRevisions::observed(
        original,
        AuthorityRevision(7),
        EvidenceRevision::observed(9),
    );
    let reconstructed = original.reconstructed([3; 32]);
    let changed = revisions.with_context(reconstructed).unwrap();
    assert_eq!(changed.context().source(), source);
    assert_eq!(changed.authority(), AuthorityRevision(7));
    assert_eq!(changed.evidence(), EvidenceRevision::observed(9));
    assert_eq!(revisions.context(), original);
    let other_source = ContextRevision::original(SourceRevision::of("Different instruction."));
    assert!(revisions.with_context(other_source).is_err());
}
