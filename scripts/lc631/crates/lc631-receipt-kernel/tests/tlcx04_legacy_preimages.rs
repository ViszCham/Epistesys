use lc631_receipt_kernel::{
    digest_text, ReceiptClass, ReceiptError, ReceiptPolicy, ReceiptScope, ReceiptVerifier,
    ReplayGuard, SubjectRevision, UntrustedReceipt,
};

// Independent .NET HMAC-SHA256/SHA256 vector; synthetic test key bytes 00..1f.
const FIXTURE: &str = concat!(
    r#"{"claims":{"schema_version":"lc631-authenticated-receipt.v1","issuer":"legacy-fixture","class":"host_seed","subject_revision":"sha256:subject","scope":"host/user-span","nonce_hex":"000102030405060708090a0b0c0d0e0f","payload_digest":"sha256:"#,
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    r#"","issued_at_epoch":10,"expires_at_epoch":20,"parent_receipt_digest":null},"mac_sha256":"sha256:e021688fd12425959b0f1dd04abdfe0ae392f7c6ef90dd739589b4276a819d65"}"#
);

fn verifier() -> ReceiptVerifier {
    ReceiptVerifier::from_key_bytes("legacy-fixture", &(0..32).collect::<Vec<u8>>()).unwrap()
}

fn policy(now: u64) -> ReceiptPolicy {
    ReceiptPolicy::exact(
        ReceiptClass::HostSeed,
        SubjectRevision::checked("sha256:subject").unwrap(),
        ReceiptScope::checked("host/user-span").unwrap(),
        format!("sha256:{}", "a".repeat(64)),
        now,
    )
}

#[test]
fn tlcx04_independent_golden_pins_custom_text_mac_and_receipt_digest() {
    let wire: UntrustedReceipt = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(
        wire.receipt_digest(),
        "sha256:8d2ad1400cf65fdf6481476f2851c628feec426291cc55a923b75bf931bbbeed"
    );
    let verified = verifier()
        .verify(wire, &policy(15), &mut ReplayGuard::default())
        .unwrap();
    assert_eq!(verified.class(), ReceiptClass::HostSeed);
}

#[test]
fn tlcx04_new_binary_domain_mac_is_not_a_legacy_mac() {
    let changed = FIXTURE.replace(
        "e021688fd12425959b0f1dd04abdfe0ae392f7c6ef90dd739589b4276a819d65",
        "0bbca34654755cde79c4260d4e33956d8695de97b695eb0f51a76d6be83afb1f",
    );
    let wire: UntrustedReceipt = serde_json::from_str(&changed).unwrap();
    let mut guard = ReplayGuard::default();
    assert!(matches!(
        verifier().verify(wire, &policy(15), &mut guard),
        Err(ReceiptError::MacMismatch)
    ));
    assert_eq!(guard.observed_count(), 0);
}

#[test]
fn tlcx04_rehashed_claims_or_json_canonicalization_cannot_authenticate() {
    let mut value: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    value["claims"]["issued_at_epoch"] = 11.into();
    let rehash = digest_text(&serde_json::to_string(&value["claims"]).unwrap());
    value["mac_sha256"] = rehash.into();
    let wire: UntrustedReceipt = serde_json::from_value(value).unwrap();
    assert!(matches!(
        verifier().verify(wire, &policy(15), &mut ReplayGuard::default()),
        Err(ReceiptError::MacMismatch)
    ));
}

#[test]
fn tlcx04_wire_field_order_is_not_the_custom_claims_mac_order() {
    let reordered = FIXTURE.replacen(
        r#""issued_at_epoch":10,"expires_at_epoch":20"#,
        r#""expires_at_epoch":20,"issued_at_epoch":10"#,
        1,
    );
    assert_ne!(reordered, FIXTURE);
    let wire: UntrustedReceipt = serde_json::from_str(&reordered).unwrap();
    verifier()
        .verify(wire, &policy(15), &mut ReplayGuard::default())
        .unwrap();
}

#[test]
fn tlcx04_strict_receipt_rejects_unknown_duplicate_and_trailing_fields() {
    let unknown = FIXTURE.replacen("{", r#"{"frame_digest":"untrusted","#, 1);
    let duplicate = FIXTURE.replacen(
        r#""class":"host_seed""#,
        r#""class":"host_seed","class":"host_seed""#,
        1,
    );
    let trailing = format!("{FIXTURE}{{}}");
    for input in [&unknown, &duplicate, &trailing] {
        assert!(serde_json::from_str::<UntrustedReceipt>(input).is_err());
    }
}

#[test]
fn tlcx04_fixed_golden_still_obeys_expiry_scope_and_nonce_consumption() {
    let wire: UntrustedReceipt = serde_json::from_str(FIXTURE).unwrap();
    let mut guard = ReplayGuard::default();
    assert!(matches!(
        verifier().verify(wire.clone(), &policy(20), &mut guard),
        Err(ReceiptError::Expired)
    ));
    let wrong_scope = ReceiptPolicy::exact(
        ReceiptClass::HostSeed,
        SubjectRevision::checked("sha256:subject").unwrap(),
        ReceiptScope::checked("another-scope").unwrap(),
        format!("sha256:{}", "a".repeat(64)),
        15,
    );
    assert!(matches!(
        verifier().verify(wire.clone(), &wrong_scope, &mut guard),
        Err(ReceiptError::ScopeMismatch)
    ));
    assert_eq!(guard.observed_count(), 0);
    verifier()
        .verify(wire.clone(), &policy(15), &mut guard)
        .unwrap();
    assert!(matches!(
        verifier().verify(wire, &policy(15), &mut guard),
        Err(ReceiptError::ReplayDetected)
    ));
    assert_eq!(guard.observed_count(), 1);
}
