#![forbid(unsafe_code)]

use lc631_core::{stable_sha256, LC631_VERSION};
use lc631_host::HostReceiptContext;
use lc631_receipt_kernel::{ReceiptClass, ReceiptScope, SubjectRevision};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_lc631")
}

fn dgcl_connection_plan() -> (PathBuf, lc631_analysis::CodingConnectionPlan) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .unwrap()
        .canonicalize()
        .unwrap();
    let relative_binary = Path::new(binary())
        .canonicalize()
        .unwrap()
        .strip_prefix(&root)
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/");
    let digest = |path: &Path| {
        let bytes = fs::read(path).unwrap();
        let mut value = String::from("sha256:");
        for byte in Sha256::digest(bytes) {
            use std::fmt::Write as _;
            write!(value, "{byte:02x}").unwrap();
        }
        value
    };
    let implementation_path = "scripts/lc631/crates/lc631-tldg/src/dg1.rs";
    let router_path = "scripts/lc631/crates/lc631-cli/src/lib.rs";
    let source = "Please parse the DG1 source.";
    (
        root.clone(),
        lc631_analysis::CodingConnectionPlan {
            schema_version: "epistesys-coding-connection-plan.v1".into(),
            source: source.into(),
            source_revision: stable_sha256(source),
            requirement_id: 1,
            coding_requirement_id: None,
            coding_task_id: None,
            coding_target_ref: None,
            coding_target_digest: None,
            expected_polarity: "positive".into(),
            expected_condition_kind: None,
            expected_scope: None,
            implementation_path: implementation_path.into(),
            implementation_digest: digest(&root.join(implementation_path)),
            implementation_symbol: "analyze_dg1".into(),
            router_path: router_path.into(),
            router_digest: digest(&root.join(router_path)),
            executable_path: relative_binary.clone(),
            executable_digest: digest(&root.join(relative_binary)),
            command: "lc631-dg1-doctor".into(),
            arguments: vec!["--prompt".into(), source.into()],
            expected_json_pointer: "/payload/schema_version".into(),
            expected_json_value: Value::String("epistesys-dg1-parse.v1".into()),
        },
    )
}

#[test]
fn dgcl_22_production_connection_mutations_fail_and_positive_route_observes() {
    use lc631_analysis::{
        verify_connection_plan, verify_connection_plan_from_cli, ConnectionError, ConnectionStatus,
    };
    let (root, plan) = dgcl_connection_plan();
    assert_eq!(
        verify_connection_plan(&root, &plan).unwrap().status,
        ConnectionStatus::Held
    );
    let static_only = verify_connection_plan(&root, &plan).unwrap();
    assert!(!static_only.runtime_command_observed);
    assert_eq!(static_only.process.exit_code, None);
    assert_eq!(
        static_only.connection_graph.route_subjects,
        vec!["command.as_str()"]
    );
    assert!(static_only.connection_graph.route_input_bound);
    assert_eq!(
        static_only
            .connection_graph
            .requirement_target_binding_state,
        lc631_analysis::StaticConnectionState::Unresolved
    );

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let plan_path = root
        .join("scripts/lc631/target")
        .join(format!("dgcl-registered-plan-{unique}.json"));
    fs::write(&plan_path, serde_json::to_vec(&plan).unwrap()).unwrap();
    let output = Command::new(binary())
        .args([
            "lc631-dgcl-verify",
            "--execute",
            "--repo",
            root.to_str().unwrap(),
            "--connection-plan",
            plan_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let _ = fs::remove_file(&plan_path);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(wire["payload"]["status"], "observed");
    assert_eq!(wire["payload"]["runtime_command_observed"], true);
    assert_eq!(
        wire["payload"]["connection_graph"]["schema_version"],
        "epistesys-static-connection-graph.v1"
    );
    assert_eq!(wire["payload"]["connection_graph"]["state"], "connected");
    assert_eq!(
        wire["payload"]["connection_graph"]["requirement_target_binding_state"],
        "unresolved"
    );
    assert_eq!(
        wire["payload"]["connection_graph"]["repository_wide_complete"],
        false
    );
    assert!(wire["payload"]["connection_graph"]["edges"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|edge| edge["kind"] != "requirement_target_candidate")
        .all(|edge| edge["state"] == "connected"));
    let trace = &wire["payload"]["runtime_trace"];
    assert!(
        trace.is_object(),
        "only a real registered run creates a trace"
    );
    assert_eq!(trace["source_revision"], plan.source_revision);
    assert_eq!(trace["requirement_id"], plan.requirement_id);
    assert_eq!(trace["input_digest"], plan.source_revision);
    assert!(trace["trace_digest"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));
    assert_eq!(trace["acceptance_oracle"]["candidate_generated"], false);
    assert_eq!(
        trace["acceptance_oracle"]["independent_semantic_gold"],
        false
    );
    assert_eq!(trace["network_os_enforced"], false);
    assert_eq!(trace["filesystem_write_monitor"], "not_observed");
    let first_run_id = trace["run_id"].as_str().unwrap().to_string();
    let first_trace_digest = trace["trace_digest"].as_str().unwrap().to_string();
    fs::write(&plan_path, serde_json::to_vec(&plan).unwrap()).unwrap();
    let repeated = Command::new(binary())
        .args([
            "lc631-dgcl-verify",
            "--execute",
            "--repo",
            root.to_str().unwrap(),
            "--connection-plan",
            plan_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let _ = fs::remove_file(&plan_path);
    assert!(repeated.status.success());
    let repeated_wire: Value = serde_json::from_slice(&repeated.stdout).unwrap();
    assert_ne!(
        repeated_wire["payload"]["runtime_trace"]["run_id"], first_run_id,
        "identical JSON from another run is a distinct trace"
    );
    assert_ne!(
        repeated_wire["payload"]["runtime_trace"]["trace_digest"],
        first_trace_digest
    );

    let mut missing_symbol = plan.clone();
    missing_symbol.implementation_symbol = "nonexistent_dgcl_symbol".into();
    assert_eq!(
        verify_connection_plan(&root, &missing_symbol)
            .unwrap()
            .status,
        ConnectionStatus::Held
    );

    let mut wrong_output = plan.clone();
    wrong_output.expected_json_pointer = "/payload/no_such_value".into();
    assert_eq!(
        verify_connection_plan(&root, &wrong_output).unwrap().status,
        ConnectionStatus::Held
    );
    assert!(matches!(
        verify_connection_plan_from_cli(&root, &wrong_output),
        Err(ConnectionError::CommandNotRegistered)
    ));

    let mut changed_binary = plan.clone();
    changed_binary.executable_digest = stable_sha256("wrong binary");
    assert!(matches!(
        verify_connection_plan(&root, &changed_binary),
        Err(ConnectionError::StaleArtifact)
    ));

    let mut wrong_polarity = plan.clone();
    wrong_polarity.expected_polarity = "negative".into();
    assert!(matches!(
        verify_connection_plan(&root, &wrong_polarity),
        Err(ConnectionError::InvalidPlan)
    ));

    let mut unregistered = plan.clone();
    unregistered.command = "lc631-not-registered".into();
    assert_eq!(
        verify_connection_plan(&root, &unregistered).unwrap().status,
        ConnectionStatus::Held
    );
    let unregistered_report = verify_connection_plan(&root, &unregistered).unwrap();
    assert!(!unregistered_report.runtime_command_observed);
    assert_eq!(unregistered_report.process.exit_code, None);

    let mut path_escape = plan;
    path_escape.implementation_path = "../other.rs".into();
    assert!(matches!(
        verify_connection_plan(&root, &path_escape),
        Err(ConnectionError::PathUnsafe)
    ));
}

#[test]
fn static_connection_observer_never_executes_a_plan_supplied_command() {
    use lc631_analysis::{
        observe_connection_plan_from_cli, verify_connection_plan, verify_connection_plan_from_cli,
        ConnectionError,
    };
    let (root, mut plan) = dgcl_connection_plan();
    plan.command = "unregistered-user-binary".into();
    plan.arguments = vec!["--execute-anything".into()];
    let report = verify_connection_plan(&root, &plan).unwrap();
    assert_eq!(report.status, lc631_analysis::ConnectionStatus::Held);
    assert!(!report.runtime_command_observed);
    assert_eq!(report.process.exit_code, None);
    assert_eq!(
        report.process.diagnostic.as_deref(),
        Some("execution_not_requested_by_static_plan_observer")
    );
    assert!(matches!(
        verify_connection_plan_from_cli(&root, &plan),
        Err(ConnectionError::CommandNotRegistered)
    ));
    assert!(matches!(
        observe_connection_plan_from_cli(&root, &plan),
        Err(ConnectionError::CommandNotRegistered)
    ));

    let (root, plan) = dgcl_connection_plan();
    assert!(matches!(
        verify_connection_plan_from_cli(&root, &plan),
        Err(ConnectionError::ExecutableNotCurrent)
    ));
    assert!(matches!(
        observe_connection_plan_from_cli(&root, &plan),
        Err(ConnectionError::ExecutableNotCurrent)
    ));
}

#[test]
fn dgcl_finalizer_rejects_missing_receipt_root_before_delivery_or_checkpoint() {
    let (root, plan) = dgcl_connection_plan();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let test_root = root
        .join("scripts/lc631/target")
        .join(format!("dgcl-preflight-{}-{unique}", std::process::id()));
    fs::create_dir(&test_root).unwrap();
    let plan_path = test_root.join("plan.json");
    let ledger_root = test_root.join("ledger");
    fs::write(&plan_path, serde_json::to_vec(&plan).unwrap()).unwrap();
    let output = Command::new(binary())
        .args([
            "lc631-dgcl-finalize",
            "--execute",
            "--repo",
            root.to_str().unwrap(),
            "--connection-plan",
            plan_path.to_str().unwrap(),
            "--ledger-root",
            ledger_root.to_str().unwrap(),
            "--authority-revision",
            "test-revision",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        !ledger_root.exists(),
        "preflight must not write a checkpoint"
    );

    let missing_receipt_root = test_root.join("unprovisioned-receipt-root");
    let missing_root_ledger = test_root.join("missing-root-ledger");
    let no_trust_root = Command::new(binary())
        .args([
            "lc631-dgcl-finalize",
            "--execute",
            "--repo",
            root.to_str().unwrap(),
            "--connection-plan",
            plan_path.to_str().unwrap(),
            "--ledger-root",
            missing_root_ledger.to_str().unwrap(),
            "--authority-revision",
            "test-revision",
            "--receipt-root",
            missing_receipt_root.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!no_trust_root.status.success());
    assert!(!missing_receipt_root.exists());
    assert!(!missing_root_ledger.exists());

    let no_execute = Command::new(binary())
        .args([
            "lc631-dgcl-verify",
            "--repo",
            root.to_str().unwrap(),
            "--connection-plan",
            plan_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!no_execute.status.success());
    assert!(String::from_utf8_lossy(&no_execute.stderr).contains("requires_--execute"));
}

#[test]
fn dgcl_connection_observation_cannot_mint_compiler_runtime_or_acceptance_evidence() {
    let (root, plan) = dgcl_connection_plan();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let test_root = root
        .join("scripts/lc631/target")
        .join(format!("dgcl-evidence-producer-{unique}"));
    fs::create_dir(&test_root).unwrap();
    let plan_path = test_root.join("plan.json");
    let ledger_root = test_root.join("ledger");
    let receipt_root = test_root.join("receipt-root");
    HostReceiptContext::load_or_create(&receipt_root).unwrap();
    fs::write(&plan_path, serde_json::to_vec(&plan).unwrap()).unwrap();
    let output = Command::new(binary())
        .args([
            "lc631-dgcl-finalize",
            "--execute",
            "--repo",
            root.to_str().unwrap(),
            "--connection-plan",
            plan_path.to_str().unwrap(),
            "--ledger-root",
            ledger_root.to_str().unwrap(),
            "--authority-revision",
            "producer-test",
            "--receipt-root",
            receipt_root.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let _ = fs::remove_file(&plan_path);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
    let gaps = wire["payload"]["closure"]["gaps"].as_array().unwrap();
    let verified = gaps
        .iter()
        .filter(|gap| gap["state"] == "verified_receipt")
        .collect::<Vec<_>>();
    assert_eq!(
        verified.len(),
        1,
        "one route observation cannot mint five kinds"
    );
    assert_eq!(verified[0]["kind"], "production_connection");
    assert!(gaps
        .iter()
        .any(|gap| { gap["kind"] == "static_validation" && gap["state"] == "unavailable" }));
    assert!(gaps
        .iter()
        .any(|gap| { gap["kind"] == "runtime_validation" && gap["state"] == "unavailable" }));
    assert!(gaps
        .iter()
        .any(|gap| { gap["kind"] == "acceptance_test" && gap["state"] == "unavailable" }));
    assert_eq!(wire["payload"]["closure"]["authority_created"], false);
    assert_eq!(wire["payload"]["closure"]["output_commit_allowed"], false);
    let candidate = &wire["payload"]["standalone_candidate"];
    assert_eq!(
        candidate["schema_version"],
        "epistesys-dgcl-standalone-candidate.v1"
    );
    assert_eq!(candidate["candidate_state"], "exact_candidate_captured");
    assert_eq!(
        candidate["candidate_digest"],
        wire["payload"]["connection"]["process"]["stdout_digest"]
    );
    assert_eq!(candidate["observed_schema_version"], "lc631-wire.v1");
    assert_eq!(candidate["host_send_authorized"], false);
    assert_eq!(candidate["output_commit_allowed"], false);
    assert_eq!(candidate["host_observation"]["precommit"], "pending");
    assert_eq!(candidate["host_observation"]["post_send"], "pending");
    assert_eq!(candidate["host_observation"]["sink_delivery"], "pending");
    assert_eq!(candidate["host_observation"]["durable_replay"], "pending");
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
fn dgcl_23_production_cli_exposes_dg1_residuals_and_never_promotes_authority() {
    let source = "Do not delete source. Quoted text: \"delete source\".";
    let output = Command::new(binary())
        .args(["lc631-dg1-doctor", "--prompt", source])
        .output()
        .expect("run DG1 production CLI");
    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid DG1 JSON");
    assert_eq!(json["schema_version"], "lc631-wire.v1");
    assert_eq!(json["payload"]["schema_version"], "epistesys-dg1-parse.v1");
    assert_eq!(
        json["payload"]["requirement_candidates"][0]["authority_grant"],
        false
    );
    assert_eq!(
        json["payload"]["semantic_backend_state"],
        "unavailable_requires_explicit_external_backend"
    );
    let accounting = json["payload"]["source_accounting"]
        .as_array()
        .expect("source accounting is emitted");
    let mut cursor = 0;
    let mut reconstructed = String::new();
    for segment in accounting {
        let start = segment["source_span"]["start"].as_u64().unwrap() as usize;
        let end = segment["source_span"]["end"].as_u64().unwrap() as usize;
        assert_eq!(start, cursor);
        reconstructed.push_str(source.get(start..end).expect("UTF-8 aligned span"));
        cursor = end;
    }
    assert_eq!(cursor, source.len());
    assert_eq!(reconstructed, source);
    assert!(json["payload"]["language_span_lattice"].is_array());
}

#[test]
fn controlled_instruction_cli_retains_ambiguous_routes_without_selecting_one() {
    let output = Command::new(binary())
        .args([
            "lc631-instruction-parse",
            "--prompt",
            "Please testしてください",
        ])
        .output()
        .expect("run controlled instruction parser");
    assert!(output.status.success());
    let wire: Value = serde_json::from_slice(&output.stdout).expect("valid instruction JSON");
    assert_eq!(
        wire["payload"]["schema_version"],
        "epistesys-instruction-parse.v1"
    );
    assert_eq!(wire["payload"]["state"], "ambiguous");
    assert!(wire["payload"]["selected"].is_null());
    assert_eq!(wire["payload"]["alternatives"].as_array().unwrap().len(), 2);
}

#[test]
fn dgcl_shadow_parse_cli_exposes_controlled_parse_and_legacy_only_candidate() {
    let source = "Please delete files.\n\nテストを実行し";
    let output = Command::new(binary())
        .args(["lc631-dgcl-shadow-parse", "--prompt", source])
        .output()
        .expect("run DGCL grammar-only shadow route");
    assert!(output.status.success());
    let wire: Value = serde_json::from_slice(&output.stdout).expect("valid shadow JSON");
    let payload = &wire["payload"];
    assert_eq!(payload["schema_version"], "epistesys-dgcl-pipeline.v1");
    assert_eq!(payload["authority_created"], false);
    assert_eq!(
        payload["program_ir"]["schema_version"],
        "epistesys-dgcl-program-ir.v1"
    );
    assert_eq!(
        payload["translation_projection"]["schema_version"],
        "epistesys-dgcl-translation-projection.v1"
    );
    assert_eq!(
        payload["translation_projection"]["authority_created"],
        false
    );
    assert_eq!(
        payload["translation_projection"]["output_commit_allowed"],
        false
    );
    assert_eq!(
        payload["completion_plan"]["schema_version"],
        "epistesys-dgcl-completion-plan.v1"
    );
    assert_eq!(payload["completion_plan"]["evidence_issued"], false);
    assert_eq!(payload["completion_plan"]["coding_closure_allowed"], false);
    let parses = payload["instruction_parses"].as_array().unwrap();
    assert_eq!(parses.len(), 2);
    assert_eq!(parses[0]["state"], "parsed");
    assert_eq!(parses[1]["state"], "unsupported");
    assert_eq!(
        parses[0]["root_source_revision"],
        payload["source_revision"]
    );
}

#[test]
fn dgcl_run_returns_one_bound_pipeline_candidate_without_host_authority() {
    let source = "Please test the package.";
    let output = Command::new(binary())
        .args(["lc631-dgcl-run", "--prompt", source])
        .current_dir(std::env::temp_dir())
        .output()
        .expect("run standalone DGCL pipeline");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(envelope["command"], "lc631-dgcl-run");
    assert_eq!(
        envelope["payload"]["schema_version"],
        "epistesys-dgcl-standalone-run.v1"
    );
    let pipeline = &envelope["payload"]["pipeline"];
    let candidate = &envelope["payload"]["candidate"];
    let candidate_text = candidate["candidate_text"].as_str().unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(candidate_text).unwrap(),
        *pipeline
    );
    assert_eq!(candidate["candidate_digest"], stable_sha256(candidate_text));
    assert_eq!(candidate["implementation_closure_status"], "hold");
    assert_eq!(candidate["host_send_authorized"], false);
    assert_eq!(candidate["output_commit_allowed"], false);
    assert_eq!(candidate["authority_created"], false);
    assert_eq!(envelope["payload"]["host_send_authorized"], false);
}

#[test]
fn dgcl_run_configured_stanza_profile_returns_one_candidate_and_keeps_truth_unclaimed() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .unwrap()
        .canonicalize()
        .unwrap();
    let python = root.join(".cache/dgcl-python/Scripts/python.exe");
    let worker = root.join("scripts/lc631/workers/dgcl_nlp_worker.py");
    let model_cache = root.join(".cache/dgcl-models");
    let manifest_en = model_cache.join("manifest-en.json");
    let manifest_ja = model_cache.join("manifest-ja.json");
    if std::env::var("EPISTESYS_TEST_CONFIGURED_NLP").as_deref() != Ok("1") {
        // Portable source packages do not redistribute models. Exercise the
        // explicit missing-backend diagnostic instead of claiming a positive.
        let missing = root.join(".cache/absent-dgcl-python/python.exe");
        let output = Command::new(binary())
            .args([
                "lc631-dgcl-run",
                "--repo",
                root.to_str().unwrap(),
                "--prompt",
                "Please test the package.",
                "--python",
                missing.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(!output.status.success());
        let error: Value = serde_json::from_slice(&output.stderr).unwrap();
        assert!(error["error"].as_str().is_some());
        return;
    }
    for required in [&python, &worker, &manifest_en, &manifest_ja] {
        assert!(
            required.is_file(),
            "missing configured local input: {required:?}"
        );
    }
    let source = "Please test the package.\n\nテストしてください。";
    let output = Command::new(binary())
        .args([
            "lc631-dgcl-run",
            "--repo",
            root.to_str().unwrap(),
            "--prompt",
            source,
            "--python",
            python.to_str().unwrap(),
            "--python-digest",
            "sha256:0b471133e110cfb53a061cad528ce8e517d7b9ac41a0a396c39ad795a487fc14",
            "--backend-script",
            worker.to_str().unwrap(),
            "--backend-script-digest",
            "sha256:5e5189626dac84fa1d6ac6c8ff98b39dc8fe42baba474b7e723c65cc56f54c6c",
            "--model-cache",
            model_cache.to_str().unwrap(),
            "--model-manifest-en",
            manifest_en.to_str().unwrap(),
            "--model-digest-en",
            "sha256:d990c2e0c1a54b652433e8a58157d37d7989c9700aa2c979605f484c09389587",
            "--model-manifest-ja",
            manifest_ja.to_str().unwrap(),
            "--model-digest-ja",
            "sha256:c2a28c20213cc0c7e0b4b403d9d2b4842b729c93aad893d9c822b2e64857e755",
        ])
        .current_dir(std::env::temp_dir())
        .output()
        .expect("run configured Stanza DGCL profile");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
    let payload = &envelope["payload"];
    assert_eq!(
        payload["input_profile"],
        "dgcl-rust-cli-ja-en-alpha2-configured-stanza.v1"
    );
    assert_eq!(
        payload["pipeline"]["language_regions"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        payload["candidate"]["source_revision"],
        payload["pipeline"]["source_revision"]
    );
    assert_eq!(payload["candidate"]["host_send_authorized"], false);
    assert_eq!(payload["candidate"]["output_commit_allowed"], false);
    assert!(payload["pipeline"]["language_regions"]
        .as_array()
        .unwrap()
        .iter()
        .all(|region| region["semantic_truth_claim"] == false));
}

#[test]
fn dgcl_23_production_closure_cli_keeps_unbound_gaps_open() {
    let output = Command::new(binary())
        .args([
            "lc631-coding-closure",
            "--prompt",
            "Please update the implementation.",
        ])
        .output()
        .expect("run coding closure CLI");
    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid closure JSON");
    assert_eq!(
        json["payload"]["schema_version"],
        "epistesys-coding-closure.v1"
    );
    assert_eq!(json["payload"]["status"], "held");
    assert_eq!(json["payload"]["authority_created"], false);
    assert_eq!(json["payload"]["output_commit_allowed"], false);
    assert_eq!(json["payload"]["gaps"].as_array().unwrap().len(), 5);
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

#[test]
fn tl_doctor_accepts_prompt_over_stdin_without_command_line_content() {
    let mut child = Command::new(binary())
        .args(["lc631-tl-doctor", "--prompt-stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn TL doctor with stdin prompt");
    child
        .stdin
        .as_mut()
        .expect("child stdin")
        .write_all(b"Compare two interpretations without taking action.")
        .expect("write prompt to stdin");
    drop(child.stdin.take());

    let output = child.wait_with_output().expect("wait for TL doctor");
    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid TL JSON");
    assert_eq!(json["command"], "lc631-tl-doctor");
    assert_eq!(json["version"], LC631_VERSION);
    assert_eq!(json["shadow_only"], true);
}

#[test]
fn prompt_argument_and_stdin_flag_are_mutually_exclusive() {
    let output = Command::new(binary())
        .args([
            "lc631-tl-doctor",
            "--prompt",
            "placeholder prompt",
            "--prompt-stdin",
        ])
        .output()
        .expect("run TL doctor with conflicting input modes");
    assert!(!output.status.success());
    let json: Value = serde_json::from_slice(&output.stderr).expect("valid CLI error JSON");
    assert_eq!(
        json["error"],
        "use_exactly_one_of_--prompt_or_--prompt-stdin"
    );
}
