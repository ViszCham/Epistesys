//! 明示30秒profileの単一正例。実model・FFmpeg・意味検証は実行しません。
//! One opt-in 30-second positive profile; no real model, FFmpeg, or semantic validation.
#![forbid(unsafe_code)]

use lc631_media::attestation_v3::verify_media_backend_with_current_authority_v3;
use lc631_media::{
    authorize_media_source, authorize_media_source_attested,
    execute_local_media_backend_attested_v3, media_source_payload_digest, media_source_scope,
    model_cache_manifest_sha256, BackendDescriptor, BackendFamily, BackendState,
    MediaBackendConfig, MediaResourceBudget, MediaSourceKind, MediaSourceRequest,
};
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptIssuer, ReceiptScope, ReceiptVerifier, ReplayGuard, SubjectRevision,
};
use std::fs;
use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

const SOURCE: &str = "synthetic positive source";
const BACKEND: &str = "beta7-positive-profile-30s";
const PYTHON_SHA256: &str =
    "sha256:4942b86a6597e5aee0128daa00050ed79bc21f6e709a78eb19cbfeb0c2f39ac9";

#[test]
#[ignore = "explicit supplemental native profile; requires the verified EPISTESYS_TEST_MEDIA_PYTHON"]
fn beta7_verify_e_media_positive_profile_supplemental_30s() {
    let python = PathBuf::from(
        std::env::var_os("EPISTESYS_TEST_MEDIA_PYTHON")
            .expect("explicit verified Python executable required; no discovery or PATH fallback"),
    );
    assert!(python.is_absolute() && python.is_file());
    let expected_python = python.canonicalize().unwrap();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "epistesys-beta7-media-positive-30s-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&root).unwrap();
    eprintln!("retained supplemental positive fixture: {}", root.display());
    let source = root.join("source.bin");
    fs::write(&source, SOURCE.as_bytes()).unwrap();
    let models = root.join("models");
    fs::create_dir(&models).unwrap();
    fs::write(models.join("fixture.bin"), b"synthetic model bytes").unwrap();
    let model = model_cache_manifest_sha256(&models).unwrap();
    let ffmpeg = root.join("ffmpeg.fixture");
    fs::write(&ffmpeg, b"fixture ffmpeg is not executed").unwrap();
    let script = root.join("backend.py");
    let starts = script.with_extension("started");
    let marker_path = script.with_extension("marker.json");
    let payload = serde_json::json!({
        "schema_version": "lc631-media-backend.v1",
        "source_sha256": lc631_core::stable_sha256(SOURCE),
        "backend_id": BACKEND,
        "backend_version": "synthetic-positive-v1",
        "model_revision": model,
        "model_cache_manifest_sha256": model,
        "model_card_license_hint": "synthetic fixture assertion only",
        "executed_families": ["decode"],
        "executed_methods": {"decode": "ffmpeg-cli"},
        "observations": [],
        "diagnostics": ["synthetic marker JSON; no model or ffmpeg executed"]
    })
    .to_string();
    fs::write(
        &script,
        format!(
            r#"import json
import sys
from pathlib import Path
pairs = dict(zip(sys.argv[1::2], sys.argv[2::2]))
if pairs['--mode'] != 'all':
    raise RuntimeError('synthetic mode mismatch')
if Path(pairs['--media']).read_bytes() != b'synthetic positive source':
    raise RuntimeError('synthetic source mismatch')
if Path(pairs['--ffmpeg']).read_bytes() != b'fixture ffmpeg is not executed':
    raise RuntimeError('synthetic ffmpeg marker mismatch')
if (Path(pairs['--model-cache']) / 'fixture.bin').read_bytes() != b'synthetic model bytes':
    raise RuntimeError('synthetic model bytes mismatch')
with Path(__file__).with_suffix('.started').open('ab') as stream:
    stream.write(b'x')
marker = dict(sys_executable=sys.executable, argv=sys.argv[1:],
              dont_write_bytecode=sys.dont_write_bytecode)
Path(__file__).with_suffix('.marker.json').write_text(json.dumps(marker), encoding='utf-8')
print({payload:?})
"#
        ),
    )
    .unwrap();
    let descriptor = BackendDescriptor::configured(
        BackendFamily::Decode,
        BACKEND,
        Some(script.clone()),
        Some(model),
        Some("synthetic fixture license assertion".into()),
    );
    let config = MediaBackendConfig {
        python_executable: python,
        backend_script: script,
        ffmpeg_executable: ffmpeg,
        model_cache: models,
        timeout_seconds: 30,
    };
    let budget = MediaResourceBudget {
        max_stdout_bytes: 65_536,
        max_stderr_bytes: 8_192,
        max_temporary_bytes: 1024 * 1024,
        max_decoded_frames: 1,
        max_pcm_samples: 100,
        max_observations: 4,
    };
    let request = MediaSourceRequest {
        kind: MediaSourceKind::LocalUserProvided,
        locator: source.to_str().unwrap().into(),
        user_asserts_rights: true,
        media_bytes_requested: true,
        network_requested: false,
    };
    let issuer = ReceiptIssuer::from_key_bytes("beta7-positive-30s", &[37; 32]).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("beta7-positive-30s", &[37; 32]).unwrap();
    let pending = authorize_media_source(&request);
    let wire = issuer
        .issue(
            ReceiptClass::MediaSource,
            SubjectRevision::checked(pending.source_snapshot_sha256.as_deref().unwrap()).unwrap(),
            ReceiptScope::checked(media_source_scope(request.kind)).unwrap(),
            media_source_payload_digest(&request, &pending).unwrap(),
            1,
            None,
            None,
        )
        .unwrap();
    let authority =
        authorize_media_source_attested(&request, wire, &verifier, &mut ReplayGuard::default(), 1)
            .unwrap();
    let mut replay = ReplayGuard::default();
    let started = Instant::now();
    // 現direct APIと同じresources=None経路。funded/owned profileの主張ではありません。
    // Same direct resources=None path; no funded actor/lease claim is made.
    let report = execute_local_media_backend_attested_v3(
        &authority,
        &descriptor,
        &config,
        &budget,
        &issuer,
        &verifier,
        &mut replay,
        1,
    )
    .expect("the explicit 30-second supplemental positive must execute its synthetic backend");
    eprintln!(
        "supplemental positive: elapsed_ms={}, exit={:?}, stdout_truncated={}, stderr_truncated={}, tree_verified={}, memory_kind={:?}, memory_bytes={:?}",
        started.elapsed().as_millis(), report.process.exit_code,
        report.process.stdout_truncated, report.process.stderr_truncated,
        report.process.process_tree_kill_verified, report.process.memory_limit_kind,
        report.process.memory_limit_bytes,
    );
    assert_eq!(fs::read(&starts).unwrap(), b"x");
    assert_eq!(report.process.exit_code, Some(0));
    assert!(!report.process.stdout_truncated && !report.process.stderr_truncated);
    assert!(report.process.process_tree_kill_verified);
    assert_eq!(report.execution_plan.python_sha256, PYTHON_SHA256);
    assert_eq!(report.execution_plan.resource_budget, budget);
    assert_eq!(
        report.executed_family_states[&BackendFamily::Decode],
        BackendState::ExecutedCandidate
    );
    assert!(report.authenticity_attestation.is_some());
    assert_eq!(replay.observed_count(), 1);
    assert!(report.graph.observations().is_empty());
    assert_eq!(report.family_validation_count, 0);
    assert!(!report.identity_inference_committed && !report.internal_state_committed);
    let marker: serde_json::Value =
        serde_json::from_slice(&fs::read(&marker_path).unwrap()).unwrap();
    let actual_python = PathBuf::from(marker["sys_executable"].as_str().unwrap());
    assert_eq!(actual_python.canonicalize().unwrap(), expected_python);
    let argv = marker["argv"].as_array().unwrap();
    let expected_options = [
        "--media",
        "--ffmpeg",
        "--model-cache",
        "--mode",
        "--max-temp-bytes",
        "--max-frames",
        "--max-pcm-samples",
        "--max-observations",
        "--max-model-entries",
        "--max-model-file-bytes",
        "--max-model-total-bytes",
        "--max-model-metadata-bytes",
    ];
    assert_eq!(argv.len(), expected_options.len() * 2);
    for (index, option) in expected_options.iter().enumerate() {
        assert_eq!(argv[index * 2].as_str(), Some(*option));
    }
    // 同childの実flagを記録するだけで、別profileや未転送envの保証にしません。
    // Record the actual flag only; do not infer another profile or forwarded env guarantee.
    eprintln!(
        "supplemental marker dont_write_bytecode={}",
        marker["dont_write_bytecode"]
    );
    let current_epoch = 1u64.checked_add(started.elapsed().as_secs()).unwrap();
    verify_media_backend_with_current_authority_v3(
        &report,
        &authority,
        &verifier,
        &mut ReplayGuard::default(),
        current_epoch,
    )
    .unwrap();
    assert_eq!(fs::read(&source).unwrap(), SOURCE.as_bytes());
    // Keep only this uniquely created fixture for central inspection; no broad deletion.
}
