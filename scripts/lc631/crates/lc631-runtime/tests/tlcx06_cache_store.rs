#![allow(clippy::drop_non_drop)] // End borrowed resource-view scopes before explicit lease release.

use lc631_receipt_kernel::{
    digest_text, ReceiptClass, ReceiptError, ReceiptIssuer, ReceiptPolicy, ReceiptScope,
    ReceiptVerifier, ReplayGuard, SubjectRevision,
};
use lc631_runtime::resource::{
    Cancellation, Deadline, Digest32, ResourceAmount, ResourceBroker, ResourceKind, ResourceLimits,
    ResourceRequest,
};
use std::time::{Duration, Instant};

// Compile the new store module in this integration-test crate without editing
// the shared runtime root module; it will use the same public runtime types.
mod resource {
    pub use lc631_runtime::resource::*;
}

#[path = "../src/store/cache.rs"]
mod cache;
#[path = "../src/store/recovery.rs"]
mod recovery;

use cache::{
    CacheError, CacheIdentity, CacheLookup, CachePolicy, CacheStore, RecomputableArtifactKind,
    RecomputablePayload,
};
use recovery::{CacheRecoveryDecision, CacheRecoveryPolicy};

fn digest(byte: u8) -> Digest32 {
    Digest32::from_bytes([byte; 32])
}

fn revisions(source: Digest32) -> [Digest32; 11] {
    [
        source,
        digest(2),
        digest(3),
        digest(4),
        digest(5),
        digest(6),
        digest(7),
        digest(8),
        digest(9),
        digest(10),
        digest(11),
    ]
}

fn identity(values: [Digest32; 11]) -> Result<CacheIdentity, CacheError> {
    let [source, context, goal, probes, logic, solver, options, model, tool, validator, codec] =
        values;
    CacheIdentity::checked(
        source, context, goal, probes, logic, solver, options, model, tool, validator, codec,
    )
}

fn resource_owner(
    source: Digest32,
) -> (
    ResourceBroker,
    lc631_runtime::resource::RunResources,
    lc631_runtime::resource::Reservation,
) {
    let owner = ResourceBroker::new(ResourceLimits::uniform(1 << 20).unwrap()).unwrap();
    let run = owner
        .begin_run(
            1,
            source,
            ResourceLimits::uniform(1 << 20).unwrap(),
            Deadline::after(Duration::from_secs(60)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let reservation = run
        .reserve(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(1 << 20).unwrap(),
        )])
        .unwrap();
    (owner, run, reservation)
}

#[test]
fn tlcx06_cache_is_off_by_default_and_disk_requires_an_owned_manifest() {
    assert_eq!(CachePolicy::default().mode(), cache::CacheMode::Off);
    assert_eq!(
        CachePolicy::request_disk_cache(8, 4096, Duration::from_secs(60), None),
        Err(CacheError::OwnerManifestRequired)
    );
    let cache = CacheStore::disabled();
    let key = identity(revisions(digest(1))).unwrap();
    assert_eq!(
        cache
            .lookup(
                &key,
                RecomputableArtifactKind::ParsedStructure,
                Instant::now()
            )
            .unwrap(),
        CacheLookup::Disabled
    );
}

#[test]
fn tlcx06_identity_requires_all_revision_components_and_rejects_zero() {
    let mut values = revisions(digest(1));
    values[5] = digest(0);
    assert_eq!(identity(values), Err(CacheError::InvalidIdentity));
}

#[test]
fn tlcx06_cache_miss_is_bound_to_every_identity_revision() {
    let source = digest(1);
    let (_owner, _run, mut reservation) = resource_owner(source);
    let view = reservation.lease();
    let policy = CachePolicy::memory(16, 4096, Duration::from_secs(60)).unwrap();
    let mut cache = CacheStore::new_memory(policy, &view).unwrap();
    assert_eq!(cache.stats().mode, cache::CacheMode::MemoryOnly);
    assert!(!cache.stats().disk_cache_enabled);
    let values = revisions(source);
    let key = identity(values).unwrap();
    let now = Instant::now();
    let payload = RecomputablePayload::fixture(
        key,
        RecomputableArtifactKind::ProgramLowering,
        b"typed-plan",
    )
    .unwrap();
    let entry_key = cache.insert(&payload, now).unwrap();

    match cache
        .lookup(
            &identity(values).unwrap(),
            RecomputableArtifactKind::ProgramLowering,
            now,
        )
        .unwrap()
    {
        CacheLookup::Hit(lease) => {
            assert_eq!(lease.key(), entry_key);
            assert_eq!(lease.key().digest(), entry_key.digest());
            assert_eq!(lease.kind(), RecomputableArtifactKind::ProgramLowering);
            assert_eq!(lease.bytes(), b"typed-plan");
        }
        other => panic!("expected a verified-by-digest recomputable cache hit, got {other:?}"),
    }

    for index in 1..values.len() {
        let mut changed = values;
        changed[index] = digest(100 + u8::try_from(index).unwrap());
        assert_eq!(
            cache
                .lookup(
                    &identity(changed).unwrap(),
                    RecomputableArtifactKind::ProgramLowering,
                    now,
                )
                .unwrap(),
            CacheLookup::Miss,
            "revision field {index} must be part of cache identity"
        );
    }
    let mut changed_source = values;
    changed_source[0] = digest(100);
    assert_eq!(
        cache.lookup(
            &identity(changed_source).unwrap(),
            RecomputableArtifactKind::ProgramLowering,
            now
        ),
        Err(CacheError::SourceMismatch)
    );
    drop(cache);
    drop(view);
    reservation.release_confirmed().unwrap();
}

#[test]
fn tlcx06_artifact_kind_is_domain_separated_in_cache_keys() {
    let source = digest(1);
    let (_owner, _run, mut reservation) = resource_owner(source);
    let view = reservation.lease();
    let policy = CachePolicy::memory(4, 4096, Duration::from_secs(30)).unwrap();
    let mut cache = CacheStore::new_memory(policy, &view).unwrap();
    let identity = identity(revisions(source)).unwrap();
    let now = Instant::now();

    assert_eq!(
        cache
            .lookup(&identity, RecomputableArtifactKind::ParsedStructure, now)
            .unwrap(),
        CacheLookup::Miss
    );

    let parsed = RecomputablePayload::fixture(
        identity,
        RecomputableArtifactKind::ParsedStructure,
        b"syntax-tree",
    )
    .unwrap();
    let parsed_key = cache.insert(&parsed, now).unwrap();
    assert_eq!(
        cache
            .lookup(&identity, RecomputableArtifactKind::ProgramLowering, now)
            .unwrap(),
        CacheLookup::Miss,
        "a parsed structure must not alias a program-lowering entry"
    );

    let lowering = RecomputablePayload::fixture(
        identity,
        RecomputableArtifactKind::ProgramLowering,
        b"program-ir",
    )
    .unwrap();
    let lowering_key = cache.insert(&lowering, now).unwrap();
    let numeric = RecomputablePayload::fixture(
        identity,
        RecomputableArtifactKind::DerivedNumericValue,
        b"42",
    )
    .unwrap();
    let numeric_key = cache.insert(&numeric, now).unwrap();
    assert_ne!(parsed_key, lowering_key);
    assert_ne!(parsed_key, numeric_key);
    assert_ne!(lowering_key, numeric_key);

    for (kind, expected_key, expected_bytes) in [
        (
            RecomputableArtifactKind::ParsedStructure,
            parsed_key,
            b"syntax-tree".as_slice(),
        ),
        (
            RecomputableArtifactKind::ProgramLowering,
            lowering_key,
            b"program-ir".as_slice(),
        ),
        (
            RecomputableArtifactKind::DerivedNumericValue,
            numeric_key,
            b"42".as_slice(),
        ),
    ] {
        match cache.lookup(&identity, kind, now).unwrap() {
            CacheLookup::Hit(lease) => {
                assert_eq!(lease.key(), expected_key);
                assert_eq!(lease.kind(), kind);
                assert_eq!(lease.bytes(), expected_bytes);
            }
            other => panic!("expected distinct typed cache hit, got {other:?}"),
        }
    }

    drop(cache);
    drop(view);
    reservation.release_confirmed().unwrap();
}

#[test]
fn tlcx06_expiry_is_monotonic_typed_and_reclaims_only_owned_memory() {
    let source = digest(1);
    let (owner, _run, mut reservation) = resource_owner(source);
    let view = reservation.lease();
    let policy = CachePolicy::memory(4, 1024, Duration::from_secs(10)).unwrap();
    let mut cache = CacheStore::new_memory(policy, &view).unwrap();
    let identity = identity(revisions(source)).unwrap();
    let start = Instant::now();
    let payload =
        RecomputablePayload::fixture(identity, RecomputableArtifactKind::ParsedStructure, b"ast")
            .unwrap();
    let key = cache.insert(&payload, start).unwrap();
    assert!(owner.used(ResourceKind::Ram).unwrap() > 0);
    assert_eq!(cache.stats().payload_bytes, 3);

    assert!(matches!(
        cache
            .lookup(
                &identity,
                RecomputableArtifactKind::ParsedStructure,
                start + Duration::from_secs(10)
            )
            .unwrap(),
        CacheLookup::Expired(expired) if expired == key
    ));
    assert_eq!(
        cache
            .reclaim_expired(start + Duration::from_secs(10))
            .unwrap(),
        1
    );
    assert_eq!(
        cache
            .lookup(
                &identity,
                RecomputableArtifactKind::ParsedStructure,
                start + Duration::from_secs(11)
            )
            .unwrap(),
        CacheLookup::Miss
    );
    drop(cache);
    drop(view);
    reservation.release_confirmed().unwrap();
    assert_eq!(owner.used(ResourceKind::Ram).unwrap(), 0);
}

#[test]
fn tlcx06_insert_reclaims_expired_entry_before_capacity_admission() {
    let source = digest(1);
    let (_owner, _run, mut reservation) = resource_owner(source);
    let view = reservation.lease();
    let policy = CachePolicy::memory(1, 128, Duration::from_secs(5)).unwrap();
    let mut cache = CacheStore::new_memory(policy, &view).unwrap();
    let identity = identity(revisions(source)).unwrap();
    let first_time = Instant::now();
    let first = RecomputablePayload::fixture(
        identity,
        RecomputableArtifactKind::ParsedStructure,
        b"first",
    )
    .unwrap();
    cache.insert(&first, first_time).unwrap();

    let second = RecomputablePayload::fixture(
        identity,
        RecomputableArtifactKind::ParsedStructure,
        b"second",
    )
    .unwrap();
    let second_time = first_time + Duration::from_secs(6);
    cache.insert(&second, second_time).unwrap();

    assert_eq!(cache.stats().entries, 1);
    match cache
        .lookup(
            &identity,
            RecomputableArtifactKind::ParsedStructure,
            second_time,
        )
        .unwrap()
    {
        CacheLookup::Hit(lease) => assert_eq!(lease.bytes(), b"second"),
        other => panic!("new entry should replace only the expired cache item: {other:?}"),
    }
    drop(cache);
    drop(view);
    reservation.release_confirmed().unwrap();
}

#[test]
fn tlcx06_cold_warm_evicted_and_corrupt_artifact_bytes_are_reproducible() {
    // This is a cache-contract test for deterministic artifact bytes, not a
    // claim that a production solver or semantic-result path consumes the cache.
    let source = digest(1);
    let (_owner, _run, mut reservation) = resource_owner(source);
    let view = reservation.lease();
    let policy = CachePolicy::memory(2, 256, Duration::from_secs(30)).unwrap();
    let mut cache = CacheStore::new_memory(policy, &view).unwrap();
    let identity = identity(revisions(source)).unwrap();
    let kind = RecomputableArtifactKind::ProgramLowering;
    let now = Instant::now();
    let recompute = || b"stable-program-lowering-v1".to_vec();

    assert_eq!(
        cache.lookup(&identity, kind, now).unwrap(),
        CacheLookup::Miss
    );
    let cold_bytes = recompute();
    let cold = RecomputablePayload::fixture(identity, kind, &cold_bytes).unwrap();
    let key = cache.insert(&cold, now).unwrap();

    let warm_bytes = match cache.lookup(&identity, kind, now).unwrap() {
        CacheLookup::Hit(lease) => lease.bytes().to_vec(),
        other => panic!("expected warm artifact hit, got {other:?}"),
    };
    assert_eq!(warm_bytes, cold_bytes);

    assert_eq!(cache.evict(key).unwrap(), cache::Eviction::Removed);
    let evicted_recompute = recompute();
    assert_eq!(evicted_recompute, cold_bytes);
    let regenerated = RecomputablePayload::fixture(identity, kind, &evicted_recompute).unwrap();
    let regenerated_key = cache.insert(&regenerated, now).unwrap();

    cache.corrupt_payload_for_test(regenerated_key).unwrap();
    assert_eq!(
        cache.lookup(&identity, kind, now).unwrap(),
        CacheLookup::Corrupt(regenerated_key)
    );
    assert_eq!(
        cache.evict(regenerated_key).unwrap(),
        cache::Eviction::Removed
    );
    let repaired = recompute();
    assert_eq!(repaired, cold_bytes);

    drop(cache);
    drop(view);
    reservation.release_confirmed().unwrap();
}

#[test]
fn tlcx06_borrowed_cache_lease_keeps_payload_until_explicit_release() {
    let source = digest(1);
    let (owner, _run, mut reservation) = resource_owner(source);
    let view = reservation.lease();
    let policy = CachePolicy::memory(2, 64, Duration::from_secs(30)).unwrap();
    let mut cache = CacheStore::new_memory(policy, &view).unwrap();
    let identity = identity(revisions(source)).unwrap();
    let now = Instant::now();
    let payload = RecomputablePayload::fixture(
        identity,
        RecomputableArtifactKind::SolverQueryPlan,
        b"query",
    )
    .unwrap();
    let key = cache.insert(&payload, now).unwrap();
    let lease = match cache
        .lookup(&identity, RecomputableArtifactKind::SolverQueryPlan, now)
        .unwrap()
    {
        CacheLookup::Hit(lease) => lease,
        other => panic!("expected borrowed cache lease, got {other:?}"),
    };
    assert_eq!(lease.key(), key);
    assert_eq!(lease.bytes(), b"query");
    assert!(lease
        .claim_boundary()
        .contains("no evidence, authority, or commit"));
    assert_eq!(cache.stats().payload_bytes, 5);
    // `lease` immutably borrows `cache`; Rust rejects `cache.evict(key)` here.
    drop(lease);
    assert_eq!(cache.evict(key).unwrap(), cache::Eviction::Removed);
    assert_eq!(cache.stats().payload_bytes, 0);

    let replacement = RecomputablePayload::fixture(
        identity,
        RecomputableArtifactKind::SolverQueryPlan,
        b"query",
    )
    .unwrap();
    cache.insert(&replacement, now).unwrap();
    drop(cache);
    let fresh_store = CacheStore::new_memory(policy, &view).unwrap();
    assert_eq!(
        fresh_store
            .lookup(&identity, RecomputableArtifactKind::SolverQueryPlan, now)
            .unwrap(),
        CacheLookup::Miss,
        "a new in-memory store cannot restore a previous cache entry"
    );
    drop(fresh_store);
    drop(view);
    reservation.release_confirmed().unwrap();
    assert_eq!(owner.used(ResourceKind::Ram).unwrap(), 0);
}

#[test]
fn tlcx06_crash_recovery_is_cold_and_a_new_owner_cannot_rehydrate_cache_hits() {
    assert_eq!(
        CacheRecoveryPolicy::after_process_restart(true),
        CacheRecoveryDecision::RecomputeFromCurrentSource
    );
    assert_eq!(
        CacheRecoveryPolicy::after_process_restart(false),
        CacheRecoveryDecision::HoldForCurrentSource
    );
    assert!(!CacheRecoveryPolicy::prior_cache_hit_is_recoverable());

    let source = digest(1);
    let identity = identity(revisions(source)).unwrap();
    let policy = CachePolicy::memory(2, 128, Duration::from_secs(30)).unwrap();
    let first_time = Instant::now();

    let (_first_owner, _first_run, mut first_reservation) = resource_owner(source);
    let first_view = first_reservation.lease();
    let mut first_store = CacheStore::new_memory(policy, &first_view).unwrap();
    let payload = RecomputablePayload::fixture(
        identity,
        RecomputableArtifactKind::ParsedStructure,
        b"volatile-only",
    )
    .unwrap();
    first_store.insert(&payload, first_time).unwrap();
    assert!(matches!(
        first_store
            .lookup(
                &identity,
                RecomputableArtifactKind::ParsedStructure,
                first_time
            )
            .unwrap(),
        CacheLookup::Hit(_)
    ));
    drop(first_store);
    drop(first_view);
    first_reservation.release_confirmed().unwrap();

    // A fresh resource owner models a new runtime/process-owned cache. No
    // prior cache object or disk manifest is accepted by the recovery policy.
    let (_next_owner, _next_run, mut next_reservation) = resource_owner(source);
    let next_view = next_reservation.lease();
    let next_store = CacheStore::new_memory(policy, &next_view).unwrap();
    assert_eq!(
        next_store
            .lookup(
                &identity,
                RecomputableArtifactKind::ParsedStructure,
                Instant::now()
            )
            .unwrap(),
        CacheLookup::Miss
    );
    drop(next_store);
    drop(next_view);
    next_reservation.release_confirmed().unwrap();
}

#[test]
fn tlcx06_capacity_and_resource_admission_fail_without_partial_publication() {
    let source = digest(1);
    let (_owner, _run, mut reservation) = resource_owner(source);
    let view = reservation.lease();
    let policy = CachePolicy::memory(1, 4, Duration::from_secs(30)).unwrap();
    let mut cache = CacheStore::new_memory(policy, &view).unwrap();
    let key = identity(revisions(source)).unwrap();
    let oversized =
        RecomputablePayload::fixture(key, RecomputableArtifactKind::ParsedStructure, b"12345");
    match oversized {
        Ok(oversized) => assert_eq!(
            cache.insert(&oversized, Instant::now()),
            Err(CacheError::EntryTooLarge)
        ),
        Err(error) => assert_eq!(error, CacheError::EntryTooLarge),
    }
    assert_eq!(cache.stats().entries, 0);
    assert_eq!(cache.stats().payload_bytes, 0);

    let large_policy = CachePolicy::memory(1, 4096, Duration::from_secs(30)).unwrap();
    drop(cache);
    drop(view);
    reservation.release_confirmed().unwrap();

    let small_owner = ResourceBroker::new(ResourceLimits::uniform(1).unwrap()).unwrap();
    let insufficient = small_owner
        .begin_run(
            2,
            source,
            ResourceLimits::uniform(1).unwrap(),
            Deadline::after(Duration::from_secs(60)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let mut small = insufficient
        .reserve(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(1).unwrap(),
        )])
        .unwrap();
    let small_view = small.lease();
    match CacheStore::new_memory(large_policy, &small_view) {
        Err(error) => assert_eq!(
            error,
            CacheError::Resource(lc631_runtime::resource::ResourceError::Exhausted(
                ResourceKind::Ram
            ))
        ),
        Ok(store) => {
            drop(store);
            panic!("underfunded cache metadata reservation was admitted")
        }
    }
    drop(small_view);
    small.release_confirmed().unwrap();
}

#[test]
fn tlcx06_cache_eviction_never_evicts_security_replay_history() {
    let source = digest(1);
    let (_owner, _run, mut reservation) = resource_owner(source);
    let view = reservation.lease();
    let policy = CachePolicy::memory(1, 128, Duration::from_secs(30)).unwrap();
    let mut cache = CacheStore::new_memory(policy, &view).unwrap();
    let identity = identity(revisions(source)).unwrap();
    let payload = RecomputablePayload::fixture(
        identity,
        RecomputableArtifactKind::ProgramLowering,
        b"program",
    )
    .unwrap();
    let key = cache.insert(&payload, Instant::now()).unwrap();

    let signing_key = [0x61_u8; 32];
    let issuer = ReceiptIssuer::from_key_bytes("tlcx06-cache-replay", &signing_key).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("tlcx06-cache-replay", &signing_key).unwrap();
    let subject = SubjectRevision::checked("tlcx06-current-source").unwrap();
    let scope = ReceiptScope::checked("tlcx06/cache-replay-separation").unwrap();
    let digest = digest_text("receipt-remains-independent-from-cache");
    let receipt = issuer
        .issue(
            ReceiptClass::Validation,
            subject.clone(),
            scope.clone(),
            digest.clone(),
            1,
            Some(100),
            None,
        )
        .unwrap();
    let duplicate = receipt.clone();
    let receipt_policy = ReceiptPolicy::exact(ReceiptClass::Validation, subject, scope, digest, 2);
    let mut replay = ReplayGuard::with_capacity(1).unwrap();

    verifier
        .verify(receipt, &receipt_policy, &mut replay)
        .unwrap();
    assert_eq!(replay.observed_count(), 1);
    assert_eq!(cache.evict(key).unwrap(), cache::Eviction::Removed);
    assert_eq!(replay.observed_count(), 1);
    assert_eq!(
        verifier.verify(duplicate, &receipt_policy, &mut replay),
        Err(ReceiptError::ReplayDetected)
    );

    drop(cache);
    reservation.release_confirmed().unwrap();
}
