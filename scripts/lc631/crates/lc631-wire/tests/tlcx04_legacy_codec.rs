use lc631_receipt_kernel::{
    ReceiptClass, ReceiptPolicy, ReceiptScope, ReceiptVerifier, ReplayGuard, SubjectRevision,
};
use lc631_wire::legacy_codec::{LegacyCodecError, LegacyJsonDocument, LegacyJsonKind};

const RECEIPT: &str = concat!(
    r#"{"claims":{"schema_version":"lc631-authenticated-receipt.v1","issuer":"legacy-fixture","class":"host_seed","subject_revision":"sha256:subject","scope":"host/user-span","nonce_hex":"000102030405060708090a0b0c0d0e0f","payload_digest":"sha256:"#,
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    r#"","issued_at_epoch":10,"expires_at_epoch":20,"parent_receipt_digest":null},"mac_sha256":"sha256:e021688fd12425959b0f1dd04abdfe0ae392f7c6ef90dd739589b4276a819d65"}"#
);
const LIMIT: usize = 16_384;

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

fn replay(version: u8) -> String {
    let authentication = if version == 2 {
        format!(
            r#","authenticity_receipt_digest":"sha256:{}""#,
            "4".repeat(64)
        )
    } else {
        String::new()
    };
    let digest = if version == 1 {
        "7a5edfd5961392279d17b66f935465c26a7bd2235faa981708e5b4edc2afb695"
    } else {
        "cad8fca5b090adfc2439808a351d89abba28cd75d39d38ecfea1a7dc3005901d"
    };
    format!(
        concat!(
            r#"{{"schema_version":"lc631-host-replay.v{}","sequence":1,"previous_entry_sha256":"sha256:{}","#,
            r#""artifact_id":7,"turn_id":11,"candidate_digest":"sha256:{}","output_digest":"sha256:{}","#,
            r#""host_callback_digest":"sha256:{}","host_seed_receipt_digest":"sha256:{}"{},"entry_sha256":"sha256:{}"}}"#
        ),
        version,
        "0".repeat(64),
        "1".repeat(64),
        "1".repeat(64),
        "2".repeat(64),
        "3".repeat(64),
        authentication,
        digest,
    )
}

#[test]
fn tlcx04_old_receipt_bytes_roundtrip_and_verify_only_with_current_policy() {
    let original = format!(" \n{RECEIPT}\t");
    let document =
        LegacyJsonDocument::decode(LegacyJsonKind::ReceiptV1, original.as_bytes(), LIMIT).unwrap();
    assert_eq!(document.original_bytes(), original.as_bytes());
    assert_eq!(document.compact_legacy_json().unwrap(), RECEIPT.as_bytes());
    assert_eq!(
        document.receipt().unwrap().receipt_digest(),
        "sha256:8d2ad1400cf65fdf6481476f2851c628feec426291cc55a923b75bf931bbbeed"
    );
    let mut replay = ReplayGuard::default();
    let verified = document
        .verify_receipt(&verifier(), &policy(15), &mut replay)
        .unwrap();
    assert_eq!(verified.class(), ReceiptClass::HostSeed);
    assert_eq!(replay.observed_count(), 1);
}

#[test]
fn tlcx04_rehashing_under_binary_domain_cannot_authenticate_legacy_receipt() {
    let forged = RECEIPT.replace(
        "e021688fd12425959b0f1dd04abdfe0ae392f7c6ef90dd739589b4276a819d65",
        "0bbca34654755cde79c4260d4e33956d8695de97b695eb0f51a76d6be83afb1f",
    );
    let document =
        LegacyJsonDocument::decode(LegacyJsonKind::ReceiptV1, forged.as_bytes(), LIMIT).unwrap();
    let mut guard = ReplayGuard::default();
    assert!(document
        .verify_receipt(&verifier(), &policy(15), &mut guard)
        .is_err());
    assert_eq!(guard.observed_count(), 0);
}

#[test]
fn tlcx04_strict_receipt_rejects_unknown_duplicate_and_future_schema_fields() {
    let unknown = RECEIPT.replacen("{", r#"{"frame_digest":"not-authentication","#, 1);
    let duplicate = RECEIPT.replacen(
        r#""class":"host_seed""#,
        r#""class":"host_seed","class":"host_seed""#,
        1,
    );
    let future = RECEIPT.replace(
        "lc631-authenticated-receipt.v1",
        "lc631-authenticated-receipt.v2",
    );
    for input in [&unknown, &duplicate, &future] {
        assert!(
            LegacyJsonDocument::decode(LegacyJsonKind::ReceiptV1, input.as_bytes(), LIMIT).is_err()
        );
    }
}

#[test]
fn tlcx04_expiry_and_replay_are_not_restored_by_roundtrip() {
    let document =
        LegacyJsonDocument::decode(LegacyJsonKind::ReceiptV1, RECEIPT.as_bytes(), LIMIT).unwrap();
    let mut guard = ReplayGuard::default();
    assert!(document
        .verify_receipt(&verifier(), &policy(20), &mut guard)
        .is_err());
    assert_eq!(guard.observed_count(), 0);
    let first =
        LegacyJsonDocument::decode(LegacyJsonKind::ReceiptV1, RECEIPT.as_bytes(), LIMIT).unwrap();
    first
        .verify_receipt(&verifier(), &policy(15), &mut guard)
        .unwrap();
    let second =
        LegacyJsonDocument::decode(LegacyJsonKind::ReceiptV1, RECEIPT.as_bytes(), LIMIT).unwrap();
    assert!(second
        .verify_receipt(&verifier(), &policy(15), &mut guard)
        .is_err());
    assert_eq!(guard.observed_count(), 1);
}

#[test]
fn tlcx04_replay_v1_v2_keep_separate_independent_golden_preimages() {
    for (version, kind) in [(1, LegacyJsonKind::ReplayV1), (2, LegacyJsonKind::ReplayV2)] {
        let input = replay(version);
        let document = LegacyJsonDocument::decode(kind, input.as_bytes(), LIMIT).unwrap();
        assert_eq!(document.original_bytes(), input.as_bytes());
        assert_eq!(document.compact_legacy_json().unwrap(), input.as_bytes());
        document.check_replay_integrity().unwrap();
        assert!(document.receipt().is_none());
        assert!(document
            .verify_receipt(&verifier(), &policy(15), &mut ReplayGuard::default())
            .is_err());
    }
}

#[test]
fn tlcx04_version_selection_does_not_fallback_or_cross_preimages() {
    let v1 = replay(1);
    let v2 = replay(2);
    assert!(LegacyJsonDocument::decode(LegacyJsonKind::ReplayV2, v1.as_bytes(), LIMIT).is_err());
    assert!(LegacyJsonDocument::decode(LegacyJsonKind::ReplayV1, v2.as_bytes(), LIMIT).is_err());
    let crossover = v2.replace(
        "cad8fca5b090adfc2439808a351d89abba28cd75d39d38ecfea1a7dc3005901d",
        "7a5edfd5961392279d17b66f935465c26a7bd2235faa981708e5b4edc2afb695",
    );
    let document =
        LegacyJsonDocument::decode(LegacyJsonKind::ReplayV2, crossover.as_bytes(), LIMIT).unwrap();
    assert!(matches!(
        document.check_replay_integrity(),
        Err(LegacyCodecError::DigestMismatch)
    ));
}

#[test]
fn tlcx04_replay_rejects_unknown_fields_and_tampered_payload() {
    let input = replay(1);
    let unknown = input.replacen("{", r#"{"encoding_revision":"new","#, 1);
    assert!(
        LegacyJsonDocument::decode(LegacyJsonKind::ReplayV1, unknown.as_bytes(), LIMIT).is_err()
    );
    let changed = input.replace(r#""artifact_id":7"#, r#""artifact_id":8"#);
    let document =
        LegacyJsonDocument::decode(LegacyJsonKind::ReplayV1, changed.as_bytes(), LIMIT).unwrap();
    assert!(document.check_replay_integrity().is_err());
}

#[test]
fn tlcx04_self_rehashed_replay_remains_unauthenticated() {
    let original = replay(1);
    let changed = original.replace(r#""artifact_id":7"#, r#""artifact_id":8"#);
    let document =
        LegacyJsonDocument::decode(LegacyJsonKind::ReplayV1, changed.as_bytes(), LIMIT).unwrap();
    let preimage = document.replay_preimage().unwrap();
    let self_hash = lc631_core::stable_sha256(std::str::from_utf8(&preimage).unwrap());
    let forged = changed.replace(
        "sha256:7a5edfd5961392279d17b66f935465c26a7bd2235faa981708e5b4edc2afb695",
        &self_hash,
    );
    let document =
        LegacyJsonDocument::decode(LegacyJsonKind::ReplayV1, forged.as_bytes(), LIMIT).unwrap();
    document.check_replay_integrity().unwrap();
    assert!(document.receipt().is_none());
    let mut guard = ReplayGuard::default();
    assert!(matches!(
        document.verify_receipt(&verifier(), &policy(15), &mut guard),
        Err(LegacyCodecError::WrongKind)
    ));
    assert_eq!(guard.observed_count(), 0);
}

#[test]
fn tlcx04_bounds_utf8_and_trailing_input_are_checked_before_admission() {
    assert!(matches!(
        LegacyJsonDocument::decode(
            LegacyJsonKind::ReceiptV1,
            RECEIPT.as_bytes(),
            RECEIPT.len() - 1
        ),
        Err(LegacyCodecError::LimitExceeded)
    ));
    assert!(matches!(
        LegacyJsonDocument::decode(LegacyJsonKind::ReceiptV1, RECEIPT.as_bytes(), 0),
        Err(LegacyCodecError::InvalidLimit)
    ));
    assert!(LegacyJsonDocument::decode(LegacyJsonKind::ReceiptV1, &[0xff], LIMIT).is_err());
    let trailing = format!("{RECEIPT}{{}}");
    assert!(
        LegacyJsonDocument::decode(LegacyJsonKind::ReceiptV1, trailing.as_bytes(), LIMIT).is_err()
    );
    LegacyJsonDocument::decode(LegacyJsonKind::ReceiptV1, RECEIPT.as_bytes(), RECEIPT.len())
        .unwrap();
}

#[test]
fn tlcx04_json_field_order_changes_do_not_change_receipt_mac_preimage() {
    let reordered = RECEIPT.replacen(
        r#""issued_at_epoch":10,"expires_at_epoch":20"#,
        r#""expires_at_epoch":20,"issued_at_epoch":10"#,
        1,
    );
    assert_ne!(reordered, RECEIPT);
    let document =
        LegacyJsonDocument::decode(LegacyJsonKind::ReceiptV1, reordered.as_bytes(), LIMIT).unwrap();
    assert_eq!(document.compact_legacy_json().unwrap(), RECEIPT.as_bytes());
    document
        .verify_receipt(&verifier(), &policy(15), &mut ReplayGuard::default())
        .unwrap();
}
