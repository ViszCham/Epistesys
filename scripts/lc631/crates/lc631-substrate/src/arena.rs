//! Safe single-owner storage with checked, process-local generation handles.
use crate::ids::{ArenaDomainId, ArenaKey, ContextId, LogicalId};
use std::fmt;
use std::mem::size_of;

// Initial TLCX run ceiling, not a process-wide or nested-payload memory guarantee.
// The runtime resource broker must additionally reserve all owned allocations.
const MAX_BACKING_BYTES: usize = 128 * 1024 * 1024;
const DEFAULT_HISTORY_MULTIPLIER: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArenaError {
    InvalidCapacity,
    InvalidByteBudget,
    StorageBudgetExceeded,
    AllocationFailed,
    DomainExhausted,
    DuplicateLogicalId,
    ReusedLogicalId,
    CapacityExceeded,
    HistoryCapacityExceeded,
    RetiredCapacityExhausted,
    ForeignContext,
    ForeignDomain,
    InvalidSlot,
    StaleGeneration,
    VacantSlot,
    InternalInvariant,
}

impl fmt::Display for ArenaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidCapacity => "arena capacity is invalid",
            Self::InvalidByteBudget => "arena backing-byte budget is invalid",
            Self::StorageBudgetExceeded => "arena backing allocation exceeds its budget",
            Self::AllocationFailed => "arena backing allocation failed",
            Self::DomainExhausted => "process-local arena domains are exhausted",
            Self::DuplicateLogicalId => "logical identity is already live in this arena",
            Self::ReusedLogicalId => "removed logical identity cannot be rebound in this arena",
            Self::CapacityExceeded => "arena live capacity is exhausted",
            Self::HistoryCapacityExceeded => "bounded stable-identity history is exhausted",
            Self::RetiredCapacityExhausted => "retired slots exhaust bounded arena storage",
            Self::ForeignContext => "arena handle belongs to a different context",
            Self::ForeignDomain => "arena handle belongs to a different arena instance",
            Self::InvalidSlot => "arena handle slot does not exist",
            Self::StaleGeneration => "arena handle generation is stale or retired",
            Self::VacantSlot => "arena handle names a vacant slot",
            Self::InternalInvariant => "arena internal accounting is inconsistent",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for ArenaError {}

struct Slot<T> {
    generation: u32,
    value: Option<(LogicalId, T)>,
    retired: bool,
}

/// One mutable owner; references borrow that owner and cannot survive removal.
/// Keys are not evidence, authority, persistent IDs, or validated semantic refs.
pub struct Arena<T> {
    domain: ArenaDomainId,
    context: ContextId,
    slots: Vec<Slot<T>>,
    seen_ids: Vec<LogicalId>,
    max_live: usize,
    max_identities: usize,
    live: usize,
}

impl<T> Arena<T> {
    pub fn new(context: ContextId, max_live: usize) -> Result<Self, ArenaError> {
        Self::with_storage_budget(context, max_live, MAX_BACKING_BYTES)
    }

    /// A lower backing-storage budget permits deterministic admission checks.
    /// Heap allocations inside T need their own runtime reservations.
    pub fn with_storage_budget(
        context: ContextId,
        max_live: usize,
        max_backing_bytes: usize,
    ) -> Result<Self, ArenaError> {
        let max_identities = max_live
            .checked_mul(DEFAULT_HISTORY_MULTIPLIER)
            .ok_or(ArenaError::InvalidCapacity)?;
        Self::with_limits(context, max_live, max_identities, max_backing_bytes)
    }

    /// Limits cover slot storage and stable-ID tombstones, not heap memory in T.
    /// The default history holds four times max_live; callers may pin a different
    /// finite history limit. Exhaustion never evicts IDs or revives old handles.
    pub fn with_limits(
        context: ContextId,
        max_live: usize,
        max_identities: usize,
        max_backing_bytes: usize,
    ) -> Result<Self, ArenaError> {
        if max_live == 0 || max_live > u32::MAX as usize {
            return Err(ArenaError::InvalidCapacity);
        }
        if max_identities < max_live || max_identities > u32::MAX as usize {
            return Err(ArenaError::InvalidCapacity);
        }
        if max_backing_bytes == 0 || max_backing_bytes > MAX_BACKING_BYTES {
            return Err(ArenaError::InvalidByteBudget);
        }
        let slot_bytes = max_live
            .checked_mul(size_of::<Slot<T>>())
            .ok_or(ArenaError::StorageBudgetExceeded)?;
        let identity_bytes = max_identities
            .checked_mul(size_of::<LogicalId>())
            .ok_or(ArenaError::StorageBudgetExceeded)?;
        let bytes = slot_bytes
            .checked_add(identity_bytes)
            .ok_or(ArenaError::StorageBudgetExceeded)?;
        if bytes > max_backing_bytes {
            return Err(ArenaError::StorageBudgetExceeded);
        }
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(max_live)
            .map_err(|_| ArenaError::AllocationFailed)?;
        let mut seen_ids = Vec::new();
        seen_ids
            .try_reserve_exact(max_identities)
            .map_err(|_| ArenaError::AllocationFailed)?;
        let reserved_slot_bytes = slots
            .capacity()
            .checked_mul(size_of::<Slot<T>>())
            .ok_or(ArenaError::StorageBudgetExceeded)?;
        let reserved_identity_bytes = seen_ids
            .capacity()
            .checked_mul(size_of::<LogicalId>())
            .ok_or(ArenaError::StorageBudgetExceeded)?;
        let reserved_bytes = reserved_slot_bytes
            .checked_add(reserved_identity_bytes)
            .ok_or(ArenaError::StorageBudgetExceeded)?;
        if reserved_bytes > max_backing_bytes {
            return Err(ArenaError::StorageBudgetExceeded);
        }
        let domain = ArenaDomainId::fresh().ok_or(ArenaError::DomainExhausted)?;
        Ok(Self {
            domain,
            context,
            slots,
            seen_ids,
            max_live,
            max_identities,
            live: 0,
        })
    }

    pub fn insert(&mut self, stable_id: LogicalId, value: T) -> Result<ArenaKey, ArenaError> {
        if self
            .slots
            .iter()
            .any(|slot| slot.value.as_ref().is_some_and(|(id, _)| *id == stable_id))
        {
            return Err(ArenaError::DuplicateLogicalId);
        }
        let identity_position = match self.seen_ids.binary_search(&stable_id) {
            Ok(_) => return Err(ArenaError::ReusedLogicalId),
            Err(position) => position,
        };
        if self.live >= self.max_live {
            return Err(ArenaError::CapacityExceeded);
        }
        if self.seen_ids.len() >= self.max_identities {
            return Err(ArenaError::HistoryCapacityExceeded);
        }
        let reusable = self
            .slots
            .iter()
            .position(|slot| slot.value.is_none() && !slot.retired);
        let index = match reusable {
            Some(index) => index,
            None if self.slots.len() < self.max_live => {
                let index = self.slots.len();
                // Constructor reserved every possible slot before publication.
                self.slots.push(Slot {
                    generation: 0,
                    value: None,
                    retired: false,
                });
                index
            }
            None => return Err(ArenaError::RetiredCapacityExhausted),
        };
        let slot_index = u32::try_from(index).map_err(|_| ArenaError::InvalidSlot)?;
        let slot = self.slots.get_mut(index).ok_or(ArenaError::InvalidSlot)?;
        slot.value = Some((stable_id, value));
        // Constructor reserved the entire bounded history, so publication does
        // not introduce an allocation between slot and identity-ledger updates.
        self.seen_ids.insert(identity_position, stable_id);
        self.live += 1;
        Ok(ArenaKey::new(
            self.domain,
            self.context,
            slot_index,
            slot.generation,
        ))
    }

    pub fn get(&self, key: ArenaKey) -> Result<&T, ArenaError> {
        let index = self.checked_index(key)?;
        self.slots[index]
            .value
            .as_ref()
            .map(|(_, value)| value)
            .ok_or(ArenaError::VacantSlot)
    }

    pub fn remove(&mut self, key: ArenaKey) -> Result<T, ArenaError> {
        let index = self.checked_index(key)?;
        let next_live = self
            .live
            .checked_sub(1)
            .ok_or(ArenaError::InternalInvariant)?;
        let slot = &mut self.slots[index];
        let (_, value) = slot.value.take().ok_or(ArenaError::VacantSlot)?;
        match slot.generation.checked_add(1) {
            Some(next_generation) => slot.generation = next_generation,
            None => slot.retired = true,
        }
        self.live = next_live;
        Ok(value)
    }

    fn checked_index(&self, key: ArenaKey) -> Result<usize, ArenaError> {
        if key.context() != self.context {
            return Err(ArenaError::ForeignContext);
        }
        if key.domain() != self.domain {
            return Err(ArenaError::ForeignDomain);
        }
        let index = key.slot() as usize;
        let slot = self.slots.get(index).ok_or(ArenaError::InvalidSlot)?;
        if slot.generation != key.generation() || slot.retired {
            return Err(ArenaError::StaleGeneration);
        }
        if slot.value.is_none() {
            return Err(ArenaError::VacantSlot);
        }
        Ok(index)
    }
}

#[cfg(test)]
mod tests {
    use super::{Arena, ArenaError};
    use crate::ids::{ArenaKey, ContextId, LogicalId};
    use crate::Digest32;

    fn context(value: u8) -> ContextId {
        ContextId::from_digest(Digest32::from_bytes([value; 32]))
    }

    #[test]
    fn tlcx_02_generation_overflow_retires_instead_of_wrapping() {
        let mut arena = Arena::new(context(1), 1).unwrap();
        let logical = LogicalId::from_digest(Digest32::from_bytes([2; 32]));
        arena.insert(logical, "old").unwrap();
        arena.slots[0].generation = u32::MAX;
        let exhausted = ArenaKey::new(arena.domain, arena.context, 0, u32::MAX);
        assert_eq!(arena.remove(exhausted).unwrap(), "old");
        assert_eq!(arena.get(exhausted), Err(ArenaError::StaleGeneration));
        assert_eq!(
            arena.insert(
                LogicalId::from_digest(Digest32::from_bytes([3; 32])),
                "replacement",
            ),
            Err(ArenaError::RetiredCapacityExhausted),
        );
        assert_eq!(arena.slots[0].generation, u32::MAX);
        assert!(arena.slots[0].retired);
    }

    #[test]
    fn tlcx_02_checked_access_rejects_each_forged_handle_component() {
        let mut arena = Arena::new(context(1), 2).unwrap();
        let id = LogicalId::from_digest(Digest32::from_bytes([2; 32]));
        let key = arena.insert(id, 9).unwrap();
        let wrong_context = ArenaKey::new(key.domain(), context(3), key.slot(), key.generation());
        assert_eq!(arena.get(wrong_context), Err(ArenaError::ForeignContext));
        let wrong_slot = ArenaKey::new(key.domain(), key.context(), 1, key.generation());
        assert_eq!(arena.get(wrong_slot), Err(ArenaError::InvalidSlot));
        let stale = ArenaKey::new(key.domain(), key.context(), key.slot(), 1);
        assert_eq!(arena.get(stale), Err(ArenaError::StaleGeneration));
        assert_eq!(arena.get(key), Ok(&9));
    }
}
