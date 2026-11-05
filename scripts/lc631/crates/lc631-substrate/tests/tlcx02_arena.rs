use lc631_substrate::arena::{Arena, ArenaError};
use lc631_substrate::ids::{ContextId, LogicalId};
use lc631_substrate::Digest32;

fn context(value: u8) -> ContextId {
    ContextId::from_digest(Digest32::from_bytes([value; 32]))
}

fn logical(value: u8) -> LogicalId {
    LogicalId::from_digest(Digest32::from_bytes([value; 32]))
}

#[test]
fn tlcx_02_arena_rejects_invalid_capacity_and_impossible_allocation() {
    assert!(Arena::<u8>::new(context(1), 0).is_err());
    assert!(Arena::<u8>::new(context(1), usize::MAX).is_err());
    // No large allocation may be attempted merely to discover this is over budget.
    assert!(Arena::<[u8; 1024]>::new(context(1), u32::MAX as usize).is_err());
}

#[test]
fn tlcx_02_arena_rejects_duplicate_live_ids_and_capacity_overflow() {
    let mut arena = Arena::new(context(1), 1).unwrap();
    let key = arena.insert(logical(1), "first").unwrap();
    assert_eq!(
        arena.insert(logical(1), "duplicate"),
        Err(ArenaError::DuplicateLogicalId),
    );
    assert_eq!(
        arena.insert(logical(2), "overflow"),
        Err(ArenaError::CapacityExceeded),
    );
    assert_eq!(arena.get(key).unwrap(), &"first");
}

#[test]
fn tlcx_02_arena_rejects_foreign_instances_even_in_the_same_context() {
    let first = Arena::<u64>::new(context(1), 2).unwrap();
    let mut other = Arena::new(context(1), 2).unwrap();
    let foreign = other.insert(logical(1), 9_u64).unwrap();
    assert_eq!(first.get(foreign), Err(ArenaError::ForeignDomain));
    assert_eq!(other.get(foreign), Ok(&9));
}

#[test]
fn tlcx_02_arena_rejects_cross_context_handles() {
    let first = Arena::<u64>::new(context(1), 2).unwrap();
    let mut other = Arena::new(context(2), 2).unwrap();
    let foreign = other.insert(logical(1), 9_u64).unwrap();
    assert_ne!(foreign.context(), context(1));
    assert!(first.get(foreign).is_err());
}

#[test]
fn tlcx_02_arena_slot_reuse_never_revives_a_stale_handle() {
    let mut arena = Arena::new(context(1), 1).unwrap();
    let old = arena.insert(logical(1), "old").unwrap();
    assert_eq!(arena.remove(old).unwrap(), "old");
    assert_eq!(arena.get(old), Err(ArenaError::StaleGeneration));
    assert_eq!(arena.remove(old), Err(ArenaError::StaleGeneration));
    let current = arena.insert(logical(2), "new").unwrap();
    assert_eq!(old.slot(), current.slot());
    assert!(current.generation() > old.generation());
    assert_ne!(old, current);
    assert_eq!(arena.get(old), Err(ArenaError::StaleGeneration));
    assert_eq!(arena.get(current), Ok(&"new"));
}

#[test]
fn tlcx_02_removed_stable_identity_cannot_be_rebound_to_changed_payload() {
    let mut arena = Arena::new(context(1), 2).unwrap();
    let old = arena.insert(logical(1), vec![1, 2]).unwrap();
    assert_eq!(arena.remove(old).unwrap(), vec![1, 2]);
    assert!(arena.insert(logical(1), vec![3, 4]).is_err());
    assert_eq!(arena.get(old), Err(ArenaError::StaleGeneration));
    let fresh = arena.insert(logical(2), vec![3, 4]).unwrap();
    assert_eq!(arena.get(fresh).unwrap(), &[3, 4]);
}

#[test]
fn tlcx_02_history_exhaustion_never_discards_stable_id_tombstones() {
    let mut arena = Arena::with_limits(context(1), 1, 2, 1024).unwrap();
    for id in [logical(1), logical(2)] {
        let key = arena.insert(id, 9).unwrap();
        assert_eq!(arena.remove(key), Ok(9));
    }
    assert_eq!(
        arena.insert(logical(3), 9),
        Err(ArenaError::HistoryCapacityExceeded),
    );
    assert_eq!(
        arena.insert(logical(1), 99),
        Err(ArenaError::ReusedLogicalId),
    );
}

#[test]
fn tlcx_02_backing_budget_includes_slots_and_identity_history() {
    assert!(matches!(
        Arena::<u64>::with_limits(context(1), 1, 2, 1),
        Err(ArenaError::StorageBudgetExceeded),
    ));
    assert!(matches!(
        Arena::<u64>::with_limits(context(1), 2, 1, 1024),
        Err(ArenaError::InvalidCapacity),
    ));
    assert!(matches!(
        Arena::<u64>::with_limits(context(1), 1, 2, 0),
        Err(ArenaError::InvalidByteBudget),
    ));
    assert!(Arena::<u64>::with_limits(context(1), 1, 2, 1024).is_ok());
}
