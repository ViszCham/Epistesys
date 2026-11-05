//! Full declared dependency pins for a turn-local cache key, not evidence.
use super::{CacheError, CacheIdentity};
use lc631_substrate::Digest32;

/// Additional measurement inputs are explicit, not implicitly hidden in a
/// source hash or silently replaced by zero. An inapplicable dependency needs
/// a nonzero, scope-bound declared policy revision, not a guessed empty value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MeasurementCacheRevisions {
    pub target: Digest32,
    pub baseline: Digest32,
    pub corpus: Digest32,
    pub oracle: Digest32,
    pub policy: Digest32,
    pub evidence: Digest32,
    pub definition: Digest32,
    pub encoding: Digest32,
}

/// New turn caches require all 19 declared pins. Legacy eleven-pin v2 cache
/// keys remain separate and cannot be passed directly to the new SDK methods.
/// Pin completeness is not independent verification of the caller's metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TurnCacheIdentity {
    pub(super) inner: CacheIdentity,
}
impl TurnCacheIdentity {
    pub fn checked(
        base: CacheIdentity,
        revisions: MeasurementCacheRevisions,
    ) -> Result<Self, CacheError> {
        Ok(Self {
            inner: base.with_measurement_revisions([
                revisions.target,
                revisions.baseline,
                revisions.corpus,
                revisions.oracle,
                revisions.policy,
                revisions.evidence,
                revisions.definition,
                revisions.encoding,
            ])?,
        })
    }
}
