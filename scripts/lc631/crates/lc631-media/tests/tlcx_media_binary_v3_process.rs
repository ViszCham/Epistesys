#![forbid(unsafe_code)]

use lc631_media::attestation_v3::{
    write_execution_plan_v3, write_process_receipt_v3, CanonicalWriter,
};
use lc631_media::{MediaExecutionPlan, MediaProcessReceipt, MediaResourceBudget};

fn plan() -> MediaExecutionPlan {
    MediaExecutionPlan {
        source_snapshot_sha256: "source".into(),
        source_bytes: 10,
        python_sha256: "python".into(),
        backend_script_sha256: "script".into(),
        ffmpeg_sha256: "ffmpeg".into(),
        model_cache_manifest_sha256: "model".into(),
        configuration_digest: "config".into(),
        resource_budget: MediaResourceBudget::default(),
        network_policy: "offline".into(),
        network_isolation_os_enforced: false,
    }
}
fn process() -> MediaProcessReceipt {
    MediaProcessReceipt {
        exit_code: Some(0),
        stdout_digest: "stdout".into(),
        stderr_digest: "stderr".into(),
        stdout_truncated: false,
        stderr_truncated: false,
        process_tree_kill_attempted: false,
        process_tree_kill_verified: false,
        memory_limit_bytes: Some(1024),
        memory_limit_kind: "fixture".into(),
    }
}
fn encode_plan(value: &MediaExecutionPlan) -> Vec<u8> {
    let mut bytes = Vec::new();
    write_execution_plan_v3(&mut CanonicalWriter::new(&mut bytes, 4096), value).unwrap();
    bytes
}
fn encode_process(value: &MediaProcessReceipt) -> Vec<u8> {
    let mut bytes = Vec::new();
    write_process_receipt_v3(&mut CanonicalWriter::new(&mut bytes, 4096), value).unwrap();
    bytes
}

#[test]
fn each_plan_and_resource_field_is_bound() {
    let original = plan();
    let baseline = encode_plan(&original);
    let mut changes = Vec::new();
    let mut v = original.clone();
    v.source_snapshot_sha256.push('x');
    changes.push(v);
    let mut v = original.clone();
    v.source_bytes += 1;
    changes.push(v);
    let mut v = original.clone();
    v.python_sha256.push('x');
    changes.push(v);
    let mut v = original.clone();
    v.backend_script_sha256.push('x');
    changes.push(v);
    let mut v = original.clone();
    v.ffmpeg_sha256.push('x');
    changes.push(v);
    let mut v = original.clone();
    v.model_cache_manifest_sha256.push('x');
    changes.push(v);
    let mut v = original.clone();
    v.configuration_digest.push('x');
    changes.push(v);
    let mut v = original.clone();
    v.network_policy.push('x');
    changes.push(v);
    let mut v = original.clone();
    v.network_isolation_os_enforced = true;
    changes.push(v);
    let mut v = original.clone();
    v.resource_budget.max_stdout_bytes += 1;
    changes.push(v);
    let mut v = original.clone();
    v.resource_budget.max_stderr_bytes += 1;
    changes.push(v);
    let mut v = original.clone();
    v.resource_budget.max_temporary_bytes += 1;
    changes.push(v);
    let mut v = original.clone();
    v.resource_budget.max_decoded_frames += 1;
    changes.push(v);
    let mut v = original.clone();
    v.resource_budget.max_pcm_samples += 1;
    changes.push(v);
    let mut v = original.clone();
    v.resource_budget.max_observations += 1;
    changes.push(v);
    for (index, value) in changes.iter().enumerate() {
        assert_ne!(encode_plan(value), baseline, "plan field {index} omitted");
    }
}

#[test]
fn each_process_field_is_bound() {
    let original = process();
    let baseline = encode_process(&original);
    let mut changes = Vec::new();
    let mut v = original.clone();
    v.exit_code = None;
    changes.push(v);
    let mut v = original.clone();
    v.stdout_digest.push('x');
    changes.push(v);
    let mut v = original.clone();
    v.stderr_digest.push('x');
    changes.push(v);
    let mut v = original.clone();
    v.stdout_truncated = true;
    changes.push(v);
    let mut v = original.clone();
    v.stderr_truncated = true;
    changes.push(v);
    let mut v = original.clone();
    v.process_tree_kill_attempted = true;
    changes.push(v);
    let mut v = original.clone();
    v.process_tree_kill_verified = true;
    changes.push(v);
    let mut v = original.clone();
    v.memory_limit_bytes = None;
    changes.push(v);
    let mut v = original.clone();
    v.memory_limit_kind.push('x');
    changes.push(v);
    for (index, value) in changes.iter().enumerate() {
        assert_ne!(
            encode_process(value),
            baseline,
            "process field {index} omitted"
        );
    }
}

#[test]
fn capacity_error_is_not_a_successful_encoding() {
    let mut bytes = Vec::new();
    assert!(write_execution_plan_v3(&mut CanonicalWriter::new(&mut bytes, 1), &plan()).is_err());
    assert!(
        write_process_receipt_v3(&mut CanonicalWriter::new(&mut bytes, 1), &process()).is_err()
    );
}

#[test]
fn minimal_process_has_literal_binary_vector() {
    let mut value = process();
    value.exit_code = None;
    value.stdout_digest.clear();
    value.stderr_digest.clear();
    value.memory_limit_bytes = None;
    value.memory_limit_kind.clear();
    let expected = [
        0x17, // absent exit code
        1, 0, 0, 0, 0, 0, 0, 0, 0, // stdout
        1, 0, 0, 0, 0, 0, 0, 0, 0, // stderr
        0x13, 0, 0x13, 0, 0x13, 0, 0x13, 0,    // flags
        0x15, // absent memory limit
        1, 0, 0, 0, 0, 0, 0, 0, 0, // limit kind
    ];
    assert_eq!(encode_process(&value), expected);
}
