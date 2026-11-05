//! 日本語: turn内だけの再計算可能コピー。既定off、disk書込みなし、終了時自動解放。
//! English: Turn-local recomputable copies; off by default, no disk writes,
//! automatically dropped at the turn boundary, including error/unwind paths.
//! Cache hits never reconstruct evidence, authority, completion or delivery.
#![forbid(unsafe_code)]
mod identity;
pub use identity::{MeasurementCacheRevisions, TurnCacheIdentity};

pub use crate::store::cache::{CacheError, CacheIdentity, RecomputableArtifactKind};
use crate::store::cache::{CacheLookup, CachePolicy, CacheStore};
use lc631_substrate::{resource::ResourceReservationView, Digest32};
use std::time::{Duration, Instant};

/// Explicit engineering preference, not a semantic or authority policy.
/// There is deliberately no disk-root or persistent-cache option.
#[derive(Clone, Copy, Debug, Default)]
pub struct TurnCachePolicy {
    inner: CachePolicy,
}
impl TurnCachePolicy {
    pub fn memory(entries: usize, bytes: u64, ttl: Duration) -> Result<Self, CacheError> {
        Ok(Self {
            inner: CachePolicy::memory(entries, bytes, ttl)?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RememberOutcome {
    Disabled,
    Copied { key: Digest32 },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CacheLookupState {
    Disabled,
    Miss,
    ExpiredAndReclaimed,
    CorruptAndReclaimed,
    Hit,
}
pub struct CacheAccess<T> {
    pub state: CacheLookupState,
    pub value: T,
}

/// This reports copies owned by the cache, not OS RSS reduction or the removal
/// of original inputs. Prior expiry/corruption reclamations are separate events.
pub struct TurnCacheOutcome<T> {
    pub value: T,
    pub reclaimed_entries_at_end: usize,
    pub reclaimed_payload_bytes_at_end: u64,
    pub disk_cache_enabled: bool,
}

/// Cannot be constructed, deserialized or retained outside with_turn_cache.
/// Only copies of caller-borrowed recomputable bytes enter this memory owner.
pub struct TurnCache<'store, 'resource> {
    store: CacheStore<'store, 'resource>,
    current: &'store ResourceReservationView<'resource>,
}
impl TurnCache<'_, '_> {
    pub fn remember(
        &mut self,
        identity: &TurnCacheIdentity,
        kind: RecomputableArtifactKind,
        bytes: &[u8],
        now: Instant,
    ) -> Result<RememberOutcome, CacheError> {
        self.current.check().map_err(CacheError::Resource)?;
        if self.store.stats().mode == crate::store::cache::CacheMode::Off {
            return Ok(RememberOutcome::Disabled);
        }
        let key = self.store.insert_copy(identity.inner, kind, bytes, now)?;
        Ok(RememberOutcome::Copied { key: key.digest() })
    }

    /// Bytes can be examined only during the callback; borrowed cache storage
    /// cannot escape, survive reclamation, or become an evidence token.
    /// ```compile_fail
    /// use lc631_runtime::turn_cache::{TurnCache, TurnCacheIdentity, RecomputableArtifactKind};
    /// fn escape<'a>(cache: &'a mut TurnCache<'_, '_>, id: &TurnCacheIdentity) -> &'a [u8] {
    ///     cache.with_cached(id, RecomputableArtifactKind::ParsedStructure,
    ///         std::time::Instant::now(), |bytes| bytes.unwrap()).unwrap().value
    /// }
    /// ```
    pub fn with_cached<T>(
        &mut self,
        identity: &TurnCacheIdentity,
        kind: RecomputableArtifactKind,
        now: Instant,
        consume: impl FnOnce(Option<&[u8]>) -> T,
    ) -> Result<CacheAccess<T>, CacheError> {
        self.current.check().map_err(CacheError::Resource)?;
        let (state, obsolete) = match self.store.lookup(&identity.inner, kind, now)? {
            CacheLookup::Hit(lease) => {
                return Ok(CacheAccess {
                    state: CacheLookupState::Hit,
                    value: consume(Some(lease.bytes())),
                });
            }
            CacheLookup::Disabled => (CacheLookupState::Disabled, None),
            CacheLookup::Miss => (CacheLookupState::Miss, None),
            CacheLookup::Expired(key) => (CacheLookupState::ExpiredAndReclaimed, Some(key)),
            CacheLookup::Corrupt(key) => (CacheLookupState::CorruptAndReclaimed, Some(key)),
        };
        if let Some(key) = obsolete {
            self.store.evict(key)?;
        }
        self.store.reclaim_expired(now)?;
        Ok(CacheAccess {
            state,
            value: consume(None),
        })
    }
}

/// An appropriate turn boundary is explicit: one bounded application operation,
/// not an invented host callback or a timer thread. All cache-owned copies and
/// claims drop BEFORE this function returns. On panic they drop during unwind;
/// no successful cleanup report is fabricated for an aborted callback.
///
/// A callback may return Result::Err and its value is preserved. This cache
/// owner neither erases post-action observations nor validates their semantics.
pub fn with_turn_cache<T>(
    policy: TurnCachePolicy,
    resources: &ResourceReservationView<'_>,
    run: impl FnOnce(&mut TurnCache<'_, '_>) -> T,
) -> Result<TurnCacheOutcome<T>, CacheError> {
    resources.check().map_err(CacheError::Resource)?;
    let store = match policy.inner.mode() {
        crate::store::cache::CacheMode::Off => CacheStore::disabled(),
        crate::store::cache::CacheMode::MemoryOnly => {
            CacheStore::new_memory(policy.inner, resources)?
        }
    };
    let mut cache = TurnCache {
        store,
        current: resources,
    };
    let value = run(&mut cache);
    let stats = cache.store.stats();
    drop(cache);
    Ok(TurnCacheOutcome {
        value,
        reclaimed_entries_at_end: stats.entries,
        reclaimed_payload_bytes_at_end: stats.payload_bytes,
        disk_cache_enabled: false,
    })
}

#[cfg(test)]
mod corruption {
    use super::*;
    use crate::resource::{
        Cancellation, Deadline, ResourceAmount, ResourceBroker, ResourceKind, ResourceLimits,
        ResourceRequest,
    };

    #[test]
    fn tlcx06_corrupt_turn_copy_is_detected_and_reclaimed_before_recompute() {
        let source = Digest32::from_bytes([1; 32]);
        let d = Digest32::from_bytes([2; 32]);
        let base = CacheIdentity::checked(source, d, d, d, d, d, d, d, d, d, d).unwrap();
        let identity = TurnCacheIdentity::checked(
            base,
            MeasurementCacheRevisions {
                target: d,
                baseline: d,
                corpus: d,
                oracle: d,
                policy: d,
                evidence: d,
                definition: d,
                encoding: d,
            },
        )
        .unwrap();
        let broker = ResourceBroker::new(ResourceLimits::uniform(1024 * 1024).unwrap()).unwrap();
        let run = broker
            .begin_run(
                1,
                source,
                ResourceLimits::uniform(1024 * 1024).unwrap(),
                Deadline::after(Duration::from_secs(60)).unwrap(),
                Cancellation::default(),
            )
            .unwrap();
        let mut reservation = run
            .reserve(&[ResourceRequest::new(
                ResourceKind::Ram,
                ResourceAmount::checked(1024 * 1024).unwrap(),
            )])
            .unwrap();
        {
            let view = reservation.lease();
            let policy = TurnCachePolicy::memory(1, 64, Duration::from_secs(10)).unwrap();
            let outcome = with_turn_cache(policy, &view, |cache| {
                let now = Instant::now();
                let key = cache
                    .store
                    .insert_copy(
                        identity.inner,
                        RecomputableArtifactKind::ParsedStructure,
                        b"ast",
                        now,
                    )
                    .unwrap();
                cache.store.corrupt_payload_for_test(key).unwrap();
                let access = cache
                    .with_cached(
                        &identity,
                        RecomputableArtifactKind::ParsedStructure,
                        now,
                        |bytes| bytes.is_none(),
                    )
                    .unwrap();
                assert_eq!(access.state, CacheLookupState::CorruptAndReclaimed);
                assert!(access.value);
                let next = cache
                    .with_cached(
                        &identity,
                        RecomputableArtifactKind::ParsedStructure,
                        now,
                        |bytes| bytes.is_none(),
                    )
                    .unwrap();
                assert_eq!(next.state, CacheLookupState::Miss);
                assert!(next.value);
            })
            .unwrap();
            assert_eq!(outcome.reclaimed_entries_at_end, 0);
        }
        reservation.release_confirmed().unwrap();
        assert_eq!(broker.used(ResourceKind::Ram).unwrap(), 0);
    }
}
