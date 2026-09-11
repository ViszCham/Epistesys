use lc631_media::{
    authorize_media_source, authorize_media_source_attested, execute_local_media_backend_attested,
    media_source_payload_digest, media_source_scope, model_cache_manifest_sha256,
    BackendDescriptor, BackendFamily, BackendState, MediaBackendConfig, MediaResourceBudget,
    MediaSourceAuthority, MediaSourceKind, MediaSourceRequest,
};
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptIssuer, ReceiptScope, ReceiptVerifier, ReplayGuard, SubjectRevision,
};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn arc631_19_rights_bool_and_existing_path_are_not_source_authority() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("lc631-media-authority-{nonce}.bin"));
    fs::write(&path, b"media").unwrap();
    let request = MediaSourceRequest {
        kind: MediaSourceKind::LocalUserProvided,
        locator: path.display().to_string(),
        user_asserts_rights: true,
        media_bytes_requested: true,
        network_requested: false,
    };
    let candidate = authorize_media_source(&request);
    assert!(!candidate.authorized);
    assert!(candidate.source_snapshot_sha256.is_some());

    let key = [47_u8; 32];
    let issuer = ReceiptIssuer::from_key_bytes("lc631-media-root", &key).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("lc631-media-root", &key).unwrap();
    let subject =
        SubjectRevision::checked(candidate.source_snapshot_sha256.clone().unwrap()).unwrap();
    let scope = ReceiptScope::checked(media_source_scope(request.kind)).unwrap();
    let wire = issuer
        .issue(
            ReceiptClass::MediaSource,
            subject,
            scope,
            media_source_payload_digest(&request, &candidate).unwrap(),
            1,
            None,
            None,
        )
        .unwrap();
    let authority =
        authorize_media_source_attested(&request, wire, &verifier, &mut ReplayGuard::default(), 1)
            .unwrap();
    assert!(authority.authorized);
    assert!(authority.attestation_verified());
    fs::remove_file(path).unwrap();
}

fn source_authority(
    request: &MediaSourceRequest,
    issuer: &ReceiptIssuer,
    verifier: &ReceiptVerifier,
    replay: &mut ReplayGuard,
) -> MediaSourceAuthority {
    let candidate = authorize_media_source(request);
    let wire = issuer
        .issue(
            ReceiptClass::MediaSource,
            SubjectRevision::checked(candidate.source_snapshot_sha256.clone().unwrap()).unwrap(),
            ReceiptScope::checked(media_source_scope(request.kind)).unwrap(),
            media_source_payload_digest(request, &candidate).unwrap(),
            1,
            None,
            None,
        )
        .unwrap();
    authorize_media_source_attested(request, wire, verifier, replay, 1).unwrap()
}

#[test]
fn arc631_20_21_arbitrary_script_output_stays_candidate_and_enters_tl_envelope() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("lc631-media-backend-{nonce}"));
    fs::create_dir_all(&root).unwrap();
    let media = root.join("media.bin");
    let script = root.join("backend.py");
    let ffmpeg = root.join(if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    });
    let model_cache = root.join("models");
    fs::create_dir_all(&model_cache).unwrap();
    fs::write(&media, b"media").unwrap();
    fs::write(&ffmpeg, b"fixture").unwrap();
    fs::write(model_cache.join("model.bin"), b"model").unwrap();
    let source_sha = lc631_core::stable_sha256("media");
    let model_sha = model_cache_manifest_sha256(&model_cache).unwrap();
    let payload = serde_json::json!({
        "schema_version": "lc631-media-backend.v1",
        "source_sha256": source_sha,
        "backend_id": "fixture-backend",
        "backend_version": "fixture-v1",
        "model_revision": model_sha,
        "model_cache_manifest_sha256": model_sha,
        "model_card_license_hint": "test-only",
        "executed_families": ["decode"],
        "executed_methods": {"decode": "ffmpeg-cli"},
        "observations": [{
            "observation_id": "obs-1",
            "kind": "object_box",
            "start_ms": 0,
            "end_ms": 1,
            "track_id": null,
            "label": "candidate",
            "evidence_digest": lc631_core::stable_sha256("untrusted-script-evidence")
        }],
        "diagnostics": []
    });
    fs::write(
        &script,
        format!(
            "import json\nprint(json.dumps(json.loads({:?}), sort_keys=True))\n",
            payload.to_string()
        ),
    )
    .unwrap();
    let python = Command::new("python")
        .args(["-c", "import sys;print(sys.executable)"])
        .output()
        .unwrap();
    assert!(python.status.success());
    let python = PathBuf::from(String::from_utf8(python.stdout).unwrap().trim());
    let key = [53_u8; 32];
    let issuer = ReceiptIssuer::from_key_bytes("lc631-media-root", &key).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("lc631-media-root", &key).unwrap();
    let mut replay = ReplayGuard::default();
    let request = MediaSourceRequest {
        kind: MediaSourceKind::LocalUserProvided,
        locator: media.display().to_string(),
        user_asserts_rights: true,
        media_bytes_requested: true,
        network_requested: false,
    };
    let authority = source_authority(&request, &issuer, &verifier, &mut replay);
    let descriptor = BackendDescriptor::configured(
        BackendFamily::Decode,
        "fixture-backend",
        Some(script.clone()),
        Some(model_sha),
        Some("test-license".into()),
    );
    let receipt = execute_local_media_backend_attested(
        &authority,
        &descriptor,
        &MediaBackendConfig {
            python_executable: python,
            backend_script: script,
            ffmpeg_executable: ffmpeg,
            model_cache,
            timeout_seconds: 10,
        },
        &MediaResourceBudget::default(),
        &issuer,
        &verifier,
        &mut replay,
        1,
    )
    .unwrap();
    assert_eq!(
        receipt.executed_family_states[&BackendFamily::Decode],
        BackendState::ExecutedCandidate
    );
    assert_eq!(receipt.family_validation_count, 0);
    assert_eq!(
        receipt.translation_envelope.domain_payloads[0].domain,
        "media"
    );
    assert!(receipt.authenticity_receipt_digest.is_some());
    fs::remove_dir_all(root).unwrap();
}
