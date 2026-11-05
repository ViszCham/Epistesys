//! Bounded regression over the actual RAM cache and process-root broker.
//! Logical reservation/byte observations are not measured RSS or OS enforcement.
#![forbid(unsafe_code)]

use lc631_runtime::resource::{
    Cancellation, Deadline, Digest32, ResourceAmount, ResourceBroker, ResourceError, ResourceKind,
    ResourceLimits, ResourceRequest,
};
use std::time::{Duration, Instant};

// Same private-cache integration seam as TLCX06; no substitute cache implementation.
mod resource {
    pub use lc631_runtime::resource::*;
}
#[allow(dead_code)]
#[path = "../src/store/cache.rs"]
mod cache;
use cache::{
    CacheError, CacheIdentity, CacheLookup, CachePolicy, CacheStore, Eviction,
    RecomputableArtifactKind, RecomputablePayload,
};

const QUOTA: u64 = 65_536;
const TURNS: u64 = 1_000;
const PAYLOAD: &[u8] = b"source-bound-plan-v1";

fn digest(value: u8) -> Digest32 {
    Digest32::from_bytes([value; 32])
}
fn request(kind: ResourceKind, amount: u64) -> ResourceRequest {
    ResourceRequest::new(kind, ResourceAmount::checked(amount).unwrap())
}
fn identity(context: u8) -> CacheIdentity {
    CacheIdentity::checked(
        digest(1),
        digest(context),
        digest(3),
        digest(4),
        digest(5),
        digest(6),
        digest(7),
        digest(8),
        digest(9),
        digest(10),
        digest(11),
    )
    .unwrap()
}
fn begin(
    owner: &ResourceBroker,
    id: u64,
    cancellation: Cancellation,
) -> lc631_runtime::resource::RunResources {
    owner
        .begin_run(
            id,
            digest(1),
            ResourceLimits::uniform(QUOTA).unwrap(),
            Deadline::after(Duration::from_secs(600)).unwrap(),
            cancellation,
        )
        .unwrap()
}
fn hit_bytes(store: &CacheStore<'_, '_>, key: &CacheIdentity, now: Instant) {
    match store
        .lookup(key, RecomputableArtifactKind::ProgramLowering, now)
        .unwrap()
    {
        CacheLookup::Hit(lease) => assert_eq!(lease.bytes(), PAYLOAD),
        other => panic!("expected exact cache bytes, observed {other:?}"),
    }
}

#[test]
fn tlcx37_thousand_turn_cache_lifecycle_reclaims_claims_and_reservations() {
    let owner = ResourceBroker::new(ResourceLimits::uniform(QUOTA).unwrap()).unwrap();
    let metadata_before = owner.metadata_backing_bytes();
    let key = identity(2);
    let kind = RecomputableArtifactKind::ProgramLowering;
    let mut completed = 0;
    for turn in 1..=TURNS {
        let mut run = begin(&owner, turn, Cancellation::default());
        let mut reservation = run.reserve(&[request(ResourceKind::Ram, QUOTA)]).unwrap();
        assert_eq!(owner.used(ResourceKind::Ram).unwrap(), QUOTA);
        {
            let view = reservation.lease();
            let now = Instant::now();
            let mut store = CacheStore::new_memory(
                CachePolicy::memory(1, 128, Duration::from_secs(2)).unwrap(),
                &view,
            )
            .unwrap();
            assert_eq!(store.lookup(&key, kind, now).unwrap(), CacheLookup::Miss);
            let payload = RecomputablePayload::fixture(key, kind, PAYLOAD).unwrap();
            let first = store.insert(&payload, now).unwrap();
            hit_bytes(&store, &key, now);
            assert_eq!(
                store.lookup(&identity(99), kind, now).unwrap(),
                CacheLookup::Miss
            );
            assert_eq!(store.stats().entries, 1);
            assert_eq!(store.stats().payload_bytes, PAYLOAD.len() as u64);
            assert!(!store.stats().disk_cache_enabled);
            assert!(matches!(
                view.claim_resources(&[request(ResourceKind::Ram, QUOTA)]),
                Err(ResourceError::Exhausted(ResourceKind::Ram))
            ));
            store.corrupt_payload_for_test(first).unwrap();
            assert_eq!(
                store.lookup(&key, kind, now).unwrap(),
                CacheLookup::Corrupt(first)
            );
            assert_eq!(store.evict(first).unwrap(), Eviction::Removed);
            assert_eq!(store.stats().payload_bytes, 0);
            store.insert(&payload, now).unwrap();
            hit_bytes(&store, &key, now);
            let expired = now.checked_add(Duration::from_secs(2)).unwrap();
            assert!(matches!(
                store.lookup(&key, kind, expired).unwrap(),
                CacheLookup::Expired(_)
            ));
            // Admission must reclaim the expired slot; a one-slot store cannot
            // accept the next payload if the production reclaim hook is cut.
            store.insert(&payload, expired).unwrap();
            hit_bytes(&store, &key, expired);
            assert_eq!(store.stats().entries, 1);
            drop(store);
            // A full-quota claim now succeeds only if cache metadata and payload
            // claims actually released their shared reservation usage.
            let reclaimed = view
                .claim_resources(&[request(ResourceKind::Ram, QUOTA)])
                .unwrap();
            drop(reclaimed);
        }
        reservation.release_confirmed().unwrap();
        assert_eq!(owner.used(ResourceKind::Ram).unwrap(), 0);
        run.close_confirmed().unwrap();
        assert_eq!(owner.metadata_backing_bytes(), metadata_before);
        completed += 1;
    }
    assert_eq!(completed, TURNS);
    assert_eq!(owner.used(ResourceKind::Ram).unwrap(), 0);
}

#[test]
fn tlcx37_reservation_slots_reused_beyond_history_capacity_without_quota_drift() {
    let owner = ResourceBroker::new(ResourceLimits::uniform(QUOTA).unwrap()).unwrap();
    let mut run = begin(&owner, 1, Cancellation::default());
    for _ in 0..5_000 {
        let mut reservation = run.reserve(&[request(ResourceKind::Ram, 1)]).unwrap();
        assert_eq!(owner.used(ResourceKind::Ram).unwrap(), 1);
        reservation.release_confirmed().unwrap();
        assert_eq!(owner.used(ResourceKind::Ram).unwrap(), 0);
    }
    run.close_confirmed().unwrap();
}

#[test]
fn tlcx37_cancelled_cache_rejects_work_but_confirmed_drop_can_refund_ram() {
    let owner = ResourceBroker::new(ResourceLimits::uniform(QUOTA).unwrap()).unwrap();
    let cancel = Cancellation::default();
    let mut run = begin(&owner, 1, cancel.clone());
    let mut reservation = run.reserve(&[request(ResourceKind::Ram, QUOTA)]).unwrap();
    {
        let view = reservation.lease();
        let mut store = CacheStore::new_memory(
            CachePolicy::memory(1, 128, Duration::from_secs(2)).unwrap(),
            &view,
        )
        .unwrap();
        let key = identity(2);
        let now = Instant::now();
        let payload =
            RecomputablePayload::fixture(key, RecomputableArtifactKind::ProgramLowering, PAYLOAD)
                .unwrap();
        store.insert(&payload, now).unwrap();
        cancel.cancel();
        assert_eq!(
            store.lookup(&key, RecomputableArtifactKind::ProgramLowering, now),
            Err(CacheError::Resource(ResourceError::Cancelled))
        );
        assert_eq!(
            store.reclaim_expired(now),
            Err(CacheError::Resource(ResourceError::Cancelled))
        );
        drop(store);
    }
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
    assert_eq!(owner.used(ResourceKind::Ram).unwrap(), 0);
}

#[test]
fn tlcx37_unknown_cleanup_is_retained_and_cannot_be_counted_as_reclaim() {
    let owner = ResourceBroker::new(ResourceLimits::uniform(QUOTA).unwrap()).unwrap();
    let mut run = begin(&owner, 1, Cancellation::default());
    let mut reservation = run
        .reserve(&[
            request(ResourceKind::Ram, 17),
            request(ResourceKind::Child, 1),
            request(ResourceKind::Cleanup, 1),
        ])
        .unwrap();
    reservation.lease().record_cleanup_unknown();
    assert_eq!(
        reservation.release_confirmed(),
        Err(ResourceError::CleanupUnverified)
    );
    drop(reservation);
    assert_eq!(owner.used(ResourceKind::Ram).unwrap(), 17);
    assert_eq!(owner.used(ResourceKind::Child).unwrap(), 1);
    assert_eq!(run.close_confirmed(), Err(ResourceError::CleanupUnverified));
}
