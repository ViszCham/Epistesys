//! Standalone funded CLI observation with a synthetic local Python backend.
#![forbid(unsafe_code)]

use lc631_host::HostReceiptContext;
use lc631_media::{
    authorize_media_source, media_source_payload_digest, media_source_scope,
    model_cache_manifest_sha256, MediaSourceKind, MediaSourceRequest,
};
use lc631_process_supervisor::{run_bounded, RunOutcome, RunRequest};
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptPolicy, ReceiptScope, ReplayGuard, SubjectRevision,
};
use serde_json::Value;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn process(executable: &Path, arguments: &[OsString], root: &Path) -> RunOutcome {
    run_bounded(RunRequest {
        executable: executable.as_os_str(),
        arguments,
        current_dir: root,
        environment: &std::env::vars_os().collect::<Vec<_>>(),
        stdin: None,
        timeout: Duration::from_secs(45),
        stdout_limit: 65_536,
        stderr_limit: 16_384,
    })
    .unwrap()
}

struct Fixture {
    root: PathBuf,
    source: PathBuf,
    source_digest: String,
    parent: String,
    context: HostReceiptContext,
    arguments: Vec<OsString>,
}

impl Fixture {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "epistesys-v3-owned-cli-{}-{nonce}",
            std::process::id(),
        ));
        std::fs::create_dir(&root).unwrap();
        let source = root.join("source.bin");
        std::fs::write(&source, b"source").unwrap();
        let models = root.join("models");
        std::fs::create_dir(&models).unwrap();
        std::fs::write(models.join("synthetic.bin"), b"model fixture").unwrap();
        let ffmpeg = root.join("ffmpeg.fixture");
        std::fs::write(&ffmpeg, b"not executed").unwrap();
        let model_digest = model_cache_manifest_sha256(&models).unwrap();
        let script = root.join("backend.py");
        let source_digest = lc631_core::stable_sha256("source");
        let payload = serde_json::json!({
            "schema_version":"lc631-media-backend.v1",
            "source_sha256":source_digest,
            "backend_id":"v3-cli-fixture",
            "backend_version":"fixture-v1",
            "model_revision":model_digest,
            "model_cache_manifest_sha256":model_digest,
            "model_card_license_hint":"synthetic test assertion only",
            "executed_families":["decode"],
            "executed_methods":{"decode":"ffmpeg-cli"},
            "observations":[],
            "diagnostics":["actual Python process; no ffmpeg/model execution"]
        });
        std::fs::write(&script, format!("print({:?})\n", payload.to_string())).unwrap();
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
        let discovered = process(
            &interpreter,
            &["-c".into(), "import sys;print(sys.executable)".into()],
            &root,
        );
        assert_eq!(discovered.exit_code, Some(0));
        assert!(!discovered.stdout_truncated && discovered.io_cleanup_complete);
        let python = PathBuf::from(String::from_utf8(discovered.stdout).unwrap().trim());
        let receipt_root = root.join("receipts");
        std::fs::create_dir(&receipt_root).unwrap();
        let context = HostReceiptContext::load_or_create(&receipt_root).unwrap();
        let request = MediaSourceRequest {
            kind: MediaSourceKind::LocalUserProvided,
            locator: source.to_str().unwrap().into(),
            user_asserts_rights: true,
            media_bytes_requested: true,
            network_requested: false,
        };
        let pending = authorize_media_source(&request);
        let now = epoch();
        let wire = context
            .issuer()
            .issue(
                ReceiptClass::MediaSource,
                SubjectRevision::checked(&source_digest).unwrap(),
                ReceiptScope::checked(media_source_scope(request.kind)).unwrap(),
                media_source_payload_digest(&request, &pending).unwrap(),
                now,
                Some(now + 300),
                None,
            )
            .unwrap();
        let parent = wire.receipt_digest();
        let source_receipt = root.join("source-receipt.json");
        std::fs::write(&source_receipt, serde_json::to_vec(&wire).unwrap()).unwrap();
        let arguments = vec![
            "--media".into(),
            source.clone().into_os_string(),
            "--rights-asserted".into(),
            "--source-receipt-file".into(),
            source_receipt.into_os_string(),
            "--receipt-root".into(),
            receipt_root.into_os_string(),
            "--python".into(),
            python.into_os_string(),
            "--ffmpeg".into(),
            ffmpeg.into_os_string(),
            "--backend-script".into(),
            script.into_os_string(),
            "--model-cache".into(),
            models.into_os_string(),
            "--backend-id".into(),
            "v3-cli-fixture".into(),
            "--model-license-receipt".into(),
            "synthetic local assertion only".into(),
        ];
        Self {
            root,
            source,
            source_digest,
            parent,
            context,
            arguments,
        }
    }

    fn run(&self, command: &str) -> RunOutcome {
        let mut arguments = vec![command.into()];
        arguments.extend_from_slice(&self.arguments);
        process(
            Path::new(env!("CARGO_BIN_EXE_lc631")),
            &arguments,
            &self.root,
        )
    }

    fn execute_report(
        &self,
    ) -> (
        lc631_media::MediaSourceAuthority,
        lc631_media::MediaBackendExecutionReceipt,
    ) {
        let request = MediaSourceRequest {
            kind: MediaSourceKind::LocalUserProvided,
            locator: self.source.to_str().unwrap().into(),
            user_asserts_rights: true,
            media_bytes_requested: true,
            network_requested: false,
        };
        let wire =
            serde_json::from_slice(&std::fs::read(self.root.join("source-receipt.json")).unwrap())
                .unwrap();
        let now = epoch();
        let authority = lc631_media::authorize_media_source_attested(
            &request,
            wire,
            self.context.verifier(),
            &mut ReplayGuard::default(),
            now,
        )
        .unwrap();
        let value = |flag: &str| {
            let position = self.arguments.iter().position(|arg| arg == flag).unwrap();
            PathBuf::from(&self.arguments[position + 1])
        };
        let config = lc631_media::MediaBackendConfig {
            python_executable: value("--python"),
            backend_script: value("--backend-script"),
            ffmpeg_executable: value("--ffmpeg"),
            model_cache: value("--model-cache"),
            timeout_seconds: 10,
        };
        let descriptor = lc631_media::BackendDescriptor::configured(
            lc631_media::BackendFamily::Decode,
            "v3-cli-fixture",
            Some(config.backend_script.clone()),
            Some(model_cache_manifest_sha256(&config.model_cache).unwrap()),
            Some("synthetic local assertion only".into()),
        );
        let report = lc631_media::execute_local_media_backend_attested_v3(
            &authority,
            &descriptor,
            &config,
            &lc631_media::MediaResourceBudget::default(),
            self.context.issuer(),
            self.context.verifier(),
            &mut ReplayGuard::default(),
            now,
        )
        .unwrap();
        (authority, report)
    }

    fn check_receipt(&self, result: RunOutcome, command: &str, scope: &str) {
        assert_eq!(
            result.exit_code,
            Some(0),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(!result.stdout_truncated && result.io_cleanup_complete);
        let output: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(output["command"], command);
        let report = &output["payload"];
        assert_eq!(report["source_sha256"], self.source_digest);
        assert_eq!(report["process"]["exit_code"], 0);
        assert_eq!(
            report["executed_family_states"]["decode"],
            "executed_candidate"
        );
        assert_eq!(report["family_validation_count"], 0);
        assert_eq!(report["identity_inference_committed"], false);
        assert_eq!(report["internal_state_committed"], false);
        let wire: lc631_receipt_kernel::UntrustedReceipt =
            serde_json::from_value(report["authenticity_attestation"].clone()).unwrap();
        assert_eq!(wire.claims().scope().as_str(), scope);
        assert_eq!(report["authenticity_receipt_digest"], wire.receipt_digest());
        let claimed_payload = wire.claims().payload_digest().to_owned();
        // Signature/source/scope/parent check only. The producer's full binary
        // report hashing is tested separately; do not claim independent decoding.
        self.context
            .verifier()
            .verify(
                wire,
                &ReceiptPolicy::exact(
                    ReceiptClass::MediaBackend,
                    SubjectRevision::checked(&self.source_digest).unwrap(),
                    ReceiptScope::checked(scope).unwrap(),
                    claimed_payload,
                    epoch(),
                )
                .with_parent(&self.parent),
                &mut ReplayGuard::default(),
            )
            .unwrap();
    }
}

#[test]
fn tlcx30_explicit_v3_cli_executes_funded_process_and_old_command_remains_v2() {
    let fixture = Fixture::new();
    fixture.check_receipt(
        fixture.run("lc631-media-execute-v3"),
        "lc631-media-execute-v3",
        "media/backend/v3/v3-cli-fixture",
    );
    fixture.check_receipt(
        fixture.run("lc631-media-execute"),
        "lc631-media-execute",
        "media/backend/v2/v3-cli-fixture",
    );
    std::fs::write(&fixture.source, b"changed source").unwrap();
    let rejected = fixture.run("lc631-media-execute-v3");
    assert_ne!(rejected.exit_code, Some(0));
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("media_source_authority"));
    eprintln!(
        "retained test-owned CLI fixture: {}",
        fixture.root.display()
    );
}

#[test]
fn tlcx30_v3_current_verifier_obeys_real_source_bound_ram_and_cancel_views() {
    use lc631_runtime::resource::{
        Cancellation, Deadline, ResourceAmount, ResourceBroker, ResourceKind, ResourceLimits,
        ResourceRequest,
    };
    let fixture = Fixture::new();
    let (authority, report) = fixture.execute_report();
    for case in 0..4 {
        let limits = ResourceLimits::uniform(1024 * 1024).unwrap();
        let broker = ResourceBroker::new(limits).unwrap();
        let cancellation = Cancellation::default();
        let source =
            lc631_core::context::SourceRevision::of(if case == 1 { "foreign" } else { "source" })
                .digest();
        let mut run = broker
            .begin_run(
                1,
                source,
                limits,
                Deadline::after(Duration::from_secs(30)).unwrap(),
                cancellation.clone(),
            )
            .unwrap();
        let mut reservation = run
            .reserve(&[ResourceRequest::new(
                ResourceKind::Ram,
                ResourceAmount::checked(if case == 0 { 65_535 } else { 65_536 }).unwrap(),
            )])
            .unwrap();
        if case == 2 {
            cancellation.cancel();
        }
        let mut replay = ReplayGuard::default();
        let result = lc631_media::attestation_v3::verify_media_backend_with_current_authority_v3_with_resources(
            &report, &authority, fixture.context.verifier(), &mut replay, epoch(),
            &reservation.lease(),
        );
        assert_eq!(result.is_ok(), case == 3, "resource case {case}");
        assert_eq!(replay.observed_count(), usize::from(case == 3));
        reservation.release_confirmed().unwrap();
        run.close_confirmed().unwrap();
        assert_eq!(broker.used(ResourceKind::Ram).unwrap(), 0);
    }
}
