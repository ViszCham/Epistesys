//! Restart policy for the volatile recomputable cache.
//!
//! Cache entries are process-local acceleration only. They are deliberately
//! not serialized, replayed, or restored after a crash/restart. Durable
//! evidence and replay state require their own separately owned mechanisms.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CacheRecoveryDecision {
    /// Rebuild a fresh cache only from the current source and full identity.
    RecomputeFromCurrentSource,
    /// No current source is available, so do not recreate or reuse a cache hit.
    HoldForCurrentSource,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct CacheRecoveryPolicy;

impl CacheRecoveryPolicy {
    /// An unclean exit and a normal restart have the same cache semantics:
    /// cold start, then recompute from current source if it is available.
    pub(crate) const fn after_process_restart(
        current_source_available: bool,
    ) -> CacheRecoveryDecision {
        if current_source_available {
            CacheRecoveryDecision::RecomputeFromCurrentSource
        } else {
            CacheRecoveryDecision::HoldForCurrentSource
        }
    }

    /// This volatile cache has no persistence or rehydration contract.
    pub(crate) const fn prior_cache_hit_is_recoverable() -> bool {
        false
    }
}
