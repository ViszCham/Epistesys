//! Adversarial local inputs only; these are not observations from a host.
use lc631_core::stable_sha256;
use lc631_host::{
    append_durable_replay_verified, bind_codex_stop_hook_attested, verify_durable_replay,
    CodexStopHookEvent,
};
use lc631_receipt_kernel::{ReceiptIssuer, ReceiptVerifier, ReplayGuard};
use std::io::Write;

const FORGED: &str = r#"{"session_id":"s","cwd":".","hook_event_name":"Stop","model":"gpt","turn_id":"t","permission_mode":"default","stop_hook_active":false,"last_assistant_message":"forged"}"#;
const DIFFERENT: &str = r#"{"session_id":"s","cwd":".","hook_event_name":"Stop","model":"gpt","turn_id":"t","permission_mode":"default","stop_hook_active":false,"last_assistant_message":"different"}"#;

#[test]
fn tlcx35_mismatched_event_and_raw_callback_are_not_exact_bound() {
    let event: CodexStopHookEvent = serde_json::from_str(FORGED).unwrap();
    let issuer = ReceiptIssuer::from_key_bytes("local-fixture", &[73; 32]).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("local-fixture", &[73; 32]).unwrap();
    let mut replay = ReplayGuard::default();
    assert!(
        bind_codex_stop_hook_attested(&event, DIFFERENT, &issuer, &verifier, &mut replay, 10,)
            .is_err()
    );
    assert_eq!(replay.observed_count(), 0);
}

#[test]
fn tlcx35_local_self_signed_payload_does_not_prove_automatic_host_callback() {
    let event: CodexStopHookEvent = serde_json::from_str(FORGED).unwrap();
    let issuer = ReceiptIssuer::from_key_bytes("local-fixture", &[73; 32]).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("local-fixture", &[73; 32]).unwrap();
    let result = bind_codex_stop_hook_attested(
        &event,
        FORGED,
        &issuer,
        &verifier,
        &mut ReplayGuard::default(),
        10,
    );
    // Rejection or an explicitly unobserved report are both safe outcomes.
    if let Ok((receipt, observation)) = result {
        assert!(!receipt.automatic_host_callback_observed);
        assert!(!receipt.persistent_supervision_observed);
        assert!(!observation.automatic_host_callback_observed);
        assert!(!observation.persistent_supervision_observed);
    }
}

#[test]
fn tlcx35_all_known_event_fields_must_match_supplied_raw_json() {
    let event: CodexStopHookEvent = serde_json::from_str(FORGED).unwrap();
    let issuer = ReceiptIssuer::from_key_bytes("local-fixture", &[73; 32]).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("local-fixture", &[73; 32]).unwrap();
    for (name, replacement) in [
        ("session_id", serde_json::json!("other-session")),
        ("transcript_path", serde_json::json!("other-transcript")),
        ("cwd", serde_json::json!("elsewhere")),
        ("hook_event_name", serde_json::json!("Other")),
        ("model", serde_json::json!("other-model")),
        ("turn_id", serde_json::json!("other-turn")),
        ("permission_mode", serde_json::json!("other-mode")),
        ("stop_hook_active", serde_json::json!(true)),
        ("last_assistant_message", serde_json::json!("other-output")),
    ] {
        let mut raw: serde_json::Value = serde_json::from_str(FORGED).unwrap();
        raw[name] = replacement;
        let mut replay = ReplayGuard::default();
        assert!(
            bind_codex_stop_hook_attested(
                &event,
                &raw.to_string(),
                &issuer,
                &verifier,
                &mut replay,
                10
            )
            .is_err(),
            "mismatch in {name}"
        );
        assert_eq!(replay.observed_count(), 0);
    }
}

#[derive(serde::Serialize)]
struct LegacyReplayBasis<'a> {
    schema_version: &'a str,
    sequence: u64,
    previous_entry_sha256: String,
    artifact_id: u64,
    turn_id: u64,
    candidate_digest: String,
    output_digest: String,
    host_callback_digest: String,
    host_seed_receipt_digest: String,
}

#[test]
fn tlcx35_local_replay_preserves_legacy_chain_without_claiming_host_callback() {
    let event: CodexStopHookEvent = serde_json::from_str(FORGED).unwrap();
    let issuer = ReceiptIssuer::from_key_bytes("local-fixture", &[73; 32]).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("local-fixture", &[73; 32]).unwrap();
    let (receipt, observation) = bind_codex_stop_hook_attested(
        &event,
        FORGED,
        &issuer,
        &verifier,
        &mut ReplayGuard::default(),
        10,
    )
    .unwrap();
    assert!(!receipt.automatic_host_callback_observed);
    assert!(!receipt.persistent_supervision_observed);
    assert!(!observation.automatic_host_callback_observed);
    assert!(!observation.persistent_supervision_observed);
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "tlcx35-local-replay-{}-{unique}",
        std::process::id()
    ));
    std::fs::create_dir(&root).unwrap();
    let ledger = root.join("legacy.jsonl");
    let old = LegacyReplayBasis {
        schema_version: "lc631-host-replay.v1",
        sequence: 1,
        previous_entry_sha256: stable_sha256("lc631-host-replay-genesis.v1"),
        artifact_id: 1,
        turn_id: 1,
        candidate_digest: stable_sha256("legacy-output"),
        output_digest: stable_sha256("legacy-output"),
        host_callback_digest: stable_sha256("legacy-callback"),
        host_seed_receipt_digest: stable_sha256("legacy-seed"),
    };
    let canonical = serde_json::to_string(&old).unwrap();
    let entry = format!(
        "{},\"entry_sha256\":\"{}\"}}\n",
        &canonical[..canonical.len() - 1],
        stable_sha256(&canonical)
    );
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&ledger)
        .unwrap();
    file.write_all(entry.as_bytes()).unwrap();
    drop(file);
    assert_eq!(verify_durable_replay(&ledger).unwrap(), 1);
    let report = append_durable_replay_verified(
        &root,
        &ledger,
        &receipt,
        &verifier,
        &mut ReplayGuard::default(),
        10,
    )
    .unwrap();
    assert_eq!(report.entry_count, 2);
    assert!(report.chain_verified);
    assert!(report.durable_replay_observed);
    assert!(!report.automatic_host_callback_observed);
    assert_eq!(verify_durable_replay(&ledger).unwrap(), 2);
    // This owned, unique test directory is deliberately retained; no user data is deleted.
}
