//! Actual bounded CLI processes; caller-Given finite tuple, never NL/host truth.
use lc631_process_supervisor::{
    required_ram_bytes, run_bounded_with_resources_observed, RunRequest,
};
use lc631_runtime::resource::*;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    ffi::{OsStr, OsString},
    time::Duration,
};

fn config() -> Value {
    let trace = json!({"times_ns":[0,10,20,30],"values":["true","true","true","true"]});
    let objects = json!({"left":{"min":[0,0],"max":[1,1]},"right":{"min":[3,0],"max":[4,1]}});
    json!({"schema_version":"epistesys-given-tl-tuple.v1","clock_label":"declared-clock","frame_label":"declared-frame","unit":"pixel","declare_identity_map":true,"spatial_probe":"strict_left","root":{"temporal":trace.clone(),"spatial":objects.clone()},"target":{"temporal":trace,"spatial":objects}})
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
            319,
            Digest32::from_bytes([19; 32]),
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
    let result = {
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
    result
}
fn evaluate(value: &Value) -> Value {
    let raw = value.to_string();
    let (exit, out, err) = cli(&["epistesys-tl-check", "--config-stdin"], raw.as_bytes());
    assert_eq!(exit, Some(0), "{err}");
    serde_json::from_str(&out).unwrap()
}
#[test]
fn tlcx_checked_tl_cli_actual_fixed_tuple_gold_and_raw_source_identity() {
    let mut c = config();
    let raw = format!("  {}\n", c);
    let (exit, out, err) = cli(&["epistesys-tl-check", "--config-stdin"], raw.as_bytes());
    assert_eq!(exit, Some(0), "{err}");
    let v: Value = serde_json::from_str(&out).unwrap();
    let digest: String = Sha256::digest(raw.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(v["source_revision_hex"], digest);
    assert_eq!(v["total"]["lower"], "0");
    assert_eq!(v["total"]["upper"], "0");
    assert_eq!(v["denominator"], 4);
    assert_eq!(v["weights"], json!([1, 3]));
    assert_eq!(v["gates"], json!(["pass", "pass"]));
    assert_eq!(v["actual_temporal_observations"], 2);
    assert_eq!(v["actual_spatial_observations"], 8);
    for field in [
        "authority_created",
        "output_commit_allowed",
        "host_final_bound",
        "source_semantics_verified",
        "verified_eligibility",
    ] {
        assert_eq!(v[field], false, "{field}");
    }
    assert_eq!(v["input_basis"], "given");
    assert_eq!(v["cross_domain_association"], "given_unverified");
    c["target"]["temporal"]["values"] = json!(["false", "false", "false", "false"]);
    let left = c["target"]["spatial"]["left"].clone();
    c["target"]["spatial"]["left"] = c["target"]["spatial"]["right"].clone();
    c["target"]["spatial"]["right"] = left;
    let v = evaluate(&c);
    assert_eq!(v["temporal"]["lower"], "1/4");
    assert_eq!(v["spatial"]["lower"], "1");
    assert_eq!(v["total"]["lower"], "13/16");
    assert_eq!(v["gates"], json!(["fail", "fail"]));
}
#[test]
fn tlcx_checked_tl_cli_unknown_unavailable_and_unsupported_are_not_zero() {
    let mut c = config();
    c["target"]["temporal"]["values"] = json!(["unknown", "unknown", "unknown", "unknown"]);
    let v = evaluate(&c);
    assert_eq!(v["total"]["lower"], "0");
    assert_eq!(v["total"]["upper"], "1/16");
    assert_eq!(v["gates"][0], "needs_evidence");
    c["declare_identity_map"] = json!(false);
    let v = evaluate(&c);
    assert!(v["total"].is_null());
    assert!(v["gaps"][0].as_str().unwrap().contains("Uncalibrated"));
    let mut c = config();
    c["spatial_probe"] = json!("continuous_distance");
    let v = evaluate(&c);
    assert!(v["total"].is_null());
    assert!(v["gaps"][1].as_str().unwrap().contains("Unsupported"));
    assert_eq!(v["denominator"], 4);
    let mut c = config();
    c["target"]["spatial"]["right"] = Value::Null;
    let v = evaluate(&c);
    assert_eq!(v["total"]["upper"], "3/4");
    assert_eq!(v["gates"][1], "needs_evidence");
}
#[test]
fn tlcx_checked_tl_cli_rejects_unbounded_malformed_injected_and_unselected_inputs() {
    for input in [
        b"{".as_slice(),
        b"{}".as_slice(),
        &[0xff, 0xfe],
        &vec![b' '; 16385],
    ] {
        let (exit, out, _) = cli(&["epistesys-tl-check", "--config-stdin"], input);
        assert!(exit.is_some_and(|c| c != 0));
        assert!(out.is_empty());
    }
    for field in [
        "weights",
        "mask",
        "total_loss",
        "hard_gate",
        "authority_created",
    ] {
        let mut c = config();
        c[field] = json!(true);
        let (exit, out, _) = cli(
            &["epistesys-tl-check", "--config-stdin"],
            c.to_string().as_bytes(),
        );
        assert!(exit.is_some_and(|c| c != 0), "{field}");
        assert!(out.is_empty());
    }
    for args in [
        vec!["epistesys-tl-check"],
        vec!["epistesys-tl-check", "--config-stdin", "--config-stdin"],
        vec!["epistesys-tl-check", "--prompt-stdin"],
    ] {
        let (exit, _, _) = cli(&args, config().to_string().as_bytes());
        assert!(exit.is_some_and(|c| c != 0));
    }
    for mutation in 0..4 {
        let mut c = config();
        match mutation {
            0 => c["root"]["temporal"]["times_ns"] = json!([0, 0, 1, 2]),
            1 => c["target"]["temporal"]["values"] = json!(["true"]),
            2 => c["target"]["spatial"]["left"]["min"] = json!([2, 2]),
            _ => c["clock_label"] = json!("x".repeat(129)),
        };
        let (exit, out, _) = cli(
            &["epistesys-tl-check", "--config-stdin"],
            c.to_string().as_bytes(),
        );
        assert!(exit.is_some_and(|c| c != 0), "mutation{mutation}");
        assert!(out.is_empty());
    }
}
