use lc631_receipt_kernel::{
    digest_text, ReceiptClass, ReceiptIssuer, ReceiptPolicy, ReceiptScope, ReceiptVerifier,
    ReplayGuard, SubjectRevision,
};
use std::collections::BTreeSet;

fn issuer(label: &str, key: u8) -> ReceiptIssuer {
    ReceiptIssuer::from_key_bytes(label, &[key; 32]).expect("test issuer")
}

#[test]
fn arc631_23_adversarial_taxonomy_fixture_is_unique_and_reject_or_hold_biased() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../fixtures/arc631-adversarial-authenticity.v1.json"
    ))
    .unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    assert!(cases.len() >= 20);
    assert_eq!(
        cases
            .iter()
            .filter_map(|case| case["id"].as_str())
            .collect::<BTreeSet<_>>()
            .len(),
        cases.len()
    );
    assert!(cases.iter().all(|case| {
        matches!(
            case["expected"].as_str(),
            Some(
                "reject"
                    | "hold"
                    | "distinguish"
                    | "record-executable-digest"
                    | "unavailable"
                    | "candidate-only"
            )
        )
    }));
}

#[test]
fn arc631_01_roundtrip_is_untrusted_until_exact_verification() {
    let signer = issuer("host", 7);
    let verifier = ReceiptVerifier::from_key_bytes("host", &[7; 32]).unwrap();
    let subject = SubjectRevision::checked("sha256:subject").unwrap();
    let scope = ReceiptScope::checked("host/user-span").unwrap();
    let wire = signer
        .issue(
            ReceiptClass::HostSeed,
            subject.clone(),
            scope.clone(),
            digest_text("payload"),
            10,
            Some(20),
            None,
        )
        .unwrap();
    let encoded = serde_json::to_string(&wire).unwrap();
    let decoded = serde_json::from_str(&encoded).unwrap();
    let mut replay = ReplayGuard::default();
    let verified = verifier
        .verify(
            decoded,
            &ReceiptPolicy::exact(
                ReceiptClass::HostSeed,
                subject,
                scope,
                digest_text("payload"),
                15,
            ),
            &mut replay,
        )
        .unwrap();
    assert_eq!(verified.class(), ReceiptClass::HostSeed);
}

#[test]
fn arc631_02_wrong_key_subject_scope_expiry_and_replay_are_rejected() {
    let signer = issuer("host", 7);
    let wrong = ReceiptVerifier::from_key_bytes("host", &[8; 32]).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("host", &[7; 32]).unwrap();
    let subject = SubjectRevision::checked("sha256:subject").unwrap();
    let scope = ReceiptScope::checked("host/user-span").unwrap();
    let wire = signer
        .issue(
            ReceiptClass::HostSeed,
            subject.clone(),
            scope.clone(),
            digest_text("payload"),
            10,
            Some(20),
            None,
        )
        .unwrap();
    let policy = ReceiptPolicy::exact(
        ReceiptClass::HostSeed,
        subject.clone(),
        scope.clone(),
        digest_text("payload"),
        15,
    );
    assert!(wrong
        .verify(wire.clone(), &policy, &mut ReplayGuard::default())
        .is_err());
    assert!(verifier
        .verify(
            wire.clone(),
            &ReceiptPolicy::exact(
                ReceiptClass::HostSeed,
                SubjectRevision::checked("sha256:other").unwrap(),
                scope.clone(),
                digest_text("payload"),
                15,
            ),
            &mut ReplayGuard::default(),
        )
        .is_err());
    assert!(verifier
        .verify(
            wire.clone(),
            &ReceiptPolicy::exact(
                ReceiptClass::HostSeed,
                subject.clone(),
                ReceiptScope::checked("host").unwrap(),
                digest_text("payload"),
                15,
            ),
            &mut ReplayGuard::default(),
        )
        .is_err());
    assert!(verifier
        .verify(
            wire.clone(),
            &ReceiptPolicy::exact(
                ReceiptClass::HostSeed,
                subject,
                scope,
                digest_text("payload"),
                21,
            ),
            &mut ReplayGuard::default(),
        )
        .is_err());
    let mut replay = ReplayGuard::default();
    verifier.verify(wire.clone(), &policy, &mut replay).unwrap();
    assert!(verifier.verify(wire, &policy, &mut replay).is_err());

    let parent = digest_text("parent");
    let parented = signer
        .issue(
            ReceiptClass::HostSeed,
            SubjectRevision::checked("sha256:subject").unwrap(),
            ReceiptScope::checked("host/user-span").unwrap(),
            digest_text("payload"),
            10,
            Some(20),
            Some(parent.clone()),
        )
        .unwrap();
    assert!(verifier
        .verify(parented.clone(), &policy, &mut ReplayGuard::default(),)
        .is_err());
    verifier
        .verify(
            parented,
            &ReceiptPolicy::exact(
                ReceiptClass::HostSeed,
                SubjectRevision::checked("sha256:subject").unwrap(),
                ReceiptScope::checked("host/user-span").unwrap(),
                digest_text("payload"),
                15,
            )
            .with_parent(parent),
            &mut ReplayGuard::default(),
        )
        .unwrap();
}
