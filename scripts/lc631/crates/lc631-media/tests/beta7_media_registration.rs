//! A registered local worker must be the script actually dispatched.
//! Only synthetic bytes and a local marker-writing Python fixture are used.
#![forbid(unsafe_code)]

use lc631_media::{
    authorize_media_source, authorize_media_source_attested,
    execute_local_media_backend_attested_v3, media_source_payload_digest, media_source_scope,
    model_cache_manifest_sha256, BackendDescriptor, BackendFamily, MediaBackendConfig, MediaError,
    MediaResourceBudget, MediaSourceKind, MediaSourceRequest,
};
use lc631_process_supervisor::{run_bounded, RunRequest};
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptIssuer, ReceiptScope, ReceiptVerifier, ReplayGuard, SubjectRevision,
};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn python(root: &Path) -> PathBuf {
    let name = if cfg!(windows) {
        "python.exe"
    } else {
        "python3"
    };
    let executable = std::env::var_os("EPISTESYS_TEST_MEDIA_PYTHON")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::split_paths(&std::env::var_os("PATH")?)
                .map(|dir| dir.join(name))
                .find(|p| p.is_file())
        })
        .expect("local fixture requires Python");
    let args = ["-c".into(), "import sys; print(sys.executable)".into()];
    let env = std::env::vars_os().collect::<Vec<_>>();
    let observed = run_bounded(RunRequest {
        executable: executable.as_os_str(),
        arguments: &args,
        current_dir: root,
        environment: &env,
        stdin: None,
        timeout: Duration::from_secs(10),
        stdout_limit: 4096,
        stderr_limit: 4096,
    })
    .unwrap();
    eprintln!(
        concat!(
            "python discovery: exit_code={:?}, timed_out={}, stdout={:?}, stderr={:?}, ",
            "stdout_truncated={}, stderr_truncated={}, io_cleanup_complete={}, ",
            "process_tree_kill_attempted={}, process_tree_kill_verified={}, diagnostic={:?}, ",
            "memory_limit_kind={:?}, memory_limit_bytes={:?}"
        ),
        observed.exit_code,
        observed.timed_out,
        String::from_utf8_lossy(&observed.stdout),
        String::from_utf8_lossy(&observed.stderr),
        observed.stdout_truncated,
        observed.stderr_truncated,
        observed.io_cleanup_complete,
        observed.process_tree_kill_attempted,
        observed.process_tree_kill_verified,
        observed.diagnostic,
        observed.memory_limit_kind,
        observed.memory_limit_bytes,
    );
    assert_eq!(observed.exit_code, Some(0));
    assert!(!observed.stdout_truncated && observed.io_cleanup_complete);
    PathBuf::from(String::from_utf8(observed.stdout).unwrap().trim())
}

fn launches(script: &Path) -> usize {
    match fs::read(script.with_extension("started")) {
        Ok(bytes) => {
            assert!(bytes.iter().all(|&b| b == b'x'));
            bytes.len()
        }
        Err(err) if err.kind() == ErrorKind::NotFound => 0,
        Err(err) => panic!("cannot read fixture invocation marker: {err}"),
    }
}

#[test]
fn beta7_media_registered_entrypoint_rejects_foreign_script_before_dispatch() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "epistesys-beta7-registered-script-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&root).unwrap();
    let source = root.join("source.bin");
    fs::write(&source, b"synthetic source").unwrap();
    let models = root.join("models");
    fs::create_dir(&models).unwrap();
    fs::write(models.join("fixture.bin"), b"synthetic model bytes").unwrap();
    let model = model_cache_manifest_sha256(&models).unwrap();
    let backend = "beta7-registered-script-fixture";
    let payload = serde_json::json!({
        "schema_version": "lc631-media-backend.v1",
        "source_sha256": lc631_core::stable_sha256("synthetic source"),
        "backend_id": backend,
        "backend_version": "fixture-v1",
        "model_revision": model,
        "model_cache_manifest_sha256": model,
        "model_card_license_hint": "synthetic fixture",
        "executed_families": ["decode"],
        "executed_methods": {"decode": "ffmpeg-cli"},
        "observations": [],
        "diagnostics": []
    });
    let registered = root.join("registered.py");
    let foreign = root.join("foreign.py");
    for (script, role) in [(&registered, "registered"), (&foreign, "foreign")] {
        fs::write(
            script,
            format!(
                "# {role}\nfrom pathlib import Path\nwith Path(__file__).with_suffix('.started').open('ab') as f:\n    f.write(b'x')\nprint({:?})\n",
                payload.to_string()
            ),
        )
        .unwrap();
    }
    let ffmpeg = root.join("ffmpeg.fixture");
    fs::write(&ffmpeg, b"fixture never invokes ffmpeg").unwrap();
    let config = MediaBackendConfig {
        python_executable: python(&root),
        backend_script: registered.clone(),
        ffmpeg_executable: ffmpeg,
        model_cache: models,
        timeout_seconds: 10,
    };
    let descriptor = BackendDescriptor::configured(
        BackendFamily::Decode,
        backend,
        Some(registered.clone()),
        Some(model),
        Some("synthetic license assertion".into()),
    );
    let request = MediaSourceRequest {
        kind: MediaSourceKind::LocalUserProvided,
        locator: source.to_str().unwrap().into(),
        user_asserts_rights: true,
        media_bytes_requested: true,
        network_requested: false,
    };
    let issuer = ReceiptIssuer::from_key_bytes("beta7-fixture", &[28; 32]).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("beta7-fixture", &[28; 32]).unwrap();
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
    let budget = MediaResourceBudget {
        max_stdout_bytes: 65536,
        max_stderr_bytes: 8192,
        max_temporary_bytes: 1024 * 1024,
        max_decoded_frames: 1,
        max_pcm_samples: 100,
        max_observations: 4,
    };
    let execute = |config: &MediaBackendConfig, replay: &mut ReplayGuard| {
        execute_local_media_backend_attested_v3(
            &authority,
            &descriptor,
            config,
            &budget,
            &issuer,
            &verifier,
            replay,
            1,
        )
    };
    let mut control = ReplayGuard::default();
    let receipt = execute(&config, &mut control).expect("the registered worker must run once");
    assert!(receipt.authenticity_attestation.is_some());
    assert_eq!(
        (
            launches(&registered),
            launches(&foreign),
            control.observed_count()
        ),
        (1, 0, 1)
    );

    // Keep the admitted descriptor, authority, model and budget. Substitute only
    // the script actually invoked; the foreign worker can emit a valid report.
    let other = MediaBackendConfig {
        backend_script: foreign.clone(),
        ..config
    };
    let mut replay = ReplayGuard::default();
    let result = execute(&other, &mut replay);
    eprintln!("foreign-script execution returned: {result:?}");
    let refused = matches!(result, Err(MediaError::BackendDescriptorMismatch));
    assert_eq!(
        (
            refused,
            launches(&registered),
            launches(&foreign),
            replay.observed_count()
        ),
        (true, 1, 0, 0),
        "foreign script must be rejected before child dispatch and backend replay"
    );
}
