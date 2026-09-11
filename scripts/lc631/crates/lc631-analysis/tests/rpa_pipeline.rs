use lc631_analysis::{
    analyze, analyze_path, analyze_with_receipts, evaluation_payload_digest,
    expected_v630_baseline_digest, stage_completion_payload_digest,
    validation_receipt_payload_digest, validation_receipt_scope, AnalysisRequest, ClosureState,
    EvidenceState, LanguageKind, ReceiptBinding, ReleaseEvidenceBundle, StageCompletionReceipt,
    ValidationEvidenceBundle, ValidationKind, ValidationReceipt,
};
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptIssuer, ReceiptScope, ReceiptVerifier, SubjectRevision, UntrustedReceipt,
};

fn receipt_root() -> (ReceiptIssuer, ReceiptVerifier) {
    let key = [29_u8; 32];
    (
        ReceiptIssuer::from_key_bytes("lc631-rpa-test-root", &key).unwrap(),
        ReceiptVerifier::from_key_bytes("lc631-rpa-test-root", &key).unwrap(),
    )
}

fn issue(
    issuer: &ReceiptIssuer,
    class: ReceiptClass,
    subject: &str,
    scope: &str,
    payload: String,
) -> UntrustedReceipt {
    issuer
        .issue(
            class,
            SubjectRevision::checked(subject).unwrap(),
            ReceiptScope::checked(scope).unwrap(),
            payload,
            1,
            None,
            None,
        )
        .unwrap()
}

#[test]
fn rpa_00_39_is_dependency_ordered_non_authorizing_and_fail_closed() {
    let report = analyze(AnalysisRequest {
        prompt: "Review Rust, Python, and assembly boundaries",
        repo: None,
        validation: None,
        release: None,
        source_name: None,
        source: None,
    });

    assert_eq!(report.stages.len(), 40);
    assert!(report.dependency_order_valid);
    assert!(report.wiring_complete);
    assert_eq!(report.stages.first().unwrap().stage_id, "RPA-00");
    assert_eq!(report.stages.last().unwrap().stage_id, "RPA-39");
    assert!(report
        .stages
        .iter()
        .all(|stage| !stage.creates_authority && !stage.creates_output_commit));
    assert!(!report.release_complete);
}

#[test]
fn rpa_corpus_manifest_is_revision_bound_and_does_not_turn_counts_into_coverage() {
    let report = analyze(AnalysisRequest {
        prompt: "audit",
        repo: None,
        validation: None,
        release: None,
        source_name: None,
        source: None,
    });

    assert_eq!(report.corpora.len(), 3);
    assert!(report.corpora.iter().all(|corpus| {
        corpus.sha256.starts_with("sha256:")
            && corpus.sha256.len() == 71
            && !corpus.count_is_coverage
    }));
    assert_eq!(
        report.stage("RPA-00").unwrap().state,
        EvidenceState::Observed
    );
    assert_eq!(
        report.stage("RPA-39").unwrap().state,
        EvidenceState::NeedsEvidence
    );
}

#[test]
fn language_selection_is_explicit_and_does_not_default() {
    let report = analyze(AnalysisRequest {
        prompt: "ordinary prose without a programming language",
        repo: None,
        validation: None,
        release: None,
        source_name: None,
        source: None,
    });
    assert!(report.languages.is_empty());
    assert!(!report.target.selection_authoritative);

    let prompt_only = analyze(AnalysisRequest {
        prompt: "review this Rust implementation",
        repo: None,
        validation: None,
        release: None,
        source_name: None,
        source: None,
    });
    assert_eq!(prompt_only.languages, vec![LanguageKind::Rust]);
    assert!(!prompt_only.target.selection_authoritative);
    assert_eq!(
        prompt_only.stage("RPA-03").unwrap().state,
        EvidenceState::Candidate
    );

    let selected = analyze(AnalysisRequest {
        prompt: "review this Rust implementation",
        repo: None,
        validation: None,
        release: None,
        source_name: Some("lib.rs"),
        source: Some("pub fn answer() -> u32 { 42 }"),
    });
    assert_eq!(selected.languages, vec![LanguageKind::Rust]);
    assert!(selected.target.selection_authoritative);
}

#[test]
fn source_extension_owns_language_and_prompt_conflict_does_not_expand_scope() {
    let report = analyze(AnalysisRequest {
        prompt: "also analyze Python and assembly",
        repo: None,
        validation: None,
        release: None,
        source_name: Some("lib.rs"),
        source: Some("pub fn answer() -> u32 { 42 }"),
    });

    assert_eq!(report.languages, vec![LanguageKind::Rust]);
    assert_eq!(report.target.selection_conflicts.len(), 2);
    assert!(report.python.is_none());
}

#[test]
fn rpa_09_18_rust_compiler_witnesses_are_real_and_non_authorizing() {
    let report = analyze(AnalysisRequest {
        prompt: "review this Rust implementation",
        repo: None,
        validation: None,
        release: None,
        source_name: Some("lib.rs"),
        source: Some("pub fn answer() -> u32 { 42 }"),
    });
    let rust = report.rust.as_ref().expect("Rust analysis");

    assert!(rust.compiler_accepted);
    assert!(rust.artifacts.iter().any(|artifact| {
        artifact.kind == lc631_analysis::CompilerArtifactKind::Mir
            && artifact.state == EvidenceState::Observed
    }));
    assert!(rust.artifacts.iter().any(|artifact| {
        artifact.kind == lc631_analysis::CompilerArtifactKind::Object
            && artifact.state == EvidenceState::Observed
    }));
    assert_eq!(
        report.stage("RPA-09").unwrap().state,
        EvidenceState::Observed
    );
    assert_eq!(
        report.stage("RPA-13").unwrap().state,
        EvidenceState::Observed
    );
    assert_eq!(
        report.stage("RPA-18").unwrap().state,
        EvidenceState::Observed,
        "{:?}",
        rust.artifacts
            .iter()
            .map(|artifact| (
                artifact.kind,
                artifact.state,
                artifact.process.diagnostic.clone()
            ))
            .collect::<Vec<_>>()
    );
    assert!(!rust.creates_authority);
    assert!(!rust.creates_output_commit);
    assert!(!report.release_complete);
}

#[test]
fn invalid_rust_source_is_refuted_not_promoted_or_executed() {
    let report = analyze(AnalysisRequest {
        prompt: "review Rust",
        repo: None,
        validation: None,
        release: None,
        source_name: Some("lib.rs"),
        source: Some("pub fn broken( { panic!(\"must never execute\") }"),
    });
    let rust = report.rust.as_ref().expect("Rust analysis");

    assert!(!rust.compiler_accepted);
    assert_eq!(
        report.stage("RPA-13").unwrap().state,
        EvidenceState::Refuted
    );
    assert!(!report.release_complete);
}

#[test]
fn rpa_06_08_cargo_metadata_is_observed_without_build_execution() {
    let report = analyze(AnalysisRequest {
        prompt: "review this Rust crate",
        repo: Some(env!("CARGO_MANIFEST_DIR")),
        validation: None,
        release: None,
        source_name: Some("lib.rs"),
        source: Some("pub fn answer() -> u32 { 42 }"),
    });
    let cargo = report.cargo.as_ref().expect("Cargo receipt");

    assert_eq!(cargo.state, EvidenceState::Observed);
    assert!(cargo.package_count >= 1);
    assert!(!cargo.build_execution_observed);
    assert_eq!(
        report.stage("RPA-06").unwrap().state,
        EvidenceState::Observed
    );
    assert_eq!(
        report.stage("RPA-07").unwrap().state,
        EvidenceState::Candidate
    );
    assert_eq!(
        report.stage("RPA-08").unwrap().state,
        EvidenceState::NotAuthorized
    );
}

#[test]
fn explicit_execution_checks_the_source_bound_cargo_package() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root =
        std::env::temp_dir().join(format!("lc631-crate-check-{}-{nonce}", std::process::id()));
    let member = root.join("member");
    let source_path = member.join("src/lib.rs");
    std::fs::create_dir_all(member.join("src")).unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"member\"]\nresolver = \"2\"\n",
    )
    .unwrap();
    std::fs::write(
        root.join("Cargo.lock"),
        "# This file is automatically @generated by Cargo.\nversion = 4\n\n[[package]]\nname = \"member\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    std::fs::write(
        member.join("Cargo.toml"),
        "[package]\nname = \"member\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    let source = "mod sibling;\npub use sibling::answer;\n";
    std::fs::write(&source_path, source).unwrap();
    std::fs::write(
        member.join("src/sibling.rs"),
        "pub fn answer() -> u32 { 42 }\n",
    )
    .unwrap();

    let report = analyze_path(
        AnalysisRequest {
            prompt: "review this Rust crate",
            repo: root.to_str(),
            validation: None,
            release: None,
            source_name: Some("lib.rs"),
            source: Some(source),
        },
        Some(&source_path),
        true,
    );
    let cargo = report.cargo.as_ref().expect("Cargo receipt");

    assert!(cargo.source_bound, "{cargo:?}");
    assert_eq!(cargo.checked_package.as_deref(), Some("member"));
    assert_eq!(cargo.crate_check_state, EvidenceState::Observed);
    assert!(cargo.build_execution_observed);
    assert_eq!(
        report.stage("RPA-13").unwrap().state,
        EvidenceState::Observed
    );
    assert!(!report.release_complete);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn rpa_19_27_accepts_only_complete_host_bound_validation_receipts() {
    let (issuer, verifier) = receipt_root();
    let source = "pub fn answer() -> u32 { 42 }";
    let source_revision = lc631_core::stable_sha256(source);
    let kinds = [
        ValidationKind::Rustfmt,
        ValidationKind::RustcCheck,
        ValidationKind::Clippy,
        ValidationKind::Tests,
        ValidationKind::Coverage,
        ValidationKind::Mutation,
        ValidationKind::Miri,
        ValidationKind::Property,
        ValidationKind::Fuzz,
        ValidationKind::Differential,
        ValidationKind::ConcurrencyModel,
        ValidationKind::FormalVerifier,
        ValidationKind::SupplyChain,
        ValidationKind::SemverMsrv,
        ValidationKind::Performance,
        ValidationKind::HumanReview,
    ];
    let mut bundle = ValidationEvidenceBundle {
        bundle_version: "validation-evidence.test-v1".to_string(),
        receipts: kinds
            .into_iter()
            .map(|kind| validation_receipt(kind, &source_revision, ReceiptBinding::HostBound))
            .collect(),
    };
    for receipt in &mut bundle.receipts {
        receipt.attestation = Some(issue(
            &issuer,
            ReceiptClass::Validation,
            &source_revision,
            &validation_receipt_scope(receipt),
            validation_receipt_payload_digest(receipt),
        ));
    }
    let report = analyze_with_receipts(
        AnalysisRequest {
            prompt: "review Rust",
            repo: None,
            validation: Some(&bundle),
            release: None,
            source_name: Some("lib.rs"),
            source: Some(source),
        },
        &verifier,
        1,
    );

    for number in 19..=27 {
        assert_eq!(
            report
                .stage(&format!("RPA-{number:02}"))
                .expect("validation stage")
                .state,
            EvidenceState::Observed,
            "RPA-{number:02}"
        );
    }
    let summary = report.validation.as_ref().expect("validation summary");
    assert_eq!(summary.accepted_receipts, 16);
    assert_eq!(summary.rejected_receipts, 0);
    assert!(summary.all_receipts_non_authorizing);
    assert!(!report.release_complete);
}

#[test]
fn self_attested_validation_never_promotes_a_stage() {
    let source = "pub fn answer() -> u32 { 42 }";
    let source_revision = lc631_core::stable_sha256(source);
    let bundle = ValidationEvidenceBundle {
        bundle_version: "validation-evidence.test-v1".to_string(),
        receipts: vec![validation_receipt(
            ValidationKind::Rustfmt,
            &source_revision,
            ReceiptBinding::SelfAttested,
        )],
    };
    let report = analyze(AnalysisRequest {
        prompt: "review Rust",
        repo: None,
        validation: Some(&bundle),
        release: None,
        source_name: Some("lib.rs"),
        source: Some(source),
    });

    assert_ne!(
        report.stage("RPA-19").unwrap().state,
        EvidenceState::Observed
    );
    assert_eq!(report.validation.as_ref().unwrap().accepted_receipts, 0);
}

#[test]
fn rpa_28_31_python_compiler_path_is_observed_without_candidate_execution() {
    let source = "import module_that_does_not_exist\nmessage = '日本語'\nraise RuntimeError('must not execute')\n";
    let report = analyze(AnalysisRequest {
        prompt: "review Python",
        repo: None,
        validation: None,
        release: None,
        source_name: Some("app.py"),
        source: Some(source),
    });
    let python = report.python.as_ref().expect("Python analysis");

    assert!(python.parsed);
    assert!(python.compiled);
    assert!(!python.candidate_executed);
    assert!(python.interpreter.is_some());
    assert!(python.code_object_digest.is_some());
    assert!(python.opcode_digest.is_some());
    assert_eq!(
        python.imports,
        vec!["module_that_does_not_exist".to_string()]
    );
    assert_eq!(
        report.stage("RPA-28").unwrap().state,
        EvidenceState::Observed
    );
    assert_eq!(
        report.stage("RPA-30").unwrap().state,
        EvidenceState::Observed
    );
    assert_eq!(
        report.stage("RPA-31").unwrap().state,
        EvidenceState::Candidate
    );
    assert!(!python.creates_authority);
    assert!(!python.creates_output_commit);
}

#[test]
fn invalid_python_source_is_refuted_and_not_run() {
    let report = analyze(AnalysisRequest {
        prompt: "review Python",
        repo: None,
        validation: None,
        release: None,
        source_name: Some("app.py"),
        source: Some("def broken(:\n    pass\n"),
    });
    let python = report.python.as_ref().expect("Python analysis");

    assert!(!python.compiled);
    assert!(python.syntax_error.is_some());
    assert!(!python.candidate_executed);
    assert_eq!(
        report.stage("RPA-28").unwrap().state,
        EvidenceState::Refuted
    );
}

#[test]
fn oversized_python_source_is_fail_visible_and_never_executed() {
    let source = "x".repeat(1024 * 1024 + 1);
    let report = analyze(AnalysisRequest {
        prompt: "review Python",
        repo: None,
        validation: None,
        release: None,
        source_name: Some("app.py"),
        source: Some(&source),
    });
    let python = report.python.as_ref().expect("Python analysis");

    assert!(!python.parsed);
    assert!(!python.compiled);
    assert!(!python.candidate_executed);
    assert!(python
        .risk_indicators
        .contains(&"source_size_limit_exceeded".to_string()));
    assert_eq!(
        report.stage("RPA-28").unwrap().state,
        EvidenceState::Unavailable
    );
}

#[test]
fn rpa_32_36_explicit_assembly_keeps_target_and_tool_gaps_visible() {
    let source = "section .text\nglobal add\nadd:\n    mov rax, rdi\n    add rax, rsi\n    ret\n";
    let report = analyze(AnalysisRequest {
        prompt: "review x86_64 NASM assembly",
        repo: None,
        validation: None,
        release: None,
        source_name: Some("add.asm"),
        source: Some(source),
    });
    let assembly = report.assembly.as_ref().expect("Assembly analysis");

    assert_eq!(assembly.target.architecture.as_deref(), Some("x86_64"));
    assert_eq!(assembly.target.dialect.as_deref(), Some("nasm"));
    assert!(assembly.metrics.instructions >= 3);
    assert_eq!(
        report.stage("RPA-32").unwrap().state,
        EvidenceState::Observed
    );
    assert_eq!(
        report.stage("RPA-33").unwrap().state,
        EvidenceState::Candidate
    );
    assert_eq!(
        report.stage("RPA-34").unwrap().state,
        EvidenceState::Candidate
    );
    assert_eq!(
        report.stage("RPA-35").unwrap().state,
        EvidenceState::NeedsEvidence
    );
    assert!(!assembly.creates_authority);
    assert!(!assembly.creates_output_commit);
}

#[test]
fn rpa_37_consumes_rust_codegen_assembly_and_object_without_claiming_runtime_abi() {
    let report = analyze(AnalysisRequest {
        prompt: "review Rust low-level code",
        repo: None,
        validation: None,
        release: None,
        source_name: Some("lib.rs"),
        source: Some("pub fn add(a: i64, b: i64) -> i64 { a + b }"),
    });
    let assembly = report.assembly.as_ref().expect("rustc assembly analysis");

    assert_eq!(assembly.source_origin, "rustc_codegen_assembly");
    assert!(assembly.object_digest.is_some());
    assert!(assembly.source_assembled);
    assert_eq!(
        report.stage("RPA-37").unwrap().state,
        EvidenceState::Candidate
    );
    assert!(!report.release_complete);
}

#[test]
fn rpa_38_39_closes_only_after_every_prior_stage_and_release_digest_is_bound() {
    let (issuer, verifier) = receipt_root();
    let source = "def add(left: int, right: int) -> int:\n    return left + right\n";
    let source_revision = lc631_core::stable_sha256(source);
    let initial = analyze(AnalysisRequest {
        prompt: "review Python program analysis",
        repo: None,
        validation: None,
        release: None,
        source_name: Some("app.py"),
        source: Some(source),
    });
    let mut stage_evidence = initial
        .stages
        .iter()
        .filter(|stage| {
            stage.stage_id != "RPA-39"
                && !matches!(
                    stage.closure_state,
                    ClosureState::Satisfied | ClosureState::NotApplicable
                )
        })
        .map(|stage| StageCompletionReceipt {
            stage_id: stage.stage_id.clone(),
            binding: ReceiptBinding::HostBound,
            source_revision: source_revision.clone(),
            evidence_digest: digest_for(&stage.stage_id),
            scope: "bounded release fixture".to_string(),
            unsupported: Vec::new(),
            attestation: None,
        })
        .collect::<Vec<_>>();
    for receipt in &mut stage_evidence {
        receipt.attestation = Some(issue(
            &issuer,
            ReceiptClass::StageCompletion,
            &source_revision,
            &format!("release/stage/{}/{}", receipt.stage_id, receipt.scope),
            stage_completion_payload_digest(receipt),
        ));
    }
    let mut bundle = ReleaseEvidenceBundle {
        bundle_version: "release-evidence.test-v1".to_string(),
        stage_evidence,
        evaluation_corpus_digest: digest_for("evaluation-corpus"),
        evaluation_result_digest: digest_for("evaluation-result"),
        semantic_reversal_observed: true,
        directional_loss_observed: true,
        dead_fixture_count: 0,
        v630_baseline_digest: expected_v630_baseline_digest(),
        exact_host_output_digest: digest_for("host-output"),
        package_manifest_digest: digest_for("package"),
        remote_ci_digest: digest_for("remote-ci"),
        evaluation_attestation: None,
        v630_baseline_attestation: None,
        exact_host_output_attestation: None,
        package_manifest_attestation: None,
        remote_ci_attestation: None,
    };
    bundle.evaluation_attestation = Some(issue(
        &issuer,
        ReceiptClass::ReleaseEvaluation,
        &source_revision,
        "release/evaluation",
        evaluation_payload_digest(&bundle),
    ));
    bundle.v630_baseline_attestation = Some(issue(
        &issuer,
        ReceiptClass::Baseline,
        "v6.3.0@0d933982fa392043c2fe825644a3a560963b6615",
        "release/baseline/v6.3.0",
        bundle.v630_baseline_digest.clone(),
    ));
    bundle.exact_host_output_attestation = Some(issue(
        &issuer,
        ReceiptClass::HostOutput,
        &source_revision,
        "release/host-output",
        bundle.exact_host_output_digest.clone(),
    ));
    bundle.package_manifest_attestation = Some(issue(
        &issuer,
        ReceiptClass::Package,
        &source_revision,
        "release/package",
        bundle.package_manifest_digest.clone(),
    ));
    bundle.remote_ci_attestation = Some(issue(
        &issuer,
        ReceiptClass::RemoteCi,
        &source_revision,
        "release/remote-ci",
        bundle.remote_ci_digest.clone(),
    ));
    let report = analyze_with_receipts(
        AnalysisRequest {
            prompt: "review Python program analysis",
            repo: None,
            validation: None,
            release: Some(&bundle),
            source_name: Some("app.py"),
            source: Some(source),
        },
        &verifier,
        1,
    );

    assert!(
        report.release_complete,
        "release={:?} unresolved={:?}",
        report.release,
        report
            .stages
            .iter()
            .filter(|stage| {
                !matches!(
                    stage.state,
                    EvidenceState::Observed | EvidenceState::NotApplicable
                )
            })
            .map(|stage| (stage.stage_id.clone(), stage.state, stage.blockers.clone()))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        report.stage("RPA-38").unwrap().state,
        EvidenceState::Observed
    );
    assert_eq!(
        report.stage("RPA-39").unwrap().state,
        EvidenceState::Observed
    );
    assert!(report.release.as_ref().unwrap().release_complete);
    assert!(!report.creates_authority);
    assert!(!report.creates_output_commit);
}

#[test]
fn wrong_v630_baseline_digest_keeps_release_closed() {
    let source = "pub fn answer() -> u32 { 42 }";
    let source_revision = lc631_core::stable_sha256(source);
    let initial = analyze(AnalysisRequest {
        prompt: "review Rust",
        repo: None,
        validation: None,
        release: None,
        source_name: Some("lib.rs"),
        source: Some(source),
    });
    let bundle = ReleaseEvidenceBundle {
        bundle_version: "release-evidence.test-v1".to_string(),
        stage_evidence: initial
            .stages
            .iter()
            .filter(|stage| {
                stage.stage_id != "RPA-39"
                    && !matches!(
                        stage.closure_state,
                        ClosureState::Satisfied | ClosureState::NotApplicable
                    )
            })
            .map(|stage| StageCompletionReceipt {
                stage_id: stage.stage_id.clone(),
                binding: ReceiptBinding::HostBound,
                source_revision: source_revision.clone(),
                evidence_digest: digest_for(&stage.stage_id),
                scope: "bounded release fixture".to_string(),
                unsupported: Vec::new(),
                attestation: None,
            })
            .collect(),
        evaluation_corpus_digest: digest_for("evaluation-corpus"),
        evaluation_result_digest: digest_for("evaluation-result"),
        semantic_reversal_observed: true,
        directional_loss_observed: true,
        dead_fixture_count: 0,
        v630_baseline_digest: digest_for("wrong-v630"),
        exact_host_output_digest: digest_for("host-output"),
        package_manifest_digest: digest_for("package"),
        remote_ci_digest: digest_for("remote-ci"),
        evaluation_attestation: None,
        v630_baseline_attestation: None,
        exact_host_output_attestation: None,
        package_manifest_attestation: None,
        remote_ci_attestation: None,
    };
    let report = analyze(AnalysisRequest {
        prompt: "review Rust",
        repo: None,
        validation: None,
        release: Some(&bundle),
        source_name: Some("lib.rs"),
        source: Some(source),
    });

    assert!(!report.release_complete);
    assert!(!report.release.as_ref().unwrap().v630_baseline_bound);
    assert_eq!(
        report.stage("RPA-39").unwrap().state,
        EvidenceState::NeedsEvidence
    );
}

fn validation_receipt(
    kind: ValidationKind,
    source_revision: &str,
    binding: ReceiptBinding,
) -> ValidationReceipt {
    ValidationReceipt {
        kind,
        binding,
        state: EvidenceState::Observed,
        source_revision: source_revision.to_string(),
        tool_revision: "tool.test-v1".to_string(),
        invocation_digest: format!("sha256:{}", "11".repeat(32)),
        output_digest: format!("sha256:{}", "22".repeat(32)),
        scope: "bounded test fixture".to_string(),
        unsupported: Vec::new(),
        attestation: None,
    }
}

fn digest_for(label: &str) -> String {
    lc631_core::stable_sha256(label)
}
