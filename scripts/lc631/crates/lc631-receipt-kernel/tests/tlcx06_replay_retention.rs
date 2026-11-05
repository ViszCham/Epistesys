use lc631_receipt_kernel::{
    digest_text, ReceiptClass, ReceiptError, ReceiptIssuer, ReceiptPolicy, ReceiptScope,
    ReceiptVerifier, ReplayCapacityPolicy, ReplayGuard, SubjectRevision,
};

#[test]
fn tlcx06_replay_capacity_is_finite_and_never_evicts_seen_receipts() {
    let key = [0x61_u8; 32];
    let issuer = ReceiptIssuer::from_key_bytes("tlcx06-replay-fixture", &key).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("tlcx06-replay-fixture", &key).unwrap();
    let make = |suffix: &str| {
        let subject = SubjectRevision::checked(format!("source-{suffix}")).unwrap();
        let scope = ReceiptScope::checked("tlcx06/replay-capacity").unwrap();
        let payload = digest_text(&format!("payload-{suffix}"));
        let receipt = issuer
            .issue(
                ReceiptClass::Validation,
                subject.clone(),
                scope.clone(),
                payload.clone(),
                1,
                Some(100),
                None,
            )
            .unwrap();
        let policy = ReceiptPolicy::exact(ReceiptClass::Validation, subject, scope, payload, 2);
        (receipt, policy)
    };
    let (first, first_policy) = make("first");
    let (second, second_policy) = make("second");
    let mut replay = ReplayGuard::with_capacity(1).unwrap();
    let first_again = first.clone();

    assert!(verifier.verify(first, &first_policy, &mut replay).is_ok());
    assert_eq!(replay.observed_count(), 1);
    assert_eq!(
        verifier.verify(second, &second_policy, &mut replay),
        Err(ReceiptError::ReplayCapacityExceeded)
    );
    assert_eq!(replay.observed_count(), 1);
    assert_eq!(
        verifier.verify(first_again, &first_policy, &mut replay),
        Err(ReceiptError::ReplayDetected)
    );
    assert_eq!(replay.observed_count(), 1);
}

#[test]
fn tlcx06_replay_capacity_constructor_rejects_invalid_or_excessive_limits() {
    assert_eq!(
        ReplayCapacityPolicy::checked(0).unwrap_err(),
        ReceiptError::InvalidReplayCapacity
    );
    assert_eq!(
        ReplayCapacityPolicy::checked(ReplayCapacityPolicy::MAX_ENTRIES + 1).unwrap_err(),
        ReceiptError::InvalidReplayCapacity
    );
}

#[test]
fn tlcx06_default_replay_guard_uses_a_finite_non_evicting_cap() {
    let replay = ReplayGuard::default();
    assert_eq!(
        replay.capacity_limit(),
        ReplayCapacityPolicy::DEFAULT_ENTRIES
    );
    assert_eq!(replay.observed_count(), 0);
}

#[test]
fn tlcx06_capacity_policy_does_not_change_legacy_serialized_fields() {
    let legacy = serde_json::to_value(ReplayGuard::default()).unwrap();
    let bounded = serde_json::to_value(ReplayGuard::with_capacity(4).unwrap()).unwrap();

    assert_eq!(legacy, bounded);
    assert_eq!(
        legacy
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        vec!["seen_nonces".to_string(), "seen_receipts".to_string()]
    );
}
