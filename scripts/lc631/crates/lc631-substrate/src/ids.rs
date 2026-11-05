//! Digest-backed stable labels and process-local handles are deliberately distinct.
//! LogicalIds are caller-computed labels; these types do not verify payload hashes
//! or content bindings. Consumers must validate those bindings independently.
use crate::Digest32;
use std::num::NonZeroU64;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LogicalId(Digest32);

impl LogicalId {
    pub const fn from_digest(digest: Digest32) -> Self {
        Self(digest)
    }

    pub const fn digest(self) -> Digest32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceId(Digest32);

impl SourceId {
    pub const fn from_digest(digest: Digest32) -> Self {
        Self(digest)
    }

    pub const fn digest(self) -> Digest32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ContextId(Digest32);

impl ContextId {
    pub const fn from_digest(digest: Digest32) -> Self {
        Self(digest)
    }

    pub const fn digest(self) -> Digest32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SymbolId(Digest32);

impl SymbolId {
    pub const fn from_digest(digest: Digest32) -> Self {
        Self(digest)
    }

    pub const fn digest(self) -> Digest32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AstId(Digest32);

impl AstId {
    pub const fn from_digest(digest: Digest32) -> Self {
        Self(digest)
    }

    pub const fn digest(self) -> Digest32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ConditionId(Digest32);

impl ConditionId {
    pub const fn from_digest(digest: Digest32) -> Self {
        Self(digest)
    }

    pub const fn digest(self) -> Digest32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskId(Digest32);

impl TaskId {
    pub const fn from_digest(digest: Digest32) -> Self {
        Self(digest)
    }

    pub const fn digest(self) -> Digest32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EvidenceId(Digest32);

impl EvidenceId {
    pub const fn from_digest(digest: Digest32) -> Self {
        Self(digest)
    }

    pub const fn digest(self) -> Digest32 {
        self.0
    }
}

/// Unique within this process, never a persistent or authenticated identifier.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ArenaDomainId(NonZeroU64);

static NEXT_DOMAIN: AtomicU64 = AtomicU64::new(1);

impl ArenaDomainId {
    pub(crate) fn fresh() -> Option<Self> {
        Self::take_from(&NEXT_DOMAIN)
    }

    fn take_from(counter: &AtomicU64) -> Option<Self> {
        // Only uniqueness is published. Arena contents have no atomic publication
        // contract, so neither release/acquire nor SeqCst is needed here.
        let value = counter
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
                next.checked_add(1)
            })
            .ok()?;
        NonZeroU64::new(value).map(Self)
    }
}

/// Process-local access identity; references returned by an arena borrow its owner.
/// No deserialize or public raw constructor exists.
/// Resume must resolve stable LogicalIds into fresh keys in a fresh arena.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ArenaKey {
    domain: ArenaDomainId,
    context: ContextId,
    slot: u32,
    generation: u32,
}

impl ArenaKey {
    pub(crate) const fn new(
        domain: ArenaDomainId,
        context: ContextId,
        slot: u32,
        generation: u32,
    ) -> Self {
        Self {
            domain,
            context,
            slot,
            generation,
        }
    }

    pub const fn domain(self) -> ArenaDomainId {
        self.domain
    }

    pub const fn context(self) -> ContextId {
        self.context
    }

    pub const fn slot(self) -> u32 {
        self.slot
    }

    pub const fn generation(self) -> u32 {
        self.generation
    }
}

#[cfg(test)]
mod tests {
    use super::{ArenaDomainId, AtomicU64, Ordering};

    #[test]
    fn tlcx_02_domain_counter_exhaustion_never_wraps_or_reissues() {
        let counter = AtomicU64::new(u64::MAX - 1);
        assert!(ArenaDomainId::take_from(&counter).is_some());
        assert!(ArenaDomainId::take_from(&counter).is_none());
        assert!(ArenaDomainId::take_from(&counter).is_none());
        assert_eq!(counter.load(Ordering::Relaxed), u64::MAX);
    }
}
