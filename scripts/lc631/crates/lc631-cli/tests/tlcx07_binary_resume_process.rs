//! Actual CLI dispatch and bounded stdout; never completion or host evidence.
use lc631_analysis::{CheckpointBody, CheckpointEventKind, DeliveryState};
use lc631_core::stable_sha256;
use lc631_host::{
    HostJournalBundleSession, HostPersistentStoreOwner, JournalBundleLimits, PersistentStoreLimits,
};
use lc631_process_supervisor::{
    required_ram_bytes, run_bounded_with_resources_observed, RunRequest,
};
use lc631_runtime::journal::{
    JournalBundleLiveSession, JournalChainLimits, JournalCursor, JournalRecordKind,
    JournalSegmentBuilder,
};
use lc631_runtime::resource::{
    Cancellation, Deadline, Digest32, Reservation, ResourceAmount, ResourceBroker, ResourceKind,
    ResourceLimits, ResourceRequest, RunResources,
};
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn resources(id: u64, ram: u64, output: u64) -> (ResourceBroker, RunResources, Reservation) {
    let limits = ResourceLimits::uniform(ram.max(output).max(1024 * 1024)).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let run = broker
        .begin_run(
            id,
            Digest32::from_bytes([7; 32]),
            limits,
            Deadline::after(Duration::from_secs(30)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let amount = |n| ResourceAmount::checked(n).unwrap();
    let lease = run
        .reserve(&[
            ResourceRequest::new(ResourceKind::Ram, amount(ram)),
            ResourceRequest::new(ResourceKind::Output, amount(output)),
            ResourceRequest::new(ResourceKind::Disk, amount(1024 * 1024)),
            ResourceRequest::new(ResourceKind::Child, amount(1)),
            ResourceRequest::new(ResourceKind::Cleanup, amount(1)),
        ])
        .unwrap();
    (broker, run, lease)
}

fn root() -> PathBuf {
    let base = std::env::temp_dir().canonicalize().unwrap();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    for attempt in 0..32 {
        let root = base.join(format!(
            "epistesys-tlcx07-cli-process-{}-{nonce}-{attempt}",
            std::process::id()
        ));
        match std::fs::create_dir(&root) {
            Ok(()) => {
                eprintln!("retained owned CLI root: {}", root.display());
                return root;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => panic!("owned CLI root: {error}"),
        }
    }
    panic!("owned CLI root unavailable")
}

fn persist(root: &Path, file_sequence: u64, source: &str, plan: &str) -> Vec<u8> {
    let body = CheckpointBody {
        schema_version: "epistesys-dgcl-checkpoint.v1".into(),
        sequence: 0,
        source_revision: source.into(),
        plan_digest: plan.into(),
        authority_revision: "authority-current".into(),
        action_key: "fixture-action".into(),
        event_kind: CheckpointEventKind::LegacyUnclassified,
        delivery: DeliveryState::ConfirmedSuccess,
        result_digest: Some(stable_sha256("fixture-result")),
        parent_digest: None,
    };
    let digest = stable_sha256(&serde_json::to_string(&body).unwrap());
    let payload = serde_json::to_vec(&serde_json::json!({"body":body,"digest":digest})).unwrap();
    let mut builder = JournalSegmentBuilder::genesis();
    builder
        .append_at(
            1,
            JournalRecordKind::Checkpoint,
            Digest32::from_bytes([8; 32]),
            &payload,
        )
        .unwrap();
    let frame = builder.finish().unwrap();
    let (_broker, mut run, mut lease) = resources(1, 16 * 1024 * 1024, 16 * 1024 * 1024);
    {
        let view = lease.lease();
        let target = root.join(format!("segment-{file_sequence:020}.bin"));
        let mut owner = HostPersistentStoreOwner::begin_binary_checkpoint_segment(
            root,
            &target,
            frame.len() as u64,
            PersistentStoreLimits::default(),
            &view,
        )
        .unwrap();
        owner.write_binary_checkpoint_segment(&frame).unwrap();
    }
    lease.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
    frame
}

fn observe_cli(root: &Path, source: &str, plan: &str) -> serde_json::Value {
    let arguments: Vec<OsString> = vec![
        "lc631-checkpoint-resume-inspect".into(),
        "--ledger-root".into(),
        root.as_os_str().into(),
        "--source-revision".into(),
        source.into(),
        "--plan-digest".into(),
        plan.into(),
        "--authority-revision".into(),
        "authority-current".into(),
    ];
    observe_command(root, arguments)
}

fn observe_command(root: &Path, arguments: Vec<OsString>) -> serde_json::Value {
    const CAP: usize = 16 * 1024;
    // Test transport setup is caller-owned; values are never printed.
    let environment: Vec<_> = std::env::vars_os().collect();
    let request = RunRequest {
        executable: OsStr::new(env!("CARGO_BIN_EXE_lc631")),
        arguments: &arguments,
        current_dir: root,
        environment: &environment,
        stdin: None,
        timeout: Duration::from_secs(20),
        stdout_limit: CAP,
        stderr_limit: CAP,
    };
    let ram = required_ram_bytes(&request).unwrap();
    let (_broker, mut run, mut lease) = resources(2, ram, (2 * CAP) as u64);
    let report = {
        let view = lease.lease();
        let observed = run_bounded_with_resources_observed(request, &view).unwrap();
        let outcome = observed.outcome();
        assert_eq!(outcome.exit_code, Some(0));
        assert!(!observed.cancelled() && !outcome.timed_out);
        assert!(!outcome.stdout_truncated && !outcome.stderr_truncated);
        assert!(observed.cleanup_complete());
        serde_json::from_slice::<serde_json::Value>(&outcome.stdout).unwrap()
    };
    lease.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
    assert_eq!(report["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(report["payload"]["authority_created"], false);
    assert_eq!(report["payload"]["completion_restored"], false);
    assert_eq!(report["payload"]["output_commit_allowed"], false);
    assert_eq!(report["payload"]["diagnostic_only"], true);
    report
}

#[test]
fn tlcx07_actual_cli_dispatch_consumes_persisted_checkpoint_and_refuses_stale_or_missing_prefix() {
    let source = stable_sha256("source");
    let plan = stable_sha256("plan");
    let valid = root();
    let frame = persist(&valid, 1, &source, &plan);
    let positive = observe_cli(&valid, &source, &plan);
    assert_eq!(positive["payload"]["status"], "candidate_unanchored");
    assert_eq!(positive["payload"]["segment_count"], 1);
    assert_eq!(positive["payload"]["record_count"], 1);
    assert_eq!(positive["payload"]["fresh_revalidation_required"], true);
    let stale = observe_cli(&valid, "stale-source", &plan);
    assert_eq!(stale["payload"]["status"], "hold");
    assert_eq!(stale["payload"]["hold_code"], "runtime_chain_hold");
    assert_eq!(
        std::fs::read(valid.join("segment-00000000000000000001.bin")).unwrap(),
        frame
    );
    let missing_prefix = root();
    let frame = persist(&missing_prefix, 2, &source, &plan);
    let rejected = observe_cli(&missing_prefix, &source, &plan);
    assert_eq!(rejected["payload"]["status"], "hold");
    assert_eq!(rejected["payload"]["hold_code"], "descriptor_sequence_gap");
    assert_eq!(
        std::fs::read(missing_prefix.join("segment-00000000000000000002.bin")).unwrap(),
        frame
    );
    assert!(!missing_prefix
        .join("segment-00000000000000000001.bin")
        .exists());
    check_bundle_cli();
    check_empty_and_missing_bundle_cli();
}

fn check_bundle_cli() {
    let root = root();
    let mut builder = JournalSegmentBuilder::genesis();
    builder
        .append_at(
            1,
            JournalRecordKind::Evidence,
            Digest32::from_bytes([3; 32]),
            b"bundle",
        )
        .unwrap();
    let frame = builder.finish().unwrap();
    let (_broker, mut run, mut lease) = resources(3, 32 * 1024 * 1024, 32 * 1024 * 1024);
    let head_digest;
    {
        let view = lease.lease();
        let mut writer = JournalBundleLiveSession::open(
            &root,
            64 * 1024,
            JournalBundleLimits::checked(10, 1024 * 1024).unwrap(),
            &view,
            JournalChainLimits::default(),
        )
        .unwrap();
        head_digest = writer
            .append(JournalCursor::genesis(), &frame)
            .unwrap()
            .head_frame_digest();
    }
    lease.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
    let args = || {
        vec![
            "lc631-journal-bundle-inspect".into(),
            "--ledger-root".into(),
            root.as_os_str().into(),
        ]
    };
    let positive = observe_command(&root, args());
    assert_eq!(
        positive["payload"]["schema_version"],
        "lc631-journal-bundle-inspect.v1"
    );
    assert_eq!(positive["payload"]["status"], "inspected_unanchored");
    assert_eq!(positive["payload"]["segment_count"], 1);
    assert_eq!(positive["payload"]["record_count"], 1);
    assert_eq!(positive["payload"]["head_state"], "unanchored");
    assert_eq!(positive["payload"]["rollback_resistance"], "unavailable");
    assert_eq!(
        positive["payload"]["local_head_frame_digest_bytes"],
        serde_json::to_value(*head_digest.as_bytes()).unwrap()
    );
    let index_path = root.join("index-00000000000000000001.bin");
    let mut corrupt = std::fs::read(&index_path).unwrap();
    corrupt[0] ^= 1;
    std::fs::write(&index_path, &corrupt).unwrap();
    let rejected = observe_command(&root, args());
    assert_eq!(rejected["payload"]["status"], "hold");
    assert_eq!(rejected["payload"]["hold_code"], "bundle_rebuild_required");
    assert!(rejected["payload"]["local_head_frame_digest_bytes"].is_null());
    assert_eq!(std::fs::read(&index_path).unwrap(), corrupt);
    assert_eq!(
        std::fs::read(root.join("segment-00000000000000000001.bin")).unwrap(),
        frame
    );
}

fn check_empty_and_missing_bundle_cli() {
    let root = root();
    let (_broker, mut run, mut lease) = resources(4, 16 * 1024 * 1024, 16 * 1024 * 1024);
    {
        let view = lease.lease();
        let _writer = HostJournalBundleSession::open(
            &root,
            0,
            JournalBundleLimits::checked(10, 1024 * 1024).unwrap(),
            &view,
        )
        .unwrap();
    }
    lease.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
    let before = std::fs::read_dir(&root).unwrap().count();
    let args = |path: &Path| {
        vec![
            "lc631-journal-bundle-inspect".into(),
            "--ledger-root".into(),
            path.as_os_str().into(),
        ]
    };
    let empty = observe_command(&root, args(&root));
    assert_eq!(empty["payload"]["status"], "hold");
    assert_eq!(empty["payload"]["hold_code"], "empty_inventory");
    assert!(empty["payload"]["record_count"].is_null());
    let missing = root.join("missing");
    let unavailable = observe_command(&root, args(&missing));
    assert_eq!(unavailable["payload"]["status"], "hold");
    assert_eq!(unavailable["payload"]["hold_code"], "storage_read_hold");
    assert!(!missing.exists());
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), before);
}
