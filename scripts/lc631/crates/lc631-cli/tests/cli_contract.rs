#![forbid(unsafe_code)]

use lc631_core::{stable_sha256, LC631_VERSION};
use lc631_host::HostReceiptContext;
use lc631_receipt_kernel::{ReceiptClass, ReceiptScope, SubjectRevision};
use serde_json::Value;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_lc631")
}

#[test]
fn world_doctor_emits_exact_budget_in_shadow_envelope() {
    let output = Command::new(binary())
        .args(["lc631-world-doctor", "--prompt", "2 + 2 = 4"])
        .output()
        .expect("run lc631 world doctor");
    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON output");
    assert_eq!(json["shadow_only"], true);
    assert_eq!(json["payload"]["distinct_worlds"], 256);
    assert_eq!(json["payload"]["evaluations"], 2_048);
}

#[test]
fn promotion_gate_is_fail_closed_and_unknown_command_uses_stderr() {
    let promotion = Command::new(binary())
        .arg("lc631-promotion-gate")
        .output()
        .expect("run promotion gate");
    assert!(promotion.status.success());
    let json: Value = serde_json::from_slice(&promotion.stdout).expect("valid promotion JSON");
    assert_eq!(json["payload"]["predicates"].as_object().unwrap().len(), 13);
    assert!(json["payload"]["predicates"]
        .as_object()
        .unwrap()
        .values()
        .all(|value| value == false));
    assert_eq!(json["payload"]["promotion_allowed"], false);
    assert_eq!(json["payload"]["automatic_promotion"], false);

    let invalid = Command::new(binary())
        .arg("lc631-unknown")
        .output()
        .expect("run invalid command");
    assert_eq!(invalid.status.code(), Some(2));
    assert!(invalid.stdout.is_empty());
    let error: Value = serde_json::from_slice(&invalid.stderr).expect("valid error JSON");
    assert_eq!(error["schema_version"], "lc631-cli-error.v1");
}

#[test]
fn promotion_evidence_requires_a_receipt_root() {
    let output = Command::new(binary())
        .args(["lc631-promotion-gate", "--evidence-file", "missing.json"])
        .output()
        .expect("run promotion gate with evidence");

    assert_eq!(output.status.code(), Some(2));
    let error: Value = serde_json::from_slice(&output.stderr).expect("valid error JSON");
    assert_eq!(
        error["error"],
        "lc631-promotion-gate_requires_--receipt-root"
    );
}

#[test]
fn promotion_evidence_accepts_only_the_complete_signed_predicate_set() {
    let root = promotion_test_root("complete");
    let evidence = signed_promotion_evidence(&root);
    let evidence_path = root.join("promotion-evidence.json");
    fs::write(&evidence_path, serde_json::to_vec(&evidence).unwrap()).unwrap();

    let output = Command::new(binary())
        .args([
            "lc631-promotion-gate",
            "--evidence-file",
            evidence_path.to_str().unwrap(),
            "--receipt-root",
            root.to_str().unwrap(),
        ])
        .output()
        .expect("run promotion gate with signed evidence");

    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid promotion JSON");
    assert_eq!(json["payload"]["predicates"].as_object().unwrap().len(), 13);
    assert_eq!(json["payload"]["evidence_complete"], true);
    assert_eq!(json["payload"]["promotion_allowed"], false);
    assert_eq!(
        json["payload"]["blocked_reasons"],
        serde_json::json!(["independent_trust_principals"])
    );
    assert_eq!(json["payload"]["automatic_promotion"], false);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn promotion_evidence_rejects_unknown_receipt_fields() {
    let root = promotion_test_root("unknown-receipt-field");
    let mut evidence = signed_promotion_evidence(&root);
    evidence["receipts"][0]["receipt"]["unexpected"] = Value::Bool(true);
    let evidence_path = root.join("promotion-evidence.json");
    fs::write(&evidence_path, serde_json::to_vec(&evidence).unwrap()).unwrap();

    let output = Command::new(binary())
        .args([
            "lc631-promotion-gate",
            "--evidence-file",
            evidence_path.to_str().unwrap(),
            "--receipt-root",
            root.to_str().unwrap(),
        ])
        .output()
        .expect("run promotion gate with unknown receipt field");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown field"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn promotion_evidence_rejects_a_tampered_mac() {
    let root = promotion_test_root("tampered-mac");
    let mut evidence = signed_promotion_evidence(&root);
    evidence["receipts"][0]["receipt"]["mac_sha256"] = Value::String(
        "sha256:0000000000000000000000000000000000000000000000000000000000000000".into(),
    );
    let evidence_path = root.join("promotion-evidence.json");
    fs::write(&evidence_path, serde_json::to_vec(&evidence).unwrap()).unwrap();

    let output = Command::new(binary())
        .args([
            "lc631-promotion-gate",
            "--evidence-file",
            evidence_path.to_str().unwrap(),
            "--receipt-root",
            root.to_str().unwrap(),
        ])
        .output()
        .expect("run promotion gate with tampered evidence");

    assert_eq!(output.status.code(), Some(2));
    let error: Value = serde_json::from_slice(&output.stderr).expect("valid error JSON");
    assert!(error["error"]
        .as_str()
        .is_some_and(|message| message.ends_with(":MacMismatch")));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn promotion_evidence_requires_both_validation_and_host_pickup_receipts() {
    for (label, predicate, slot) in [
        ("missing-ci", "full_validation", "remote-ci"),
        (
            "missing-pickup",
            "package_installed_host_pickup",
            "host-pickup",
        ),
    ] {
        let root = promotion_test_root(label);
        let mut evidence = signed_promotion_evidence(&root);
        evidence["receipts"]
            .as_array_mut()
            .unwrap()
            .retain(|entry| entry["predicate"] != predicate || entry["slot"] != slot);
        let evidence_path = root.join("promotion-evidence.json");
        fs::write(&evidence_path, serde_json::to_vec(&evidence).unwrap()).unwrap();

        let output = Command::new(binary())
            .args([
                "lc631-promotion-gate",
                "--evidence-file",
                evidence_path.to_str().unwrap(),
                "--receipt-root",
                root.to_str().unwrap(),
            ])
            .output()
            .expect("run promotion gate with incomplete evidence");

        assert!(output.status.success());
        let json: Value = serde_json::from_slice(&output.stdout).expect("valid promotion JSON");
        assert_eq!(json["payload"]["predicates"][predicate], false);
        assert_eq!(json["payload"]["promotion_allowed"], false);
        assert!(json["payload"]["rejected_evidence"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == &format!("{predicate}:missing:{slot}")));
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn promotion_evidence_rejects_dirty_or_stale_inputs() {
    for (label, expected) in [
        ("dirty", "promotion_evidence_dirty_worktree"),
        ("stale", "promotion_evidence_stale:v630_frozen:baseline"),
        (
            "missing-expiry",
            "promotion_evidence_expiry_required:v630_frozen:baseline",
        ),
    ] {
        let root = promotion_test_root(label);
        let mut evidence = signed_promotion_evidence(&root);
        match label {
            "dirty" => evidence["binding"]["worktree_clean"] = Value::Bool(false),
            "stale" => {
                evidence["receipts"][0]["receipt"]["claims"]["issued_at_epoch"] = Value::from(0)
            }
            "missing-expiry" => {
                evidence["receipts"][0]["receipt"]["claims"]["expires_at_epoch"] = Value::Null
            }
            _ => unreachable!("fixed evidence case"),
        }
        let evidence_path = root.join("promotion-evidence.json");
        fs::write(&evidence_path, serde_json::to_vec(&evidence).unwrap()).unwrap();

        let output = Command::new(binary())
            .args([
                "lc631-promotion-gate",
                "--evidence-file",
                evidence_path.to_str().unwrap(),
                "--receipt-root",
                root.to_str().unwrap(),
            ])
            .output()
            .expect("run promotion gate with rejected evidence");

        assert_eq!(output.status.code(), Some(2));
        let error: Value = serde_json::from_slice(&output.stderr).expect("valid error JSON");
        assert_eq!(error["error"], expected);
        fs::remove_dir_all(root).unwrap();
    }
}

fn promotion_test_root(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("lc631-promotion-{label}-{nonce}"))
}

fn signed_promotion_evidence(root: &Path) -> Value {
    let context = HostReceiptContext::load_or_create(root).unwrap();
    let now_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let binding = promotion_binding();
    let binding_digest = promotion_binding_digest(&binding);
    let subject = SubjectRevision::checked(binding_digest.clone()).unwrap();
    let mut receipts = Vec::new();
    for (predicate, requirements) in promotion_requirements() {
        for (slot, class) in requirements {
            let scope = ReceiptScope::checked(format!("promotion/{predicate}/{slot}")).unwrap();
            let payload_digest = stable_sha256(&format!(
                "lc631-promotion-evidence.v1\0{binding_digest}\0{predicate}\0{slot}\0pass"
            ));
            let receipt = context
                .issuer()
                .issue(
                    *class,
                    subject.clone(),
                    scope,
                    payload_digest,
                    now_epoch.saturating_sub(1),
                    Some(now_epoch + 60),
                    None,
                )
                .unwrap();
            receipts.push(serde_json::json!({
                "predicate": predicate,
                "slot": slot,
                "receipt": receipt,
            }));
        }
    }
    serde_json::json!({
        "schema_version": "lc631-promotion-evidence.v1",
        "binding": binding,
        "receipts": receipts,
    })
}

fn promotion_binding() -> Value {
    serde_json::json!({
        "candidate_commit": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "worktree_clean": true,
        "engine_version": LC631_VERSION,
        "v630_baseline_commit": "0d933982fa392043c2fe825644a3a560963b6615",
        "package_binary_sha256": test_digest('1'),
        "installed_binary_sha256": test_digest('1'),
        "host_identity_sha256": test_digest('2'),
        "platform": "test-platform",
        "device_identity_sha256": test_digest('3'),
        "driver_sha256": test_digest('4'),
        "corpus_sha256": test_digest('5'),
        "benchmark_settings_sha256": test_digest('6'),
    })
}

fn promotion_binding_digest(binding: &Value) -> String {
    stable_sha256(&format!(
        "{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}",
        binding["candidate_commit"].as_str().unwrap(),
        binding["worktree_clean"].as_bool().unwrap(),
        binding["engine_version"].as_str().unwrap(),
        binding["v630_baseline_commit"].as_str().unwrap(),
        binding["package_binary_sha256"].as_str().unwrap(),
        binding["installed_binary_sha256"].as_str().unwrap(),
        binding["host_identity_sha256"].as_str().unwrap(),
        binding["platform"].as_str().unwrap(),
        binding["device_identity_sha256"].as_str().unwrap(),
        binding["driver_sha256"].as_str().unwrap(),
        binding["corpus_sha256"].as_str().unwrap(),
        binding["benchmark_settings_sha256"].as_str().unwrap(),
    ))
}

fn test_digest(byte: char) -> String {
    format!("sha256:{}", byte.to_string().repeat(64))
}

fn promotion_requirements() -> [(&'static str, &'static [(&'static str, ReceiptClass)]); 13] {
    [
        ("v630_frozen", &[("baseline", ReceiptClass::Baseline)]),
        (
            "namespace_collision_free",
            &[("validation", ReceiptClass::Validation)],
        ),
        (
            "translation_witness_end_to_end",
            &[("closure", ReceiptClass::Closure)],
        ),
        (
            "world_budget_exact_2048",
            &[("validation", ReceiptClass::Validation)],
        ),
        (
            "world_distinctness",
            &[("validation", ReceiptClass::Validation)],
        ),
        (
            "gpu_mandatory_lane_coverage",
            &[("accelerator", ReceiptClass::Accelerator)],
        ),
        (
            "gpu_fault_contained",
            &[("fault", ReceiptClass::Accelerator)],
        ),
        (
            "media_source_authority",
            &[("source", ReceiptClass::MediaSource)],
        ),
        (
            "media_backends_observed",
            &[("backend", ReceiptClass::MediaBackend)],
        ),
        (
            "host_output_bound",
            &[("host-output", ReceiptClass::HostOutput)],
        ),
        (
            "paired_v630_v631_regression",
            &[("evaluation", ReceiptClass::ReleaseEvaluation)],
        ),
        (
            "full_validation",
            &[
                ("local-validation", ReceiptClass::Validation),
                ("remote-ci", ReceiptClass::RemoteCi),
            ],
        ),
        (
            "package_installed_host_pickup",
            &[
                ("package", ReceiptClass::Package),
                ("host-pickup", ReceiptClass::ToolExecution),
            ],
        ),
    ]
}

#[test]
#[cfg(unix)]
fn closed_stdout_is_a_diagnostic_error_not_a_panic() {
    let mut reader = Command::new(binary())
        .arg("lc631-unknown")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn pipe reader");
    let closed_pipe = reader.stdin.take().expect("pipe writer");
    assert_eq!(reader.wait().expect("wait for pipe reader").code(), Some(2));
    let output = Command::new(binary())
        .args(["lc631-rpa-doctor", "--prompt", "audit Rust Python assembly"])
        .stdout(Stdio::from(closed_pipe))
        .stderr(Stdio::piped())
        .output()
        .expect("run RPA doctor with unwritable stdout");

    assert_eq!(output.status.code(), Some(2));
    let error: Value = serde_json::from_slice(&output.stderr).expect("valid error JSON");
    assert!(error["error"]
        .as_str()
        .is_some_and(|message| message.starts_with("stdout_write:")));
}

#[test]
fn active_stop_hook_reentry_exits_cleanly_without_requiring_plugin_data() {
    let mut child = Command::new(binary())
        .arg("lc631-host-stop-hook")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn host stop hook");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(br#"{"session_id":"s","cwd":".","hook_event_name":"Stop","permission_mode":"default","stop_hook_active":true,"last_assistant_message":"retry"}"#)
        .unwrap();
    let output = child.wait_with_output().expect("wait for host stop hook");

    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid hook JSON");
    assert_eq!(json["continue"], true);
}

#[test]
fn tldg_doctor_requires_authenticated_builtin_execution_receipts() {
    let output = Command::new(binary())
        .args([
            "lc631-tldg-doctor",
            "--prompt",
            "説明:\n~~~rust\nfn main() {}\n~~~",
        ])
        .output()
        .expect("run TLDG doctor");
    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid TLDG JSON");
    assert_eq!(json["payload"]["local_source_ready"], false);
    assert_eq!(json["payload"]["builtin_profiles_validated"], false);
    assert_eq!(json["payload"]["external_backends_observed"], false);
    assert_eq!(json["payload"]["output"]["output_commit_allowed"], false);
}

#[test]
fn tldg_release_gate_is_fail_closed_for_unobserved_external_surfaces() {
    let output = Command::new(binary())
        .args([
            "lc631-tldg-release-gate",
            "--prompt",
            "Build a unified parser.",
        ])
        .output()
        .expect("run TLDG release gate");
    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid release JSON");
    assert_eq!(json["payload"]["local_source_implemented"], false);
    assert_eq!(json["payload"]["release_complete"], false);
    assert!(json["payload"]["blocked_reasons"]
        .as_array()
        .is_some_and(|reasons| !reasons.is_empty()));
}

#[test]
fn rpa_doctor_exposes_all_40_dependency_ordered_stages_and_fails_release_closed() {
    let output = Command::new(binary())
        .args(["lc631-rpa-doctor", "--prompt", "audit Rust Python assembly"])
        .output()
        .expect("run RPA doctor");
    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid RPA JSON");

    assert_eq!(json["payload"]["stages"].as_array().unwrap().len(), 40);
    assert_eq!(json["payload"]["wiring_complete"], true);
    assert_eq!(json["payload"]["dependency_order_valid"], true);
    assert_eq!(json["payload"]["release_complete"], false);
    assert_eq!(json["payload"]["creates_authority"], false);
    assert_eq!(json["payload"]["creates_output_commit"], false);
}

#[test]
fn tl_doctor_no_longer_auto_claims_an_exact_end_to_end_chain() {
    let output = Command::new(binary())
        .args([
            "lc631-tl-doctor",
            "--prompt",
            "Do not delete files. Delete all files.",
        ])
        .output()
        .expect("run connected TL doctor");
    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid TL JSON");
    assert_eq!(json["payload"]["source_roundtrip_exact"], true);
    assert_eq!(json["payload"]["chain_complete"], false);
    assert_eq!(json["payload"]["gate"], "clarify");
    assert_eq!(json["payload"]["geometry_direct_promotions"], 0);
}
