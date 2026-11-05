//! Actual CLI processes for the explicit Given-K6 profile, not NL/host closure.
use lc631_process_supervisor::{
    required_ram_bytes, run_bounded_with_resources_observed, RunRequest,
};
use lc631_runtime::resource::{
    Cancellation, Deadline, Digest32, ResourceAmount, ResourceBroker, ResourceKind, ResourceLimits,
    ResourceRequest,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    ffi::{OsStr, OsString},
    time::Duration,
};

const ROOT: &str = "0403016602030161030162";
const DROP: &str = "0403016601030161";
fn pin(n: u8) -> String {
    format!("{:02x}", n).repeat(32)
}
fn independent_sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn config() -> Value {
    json!({"schema_version":"epistesys-given-k6-path.v1", "root_hex":ROOT, "final_hex":ROOT,
      "edges":[], "namespace":{"id_hex":pin(6),"revision_hex":pin(7)},
      "scope":{"id_hex":pin(1),"baseline_hex":pin(2),"root_goal_hex":pin(3),"probes_hex":pin(4),"policy_hex":pin(5)},
      "probes":[{"obligation_hex":pin(10),"property_hex":pin(11),"path":[0],"kind":"symbol","weight_numerator":1,"weight_denominator":1,"hard":true},
                {"obligation_hex":pin(10),"property_hex":pin(12),"path":[1],"kind":"symbol","weight_numerator":3,"weight_denominator":1,"hard":true}]})
}
fn cli(args: &[&str], input: &[u8]) -> (Option<i32>, String, String) {
    let args: Vec<OsString> = args.iter().map(OsString::from).collect();
    let environment: Vec<_> = std::env::vars_os().collect();
    let cwd = std::env::current_dir().unwrap();
    let request = RunRequest {
        executable: OsStr::new(env!("CARGO_BIN_EXE_lc631")),
        arguments: &args,
        current_dir: &cwd,
        environment: &environment,
        stdin: Some(input),
        timeout: Duration::from_secs(20),
        stdout_limit: 65536,
        stderr_limit: 16384,
    };
    let ram = required_ram_bytes(&request).unwrap();
    let limits = ResourceLimits::uniform(ram.max(1024 * 1024)).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let mut run = broker
        .begin_run(
            381,
            Digest32::from_bytes([38; 32]),
            limits,
            Deadline::after(Duration::from_secs(25)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let amount = |n| ResourceAmount::checked(n).unwrap();
    let mut reservation = run
        .reserve(&[
            ResourceRequest::new(ResourceKind::Ram, amount(ram)),
            ResourceRequest::new(ResourceKind::Output, amount(81920)),
            ResourceRequest::new(ResourceKind::Child, amount(1)),
            ResourceRequest::new(ResourceKind::Cleanup, amount(1)),
        ])
        .unwrap();
    let answer = {
        let view = reservation.lease();
        let observed = run_bounded_with_resources_observed(request, &view).unwrap();
        let out = observed.outcome();
        assert!(
            !observed.cancelled()
                && !out.timed_out
                && !out.stdout_truncated
                && !out.stderr_truncated
        );
        assert!(observed.cleanup_complete());
        (
            out.exit_code,
            String::from_utf8(out.stdout.clone()).unwrap(),
            String::from_utf8(out.stderr.clone()).unwrap(),
        )
    };
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
    answer
}
fn run(value: &Value) -> (Option<i32>, String, String) {
    cli(
        &["epistesys-path-check", "--config-stdin"],
        value.to_string().as_bytes(),
    )
}
#[test]
fn tlcx_path_cli_drop_restore_fresh_final_and_unverified_boundaries() {
    let mut c = config();
    c["edges"] = json!([{"from_hex":ROOT,"to_hex":DROP,"namespace":{"id_hex":pin(6),"revision_hex":pin(7)}},{"from_hex":DROP,"to_hex":ROOT,"namespace":{"id_hex":pin(6),"revision_hex":pin(7)}}]);
    let (exit, out, err) = run(&c);
    assert_eq!(exit, Some(0), "{err}");
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["history"][0]["loss"]["lower"], "3/4");
    assert_eq!(v["final_observation"]["loss"]["lower"], "0");
    assert_eq!(v["actual_evaluation_count"], 6);
    assert_eq!(v["gate"], "pass_in_given_finite_profile");
    assert_eq!(v["input_basis"], "given");
    assert_eq!(v["semantic_validation"], "unverified");
    assert_eq!(v["authority_created"], false);
    assert_eq!(v["host_final_bound"], false);
    assert_eq!(v["original_text_to_k6_verified"], false);
    assert_eq!(v["unique_defect_count"], 1);
    let expected = |artifact_hex: &str| {
        independent_sha256(
            format!("epistesys.given-k6-path.artifact.v1\0{artifact_hex}").as_bytes(),
        )
    };
    assert_eq!(v["root_artifact_digest_hex"], expected(ROOT));
    assert_eq!(v["final_artifact_digest_hex"], expected(ROOT));
    assert_eq!(
        v["history"][0]["target_artifact_digest_hex"],
        expected(DROP)
    );
    assert_eq!(
        v["history"][1]["target_artifact_digest_hex"],
        expected(ROOT)
    );
    assert_eq!(
        v["final_observation"]["target_artifact_digest_hex"],
        expected(ROOT)
    );
}
#[test]
fn tlcx_path_cli_hard_violation_and_unsupported_are_not_zero() {
    let mut c = config();
    c["edges"] = json!([{"from_hex":ROOT,"to_hex":DROP,"namespace":{"id_hex":pin(6),"revision_hex":pin(7)}}]);
    c["final_hex"] = json!(DROP);
    let (exit, out, err) = run(&c);
    assert_eq!(exit, Some(0), "{err}");
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["gate"], "fail_in_given_finite_profile");
    assert_eq!(v["final_observation"]["loss"]["lower"], "3/4");
    c["edges"] = json!([{"from_hex":ROOT,"to_hex":"ff","namespace":{"id_hex":pin(6),"revision_hex":pin(7)}}]);
    c["final_hex"] = json!("ff");
    let (exit, out, err) = run(&c);
    assert_eq!(exit, Some(0), "{err}");
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["gate"], "needs_evidence");
    assert!(v["final_observation"]["loss"].is_null());
}
#[test]
fn tlcx_path_cli_rejects_bad_config_zero_pin_chain_and_no_explicit_mode() {
    for input in [b"{".as_slice(), b"{}".as_slice()] {
        let (exit, out, _) = cli(&["epistesys-path-check", "--config-stdin"], input);
        assert!(exit.is_some_and(|v| v != 0));
        assert!(out.is_empty());
    }
    let mut c = config();
    c["unexpected"] = json!(true);
    let (exit, _, _) = run(&c);
    assert!(exit.is_some_and(|v| v != 0));
    let mut c = config();
    c["namespace"]["id_hex"] = json!(pin(0));
    let (exit, _, _) = run(&c);
    assert!(exit.is_some_and(|v| v != 0));
    let mut c = config();
    c["edges"] = json!([{"from_hex":DROP,"to_hex":ROOT,"namespace":{"id_hex":pin(6),"revision_hex":pin(7)}}]);
    let (exit, _, _) = run(&c);
    assert!(exit.is_some_and(|v| v != 0));
    let mut c = config();
    c["edges"] = json!([{"from_hex":ROOT,"to_hex":ROOT,"namespace":{"id_hex":pin(9),"revision_hex":pin(7)}}]);
    let (exit, _, _) = run(&c);
    assert!(exit.is_some_and(|v| v != 0));
    let mut c = config();
    c["final_hex"] = json!(DROP);
    let (exit, out, _) = run(&c);
    assert!(exit.is_some_and(|v| v != 0));
    assert!(out.is_empty());
    let (exit, out, _) = cli(&["epistesys-path-check"], config().to_string().as_bytes());
    assert!(exit.is_some_and(|v| v != 0));
    assert!(out.is_empty());
}
#[test]
fn tlcx_path_cli_exact_raw_config_identity_changes_without_changing_artifact_pin() {
    let raw = config().to_string();
    let with_space = format!("{raw}\n");
    let (a, out_a, err_a) = cli(&["epistesys-path-check", "--config-stdin"], raw.as_bytes());
    let (b, out_b, err_b) = cli(
        &["epistesys-path-check", "--config-stdin"],
        with_space.as_bytes(),
    );
    assert_eq!(a, Some(0), "{err_a}");
    assert_eq!(b, Some(0), "{err_b}");
    let va: Value = serde_json::from_str(&out_a).unwrap();
    let vb: Value = serde_json::from_str(&out_b).unwrap();
    assert_eq!(
        va["source_revision_hex"],
        independent_sha256(raw.as_bytes())
    );
    assert_eq!(
        vb["source_revision_hex"],
        independent_sha256(with_space.as_bytes())
    );
    assert_ne!(va["source_revision_hex"], vb["source_revision_hex"]);
    assert_eq!(
        va["root_artifact_digest_hex"],
        vb["root_artifact_digest_hex"]
    );
}
#[test]
fn tlcx_path_cli_rejects_oversized_stdin_before_parsing() {
    let input = vec![b' '; 256 * 1024 + 1];
    let (exit, out, _) = cli(&["epistesys-path-check", "--config-stdin"], &input);
    assert!(exit.is_some_and(|v| v != 0));
    assert!(out.is_empty());
}
