#![forbid(unsafe_code)]

use lc631_receipt_kernel::{
    ReceiptClass, ReceiptPolicy, ReceiptScope, ReceiptVerifier, ReplayGuard, SubjectRevision,
    UntrustedReceipt, RECEIPT_SCHEMA,
};

const FIXTURE: &str = concat!(
    r#"{"claims":{"schema_version":"lc631-authenticated-receipt.v1","issuer":"legacy-fixture","class":"host_seed","subject_revision":"sha256:subject","scope":"host/user-span","nonce_hex":"000102030405060708090a0b0c0d0e0f","payload_digest":"sha256:"#,
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    r#"","issued_at_epoch":10,"expires_at_epoch":20,"parent_receipt_digest":null},"mac_sha256":"sha256:e021688fd12425959b0f1dd04abdfe0ae392f7c6ef90dd739589b4276a819d65"}"#
);

fn changed_value(pointer: &str, text: String) -> serde_json::Value {
    let mut value: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    *value.pointer_mut(pointer).unwrap() = text.into();
    value
}

fn assert_text_limit(pointer: &str, limit: usize) {
    let at_limit = changed_value(pointer, "x".repeat(limit));
    let wire = serde_json::from_value::<UntrustedReceipt>(at_limit.clone()).unwrap();
    wire.check_size_bounds().unwrap();
    let json = serde_json::to_string(&at_limit).unwrap();
    assert!(serde_json::from_str::<UntrustedReceipt>(&json).is_ok());

    let over_limit = changed_value(pointer, "x".repeat(limit + 1));
    let json = serde_json::to_string(&over_limit).unwrap();
    assert!(serde_json::from_value::<UntrustedReceipt>(over_limit).is_err());
    assert!(serde_json::from_str::<UntrustedReceipt>(&json).is_err());
    // Admission is intentionally not format/MAC verification: arbitrary 'x'
    // bytes at a digest/schema boundary above do not become authentic claims.
}

#[test]
fn tlcx05_receipt_raw_text_exact_limits_and_one_byte_over() {
    for (pointer, limit) in [
        ("/claims/schema_version", RECEIPT_SCHEMA.len()),
        ("/claims/nonce_hex", 32),
        ("/claims/payload_digest", 71),
        ("/claims/parent_receipt_digest", 71),
        ("/mac_sha256", 71),
    ] {
        assert_text_limit(pointer, limit);
    }
}

#[test]
fn tlcx05_receipt_checked_text_exact_limits_and_one_byte_over() {
    for (pointer, limit) in [
        ("/claims/issuer", 128),
        ("/claims/subject_revision", 256),
        ("/claims/scope", 512),
    ] {
        assert_text_limit(pointer, limit);
    }
}

#[test]
fn tlcx05_receipt_text_limits_measure_decoded_utf8_bytes() {
    let at_limit = changed_value("/claims/issuer", "é".repeat(64));
    let over_limit = changed_value("/claims/issuer", format!("{}x", "é".repeat(64)));
    assert!(serde_json::from_value::<UntrustedReceipt>(at_limit).is_ok());
    assert!(serde_json::from_value::<UntrustedReceipt>(over_limit).is_err());
}

#[test]
fn tlcx05_receipt_escaped_json_limits_apply_after_decoding() {
    let original = "000102030405060708090a0b0c0d0e0f";
    let at_limit = FIXTURE.replace(original, &r"\u0078".repeat(32));
    let over_limit = FIXTURE.replace(original, &r"\u0078".repeat(33));
    assert!(serde_json::from_str::<UntrustedReceipt>(&at_limit).is_ok());
    assert!(serde_json::from_str::<UntrustedReceipt>(&over_limit).is_err());
    let unicode_over = FIXTURE.replace(original, &r"\u00e9".repeat(17));
    assert!(serde_json::from_str::<UntrustedReceipt>(&unicode_over).is_err());
}

#[test]
fn tlcx05_receipt_missing_and_null_parent_keep_legacy_mac_and_wire_shape() {
    let missing_parent = FIXTURE.replace(",\"parent_receipt_digest\":null", "");
    let verifier =
        ReceiptVerifier::from_key_bytes("legacy-fixture", &(0..32).collect::<Vec<u8>>()).unwrap();
    let policy = ReceiptPolicy::exact(
        ReceiptClass::HostSeed,
        SubjectRevision::checked("sha256:subject").unwrap(),
        ReceiptScope::checked("host/user-span").unwrap(),
        format!("sha256:{}", "a".repeat(64)),
        15,
    );
    for input in [FIXTURE, missing_parent.as_str()] {
        let wire: UntrustedReceipt = serde_json::from_str(input).unwrap();
        assert_eq!(wire.claims().parent_receipt_digest(), None);
        let rendered = serde_json::to_value(&wire).unwrap();
        assert!(rendered["claims"]["parent_receipt_digest"].is_null());
        let mut replay = ReplayGuard::default();
        verifier.verify(wire, &policy, &mut replay).unwrap();
        assert_eq!(replay.observed_count(), 1);
    }
}
