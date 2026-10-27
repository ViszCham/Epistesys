//! Opt-in observation of installed local backends, never an accuracy benchmark.
use lc631_media::{
    authorize_media_source, authorize_media_source_attested, execute_local_media_backend_attested,
    media_source_payload_digest, media_source_scope, model_cache_manifest_sha256,
    BackendDescriptor, BackendFamily, BackendState, MediaBackendConfig, MediaResourceBudget,
    MediaSourceKind, MediaSourceRequest, ObservationKind,
};
use lc631_process_supervisor::{run_bounded, RunRequest};
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptIssuer, ReceiptScope, ReceiptVerifier, ReplayGuard, SubjectRevision,
};
use std::ffi::OsString;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[test]
#[ignore = "requires explicitly configured local Python/FFmpeg/model paths; no backend acquisition"]
fn real_local_decode_vision_and_asr_remain_authenticated_candidates() {
    let python =
        PathBuf::from(std::env::var_os("EPISTESYS_TEST_MEDIA_PYTHON").expect("Python path"));
    let ffmpeg =
        PathBuf::from(std::env::var_os("EPISTESYS_TEST_MEDIA_FFMPEG").expect("FFmpeg path"));
    let models =
        PathBuf::from(std::env::var_os("EPISTESYS_TEST_MEDIA_MODELS").expect("model root"));
    let speech = PathBuf::from(
        std::env::var_os("EPISTESYS_TEST_MEDIA_SPEECH").expect("owned synthetic speech WAV"),
    );
    assert!(speech.is_file());
    assert!(python.is_file() && ffmpeg.is_file() && models.is_dir());
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("epistesys-owned-real-media-{nonce}"));
    std::fs::create_dir(&root).unwrap();
    let source = root.join("owned-synthetic.avi");
    let arguments = [
        "-nostdin",
        "-hide_banner",
        "-loglevel",
        "error",
        "-f",
        "lavfi",
        "-i",
        "color=c=black:s=64x64:r=2",
        "-f",
        "wav",
        "-i",
        speech.to_str().unwrap(),
        "-t",
        "6",
        "-c:v",
        "rawvideo",
        "-pix_fmt",
        "bgr24",
        "-c:a",
        "pcm_s16le",
        source.to_str().unwrap(),
    ]
    .map(OsString::from);
    let environment = std::env::vars_os().collect::<Vec<_>>();
    let generated = run_bounded(RunRequest {
        executable: ffmpeg.as_os_str(),
        arguments: &arguments,
        current_dir: &root,
        environment: &environment,
        stdin: None,
        timeout: Duration::from_secs(20),
        stdout_limit: 4096,
        stderr_limit: 16384,
    })
    .unwrap();
    assert_eq!(
        generated.exit_code,
        Some(0),
        "{}",
        String::from_utf8_lossy(&generated.stderr)
    );
    assert!(generated.io_cleanup_complete && generated.process_tree_kill_verified);

    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../media_backend.py")
        .canonicalize()
        .unwrap();
    let request = MediaSourceRequest {
        kind: MediaSourceKind::LocalUserProvided,
        locator: source.display().to_string(),
        user_asserts_rights: true,
        media_bytes_requested: true,
        network_requested: false,
    };
    let issuer = ReceiptIssuer::from_key_bytes("owned-media-fixture-root", &[61; 32]).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("owned-media-fixture-root", &[61; 32]).unwrap();
    let candidate = authorize_media_source(&request);
    let wire = issuer
        .issue(
            ReceiptClass::MediaSource,
            SubjectRevision::checked(candidate.source_snapshot_sha256.clone().unwrap()).unwrap(),
            ReceiptScope::checked(media_source_scope(request.kind)).unwrap(),
            media_source_payload_digest(&request, &candidate).unwrap(),
            1,
            Some(600),
            None,
        )
        .unwrap();
    let authority =
        authorize_media_source_attested(&request, wire, &verifier, &mut ReplayGuard::default(), 1)
            .unwrap();
    let model_digest = model_cache_manifest_sha256(&models).unwrap();
    let descriptor = BackendDescriptor::configured(
        BackendFamily::Decode,
        "lc631-local-ffmpeg-opencv-whisper-anonymous-diarization",
        Some(script.clone()),
        Some(model_digest),
        // An operator assertion for this owned local test, not a legal audit.
        Some("owned-local-model-test-assertion;license-audit-unobserved".into()),
    );
    let budget = MediaResourceBudget {
        max_decoded_frames: 2,
        max_pcm_samples: 16000 * 8,
        max_temporary_bytes: 4 * 1024 * 1024,
        ..MediaResourceBudget::default()
    };
    let receipt = execute_local_media_backend_attested(
        &authority,
        &descriptor,
        &MediaBackendConfig {
            python_executable: python,
            backend_script: script,
            ffmpeg_executable: ffmpeg,
            model_cache: models,
            timeout_seconds: 120,
        },
        &budget,
        &issuer,
        &verifier,
        &mut ReplayGuard::default(),
        1,
    )
    .unwrap();
    for family in [
        BackendFamily::Decode,
        BackendFamily::ObjectDetection,
        BackendFamily::Asr,
    ] {
        assert_eq!(
            receipt.executed_family_states[&family],
            BackendState::ExecutedCandidate
        );
    }
    assert!(receipt.source_authority_preserved && receipt.authenticity_receipt_digest.is_some());
    assert_eq!(receipt.family_validation_count, 0);
    assert!(!receipt.identity_inference_committed && !receipt.internal_state_committed);
    assert!(receipt
        .graph
        .observations()
        .iter()
        .any(
            |observation| observation.kind == ObservationKind::TranscriptCandidate
                && !observation.label.is_empty()
        ));
    assert!(!receipt.execution_plan.network_isolation_os_enforced);
    println!("local decode/vision/ASR observed as candidates; no semantic accuracy or license-audit claim");
    println!("observed_model_revision={}", receipt.model_revision);
}
