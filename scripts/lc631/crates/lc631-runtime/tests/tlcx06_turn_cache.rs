//! Actual SDK-turn cache ownership tests; no semantic/evidence promotion.
use lc631_runtime::resource::{
    Cancellation, Deadline, ResourceAmount, ResourceBroker, ResourceKind, ResourceLimits,
    ResourceRequest,
};
use lc631_runtime::turn_cache::{
    with_turn_cache, CacheError, CacheIdentity, CacheLookupState, MeasurementCacheRevisions,
    RecomputableArtifactKind as Kind, RememberOutcome, TurnCacheIdentity, TurnCachePolicy,
};
use lc631_substrate::Digest32;
use std::time::{Duration, Instant};

fn id(source: Digest32) -> TurnCacheIdentity {
    let d = Digest32::from_bytes([2; 32]);
    let base = CacheIdentity::checked(source, d, d, d, d, d, d, d, d, d, d).unwrap();
    TurnCacheIdentity::checked(base, revisions()).unwrap()
}

fn revisions() -> MeasurementCacheRevisions {
    let d = Digest32::from_bytes([3; 32]);
    MeasurementCacheRevisions {
        target: d,
        baseline: d,
        corpus: d,
        oracle: d,
        policy: d,
        evidence: d,
        definition: d,
        encoding: d,
    }
}

#[test]
fn tlcx06_each_additional_measurement_revision_invalidates_a_hit() {
    fixture(|view, _| {
        let d = Digest32::from_bytes([2; 32]);
        let base =
            CacheIdentity::checked(view.source_revision(), d, d, d, d, d, d, d, d, d, d).unwrap();
        let identity = TurnCacheIdentity::checked(base, revisions()).unwrap();
        let policy = TurnCachePolicy::memory(1, 64, Duration::from_secs(10)).unwrap();
        let outcome = with_turn_cache(policy, view, |cache| {
            let now = Instant::now();
            cache
                .remember(&identity, Kind::DerivedNumericValue, b"numeric-result", now)
                .unwrap();
            for index in 0..8 {
                let mut changed = revisions();
                let different = Digest32::from_bytes([99; 32]);
                match index {
                    0 => changed.target = different,
                    1 => changed.baseline = different,
                    2 => changed.corpus = different,
                    3 => changed.oracle = different,
                    4 => changed.policy = different,
                    5 => changed.evidence = different,
                    6 => changed.definition = different,
                    7 => changed.encoding = different,
                    _ => unreachable!(),
                }
                let updated = TurnCacheIdentity::checked(base, changed).unwrap();
                let access = cache
                    .with_cached(&updated, Kind::DerivedNumericValue, now, |bytes| {
                        bytes.is_none()
                    })
                    .unwrap();
                assert_eq!(access.state, CacheLookupState::Miss, "revision {index}");
                assert!(access.value);
            }
            let mut invalid = revisions();
            invalid.evidence = Digest32::from_bytes([0; 32]);
            assert_eq!(
                TurnCacheIdentity::checked(base, invalid),
                Err(CacheError::InvalidIdentity)
            );
            let original = cache
                .with_cached(&identity, Kind::DerivedNumericValue, now, |bytes| {
                    bytes.unwrap().len()
                })
                .unwrap();
            assert_eq!(original.state, CacheLookupState::Hit);
            assert_eq!(original.value, 14);
        })
        .unwrap();
        assert_eq!(outcome.reclaimed_entries_at_end, 1);
    });
}

#[test]
fn tlcx37_thousand_scoped_turns_are_cold_and_release_every_claim() {
    fixture(|view, _| {
        let identity = id(view.source_revision());
        let policy = TurnCachePolicy::memory(1, 64, Duration::from_secs(10)).unwrap();
        for _ in 0..1000 {
            let outcome = with_turn_cache(policy, view, |cache| {
                let now = Instant::now();
                let before = cache
                    .with_cached(&identity, Kind::ParsedStructure, now, |bytes| {
                        bytes.is_none()
                    })
                    .unwrap();
                assert_eq!(before.state, CacheLookupState::Miss);
                assert!(before.value);
                cache
                    .remember(&identity, Kind::ParsedStructure, b"ast", now)
                    .unwrap();
                let after = cache
                    .with_cached(&identity, Kind::ParsedStructure, now, |bytes| {
                        bytes == Some(b"ast".as_slice())
                    })
                    .unwrap();
                assert_eq!(after.state, CacheLookupState::Hit);
                assert!(after.value);
            })
            .unwrap();
            assert_eq!(outcome.reclaimed_entries_at_end, 1);
            assert_eq!(outcome.reclaimed_payload_bytes_at_end, 3);
            // Independently consume the whole reservation after the owner has
            // dropped: any leaked metadata/payload claim makes this fail.
            drop(view.claim_buffer(ResourceKind::Ram).unwrap());
        }
    });
}

fn fixture(
    work: impl FnOnce(&lc631_substrate::resource::ResourceReservationView<'_>, &Cancellation),
) {
    let source = Digest32::from_bytes([1; 32]);
    let owner = ResourceBroker::new(ResourceLimits::uniform(1024 * 1024).unwrap()).unwrap();
    let cancel = Cancellation::default();
    let run = owner
        .begin_run(
            1,
            source,
            ResourceLimits::uniform(1024 * 1024).unwrap(),
            Deadline::after(Duration::from_secs(60)).unwrap(),
            cancel.clone(),
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
        work(&view, &cancel);
    }
    reservation.release_confirmed().unwrap();
    assert_eq!(owner.used(ResourceKind::Ram).unwrap(), 0);
}

#[test]
fn tlcx06_default_turn_cache_never_copies_or_persists() {
    fixture(|view, _| {
        let identity = id(view.source_revision());
        let outcome = with_turn_cache(TurnCachePolicy::default(), view, |cache| {
            let now = Instant::now();
            assert_eq!(
                cache
                    .remember(&identity, Kind::ParsedStructure, b"payload", now)
                    .unwrap(),
                RememberOutcome::Disabled
            );
            let read = cache
                .with_cached(&identity, Kind::ParsedStructure, now, |bytes| {
                    assert!(bytes.is_none());
                })
                .unwrap();
            assert_eq!(read.state, CacheLookupState::Disabled);
        })
        .unwrap();
        assert_eq!(outcome.reclaimed_entries_at_end, 0);
        assert_eq!(outcome.reclaimed_payload_bytes_at_end, 0);
        assert!(!outcome.disk_cache_enabled);
    });
}

#[test]
fn tlcx06_expiry_lookup_reclaims_without_waiting_for_another_insert() {
    fixture(|view, _| {
        let identity = id(view.source_revision());
        let policy = TurnCachePolicy::memory(1, 64, Duration::from_secs(5)).unwrap();
        let outcome = with_turn_cache(policy, view, |cache| {
            let start = Instant::now();
            cache
                .remember(&identity, Kind::ParsedStructure, b"ast", start)
                .unwrap();
            let expiry = start + Duration::from_secs(5);
            let expired = cache
                .with_cached(&identity, Kind::ParsedStructure, expiry, |bytes| {
                    assert!(bytes.is_none());
                })
                .unwrap();
            assert_eq!(expired.state, CacheLookupState::ExpiredAndReclaimed);
            let miss = cache
                .with_cached(&identity, Kind::ParsedStructure, expiry, |bytes| {
                    bytes.is_none()
                })
                .unwrap();
            assert_eq!(miss.state, CacheLookupState::Miss);
            assert!(miss.value);
        })
        .unwrap();
        assert_eq!(outcome.reclaimed_entries_at_end, 0);
        assert_eq!(outcome.reclaimed_payload_bytes_at_end, 0);
    });
}

#[test]
fn tlcx06_next_turn_is_cold_and_callback_error_is_preserved() {
    fixture(|view, _| {
        let identity = id(view.source_revision());
        let policy = TurnCachePolicy::memory(2, 64, Duration::from_secs(5)).unwrap();
        let first = with_turn_cache(policy, view, |cache| {
            cache
                .remember(&identity, Kind::ParsedStructure, b"ast", Instant::now())
                .unwrap();
            Err::<(), _>("application observation remains an error")
        })
        .unwrap();
        assert_eq!(first.value, Err("application observation remains an error"));
        assert_eq!(first.reclaimed_entries_at_end, 1);
        let next = with_turn_cache(policy, view, |cache| {
            cache
                .with_cached(&identity, Kind::ParsedStructure, Instant::now(), |bytes| {
                    assert!(bytes.is_none());
                })
                .unwrap()
                .state
        })
        .unwrap();
        assert_eq!(next.value, CacheLookupState::Miss);
    });
}

#[test]
fn tlcx06_panic_and_cancellation_drop_only_cache_owned_copies() {
    fixture(|view, _| {
        let identity = id(view.source_revision());
        let policy = TurnCachePolicy::memory(1, 64, Duration::from_secs(5)).unwrap();
        let aborted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = with_turn_cache(policy, view, |cache| {
                cache
                    .remember(&identity, Kind::ParsedStructure, b"ast", Instant::now())
                    .unwrap();
                panic!("intentional cache-turn unwind");
            });
        }));
        assert!(aborted.is_err());
    });
    fixture(|view, cancel| {
        let identity = id(view.source_revision());
        let policy = TurnCachePolicy::memory(1, 64, Duration::from_secs(5)).unwrap();
        let outcome = with_turn_cache(policy, view, |cache| {
            cache
                .remember(&identity, Kind::ParsedStructure, b"ast", Instant::now())
                .unwrap();
            cancel.cancel();
            assert!(matches!(
                cache.remember(&identity, Kind::ParsedStructure, b"new", Instant::now()),
                Err(CacheError::Resource(_))
            ));
        })
        .unwrap();
        assert_eq!(outcome.reclaimed_entries_at_end, 1);
    });
}

#[test]
fn tlcx06_turn_cache_limits_and_foreign_source_fail_before_copy() {
    assert!(matches!(
        TurnCachePolicy::memory(0, 1, Duration::from_secs(1)),
        Err(CacheError::InvalidPolicy)
    ));
    assert!(matches!(
        TurnCachePolicy::memory(1, 0, Duration::from_secs(1)),
        Err(CacheError::InvalidPolicy)
    ));
    assert!(matches!(
        TurnCachePolicy::memory(1, 1, Duration::ZERO),
        Err(CacheError::InvalidPolicy)
    ));
    fixture(|view, _| {
        let identity = id(view.source_revision());
        let other = id(Digest32::from_bytes([99; 32]));
        let policy = TurnCachePolicy::memory(1, 3, Duration::from_secs(5)).unwrap();
        let outcome = with_turn_cache(policy, view, |cache| {
            let now = Instant::now();
            assert!(matches!(
                cache.remember(&other, Kind::ParsedStructure, b"ast", now),
                Err(CacheError::SourceMismatch)
            ));
            assert!(matches!(
                cache.remember(&identity, Kind::ParsedStructure, b"long", now),
                Err(CacheError::EntryTooLarge)
            ));
            cache
                .remember(&identity, Kind::ParsedStructure, b"ast", now)
                .unwrap();
            assert!(matches!(
                cache.with_cached(
                    &identity,
                    Kind::ParsedStructure,
                    now - Duration::from_secs(1),
                    |_| ()
                ),
                Err(CacheError::ClockRewound)
            ));
        })
        .unwrap();
        assert_eq!(outcome.reclaimed_entries_at_end, 1);
    });
}

#[test]
fn tlcx06_turn_cache_releases_owned_copies_before_returning() {
    let source = Digest32::from_bytes([1; 32]);
    let owner = ResourceBroker::new(ResourceLimits::uniform(1024 * 1024).unwrap()).unwrap();
    let run = owner
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
    let original = b"recomputable typed query";
    {
        let view = reservation.lease();
        let identity = id(source);
        let policy = TurnCachePolicy::memory(2, 1024, Duration::from_secs(10)).unwrap();
        let outcome = with_turn_cache(policy, &view, |cache| {
            let now = Instant::now();
            assert!(matches!(
                cache
                    .remember(&identity, Kind::SolverQueryPlan, original, now)
                    .unwrap(),
                RememberOutcome::Copied { .. }
            ));
            let read = cache
                .with_cached(&identity, Kind::SolverQueryPlan, now, |bytes| {
                    assert_eq!(bytes, Some(original.as_slice()));
                    42
                })
                .unwrap();
            assert_eq!(read.state, CacheLookupState::Hit);
            read.value
        })
        .unwrap();
        assert_eq!(outcome.value, 42);
        assert_eq!(outcome.reclaimed_entries_at_end, 1);
        assert_eq!(
            outcome.reclaimed_payload_bytes_at_end,
            original.len() as u64
        );
        assert!(!outcome.disk_cache_enabled);
    }
    assert_eq!(original, b"recomputable typed query");
    reservation.release_confirmed().unwrap();
    assert_eq!(owner.used(ResourceKind::Ram).unwrap(), 0);
}
