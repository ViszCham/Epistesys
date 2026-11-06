//! Actual local Python process fixture, not real model or semantic evidence.
#![forbid(unsafe_code)]

use lc631_media::attestation_v2::verify_media_backend_attestation_v2;
use lc631_media::attestation_v3::{
    media_backend_scope_v3, verify_media_backend_attestation_v3,
    verify_media_backend_with_current_authority_v3,
};
use lc631_media::{
    authorize_media_source, authorize_media_source_attested, execute_local_media_backend_attested,
    execute_local_media_backend_attested_v3, media_source_payload_digest, media_source_scope,
    model_cache_manifest_sha256, BackendDescriptor, BackendFamily, BackendState,
    MediaBackendConfig, MediaBackendExecutionReceipt, MediaResourceBudget, MediaSourceAuthority,
    MediaSourceKind, MediaSourceRequest,
};
use lc631_process_supervisor::{run_bounded, RunRequest};
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptIssuer, ReceiptScope, ReceiptVerifier, ReplayGuard, SubjectRevision,
};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

struct Fixture {
    source: PathBuf,
    authority: MediaSourceAuthority,
    descriptor: BackendDescriptor,
    config: MediaBackendConfig,
    issuer: ReceiptIssuer,
    verifier: ReceiptVerifier,
}

impl Fixture {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "epistesys-v3-owned-process-{}-{nonce}",
            std::process::id(),
        ));
        std::fs::create_dir(&root).unwrap();
        let source = root.join("source.bin");
        std::fs::write(&source, b"source").unwrap();
        let models = root.join("models");
        std::fs::create_dir(&models).unwrap();
        std::fs::write(models.join("fixture.bin"), b"fixture").unwrap();
        let ffmpeg = root.join("ffmpeg.fixture");
        std::fs::write(&ffmpeg, b"not executed by the fixture").unwrap();
        let model_digest = model_cache_manifest_sha256(&models).unwrap();
        let script = root.join("backend.py");
        std::fs::write(root.join("local_fixture_module.py"), b"VALUE = 1\n").unwrap();
        let payload = serde_json::json!({
            "schema_version": "lc631-media-backend.v1",
            "source_sha256": lc631_core::stable_sha256("source"),
            "backend_id": "v3-process-fixture",
            "backend_version": "fixture-v1",
            "model_revision": model_digest,
            "model_cache_manifest_sha256": model_digest,
            "model_card_license_hint": "synthetic test only",
            "executed_families": ["decode"],
            "executed_methods": {"decode":"ffmpeg-cli"},
            "observations": [],
            "diagnostics": ["actual Python, no model or ffmpeg executed"]
        });
        std::fs::write(
            &script,
            format!(
                "import sys\nassert sys.dont_write_bytecode, 'bytecode cache generation enabled'\nimport local_fixture_module\nassert local_fixture_module.VALUE == 1\nprint({:?})\n",
                payload.to_string()
            ),
        )
        .unwrap();
        let interpreter_name = if cfg!(windows) {
            "python.exe"
        } else {
            "python3"
        };
        let interpreter = std::env::var_os("EPISTESYS_TEST_MEDIA_PYTHON")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::split_paths(&std::env::var_os("PATH")?)
                    .map(|directory| directory.join(interpreter_name))
                    .find(|path| path.is_file())
            })
            .expect("installed absolute Python executable required");
        let arguments = ["-c".into(), "import sys;print(sys.executable)".into()];
        let environment = std::env::vars_os().collect::<Vec<_>>();
        let python = run_bounded(RunRequest {
            executable: interpreter.as_os_str(),
            arguments: &arguments,
            current_dir: &root,
            environment: &environment,
            stdin: None,
            timeout: Duration::from_secs(10),
            stdout_limit: 4096,
            stderr_limit: 4096,
        })
        .unwrap();
        assert_eq!(python.exit_code, Some(0));
        assert!(!python.stdout_truncated && python.io_cleanup_complete);
        let python = PathBuf::from(String::from_utf8(python.stdout).unwrap().trim());
        let issuer = ReceiptIssuer::from_key_bytes("v3-process-fixture", &[92; 32]).unwrap();
        let verifier = ReceiptVerifier::from_key_bytes("v3-process-fixture", &[92; 32]).unwrap();
        let request = MediaSourceRequest {
            kind: MediaSourceKind::LocalUserProvided,
            locator: source.to_str().unwrap().into(),
            user_asserts_rights: true,
            media_bytes_requested: true,
            network_requested: false,
        };
        let pending = authorize_media_source(&request);
        let wire = issuer
            .issue(
                ReceiptClass::MediaSource,
                SubjectRevision::checked(pending.source_snapshot_sha256.as_ref().unwrap()).unwrap(),
                ReceiptScope::checked(media_source_scope(request.kind)).unwrap(),
                media_source_payload_digest(&request, &pending).unwrap(),
                1,
                None,
                None,
            )
            .unwrap();
        let authority = authorize_media_source_attested(
            &request,
            wire,
            &verifier,
            &mut ReplayGuard::default(),
            1,
        )
        .unwrap();
        Self {
            source,
            authority,
            descriptor: BackendDescriptor::configured(
                BackendFamily::Decode,
                "v3-process-fixture",
                Some(script.clone()),
                Some(model_digest),
                Some("synthetic fixture license assertion only".into()),
            ),
            config: MediaBackendConfig {
                python_executable: python,
                backend_script: script,
                ffmpeg_executable: ffmpeg,
                model_cache: models,
                timeout_seconds: 10,
            },
            issuer,
            verifier,
        }
    }

    fn execute(&self, v3: bool) -> MediaBackendExecutionReceipt {
        let execute = if v3 {
            execute_local_media_backend_attested_v3
        } else {
            execute_local_media_backend_attested
        };
        let receipt = execute(
            &self.authority,
            &self.descriptor,
            &self.config,
            &MediaResourceBudget::default(),
            &self.issuer,
            &self.verifier,
            &mut ReplayGuard::default(),
            1,
        )
        .unwrap();
        assert!(!self.source.parent().unwrap().join("__pycache__").exists());
        receipt
    }
}

#[test]
fn tlcx30_actual_python_v3_is_explicit_and_existing_v2_issuance_stays_v2() {
    let fixture = Fixture::new();
    let parent = fixture
        .authority
        .attestation_receipt_digest
        .as_deref()
        .unwrap();
    let old = fixture.execute(false);
    assert_eq!(
        old.authenticity_attestation
            .as_ref()
            .unwrap()
            .claims()
            .scope()
            .as_str(),
        "media/backend/v2/v3-process-fixture"
    );
    verify_media_backend_attestation_v2(
        &old,
        &fixture.verifier,
        &mut ReplayGuard::default(),
        120,
        parent,
    )
    .unwrap();
    assert!(verify_media_backend_attestation_v3(
        &old,
        &fixture.verifier,
        &mut ReplayGuard::default(),
        120,
        parent,
    )
    .is_err());

    let new = fixture.execute(true);
    assert_eq!(
        new.authenticity_attestation
            .as_ref()
            .unwrap()
            .claims()
            .scope()
            .as_str(),
        media_backend_scope_v3(&new.backend_id)
    );
    assert_eq!(new.process.exit_code, Some(0));
    assert_eq!(
        new.executed_family_states[&BackendFamily::Decode],
        BackendState::ExecutedCandidate
    );
    assert!(new.graph.observations().is_empty());
    assert_eq!(new.family_validation_count, 0);
    assert!(!new.identity_inference_committed && !new.internal_state_committed);
    verify_media_backend_with_current_authority_v3(
        &new,
        &fixture.authority,
        &fixture.verifier,
        &mut ReplayGuard::default(),
        120,
    )
    .unwrap();
    assert!(verify_media_backend_attestation_v2(
        &new,
        &fixture.verifier,
        &mut ReplayGuard::default(),
        120,
        parent,
    )
    .is_err());

    // A genuine signed process report must not validate a later changed source.
    std::fs::write(&fixture.source, b"changed").unwrap();
    let mut replay = ReplayGuard::default();
    assert!(verify_media_backend_with_current_authority_v3(
        &new,
        &fixture.authority,
        &fixture.verifier,
        &mut replay,
        120,
    )
    .is_err());
    assert_eq!(replay.observed_count(), 0);
    // Retain only uniquely created fixture files; no broad cleanup/deletion.
    eprintln!(
        "retained test-owned process fixture: {}",
        fixture.source.parent().unwrap().display()
    );
}

/// Reuse the authorization timestamp the same way the legacy CLI does after
/// preparing its descriptor. A spent source window must suppress dispatch,
/// independently of whether a later output verifier would reject the receipt.
#[test]
fn beta7_v2_elapsed_before_execution_refuses_expired_parent_without_starting_backend() {
    const NOW: u64 = 100;
    const WINDOW: u64 = 2;
    let fixture = Fixture::new();
    let marker = fixture.source.with_file_name("dispatch-count");
    let script = &fixture.config.backend_script;
    let body = std::fs::read_to_string(script).unwrap();
    std::fs::write(
        script,
        format!(
            "from pathlib import Path\nwith Path(__file__).with_name('dispatch-count').open('ab') as f: f.write(b'x')\n{body}"
        ),
    )
    .unwrap();
    // Positive control uses the same synthetic worker, source and legacy v2 wire.
    let control = fixture.execute(false);
    assert_eq!(control.process.exit_code, Some(0));
    assert_eq!(std::fs::read(&marker).unwrap(), b"x");
    assert_eq!(
        control
            .authenticity_attestation
            .as_ref()
            .unwrap()
            .claims()
            .scope()
            .as_str(),
        "media/backend/v2/v3-process-fixture"
    );

    let request = MediaSourceRequest {
        kind: MediaSourceKind::LocalUserProvided,
        locator: fixture.source.to_str().unwrap().to_owned(),
        user_asserts_rights: true,
        media_bytes_requested: true,
        network_requested: false,
    };
    let pending = authorize_media_source(&request);
    let wire = fixture
        .issuer
        .issue(
            ReceiptClass::MediaSource,
            SubjectRevision::checked(pending.source_snapshot_sha256.as_ref().unwrap()).unwrap(),
            ReceiptScope::checked(media_source_scope(request.kind)).unwrap(),
            media_source_payload_digest(&request, &pending).unwrap(),
            NOW,
            Some(NOW + WINDOW),
            None,
        )
        .unwrap();
    let parent = authorize_media_source_attested(
        &request,
        wire,
        &fixture.verifier,
        &mut ReplayGuard::default(),
        NOW,
    )
    .unwrap();
    // The verified parent is current at NOW. Monotonic preparation time takes
    // it across its exclusive upper bound before the executor is entered.
    std::thread::sleep(Duration::from_millis(WINDOW * 1000 + 50));
    let mut backend_replay = ReplayGuard::default();
    let result = execute_local_media_backend_attested(
        &parent,
        &fixture.descriptor,
        &fixture.config,
        &MediaResourceBudget::default(),
        &fixture.issuer,
        &fixture.verifier,
        &mut backend_replay,
        NOW,
    );
    let observed_starts = std::fs::read(&marker).unwrap();
    eprintln!(
        "expiry dispatch returned: {result:?}; marker={observed_starts:?}; backend replay={}",
        backend_replay.observed_count()
    );
    assert_eq!(
        observed_starts, b"x",
        "expired source authority started a second backend during legacy dispatch"
    );
    assert!(matches!(
        result,
        Err(lc631_media::MediaError::SourceReceiptRejected)
    ));
    assert_eq!(backend_replay.observed_count(), 0);
}

/// Model output may name a method without supplying the fragments on which it ran.
#[test]
fn beta7_unbound_temporal_co_presence_never_issues_a_backend_receipt() {
    let fixture = Fixture::new();
    let control = fixture.execute(true);
    assert_eq!(control.process.exit_code, Some(0));
    let payload = serde_json::json!({
        "schema_version":"lc631-media-backend.v1",
        "source_sha256":control.source_sha256,
        "backend_id":fixture.descriptor.backend_id,
        "backend_version":"fixture-v1",
        "model_revision":fixture.descriptor.model_sha256,
        "model_cache_manifest_sha256":fixture.descriptor.model_sha256,
        "model_card_license_hint":"synthetic fixture only",
        "executed_families":["asr","temporal_co_presence"],
        "executed_methods":{"asr":"faster-whisper-local-cache","temporal_co_presence":"timestamp-overlap-only"},
        "observations":[
            {"observation_id":"audio", "kind":"transcript_candidate","start_ms":10000,"end_ms":11000,
             "label":"synthetic transcript","evidence_digest":"audio-evidence"},
            {"observation_id":"pair", "kind":"temporal_co_presence_candidate","start_ms":0,"end_ms":0,
             "label":"unbound pair","evidence_digest":"pair-evidence"}
        ],
        "diagnostics":[]
    });
    std::fs::write(
        &fixture.config.backend_script,
        format!("print({:?})\n", payload.to_string()),
    )
    .unwrap();
    let mut replay = ReplayGuard::default();
    let result = execute_local_media_backend_attested_v3(
        &fixture.authority,
        &fixture.descriptor,
        &fixture.config,
        &MediaResourceBudget::default(),
        &fixture.issuer,
        &fixture.verifier,
        &mut replay,
        1,
    );
    assert!(
        matches!(
            result,
            Err(lc631_media::MediaError::InvalidBackendObservation)
        ),
        "unbound 0..0 co-presence was accepted and signed (success={}, backend replay={})",
        result.is_ok(),
        replay.observed_count()
    );
    assert_eq!(replay.observed_count(), 0);
}

#[test]
fn beta7_temporal_receipt_retains_frame_and_transcript_occurrences() {
    let fixture = Fixture::new();
    let mut fragments = Vec::new();
    for frame in ["first synthetic frame", "other synthetic frame"] {
        let frame_digest = lc631_core::stable_sha256(frame);
        let model = fixture.descriptor.model_sha256.as_deref().unwrap();
        // Keep source, time, reported pair ID and raw evidence text fixed.
        // Only the frame occurrence changes.
        let payload = serde_json::json!({
            "schema_version": "lc631-media-backend.v1",
            "source_sha256": fixture.authority.source_snapshot_sha256,
            "backend_id": fixture.descriptor.backend_id,
            "backend_version": "fixture-v1", "model_revision": model,
            "model_cache_manifest_sha256": model, "model_card_license_hint": "fixture",
            "executed_families": ["decode", "asr", "temporal_co_presence"],
            "executed_methods": {"decode":"ffmpeg-cli", "asr":"faster-whisper-local-cache",
                                 "temporal_co_presence":"timestamp-overlap-only"},
            "observations": [
                {"observation_id":"transcript-1", "kind":"transcript_candidate",
                 "start_ms":100, "end_ms":650, "label":"synthetic transcript",
                 "evidence_digest":lc631_core::stable_sha256("transcript")},
                {"observation_id":"pair-1", "kind":"temporal_co_presence_candidate",
                 "start_ms":100, "end_ms":500, "label":"reported overlap",
                 "evidence_digest":lc631_core::stable_sha256("pair"),
                 "temporal_overlap":{"profile":"ffmpeg-shared-origin-fps2-v1",
                     "clock_digest":lc631_core::stable_sha256("declared common origin"),
                     "transcript_id":"transcript-1", "frame_digest":frame_digest,
                     "frame_index":0, "frame_start_ms":0, "frame_end_ms":500, "pcm_end_ms":700}}
            ],
            "diagnostics": []
        });
        std::fs::write(
            &fixture.config.backend_script,
            format!("print({:?})\n", payload.to_string()),
        )
        .unwrap();
        let report = fixture.execute(true);
        let pair = report
            .graph
            .observations()
            .iter()
            .find(|o| o.observation_id == "pair-1")
            .unwrap();
        assert_eq!((pair.span.start.ticks, pair.span.end.ticks), (100, 500));
        assert!(pair.candidate_only && !pair.identity_committed && !pair.internal_state_committed);
        fragments.push(pair.source_fragment_digest.clone());
    }
    assert_ne!(
        fragments[0], fragments[1],
        "different sampled frame occurrences collapsed into one signed source fragment"
    );
}
