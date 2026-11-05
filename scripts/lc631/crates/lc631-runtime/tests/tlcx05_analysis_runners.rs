//! Tests for explicitly injected high-level runners. Configured JA/EN tests
//! are a separate opt-in profile, never counted as observed by a portable run.
use lc631_analysis::{
    analyze_path_with_resources, analyze_with_resources, build_dgcl_pipeline_with_runner,
    load_dgcl_runtime_pins, run_bounded_tool_with_resources, run_language_worker_with_resources,
    AnalysisRequest, BoundedToolRequest, DgclBackendSet, DgclWorkerRunner, LanguageObservation,
    LanguageWorkerConfig, LanguageWorkerError, ResourceToolResult, ToolProcessRunner, ToolRunState,
};
use lc631_core::context::SourceRevision;
use lc631_runtime::resource::{
    Cancellation, Deadline, ResourceAmount, ResourceBroker, ResourceError, ResourceKind,
    ResourceLimits, ResourceRequest, RunResources,
};
use std::time::Duration;

fn quota(kind: ResourceKind, amount: u64) -> ResourceRequest {
    ResourceRequest::new(kind, ResourceAmount::checked(amount).unwrap())
}
fn owner(source: &str) -> (ResourceBroker, RunResources, Cancellation) {
    let limits =
        ResourceLimits::checked([128 * 1024 * 1024, 0, 16 * 1024 * 1024, 0, 2, 2]).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let cancel = Cancellation::default();
    let run = broker
        .begin_run(
            1,
            SourceRevision::of(source).digest(),
            limits,
            Deadline::after(Duration::from_secs(180)).unwrap(),
            cancel.clone(),
        )
        .unwrap();
    (broker, run, cancel)
}

struct CancelAtTool<'a> {
    run: &'a RunResources,
    cancel: Cancellation,
    calls: usize,
}
impl ToolProcessRunner for CancelAtTool<'_> {
    fn with_input(
        &mut self,
        _source: &str,
        _cap: usize,
        _consume: &mut dyn FnMut(
            &lc631_runtime::resource::ResourceReservationView<'_>,
        ) -> Result<(), ResourceError>,
    ) -> Result<(), ResourceError> {
        self.cancel.cancel();
        Err(ResourceError::Cancelled)
    }
    fn run(
        &mut self,
        source: &str,
        _request: BoundedToolRequest<'_>,
        _consume: &mut dyn FnMut(&ResourceToolResult<'_>) -> Result<(), ResourceError>,
    ) -> Result<(), ResourceError> {
        self.calls += 1;
        self.cancel.cancel();
        match self.run.reserve_for_source(
            SourceRevision::of(source).digest(),
            &[quota(ResourceKind::Child, 1)],
        ) {
            Err(error) => Err(error),
            Ok(_) => panic!("cancelled task must reject admission"),
        }
    }
}

#[test]
fn tlcx05_analysis_high_level_domain_routes_never_fallback_after_resource_failure() {
    for (name, source, prompt) in [
        (
            "unit.rs",
            "pub fn value() -> u8 { 1 }",
            "Analyze Rust source",
        ),
        (
            "unit.py",
            "def value():\n    return 1\n",
            "Analyze Python source",
        ),
        (
            "unit.s",
            ".text\nmov rax, rbx\nret\n",
            "Analyze x86_64 assembly source",
        ),
    ] {
        let (broker, mut run, cancel) = owner(source);
        let mut runner = CancelAtTool {
            run: &run,
            cancel,
            calls: 0,
        };
        let result = analyze_with_resources(
            AnalysisRequest {
                prompt,
                repo: None,
                validation: None,
                release: None,
                source_name: Some(name),
                source: Some(source),
            },
            &mut runner,
        );
        assert!(matches!(result, Err(ResourceError::Cancelled)), "{name}");
        assert_eq!(runner.calls, 1, "no hidden legacy tool invocation: {name}");
        assert_eq!(broker.used(ResourceKind::Child).unwrap(), 0);
        run.close_confirmed().unwrap();
    }
}

#[test]
fn tlcx05_analysis_path_route_uses_the_injected_runner() {
    let source = "pub fn value() -> u8 { 1 }";
    let (_, mut run, cancel) = owner(source);
    let mut runner = CancelAtTool {
        run: &run,
        cancel,
        calls: 0,
    };
    assert!(matches!(
        analyze_path_with_resources(
            AnalysisRequest {
                prompt: "Analyze Rust source",
                repo: None,
                validation: None,
                release: None,
                source_name: Some("unit.rs"),
                source: Some(source)
            },
            None,
            false,
            &mut runner
        ),
        Err(ResourceError::Cancelled)
    ));
    assert_eq!(runner.calls, 1);
    run.close_confirmed().unwrap();
}

#[test]
fn tlcx05_repo_only_route_rejects_cancelled_manifest_ingress_before_any_child() {
    let source = "repo-only ingress";
    let (_, mut run, cancel) = owner(source);
    let mut runner = CancelAtTool {
        run: &run,
        cancel,
        calls: 0,
    };
    let result = analyze_with_resources(
        AnalysisRequest {
            prompt: "Inspect repository",
            repo: Some("."),
            validation: None,
            release: None,
            source_name: None,
            source: None,
        },
        &mut runner,
    );
    assert!(matches!(result, Err(ResourceError::Cancelled)));
    assert_eq!(
        runner.calls, 0,
        "cancelled ingress cannot fall through to cargo"
    );
    run.close_confirmed().unwrap();
}

struct SkipCallbacks;
impl ToolProcessRunner for SkipCallbacks {
    fn run(
        &mut self,
        _source: &str,
        _request: BoundedToolRequest<'_>,
        _consume: &mut dyn FnMut(&ResourceToolResult<'_>) -> Result<(), ResourceError>,
    ) -> Result<(), ResourceError> {
        Ok(())
    }
    fn with_input(
        &mut self,
        _source: &str,
        _cap: usize,
        _consume: &mut dyn FnMut(
            &lc631_runtime::resource::ResourceReservationView<'_>,
        ) -> Result<(), ResourceError>,
    ) -> Result<(), ResourceError> {
        Ok(())
    }
}

#[test]
fn tlcx05_skipped_input_or_process_callback_cannot_manufacture_a_report() {
    for repo in [None, Some(".")] {
        let result = analyze_with_resources(
            AnalysisRequest {
                prompt: "Inspect Rust",
                repo,
                validation: None,
                release: None,
                source_name: repo.is_none().then_some("unit.rs"),
                source: repo.is_none().then_some("pub fn value()->u8 {1}"),
            },
            &mut SkipCallbacks,
        );
        assert!(matches!(result, Err(ResourceError::InvalidConfig)));
    }
}

struct ReboundResult<'a> {
    run: &'a RunResources,
    foreign_source: bool,
}
impl ToolProcessRunner for ReboundResult<'_> {
    fn run(
        &mut self,
        source: &str,
        request: BoundedToolRequest<'_>,
        consume: &mut dyn FnMut(&ResourceToolResult<'_>) -> Result<(), ResourceError>,
    ) -> Result<(), ResourceError> {
        let digest = SourceRevision::of(if self.foreign_source {
            "different source"
        } else {
            source
        })
        .digest();
        let mut reservation = self.run.reserve_for_source(
            digest,
            &[
                quota(ResourceKind::Ram, 1024 * 1024),
                quota(ResourceKind::Output, 8192),
                quota(ResourceKind::Child, 1),
                quota(ResourceKind::Cleanup, 1),
            ],
        )?;
        let view = reservation.lease();
        let capture = run_bounded_tool_with_resources(
            BoundedToolRequest {
                program: "epistesys-explicitly-absent-rebinding-test-05",
                ..request
            },
            &view,
        )?;
        let result = consume(&capture);
        drop(capture);
        reservation.release_confirmed()?;
        result
    }
    fn with_input(
        &mut self,
        _source: &str,
        _cap: usize,
        _consume: &mut dyn FnMut(
            &lc631_runtime::resource::ResourceReservationView<'_>,
        ) -> Result<(), ResourceError>,
    ) -> Result<(), ResourceError> {
        Err(ResourceError::InvalidConfig)
    }
}

#[test]
fn tlcx05_result_rebinding_across_source_or_command_is_rejected_before_consumption() {
    for foreign_source in [false, true] {
        let source = "pub fn value()->u8 {1}";
        let (_, mut run, _) = owner(source);
        let mut runner = ReboundResult {
            run: &run,
            foreign_source,
        };
        let result = analyze_with_resources(
            AnalysisRequest {
                prompt: "Inspect Rust",
                repo: None,
                validation: None,
                release: None,
                source_name: Some("unit.rs"),
                source: Some(source),
            },
            &mut runner,
        );
        assert!(matches!(result, Err(ResourceError::SourceMismatch)));
        run.close_confirmed().unwrap();
    }
}

#[test]
fn tlcx05_analysis_missing_optional_tool_is_not_an_observation_or_a_quota_error() {
    let source = "optional tool capability";
    let (_, mut run, _) = owner(source);
    let mut reservation = run
        .reserve_for_source(
            SourceRevision::of(source).digest(),
            &[
                quota(ResourceKind::Ram, 1024 * 1024),
                quota(ResourceKind::Output, 8192),
                quota(ResourceKind::Child, 1),
                quota(ResourceKind::Cleanup, 1),
            ],
        )
        .unwrap();
    let view = reservation.lease();
    let result = run_bounded_tool_with_resources(
        BoundedToolRequest {
            program: "epistesys-explicitly-absent-optional-capability-05",
            args: &["--version"],
            stdin: None,
            timeout_ms: 2000,
            stdout_limit: 4096,
            stderr_limit: 4096,
        },
        &view,
    )
    .unwrap();
    assert_eq!(result.reported().state, ToolRunState::Unavailable);
    assert_eq!(result.reported().exit_code, None);
    assert!(!result.reported().process_tree_kill_attempted);
    assert!(result.stdout().is_empty());
    drop(result);
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}

#[test]
fn tlcx05_optional_result_retains_its_exact_metadata_storage_claim() {
    let source = "optional metadata storage";
    let (_, mut run, _) = owner(source);
    let amount = u64::try_from(std::mem::size_of::<lc631_analysis::BoundedToolResult>()).unwrap();
    let mut reservation = run
        .reserve_for_source(
            SourceRevision::of(source).digest(),
            &[quota(ResourceKind::Ram, amount)],
        )
        .unwrap();
    let view = reservation.lease();
    let result = run_bounded_tool_with_resources(
        BoundedToolRequest {
            program: "epistesys-explicitly-absent-metadata-storage-05",
            args: &[],
            stdin: None,
            timeout_ms: 2000,
            stdout_limit: 1,
            stderr_limit: 1,
        },
        &view,
    )
    .unwrap();
    assert_eq!(result.reported().state, ToolRunState::Unavailable);
    assert!(matches!(
        view.claim_resources(&[quota(ResourceKind::Ram, 1)]),
        Err(ResourceError::Exhausted(ResourceKind::Ram))
    ));
    drop(result);
    let released = view
        .claim_resources(&[quota(ResourceKind::Ram, 1)])
        .unwrap();
    drop(released);
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}

struct BoundWorker<'a> {
    run: &'a RunResources,
    wrong_source: bool,
    calls: Vec<(String, String)>,
}
impl DgclWorkerRunner for BoundWorker<'_> {
    fn run(
        &mut self,
        source: &str,
        language: &str,
        config: &LanguageWorkerConfig,
    ) -> Result<LanguageObservation, LanguageWorkerError> {
        self.calls.push((language.into(), source.into()));
        let requests = [
            quota(ResourceKind::Ram, 16 * 1024 * 1024),
            quota(ResourceKind::Output, 4 * 1024 * 1024),
            quota(ResourceKind::Child, 1),
            quota(ResourceKind::Cleanup, 1),
        ];
        let mut reservation = if self.wrong_source {
            self.run.reserve(&requests)
        } else {
            self.run
                .reserve_for_source(SourceRevision::of(source).digest(), &requests)
        }
        .map_err(|e| LanguageWorkerError::Resource(format!("{e:?}")))?;
        let result =
            run_language_worker_with_resources(source, language, config, &reservation.lease());
        reservation
            .release_confirmed()
            .map_err(|e| LanguageWorkerError::Resource(format!("{e:?}")))?;
        result
    }
}

fn configured_profile_requested() -> bool {
    cfg!(windows) && std::env::var("EPISTESYS_TEST_CONFIGURED_NLP").as_deref() == Ok("1")
}

// Always-run, resource wiring only. It cannot return LanguageObservation and
// deliberately leaves all NLP backend semantics unobserved/unsupported.
struct RecordingSourceObserver<'a> {
    run: &'a RunResources,
    broker: &'a ResourceBroker,
    wrong_source: bool,
    calls: Vec<(String, String)>,
    held: Vec<lc631_runtime::resource::Reservation>,
}
impl lc631_analysis::DgclSourceObserver for RecordingSourceObserver<'_> {
    fn inspect(&mut self, source: &str, language: &str) -> Result<(), LanguageWorkerError> {
        const CAP: u64 = 4096;
        self.calls.push((language.into(), source.into()));
        let requests = [quota(ResourceKind::Ram, CAP)];
        let mut reservation = if self.wrong_source {
            self.run.reserve(&requests)
        } else {
            self.run
                .reserve_for_source(SourceRevision::of(source).digest(), &requests)
        }
        .map_err(|e| LanguageWorkerError::Resource(format!("{e:?}")))?;
        let checked = (|| {
            let view = reservation.lease();
            let mut bytes = std::io::Cursor::new(source.as_bytes());
            let input = lc631_analysis::bounded_input::AnalysisText::read(
                &mut bytes,
                &view,
                SourceRevision::of(source).digest(),
                CAP as usize,
            )
            .map_err(|e| LanguageWorkerError::Resource(format!("{e:?}")))?;
            assert_eq!(input.as_str().unwrap(), source);
            assert_eq!(view.source_revision(), SourceRevision::of(source).digest());
            Ok(())
        })();
        if let Err(error) = checked {
            reservation
                .release_confirmed()
                .map_err(|e| LanguageWorkerError::Resource(format!("{e:?}")))?;
            return Err(error);
        }
        // These reservations intentionally overlap, proving that adding a
        // second synthetic payload did not create another quota root/run.
        self.held.push(reservation);
        assert_eq!(
            self.broker.used(ResourceKind::Ram).unwrap(),
            CAP * self.held.len() as u64
        );
        assert!(matches!(
            self.run.reserve_for_source(
                SourceRevision::of(source).digest(),
                &[quota(ResourceKind::Ram, 128 * 1024 * 1024)]
            ),
            Err(ResourceError::Exhausted(ResourceKind::Ram))
        ));
        Ok(())
    }
}

#[test]
fn tlcx05_portable_ja_en_resource_wiring_preserves_exact_payloads_without_backend_claims() {
    let source = "Please verify the result.\n\n結果を確認してください。";
    let (broker, mut run, _) = owner(source);
    let mut observer = RecordingSourceObserver {
        run: &run,
        broker: &broker,
        wrong_source: false,
        calls: Vec::new(),
        held: Vec::new(),
    };
    let report =
        lc631_analysis::build_dgcl_pipeline_with_source_observer(source, &mut observer).unwrap();
    assert_eq!(observer.calls.len(), 2);
    assert_eq!(observer.calls[0].0, "en");
    assert_eq!(observer.calls[0].1.trim(), "Please verify the result.");
    assert_eq!(observer.calls[1].0, "ja");
    assert_eq!(observer.calls[1].1.trim(), "結果を確認してください。");
    assert!(observer
        .calls
        .iter()
        .all(|(_, payload)| SourceRevision::of(payload) != SourceRevision::of(source)));
    assert!(
        report.language_regions.is_empty(),
        "source observation is not NLP evidence"
    );
    assert!(!report.unsupported_operative_region_ids.is_empty());
    assert!(!report.authority_created && !report.complete_instruction_grammar_claim);
    for mut reservation in observer.held.drain(..) {
        reservation.release_confirmed().unwrap();
    }
    assert_eq!(broker.used(ResourceKind::Ram).unwrap(), 0);
    assert_eq!(broker.used(ResourceKind::Child).unwrap(), 0);
    run.close_confirmed().unwrap();
}

#[test]
fn tlcx05_portable_resource_wiring_rejects_root_view_for_synthetic_payload() {
    let source = "Please verify the result.\n\n結果を確認してください。";
    let (broker, mut run, _) = owner(source);
    let mut observer = RecordingSourceObserver {
        run: &run,
        broker: &broker,
        wrong_source: true,
        calls: Vec::new(),
        held: Vec::new(),
    };
    let error = lc631_analysis::build_dgcl_pipeline_with_source_observer(source, &mut observer)
        .unwrap_err();
    assert!(format!("{error:?}").contains("SourceMismatch"));
    assert_eq!(observer.calls.len(), 1);
    assert!(observer.held.is_empty());
    assert_eq!(broker.used(ResourceKind::Ram).unwrap(), 0);
    run.close_confirmed().unwrap();
}

#[test]
fn configured_tlcx05_two_language_payloads_use_one_parent_run_and_exact_source_views() {
    if !configured_profile_requested() {
        return;
    }
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .unwrap();
    let pins = load_dgcl_runtime_pins(repo).expect("configured profile requires pinned runtime");
    let en = pins.bind_pinned_worker("en", 60_000).unwrap();
    let ja = pins.bind_pinned_worker("ja", 60_000).unwrap();
    let source = "Please verify the result.\n\n結果を確認してください。";
    let (broker, mut run, _) = owner(source);
    let mut runner = BoundWorker {
        run: &run,
        wrong_source: false,
        calls: Vec::new(),
    };
    let report = build_dgcl_pipeline_with_runner(
        source,
        DgclBackendSet {
            english: Some(&en),
            japanese: Some(&ja),
        },
        &mut runner,
    )
    .unwrap();
    assert_eq!(runner.calls.len(), 2);
    assert!(runner.calls.iter().any(|(language, _)| language == "en"));
    assert!(runner.calls.iter().any(|(language, _)| language == "ja"));
    assert!(runner.calls.iter().all(|(_, payload)| payload != source));
    assert_eq!(report.language_regions.len(), 2);
    assert!(report
        .language_regions
        .iter()
        .all(
            |r| r.root_source_revision == lc631_core::stable_sha256(source)
                && !r.semantic_truth_claim
                && !r.sentences.is_empty()
        ));
    assert_eq!(broker.used(ResourceKind::Child).unwrap(), 0);
    run.close_confirmed().unwrap();
}

#[test]
fn configured_tlcx05_root_view_is_rejected_for_a_different_synthetic_payload() {
    if !configured_profile_requested() {
        return;
    }
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .unwrap();
    let pins = load_dgcl_runtime_pins(repo).unwrap();
    let en = pins.bind_pinned_worker("en", 60_000).unwrap();
    let source = "Please verify the result.\n\n背景。";
    let (_, mut run, _) = owner(source);
    let mut runner = BoundWorker {
        run: &run,
        wrong_source: true,
        calls: Vec::new(),
    };
    let error = build_dgcl_pipeline_with_runner(
        source,
        DgclBackendSet {
            english: Some(&en),
            japanese: None,
        },
        &mut runner,
    )
    .unwrap_err();
    assert!(format!("{error:?}").contains("SourceMismatch"));
    assert_eq!(runner.calls.len(), 1);
    run.close_confirmed().unwrap();
}
