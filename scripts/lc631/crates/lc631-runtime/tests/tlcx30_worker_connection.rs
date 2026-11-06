//! Actual resource and pinned-worker admission failures are not NLP positives.
use lc631_core::context::{
    ContextConsumer, ContextLimits, ContextRevision, ContextScope, OriginalContext, SourceRevision,
};
use lc631_core::AuthorityRevision;
use lc631_runtime::observation_bridge::worker::{
    observe_registered_language, LanguageExecutionError, LanguageExecutionRequest,
};
use lc631_runtime::observation_bridge::{
    LanguageRegistration, ObservationBridgeError, ObservationBridgeLimits,
};
use lc631_runtime::resource::{
    Cancellation, Deadline, ResourceAmount, ResourceBroker, ResourceKind, ResourceLimits,
    ResourceRequest,
};
use lc631_substrate::{source::SourceStore, Digest32};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const TEXT: &str = "alpha";
fn d(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}
fn context() -> (SourceStore, ContextConsumer) {
    let store = SourceStore::new(Arc::from(TEXT), 256).unwrap();
    let original = OriginalContext::checked(
        SourceRevision::of(TEXT),
        ContextScope::declared(d(1), d(2), d(3), d(4)),
        AuthorityRevision(1),
        vec![],
        ContextLimits {
            max_contexts: 4,
            max_premises: 8,
            max_edits: 4,
            max_statement_bytes: 256,
        },
    )
    .unwrap();
    (store, ContextConsumer::new(original).unwrap())
}
fn request(source: &SourceStore, parent: ContextRevision) -> LanguageExecutionRequest<'_> {
    LanguageExecutionRequest {
        source,
        parent,
        authority: AuthorityRevision(1),
        registration: LanguageRegistration {
            language: "en",
            backend: "stanza-1.14.0",
            model: d(5),
            runtime: d(6),
            requirements: d(7),
        },
        limits: ObservationBridgeLimits {
            max_sentences: 4,
            max_tokens: 4,
            max_fragment_bytes: 64,
        },
        timeout_ms: 1_000,
    }
}
fn root() -> PathBuf {
    let base = std::env::temp_dir().canonicalize().unwrap();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    for attempt in 0..64 {
        let path = base.join(format!(
            "epistesys-tlcx30-worker-{}-{stamp}-{attempt}",
            std::process::id()
        ));
        match std::fs::create_dir(&path) {
            Ok(()) => {
                eprintln!("retained owned worker fixture: {}", path.display());
                return path;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => panic!("worker fixture directory: {error}"),
        }
    }
    panic!("worker fixture directory collision budget exhausted");
}

#[test]
fn tlcx30_actual_missing_worker_pins_retains_error_and_empty_context() {
    let (source, mut consumer) = context();
    let parent = consumer.original().state().revision();
    let cap = ResourceLimits::uniform(16 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(cap).unwrap();
    let mut run = broker
        .begin_run(
            3001,
            source.id().digest(),
            cap,
            Deadline::after(Duration::from_secs(10)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let mut reservation = run
        .reserve(&[ResourceKind::Ram, ResourceKind::Output].map(|kind| {
            ResourceRequest::new(kind, ResourceAmount::checked(2 * 1024 * 1024).unwrap())
        }))
        .unwrap();
    let owned = root();
    {
        let lease = reservation.lease();
        let result =
            observe_registered_language(&owned, request(&source, parent), &lease, &mut consumer);
        assert!(matches!(result, Err(LanguageExecutionError::Worker(_))));
        assert_eq!(consumer.states().len(), 1);
        assert!(consumer.original().state().premises().is_empty());
    }
    // The actual loader must not acquire models or manufacture pin files.
    assert_eq!(std::fs::read_dir(&owned).unwrap().count(), 0);
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}

#[test]
fn tlcx30_actual_cancelled_lease_prevents_pins_io_and_admission() {
    let (source, mut consumer) = context();
    let parent = consumer.original().state().revision();
    let cap = ResourceLimits::uniform(16 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(cap).unwrap();
    let cancel = Cancellation::default();
    let mut run = broker
        .begin_run(
            3002,
            source.id().digest(),
            cap,
            Deadline::after(Duration::from_secs(10)).unwrap(),
            cancel.clone(),
        )
        .unwrap();
    let mut reservation = run
        .reserve(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(2 * 1024 * 1024).unwrap(),
        )])
        .unwrap();
    cancel.cancel();
    {
        let lease = reservation.lease();
        let result = observe_registered_language(
            Path::new("missing-worker-root"),
            request(&source, parent),
            &lease,
            &mut consumer,
        );
        assert!(matches!(result, Err(LanguageExecutionError::Resource(_))));
        assert_eq!(consumer.states().len(), 1);
    }
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}

#[test]
fn tlcx30_source_parent_authority_and_registration_cuts_precede_backend() {
    let (source, mut consumer) = context();
    let parent = consumer.original().state().revision();
    let cap = ResourceLimits::uniform(16 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(cap).unwrap();
    let mut run = broker
        .begin_run(
            3003,
            source.id().digest(),
            cap,
            Deadline::after(Duration::from_secs(10)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let mut reservation = run
        .reserve(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(2 * 1024 * 1024).unwrap(),
        )])
        .unwrap();
    let foreign = SourceStore::new(Arc::from("other"), 256).unwrap();
    {
        let lease = reservation.lease();
        let mut cut = request(&foreign, parent);
        assert!(matches!(
            observe_registered_language(
                Path::new("missing-worker-root"),
                cut,
                &lease,
                &mut consumer
            ),
            Err(LanguageExecutionError::Bridge(
                ObservationBridgeError::SourceMismatch
            ))
        ));
        cut = request(&source, parent.reconstructed([99; 32]));
        assert!(matches!(
            observe_registered_language(
                Path::new("missing-worker-root"),
                cut,
                &lease,
                &mut consumer
            ),
            Err(LanguageExecutionError::UnknownParent)
        ));
        cut = request(&source, parent);
        cut.authority = AuthorityRevision(2);
        assert!(matches!(
            observe_registered_language(
                Path::new("missing-worker-root"),
                cut,
                &lease,
                &mut consumer
            ),
            Err(LanguageExecutionError::AuthorityRevisionMismatch)
        ));
        cut = request(&source, parent);
        cut.registration.language = "unsupported";
        assert!(matches!(
            observe_registered_language(
                Path::new("missing-worker-root"),
                cut,
                &lease,
                &mut consumer
            ),
            Err(LanguageExecutionError::Bridge(
                ObservationBridgeError::RegistrationMismatch
            ))
        ));
        assert_eq!(consumer.states().len(), 1);
    }
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}

fn pin_digest(text: &str) -> Digest32 {
    let hex = text.strip_prefix("sha256:").unwrap();
    assert_eq!(hex.len(), 64);
    let mut bytes = [0_u8; 32];
    for (i, slot) in bytes.iter_mut().enumerate() {
        *slot = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap();
    }
    Digest32::from_bytes(bytes)
}

#[test]
fn tlcx30_configured_ja_en_worker_admits_only_proposed_fragments() {
    if !cfg!(windows) || std::env::var("EPISTESYS_TEST_CONFIGURED_NLP").as_deref() != Ok("1") {
        // Portable mode really exercises missing pins; it is NOT a configured
        // worker positive. The central configured gate MUST set the flag.
        tlcx30_actual_missing_worker_pins_retains_error_and_empty_context();
        eprintln!("TLCX30 portable missing-profile diagnostic only; configured JA/EN positive remains unobserved");
        return;
    }
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .unwrap();
    let pins: serde_json::Value = serde_json::from_slice(
        &std::fs::read(repository.join("validation/dgcl-nlp-model-pins.v1.json")).unwrap(),
    )
    .unwrap();
    let runtime = pin_digest(pins["runtime_manifest"]["sha256"].as_str().unwrap());
    let requirements = pin_digest(pins["requirements_lock"]["sha256"].as_str().unwrap());
    for (id, language, text) in [
        (3010, "en", "Please verify the result."),
        (3011, "ja", "結果を確認してください。"),
    ] {
        let source = SourceStore::new(Arc::from(text), 256).unwrap();
        let span = lc631_substrate::source::SourceSpan::checked(&source, 0, text.len()).unwrap();
        let hard = lc631_core::context::ContextPremise::from_source(
            d(25),
            &source,
            span,
            lc631_core::context::PremiseBasis::Given,
            false,
        )
        .unwrap();
        let original = OriginalContext::checked(
            SourceRevision::of(text),
            ContextScope::declared(d(1), d(2), d(3), d(4)),
            AuthorityRevision(1),
            vec![hard],
            ContextLimits {
                max_contexts: 4,
                max_premises: 16,
                max_edits: 8,
                max_statement_bytes: 4096,
            },
        )
        .unwrap();
        let mut consumer = ContextConsumer::new(original).unwrap();
        let parent = consumer.original().state().revision();
        let cap = ResourceLimits::uniform(128 * 1024 * 1024).unwrap();
        let broker = ResourceBroker::new(cap).unwrap();
        let mut run = broker
            .begin_run(
                id,
                source.id().digest(),
                cap,
                Deadline::after(Duration::from_secs(90)).unwrap(),
                Cancellation::default(),
            )
            .unwrap();
        let mut reservation = run
            .reserve(&[
                ResourceRequest::new(
                    ResourceKind::Ram,
                    ResourceAmount::checked(16 * 1024 * 1024).unwrap(),
                ),
                ResourceRequest::new(
                    ResourceKind::Output,
                    ResourceAmount::checked(4 * 1024 * 1024).unwrap(),
                ),
                ResourceRequest::new(ResourceKind::Child, ResourceAmount::checked(1).unwrap()),
                ResourceRequest::new(ResourceKind::Cleanup, ResourceAmount::checked(1).unwrap()),
            ])
            .unwrap();
        {
            let view = reservation.lease();
            let admission = observe_registered_language(
                repository,
                LanguageExecutionRequest {
                    source: &source,
                    parent,
                    authority: AuthorityRevision(1),
                    registration: LanguageRegistration {
                        language,
                        backend: "stanza-1.14.0",
                        model: pin_digest(pins["manifests"][language].as_str().unwrap()),
                        runtime,
                        requirements,
                    },
                    limits: ObservationBridgeLimits {
                        max_sentences: 8,
                        max_tokens: 128,
                        max_fragment_bytes: 4096,
                    },
                    timeout_ms: 60_000,
                },
                &view,
                &mut consumer,
            )
            .unwrap();
            assert!(admission.candidate_count() > 0);
            let lc631_core::context::ContextAdmission::Inserted(revision) = admission.admission()
            else {
                panic!("fresh actual fragment must be admitted")
            };
            let branch = consumer.get(revision).unwrap();
            assert_eq!(branch.premises().len(), 1 + admission.candidate_count());
            assert_eq!(consumer.original().state().premises().len(), 1);
            for p in &branch.premises()[1..] {
                assert_eq!(p.basis(), lc631_core::context::PremiseBasis::Proposed);
                assert!(p.source_span().is_some());
            }
            let actual = admission.observation();
            assert_eq!(actual.language, language);
            assert_eq!(actual.source_revision, lc631_core::stable_sha256(text));
            assert_eq!(actual.process.state, lc631_analysis::ToolRunState::Observed);
            assert_eq!(actual.process.exit_code, Some(0));
            assert!(!actual.semantic_truth_claim);
            assert!(!actual.sentences.is_empty());
            eprintln!("TLCX30 configured actual language={language}, source_bytes={}, candidates={}, process_exit=0, basis=Proposed, semantic_truth=false",
                text.len(),admission.candidate_count());
        }
        reservation.release_confirmed().unwrap();
        run.close_confirmed().unwrap();
    }
}
