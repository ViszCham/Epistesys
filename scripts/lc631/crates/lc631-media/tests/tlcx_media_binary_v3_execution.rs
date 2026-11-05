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
