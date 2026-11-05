//! Explicit local signed fixtures only. This does not authenticate an actual host.
use lc631_core::{stable_sha256, ArtifactId, SourceSpan, TurnId};
use lc631_host::{
    host_output_payload_digest, host_seed_payload_digest, HostOutputCandidate, HostReceiptContext,
    HostSeedEnvelope,
};
use lc631_receipt_kernel::{ReceiptClass, ReceiptScope, ReplayGuard, SubjectRevision};
use std::io::Write;
use std::path::Path;
use std::process::Command;

fn create_file(path: &Path, bytes: &[u8]) {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap();
    file.write_all(bytes).unwrap();
}

#[test]
fn tlcx35_host_bind_local_attestations_replay_without_host_promotion() {
    const SEED: &str = "fixture seed";
    const OUTPUT: &str = "fixture output";
    const CALLBACK: &str = "fixture callback";
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    let root = std::env::temp_dir().join(format!(
        "tlcx35-local-bind-{}-{}",
        std::process::id(),
        duration.as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let store = root.join("store");
    let receipt_root = store.join("receipt-root");
    let context = HostReceiptContext::load_or_create(&receipt_root).unwrap();
    let artifact = ArtifactId(43);
    let turn = TurnId(2);
    let span = SourceSpan::checked(SEED, 0, SEED.len()).unwrap();
    let issued = duration.as_secs().saturating_sub(1);
    let expiry = duration.as_secs().checked_add(1_200).unwrap();
    let seed_wire = context
        .issuer()
        .issue(
            ReceiptClass::HostSeed,
            SubjectRevision::checked(stable_sha256(SEED)).unwrap(),
            ReceiptScope::checked("host/user-span/43/2").unwrap(),
            host_seed_payload_digest(artifact, turn, SEED, span),
            issued,
            Some(expiry),
            None,
        )
        .unwrap();
    let verified_seed = HostSeedEnvelope::verify_attested(
        artifact,
        turn,
        SEED.into(),
        span,
        seed_wire.clone(),
        context.verifier(),
        &mut ReplayGuard::default(),
        duration.as_secs(),
    )
    .unwrap();
    let candidate = HostOutputCandidate {
        artifact_id: artifact,
        turn_id: turn,
        candidate_digest: stable_sha256(OUTPUT),
        output_text: OUTPUT.into(),
    };
    let output_wire = context
        .issuer()
        .issue(
            ReceiptClass::HostOutput,
            SubjectRevision::checked(stable_sha256(&format!(
                "43:2:{}",
                candidate.candidate_digest
            )))
            .unwrap(),
            ReceiptScope::checked("host/output/43/2").unwrap(),
            host_output_payload_digest(&verified_seed, &candidate, CALLBACK).unwrap(),
            issued,
            Some(expiry),
            None,
        )
        .unwrap();
    let seed_file = root.join("seed.txt");
    let output_file = root.join("output.txt");
    let altered_file = root.join("altered.txt");
    let callback_file = root.join("callback.txt");
    let attestations = root.join("attestations.json");
    create_file(&seed_file, SEED.as_bytes());
    create_file(&output_file, OUTPUT.as_bytes());
    create_file(&altered_file, b"different output");
    create_file(&callback_file, CALLBACK.as_bytes());
    create_file(
        &attestations,
        &serde_json::to_vec(&serde_json::json!({"seed":seed_wire,"output":output_wire})).unwrap(),
    );
    let ledger = store.join("host-output-v2.jsonl");
    let execute = |output_path: &Path| {
        Command::new(env!("CARGO_BIN_EXE_lc631"))
            .arg("lc631-host-bind")
            .arg("--seed-file")
            .arg(&seed_file)
            .arg("--output-file")
            .arg(output_path)
            .arg("--callback-file")
            .arg(&callback_file)
            .arg("--host-receipt-file")
            .arg(&attestations)
            .arg("--ledger-root")
            .arg(&store)
            .arg("--ledger")
            .arg(&ledger)
            .args(["--artifact", "43", "--turn", "2"])
            .output()
            .unwrap()
    };
    let altered = execute(&altered_file);
    assert!(!altered.status.success());
    assert!(!ledger.exists());
    let output = execute(&output_file);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["payload"]["replay"]["chain_verified"], true);
    assert_eq!(value["payload"]["automatic_host_callback_observed"], false);
    assert_eq!(
        value["payload"]["replay"]["automatic_host_callback_observed"],
        false
    );
    // The unique test-owned directory is intentionally retained; no deletion.
}
