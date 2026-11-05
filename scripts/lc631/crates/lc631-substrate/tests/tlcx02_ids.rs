use lc631_substrate::ids::{
    AstId, ConditionId, ContextId, EvidenceId, LogicalId, SourceId, SymbolId, TaskId,
};
use lc631_substrate::Digest32;
use std::any::TypeId;
use std::collections::BTreeSet;

#[test]
fn tlcx_02_stable_id_types_cannot_collapse_to_aliases() {
    let types = [
        TypeId::of::<AstId>(),
        TypeId::of::<ConditionId>(),
        TypeId::of::<ContextId>(),
        TypeId::of::<EvidenceId>(),
        TypeId::of::<LogicalId>(),
        TypeId::of::<SourceId>(),
        TypeId::of::<SymbolId>(),
        TypeId::of::<TaskId>(),
    ];
    assert_eq!(types.into_iter().collect::<BTreeSet<_>>().len(), 8);
}

#[test]
fn tlcx_02_stable_ids_preserve_digest_without_runtime_slot_identity() {
    let digest = Digest32::from_bytes([7; 32]);
    assert_eq!(AstId::from_digest(digest).digest(), digest);
    assert_eq!(ConditionId::from_digest(digest).digest(), digest);
    assert_eq!(ContextId::from_digest(digest).digest(), digest);
    assert_eq!(EvidenceId::from_digest(digest).digest(), digest);
    assert_eq!(LogicalId::from_digest(digest).digest(), digest);
    assert_eq!(SourceId::from_digest(digest).digest(), digest);
    assert_eq!(SymbolId::from_digest(digest).digest(), digest);
    assert_eq!(TaskId::from_digest(digest).digest(), digest);
}
