//! Linux watchdog: only a uniquely created test source is replaced by a FIFO.
#![forbid(unsafe_code)]
#![cfg(target_os = "linux")]

use lc631_media::attestation_v3::{
    media_backend_payload_digest_v3, media_backend_scope_v3,
    verify_media_backend_with_current_authority_v3,
};
use lc631_media::{
    authorize_media_source, authorize_media_source_attested, media_source_payload_digest,
    media_source_scope, MediaBackendExecutionReceipt, MediaExecutionPlan, MediaProcessReceipt,
    MediaResourceBudget, MediaSourceKind, MediaSourceRequest, MultimodalEventGraph,
    MEDIA_BACKEND_SCHEMA,
};
use lc631_process_supervisor::{run_bounded, RunRequest};
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptIssuer, ReceiptScope, ReceiptVerifier, ReplayGuard, SubjectRevision,
};
use lc631_tl::{CanonicalTranslationEnvelope, ProjectionDefectGraphV3, SourceLedgerId};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const WORKER: &str = "EPISTESYS_TEST_V3_FIFO_WORKER";
const TEST: &str = "tlcx30_v3_swapped_fifo_is_rejected_without_blocking";

fn exercise_fifo_swap() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "epistesys-v3-owned-fifo-{}-{nonce}",
        std::process::id(),
    ));
    std::fs::create_dir(&root).unwrap();
    let source = root.join("source.bin");
    std::fs::write(&source, b"source").unwrap();
    let request = MediaSourceRequest {
        kind: MediaSourceKind::LocalUserProvided,
        locator: source.to_str().unwrap().into(),
        user_asserts_rights: true,
        media_bytes_requested: true,
        network_requested: false,
    };
    let issuer = ReceiptIssuer::from_key_bytes("v3-fifo-fixture", &[81; 32]).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("v3-fifo-fixture", &[81; 32]).unwrap();
    let pending = authorize_media_source(&request);
    let digest = pending.source_snapshot_sha256.clone().unwrap();
    let wire = issuer
        .issue(
            ReceiptClass::MediaSource,
            SubjectRevision::checked(&digest).unwrap(),
            ReceiptScope::checked(media_source_scope(request.kind)).unwrap(),
            media_source_payload_digest(&request, &pending).unwrap(),
            10,
            Some(100),
            None,
        )
        .unwrap();
    let authority =
        authorize_media_source_attested(&request, wire, &verifier, &mut ReplayGuard::default(), 10)
            .unwrap();
    let projection = ProjectionDefectGraphV3::new(SourceLedgerId(1), "rev", "source").unwrap();
    let mut report = MediaBackendExecutionReceipt {
        schema_version: MEDIA_BACKEND_SCHEMA,
        source_sha256: digest.clone(),
        backend_id: "fifo-fixture".into(),
        backend_version: "1".into(),
        model_revision: "synthetic".into(),
        model_cache_manifest_sha256: digest.clone(),
        model_card_license_hint: "synthetic fixture only".into(),
        backend_receipt_sha256: digest.clone(),
        execution_plan: MediaExecutionPlan {
            source_snapshot_sha256: digest.clone(),
            source_bytes: 6,
            python_sha256: digest.clone(),
            backend_script_sha256: digest.clone(),
            ffmpeg_sha256: digest.clone(),
            model_cache_manifest_sha256: digest.clone(),
            configuration_digest: digest.clone(),
            resource_budget: MediaResourceBudget::default(),
            network_policy: "offline".into(),
            network_isolation_os_enforced: false,
        },
        process: MediaProcessReceipt {
            exit_code: Some(0),
            stdout_digest: digest.clone(),
            stderr_digest: digest.clone(),
            stdout_truncated: false,
            stderr_truncated: false,
            process_tree_kill_attempted: false,
            process_tree_kill_verified: false,
            memory_limit_bytes: None,
            memory_limit_kind: "unobserved".into(),
        },
        executed_family_states: BTreeMap::new(),
        graph: MultimodalEventGraph::default(),
        diagnostics: vec!["no real backend/model execution".into()],
        source_authority_preserved: true,
        identity_inference_committed: false,
        internal_state_committed: false,
        family_validation_count: 0,
        authenticity_receipt_digest: None,
        authenticity_attestation: None,
        translation_envelope: CanonicalTranslationEnvelope::from_graph(&projection, vec![]),
        claim_boundary: "synthetic fixture only",
    };
    let wire = issuer
        .issue(
            ReceiptClass::MediaBackend,
            SubjectRevision::checked(&digest).unwrap(),
            ReceiptScope::checked(media_backend_scope_v3(&report.backend_id)).unwrap(),
            media_backend_payload_digest_v3(&report).unwrap(),
            10,
            None,
            authority.attestation_receipt_digest.clone(),
        )
        .unwrap();
    report.authenticity_receipt_digest = Some(wire.receipt_digest());
    report.authenticity_attestation = Some(wire);
    verify_media_backend_with_current_authority_v3(
        &report,
        &authority,
        &verifier,
        &mut ReplayGuard::default(),
        10,
    )
    .unwrap();

    // Both exact paths are fresh children of this test's newly created root.
    // Retain the original; never remove or overwrite an existing user source.
    std::fs::rename(&source, root.join("source.before.bin")).unwrap();
    let fifo = run_bounded(RunRequest {
        executable: Path::new("/usr/bin/mkfifo").as_os_str(),
        arguments: &[source.clone().into_os_string()],
        current_dir: &root,
        environment: &std::env::vars_os().collect::<Vec<_>>(),
        stdin: None,
        timeout: Duration::from_secs(2),
        stdout_limit: 4096,
        stderr_limit: 4096,
    })
    .unwrap();
    assert_eq!(fifo.exit_code, Some(0));
    assert!(!fifo.timed_out && fifo.io_cleanup_complete && fifo.process_tree_kill_verified);
    let mut replay = ReplayGuard::default();
    assert!(verify_media_backend_with_current_authority_v3(
        &report,
        &authority,
        &verifier,
        &mut replay,
        10,
    )
    .is_err());
    assert_eq!(replay.observed_count(), 0);
    eprintln!("retained test-owned FIFO and original: {}", root.display());
}

#[test]
fn tlcx30_v3_swapped_fifo_is_rejected_without_blocking() {
    if std::env::var_os(WORKER).is_some() {
        exercise_fifo_swap();
        return;
    }
    let executable = std::env::current_exe().unwrap();
    let root = executable.parent().unwrap();
    let mut environment = std::env::vars_os()
        .filter(|(key, _)| key != WORKER)
        .collect::<Vec<_>>();
    environment.push((WORKER.into(), "1".into()));
    let outcome = run_bounded(RunRequest {
        executable: executable.as_os_str(),
        arguments: &["--exact".into(), TEST.into(), "--nocapture".into()],
        current_dir: root,
        environment: &environment,
        stdin: None,
        timeout: Duration::from_secs(5),
        stdout_limit: 4096,
        stderr_limit: 8192,
    })
    .unwrap();
    assert!(outcome.io_cleanup_complete && outcome.process_tree_kill_verified);
    assert!(
        !outcome.timed_out,
        "source revalidation blocked on swapped FIFO; watchdog killed worker"
    );
    assert_eq!(
        outcome.exit_code,
        Some(0),
        "{}",
        String::from_utf8_lossy(&outcome.stderr)
    );
}
