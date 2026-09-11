use lc631_core::{stable_sha256, ArtifactId, SourceSpan, TurnId};
use lc631_host::{
    append_durable_replay, append_durable_replay_verified, bind_host_output_attested,
    host_output_payload_digest, host_seed_payload_digest, HostError, HostOutputCandidate,
    HostOutputReceipt, HostSeedEnvelope,
};
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptIssuer, ReceiptScope, ReceiptVerifier, ReplayGuard, SubjectRevision,
};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn root() -> (ReceiptIssuer, ReceiptVerifier) {
    let key = [19_u8; 32];
    (
        ReceiptIssuer::from_key_bytes("lc631-host-root", &key).unwrap(),
        ReceiptVerifier::from_key_bytes("lc631-host-root", &key).unwrap(),
    )
}

fn verified_seed(
    issuer: &ReceiptIssuer,
    verifier: &ReceiptVerifier,
    replay: &mut ReplayGuard,
) -> HostSeedEnvelope {
    let raw = "edit this repository".to_string();
    let span = SourceSpan::checked(&raw, 0, raw.len()).unwrap();
    let payload = host_seed_payload_digest(ArtifactId(7), TurnId(9), &raw, span);
    let subject = SubjectRevision::checked(stable_sha256(&raw)).unwrap();
    let scope = ReceiptScope::checked("host/user-span/7/9").unwrap();
    let wire = issuer
        .issue(
            ReceiptClass::HostSeed,
            subject.clone(),
            scope.clone(),
            payload.clone(),
            1,
            None,
            None,
        )
        .unwrap();
    HostSeedEnvelope::verify_attested(
        ArtifactId(7),
        TurnId(9),
        raw,
        span,
        wire,
        verifier,
        replay,
        1,
    )
    .unwrap()
}

#[test]
fn arc631_03_arbitrary_receipt_string_is_not_verified_host_input() {
    let raw = "edit".to_string();
    assert_eq!(
        HostSeedEnvelope::verified(
            ArtifactId(1),
            TurnId(1),
            raw.clone(),
            SourceSpan::checked(&raw, 0, raw.len()).unwrap(),
            "nonempty-but-unsigned",
        ),
        Err(HostError::LegacyReceiptRejected)
    );
}

#[test]
fn arc631_05_candidate_digest_is_recomputed_before_output_binding() {
    let (issuer, verifier) = root();
    let mut replay = ReplayGuard::default();
    let seed = verified_seed(&issuer, &verifier, &mut replay);
    let candidate = HostOutputCandidate {
        artifact_id: ArtifactId(7),
        turn_id: TurnId(9),
        candidate_digest: stable_sha256("different bytes"),
        output_text: "actual output".into(),
    };
    assert_eq!(
        bind_host_output_attested(
            &seed,
            &candidate,
            "callback",
            None,
            &verifier,
            &mut replay,
            1,
        ),
        Err(HostError::CandidateDigestMismatch)
    );
}

#[test]
fn arc631_05_public_exact_bool_cannot_enter_durable_replay() {
    let receipt = HostOutputReceipt::legacy_fixture_for_test(
        ArtifactId(7),
        TurnId(9),
        stable_sha256("output"),
    );
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("lc631-untrusted-replay-{nonce}"));
    let ledger = root.join("replay.jsonl");
    assert_eq!(
        append_durable_replay(&root, &ledger, &receipt),
        Err(HostError::LegacyReceiptRejected)
    );
}

#[test]
fn arc631_05_attested_output_replays_only_once_per_verification_context() {
    let (issuer, verifier) = root();
    let mut replay = ReplayGuard::default();
    let seed = verified_seed(&issuer, &verifier, &mut replay);
    let candidate = HostOutputCandidate {
        artifact_id: ArtifactId(7),
        turn_id: TurnId(9),
        candidate_digest: stable_sha256("actual output"),
        output_text: "actual output".into(),
    };
    let payload = host_output_payload_digest(&seed, &candidate, "callback").unwrap();
    let subject = SubjectRevision::checked(stable_sha256(&format!(
        "7:9:{}",
        stable_sha256("actual output")
    )))
    .unwrap();
    let scope = ReceiptScope::checked("host/output/7/9").unwrap();
    let wire = issuer
        .issue(
            ReceiptClass::HostOutput,
            subject,
            scope,
            payload,
            1,
            None,
            None,
        )
        .unwrap();
    let receipt = bind_host_output_attested(
        &seed,
        &candidate,
        "callback",
        Some(wire),
        &verifier,
        &mut replay,
        1,
    )
    .unwrap();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("lc631-attested-replay-{nonce}"));
    fs::create_dir_all(&root).unwrap();
    let ledger = root.join("replay.jsonl");
    let mut replay_at_sink = ReplayGuard::default();
    append_durable_replay_verified(&root, &ledger, &receipt, &verifier, &mut replay_at_sink, 1)
        .unwrap();
    assert_eq!(
        append_durable_replay_verified(
            &root,
            &ledger,
            &receipt,
            &verifier,
            &mut ReplayGuard::default(),
            1,
        ),
        Err(HostError::ReceiptReplay)
    );
    fs::remove_file(ledger).unwrap();
    fs::remove_dir(root).unwrap();
}
