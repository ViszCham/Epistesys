//! Owned synthetic local process fixture; no model or network acquisition.
#![allow(dead_code)] // Shared by separate integration targets with different assertions.
use lc631_core::{context::*, AuthorityRevision};
use lc631_host::HostReceiptContext;
use lc631_media::*;
use lc631_receipt_kernel::{ReceiptClass, ReceiptScope, ReplayGuard, SubjectRevision};
use lc631_runtime::{observation_bridge::media::*, resource::*};
use lc631_substrate::Digest32;
use sha2::{Digest, Sha256};
use std::{path::PathBuf, time::{Duration, SystemTime, UNIX_EPOCH}};

pub fn epoch() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() }
pub fn d(n: u8) -> Digest32 { Digest32::from_bytes([n; 32]) }
pub const TASK: &str = "Compare the timing of these local media candidates.";
pub fn consumer() -> ContextConsumer {
    ContextConsumer::new(OriginalContext::checked(
        SourceRevision::of(TASK), ContextScope::declared(d(1), d(2), d(3), d(4)),
        AuthorityRevision(1), vec![], ContextLimits { max_contexts: 8, max_premises: 16,
            max_edits: 8, max_statement_bytes: 4096 },
    ).unwrap()).unwrap()
}
pub struct Fixture {
    pub root: PathBuf,
    pub source: PathBuf,
    pub authority: MediaSourceAuthority,
    pub context: HostReceiptContext,
    pub descriptor: BackendDescriptor,
    pub config: MediaBackendConfig,
    pub budget: MediaResourceBudget,
    pub registration: MediaRegistration,
}
impl Fixture {
    pub fn new(second_start: i64, empty: bool) -> Self {
        let root = std::env::temp_dir().join(format!("epistesys-tlcx30-media-{}-{}",
            std::process::id(), SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir(&root).unwrap();
        let source = root.join("source.bin");
        std::fs::write(&source, b"source").unwrap();
        let models = root.join("models");
        std::fs::create_dir(&models).unwrap();
        std::fs::write(models.join("fixture.bin"), b"synthetic").unwrap();
        let model = model_cache_manifest_sha256(&models).unwrap();
        let ffmpeg = root.join("ffmpeg.fixture");
        std::fs::write(&ffmpeg, b"not executed").unwrap();
        let observations = if empty { vec![] } else { vec![
            serde_json::json!({"observation_id":"first","kind":"shot_boundary","start_ms":0,
                "end_ms":1000,"track_id":null,"label":"first candidate","evidence_digest":lc631_core::stable_sha256("first")}),
            serde_json::json!({"observation_id":"second","kind":"shot_boundary","start_ms":second_start,
                "end_ms":2000,"track_id":null,"label":"second candidate","evidence_digest":lc631_core::stable_sha256("second")}),
        ] };
        let payload = serde_json::json!({"schema_version":"lc631-media-backend.v1",
            "source_sha256":lc631_core::stable_sha256("source"),"backend_id":"tlcx30-fixture",
            "backend_version":"fixture-v1","model_revision":model,"model_cache_manifest_sha256":model,
            "model_card_license_hint":"synthetic fixture only","executed_families":["shot_boundary"],
            "executed_methods":{"shot_boundary":"opencv-hsv-histogram-bhattacharyya"},
            "observations":observations,"diagnostics":["actual Python; no model execution"]});
        let script = root.join("backend.py");
        std::fs::write(&script, format!("from pathlib import Path\nPath(__file__).with_suffix('.ran').write_text('executed')\nprint({:?})\n", payload.to_string())).unwrap();
        let executable = if cfg!(windows) { "python.exe" } else { "python3" };
        let python = std::env::var_os("EPISTESYS_TEST_MEDIA_PYTHON").map(PathBuf::from)
            .or_else(|| std::env::split_paths(&std::env::var_os("PATH")?).map(|p| p.join(executable)).find(|p| p.is_file()))
            .expect("installed absolute Python required; no acquisition");
        let config = MediaBackendConfig { python_executable: python, backend_script: script,
            ffmpeg_executable: ffmpeg, model_cache: models, timeout_seconds: 10 };
        let budget = MediaResourceBudget { max_stdout_bytes: 65536, max_stderr_bytes: 16384,
            max_temporary_bytes: 1024 * 1024, max_decoded_frames: 2, max_pcm_samples: 1024, max_observations: 8 };
        let hash = |p: &PathBuf| {
            let digest = Sha256::digest(std::fs::read(p).unwrap());
            let mut value = String::with_capacity(71);
            value.push_str("sha256:");
            for byte in digest.iter() { value.push_str(&format!("{byte:02x}")); }
            value
        };
        let configuration = lc631_core::stable_sha256(&format!("{}\0{}\0{}\0{}\0{}\0{:?}",
            hash(&config.python_executable), hash(&config.backend_script), hash(&config.ffmpeg_executable), model, config.timeout_seconds, budget));
        let registration = MediaRegistration { backend: "tlcx30-fixture".into(),
            model: model.clone(), configuration };
        let descriptor = BackendDescriptor::configured(BackendFamily::ShotBoundary,
            "tlcx30-fixture", Some(config.backend_script.clone()), Some(model), Some("fixture rights assertion".into()));
        let receipts = root.join("receipts");
        std::fs::create_dir(&receipts).unwrap();
        let context = HostReceiptContext::load_or_create(&receipts).unwrap();
        let request = MediaSourceRequest { kind: MediaSourceKind::LocalUserProvided,
            locator: source.to_str().unwrap().into(), user_asserts_rights: true,
            media_bytes_requested: true, network_requested: false };
        let pending = authorize_media_source(&request);
        let now = epoch();
        let wire = context.issuer().issue(ReceiptClass::MediaSource,
            SubjectRevision::checked(lc631_core::stable_sha256("source")).unwrap(),
            ReceiptScope::checked(media_source_scope(request.kind)).unwrap(),
            media_source_payload_digest(&request, &pending).unwrap(), now, Some(now + 300), None).unwrap();
        let authority = authorize_media_source_attested(&request, wire, context.verifier(),
            &mut ReplayGuard::default(), now).unwrap();
        eprintln!("retained owned media fixture: {}", root.display());
        Self { root, source, authority, context, descriptor, config, budget, registration }
    }
    pub fn binding(&self, consumer: &ContextConsumer) -> MediaContextBinding {
        MediaContextBinding { task_source: SourceRevision::of(TASK).digest(),
            parent: consumer.original().state().revision(), authority: AuthorityRevision(1),
            media_source: SourceRevision::of("source").digest() }
    }
    pub fn funded<T>(&self, cancellation: Cancellation, f: impl FnOnce(&ResourceReservationView<'_>) -> T) -> T {
        let cap = ResourceLimits::uniform(64 * 1024 * 1024).unwrap();
        let broker = ResourceBroker::new(cap).unwrap();
        let live_cancellation = if cancellation.is_cancelled() { Cancellation::default() } else { cancellation.clone() };
        let mut run = broker.begin_run(1, SourceRevision::of(TASK).digest(), cap,
            Deadline::after(Duration::from_secs(60)).unwrap(), live_cancellation.clone()).unwrap();
        let mut reservation = run.reserve_for_source(SourceRevision::of("source").digest(), &[
            (ResourceKind::Ram, 32 * 1024 * 1024), (ResourceKind::Output, 16 * 1024 * 1024),
            // Existing source-snapshot and child owners each hold Cleanup(1) concurrently.
            (ResourceKind::Disk, 2 * 1024 * 1024), (ResourceKind::Child, 1), (ResourceKind::Cleanup, 2),
        ].map(|(kind, amount)| ResourceRequest::new(kind, ResourceAmount::checked(amount).unwrap()))).unwrap();
        if cancellation.is_cancelled() { live_cancellation.cancel(); }
        let result = f(&reservation.lease());
        reservation.release_confirmed().unwrap();
        run.close_confirmed().unwrap();
        assert_eq!(broker.used(ResourceKind::Ram).unwrap(), 0);
        result
    }
    pub fn execute<'view>(&self, view: &'view ResourceReservationView<'view>,
        consumer: &mut ContextConsumer) -> Result<ExecutedMediaAdmission<'view>, MediaConnectionError> {
        let binding = self.binding(consumer);
        observe_registered_media(MediaExecutionRequest {
            binding, registration: &self.registration, source_authority: &self.authority,
            descriptor: &self.descriptor, config: &self.config, budget: &self.budget,
            limits: MediaAdmissionLimits { max_observations: 8, max_label_bytes: 4096 },
        }, self.context.issuer(), self.context.verifier(), &mut ReplayGuard::default(), epoch(), view, consumer)
    }
}
