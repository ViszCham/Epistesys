use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use lc631_core::stable_sha256;
use lc631_receipt_kernel::{ReceiptVerifier, ReplayGuard};
use lc631_tl::{
    CanonicalTranslationEnvelope, DomainTranslationPayload, Obligation, ObligationKind,
    ObligationPolarity, ObligationStrength, PreservationState, ProjectionDefectGraphV3,
    ProjectionEdgeId, ProjectionEdgeV3, SourceLedgerId, TargetClaim, TargetClaimId,
    VerificationReceipt, VerificationStatus, VerifierKind,
};

use crate::assembly::{apply_assembly_stages, from_explicit_source, from_rust_codegen};
use crate::cargo_world::{apply_cargo_stages, observe_cargo_world};
use crate::corpus::audited_corpora;
use crate::python::{analyze_python_source, apply_python_stages};
use crate::release::apply_release_evidence;
use crate::rust::{analyze_rust_source, apply_rust_stages};
use crate::validation::apply_validation_stages;
use crate::{
    AnalysisTargetProfile, ClosureState, DirectionalLossReceipt, EvidenceState, LanguageKind,
    ProgramAnalysisReport, StageReceipt, PROGRAM_ANALYSIS_SCHEMA,
};

pub struct AnalysisRequest<'a> {
    pub prompt: &'a str,
    pub repo: Option<&'a str>,
    pub validation: Option<&'a crate::ValidationEvidenceBundle>,
    pub release: Option<&'a crate::ReleaseEvidenceBundle>,
    pub source_name: Option<&'a str>,
    pub source: Option<&'a str>,
}

pub fn analyze(request: AnalysisRequest<'_>) -> ProgramAnalysisReport {
    analyze_inner(request, None, false, None, 0)
}

pub fn analyze_path(
    request: AnalysisRequest<'_>,
    source_path: Option<&Path>,
    execute: bool,
) -> ProgramAnalysisReport {
    analyze_inner(request, source_path, execute, None, 0)
}

pub fn analyze_with_receipts(
    request: AnalysisRequest<'_>,
    verifier: &ReceiptVerifier,
    now_epoch: u64,
) -> ProgramAnalysisReport {
    analyze_inner(request, None, false, Some(verifier), now_epoch)
}

fn analyze_inner(
    request: AnalysisRequest<'_>,
    source_path: Option<&Path>,
    execute: bool,
    verifier: Option<&ReceiptVerifier>,
    now_epoch: u64,
) -> ProgramAnalysisReport {
    let corpora = audited_corpora();
    let language_selection = select_languages(request.prompt, request.source_name);
    let languages = language_selection.languages;
    let source_revision = request
        .source
        .filter(|source| !source.trim().is_empty())
        .map(stable_sha256);
    let rust = (languages.contains(&LanguageKind::Rust))
        .then(|| request.source.filter(|source| !source.trim().is_empty()))
        .flatten()
        .map(analyze_rust_source);
    let rust_source_path = languages
        .contains(&LanguageKind::Rust)
        .then_some(source_path)
        .flatten();
    let cargo = request.repo.map(|repo| {
        observe_cargo_world(
            repo,
            rust_source_path,
            source_revision.as_deref(),
            execute && rust_source_path.is_some(),
        )
    });
    let python = (languages.contains(&LanguageKind::Python))
        .then(|| request.source.filter(|source| !source.trim().is_empty()))
        .flatten()
        .map(analyze_python_source);
    let assembly = if languages.contains(&LanguageKind::Assembly) {
        request
            .source
            .filter(|source| !source.trim().is_empty())
            .map(|source| from_explicit_source(source, request.prompt))
    } else {
        rust.as_ref()
            .and_then(|rust| from_rust_codegen(rust, request.prompt))
    };
    let target = AnalysisTargetProfile {
        languages: languages.clone(),
        source_name: request.source_name.map(str::to_string),
        source_revision: source_revision.clone(),
        selection_owner: language_selection.owner,
        selection_authoritative: language_selection.authoritative,
        selection_conflicts: language_selection.conflicts,
        target_triple: None,
        toolchain_revision: rust
            .as_ref()
            .and_then(|analysis| analysis.toolchain_identity.clone()),
        unknowns: vec![
            "target triple and toolchain are unbound until a backend receipt is executed"
                .to_string(),
        ],
    };
    let mut stages = base_stages(
        &languages,
        source_revision.as_deref(),
        target.selection_authoritative,
    );
    if let Some(rust) = &rust {
        apply_rust_stages(&mut stages, rust);
    }
    if let Some(cargo) = &cargo {
        apply_cargo_stages(&mut stages, cargo);
    }
    if let Some(python) = &python {
        apply_python_stages(&mut stages, python);
    }
    if let Some(assembly) = &assembly {
        apply_assembly_stages(&mut stages, assembly);
    }
    let mut receipt_replay = ReplayGuard::default();
    let validation = request.validation.map(|bundle| {
        apply_validation_stages(
            &mut stages,
            bundle,
            source_revision.as_deref(),
            verifier,
            &mut receipt_replay,
            now_epoch,
        )
    });
    normalize_stages(&mut stages);
    let release = request.release.map(|bundle| {
        apply_release_evidence(
            &mut stages,
            bundle,
            source_revision.as_deref(),
            verifier,
            &mut receipt_replay,
            now_epoch,
        )
    });
    normalize_stages(&mut stages);
    let expected = (0..=39)
        .map(|id| format!("RPA-{id:02}"))
        .collect::<Vec<_>>();
    let actual = stages
        .iter()
        .map(|stage| stage.stage_id.clone())
        .collect::<Vec<_>>();
    let wiring_complete = actual == expected;
    let dependency_order_valid = wiring_complete && dependency_order_valid(&stages);
    let observed_count = stages
        .iter()
        .filter(|stage| stage.state == EvidenceState::Observed)
        .count();
    let unresolved_count = stages
        .iter()
        .filter(|stage| {
            !matches!(
                stage.state,
                EvidenceState::Observed | EvidenceState::NotApplicable
            )
        })
        .count();
    let satisfied_count = stages
        .iter()
        .filter(|stage| stage.closure_state == ClosureState::Satisfied)
        .count();
    let blocked_count = stages
        .iter()
        .filter(|stage| stage.closure_state == ClosureState::Blocked)
        .count();
    let release_complete = release
        .as_ref()
        .is_some_and(|release| release.release_complete);
    let directional_loss = build_directional_loss(
        source_revision.as_deref(),
        rust.as_ref(),
        python.as_ref(),
        assembly.as_ref(),
    );
    let canonical_translation = request
        .source
        .and_then(|source| build_canonical_translation(source, &directional_loss).ok());

    ProgramAnalysisReport {
        schema_version: PROGRAM_ANALYSIS_SCHEMA.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        shadow_only: true,
        corpora,
        languages,
        target,
        rust,
        cargo,
        python,
        assembly,
        validation,
        release,
        stages,
        directional_loss,
        canonical_translation,
        dependency_order_valid,
        wiring_complete,
        observed_count,
        unresolved_count,
        satisfied_count,
        blocked_count,
        release_complete,
        creates_authority: false,
        creates_output_commit: false,
        residual_risks: vec![
            "v6.3.0 Artifact import receipt is unbound".to_string(),
            "host-final output receipt is unbound".to_string(),
            "external compiler, runtime, binary, formal, and CI evidence is incomplete"
                .to_string(),
        ],
        claim_boundary: "typed v6.3.1 program-analysis wiring and local audit metadata only; not v6.3.0 replacement, repository-wide analysis, compiler truth, runtime safety, proof, mutation authority, release approval, or host output commit"
            .to_string(),
    }
}

fn build_canonical_translation(
    source: &str,
    directional_loss: &[DirectionalLossReceipt],
) -> Result<CanonicalTranslationEnvelope, String> {
    let source_revision = stable_sha256(source);
    let mut graph = ProjectionDefectGraphV3::new(SourceLedgerId(631_038), &source_revision, source)
        .map_err(|error| format!("translation_graph:{error:?}"))?;
    let obligation = Obligation {
        id: lc631_core::ObligationId(631_038),
        kind: ObligationKind::Structural,
        strength: ObligationStrength::Must,
        polarity: ObligationPolarity::Positive,
        scope: "program-analysis-directional-loss".into(),
        source_span: lc631_core::SourceSpan::checked(source, 0, source.len())
            .map_err(|error| format!("translation_span:{error:?}"))?,
        source_text: source.into(),
    };
    graph
        .add_obligation(obligation)
        .map_err(|error| format!("translation_obligation:{error:?}"))?;
    let anchor = graph
        .obligation_anchor(lc631_core::ObligationId(631_038))
        .cloned()
        .ok_or_else(|| "translation_anchor_missing".to_string())?;
    let payload = serde_json::to_string(directional_loss)
        .map_err(|error| format!("translation_payload:{error}"))?;
    graph
        .add_target_claim(
            TargetClaim::checked(
                TargetClaimId(631_038),
                "lc631-program-analysis",
                "directional_loss",
                env!("CARGO_PKG_VERSION"),
                &payload,
            )
            .map_err(|error| format!("translation_target:{error:?}"))?,
        )
        .map_err(|error| format!("translation_target_insert:{error:?}"))?;
    for (index, stage) in ProjectionDefectGraphV3::required_output_stages()
        .into_iter()
        .enumerate()
    {
        graph
            .add_edge(ProjectionEdgeV3 {
                id: ProjectionEdgeId(index as u64 + 1),
                obligation_id: Some(lc631_core::ObligationId(631_038)),
                source: Some(anchor.clone()),
                targets: vec![TargetClaimId(631_038)],
                stage,
                state: PreservationState::Unresolved,
                rule_id: "rpa-directional-loss-shadow.v1".into(),
                verifier: VerificationReceipt {
                    verifier: VerifierKind::ModelAdvisory,
                    status: VerificationStatus::NeedsEvidence,
                    revision: "rpa-shadow-unbound.v1".into(),
                    evidence_digest: None,
                },
            })
            .map_err(|error| format!("translation_edge:{error:?}"))?;
    }
    Ok(CanonicalTranslationEnvelope::from_graph(
        &graph,
        vec![DomainTranslationPayload {
            domain: "rpa".into(),
            schema_version: PROGRAM_ANALYSIS_SCHEMA.into(),
            payload_digest: stable_sha256(&payload),
            unknowns: vec![
                "compiler/runtime/host evidence remains typed outside the canonical edge graph"
                    .into(),
            ],
        }],
    ))
}

fn build_directional_loss(
    source_revision: Option<&str>,
    rust: Option<&crate::RustAnalysisSurface>,
    python: Option<&crate::PythonAnalysisSurface>,
    assembly: Option<&crate::AssemblyAnalysisSurface>,
) -> Vec<DirectionalLossReceipt> {
    let mut receipts = vec![DirectionalLossReceipt {
        from: "source".to_string(),
        to: "program_analysis_ir".to_string(),
        observed_defects: if source_revision.is_some() {
            Vec::new()
        } else {
            vec!["source_unbound".to_string()]
        },
        unknowns: vec![
            "TaskContract and v6.3.0 Artifact import remain separate external bindings".to_string(),
        ],
        scalar_aggregate_used: false,
    }];
    if let Some(rust) = rust {
        receipts.push(DirectionalLossReceipt {
            from: "rust_source".to_string(),
            to: "rust_hir_thir_mir".to_string(),
            observed_defects: rust
                .artifacts
                .iter()
                .filter(|artifact| artifact.state != EvidenceState::Observed)
                .map(|artifact| format!("{:?}_{:?}", artifact.kind, artifact.state))
                .collect(),
            unknowns: vec![
                "desugaring, macro hygiene, compiler-internal stability, and source mapping are not lossless"
                    .to_string(),
            ],
            scalar_aggregate_used: false,
        });
    }
    if let Some(python) = python {
        receipts.push(DirectionalLossReceipt {
            from: "python_source".to_string(),
            to: "cpython_code_object".to_string(),
            observed_defects: if python.compiled {
                Vec::new()
            } else {
                vec!["compiler_rejected_or_unavailable".to_string()]
            },
            unknowns: vec![
                "comments, formatting, import execution, typing, specialization, JIT, and runtime behavior are not preserved by a code-object digest"
                    .to_string(),
            ],
            scalar_aggregate_used: false,
        });
    }
    if let Some(assembly) = assembly {
        receipts.push(DirectionalLossReceipt {
            from: "assembly".to_string(),
            to: "object_and_runtime".to_string(),
            observed_defects: if assembly.object_digest.is_some() {
                Vec::new()
            } else {
                vec!["object_unbound".to_string()]
            },
            unknowns: vec![
                "encoding, relocation, link, load, unwind, fault, memory order, runtime, and performance remain directional losses"
                    .to_string(),
            ],
            scalar_aggregate_used: false,
        });
    }
    receipts
}

fn base_stages(
    languages: &[LanguageKind],
    source_revision: Option<&str>,
    language_selection_authoritative: bool,
) -> Vec<StageReceipt> {
    let corpus_digests = audited_corpora()
        .into_iter()
        .map(|corpus| corpus.sha256)
        .collect::<Vec<_>>();
    let source_evidence = source_revision
        .map(|digest| vec![digest.to_string()])
        .unwrap_or_default();
    let selected = !languages.is_empty();
    let bounded_runner = vec![stable_sha256("lc631-bounded-tool-runner.v1")];
    let mut stages = vec![
        stage(
            "RPA-00",
            "foundation",
            &[],
            EvidenceState::Observed,
            &corpus_digests,
            &["corpus bodies are not packaged and record counts are not coverage"],
        ),
        stage(
            "RPA-01",
            "foundation",
            &["RPA-00"],
            EvidenceState::Observed,
            &[],
            &["record-level authority classes remain to be materialized"],
        ),
        stage(
            "RPA-02",
            "foundation",
            &["RPA-00", "RPA-01"],
            EvidenceState::Observed,
            &source_evidence,
            &["remote branch freshness and all section loci are not re-fetched at runtime"],
        ),
        stage(
            "RPA-03",
            "foundation",
            &["RPA-02"],
            if language_selection_authoritative {
                EvidenceState::Observed
            } else if selected {
                EvidenceState::Candidate
            } else {
                EvidenceState::NeedsEvidence
            },
            &source_evidence,
            if language_selection_authoritative {
                &[]
            } else if selected {
                &["prompt markers are a candidate target only; bind a source extension or repository target"]
            } else {
                &["no explicit language target"]
            },
        ),
        stage(
            "RPA-04",
            "foundation",
            &["RPA-03"],
            EvidenceState::Observed,
            &bounded_runner,
            &["bounded execution does not prove process-tree termination on every host"],
        ),
        stage(
            "RPA-05",
            "foundation",
            &["RPA-04"],
            EvidenceState::Observed,
            &source_evidence,
            &["language-specific backend witnesses are pending"],
        ),
        stage(
            "RPA-06",
            "rust",
            &["RPA-03", "RPA-04", "RPA-05"],
            EvidenceState::NeedsEvidence,
            &[],
            &["Cargo workspace and resolution receipt unavailable"],
        ),
        stage(
            "RPA-07",
            "rust",
            &["RPA-06"],
            EvidenceState::NeedsEvidence,
            &[],
            &["Rust build-world matrix unavailable"],
        ),
        stage(
            "RPA-08",
            "rust",
            &["RPA-04", "RPA-07"],
            EvidenceState::NotAuthorized,
            &[],
            &["build.rs and proc-macro execution require sandboxed execution authority"],
        ),
        stage(
            "RPA-09",
            "rust",
            &["RPA-07", "RPA-08"],
            EvidenceState::NeedsEvidence,
            &[],
            &["lossless Rust parser receipt unavailable"],
        ),
        stage(
            "RPA-10",
            "rust",
            &["RPA-09"],
            EvidenceState::NeedsEvidence,
            &[],
            &["macro expansion and hygiene receipt unavailable"],
        ),
        stage(
            "RPA-11",
            "rust",
            &["RPA-06", "RPA-10"],
            EvidenceState::NeedsEvidence,
            &[],
            &["crate graph and rust-analyzer receipt unavailable"],
        ),
        stage(
            "RPA-12",
            "rust",
            &["RPA-10", "RPA-11"],
            EvidenceState::NeedsEvidence,
            &[],
            &["AST/HIR/THIR/MIR witness unavailable"],
        ),
        stage(
            "RPA-13",
            "rust",
            &["RPA-12"],
            EvidenceState::NeedsEvidence,
            &[],
            &["type, trait, const, pattern, and borrow obligations unavailable"],
        ),
        stage(
            "RPA-14",
            "rust",
            &["RPA-12", "RPA-13"],
            EvidenceState::NeedsEvidence,
            &[],
            &["unsafe and safe-API soundness obligations unavailable"],
        ),
        stage(
            "RPA-15",
            "rust",
            &["RPA-13", "RPA-14"],
            EvidenceState::NeedsEvidence,
            &[],
            &["layout, ABI, FFI, and unwind evidence unavailable"],
        ),
        stage(
            "RPA-16",
            "rust",
            &["RPA-12", "RPA-14"],
            EvidenceState::NeedsEvidence,
            &[],
            &["async, Pin, drop, and cancellation evidence unavailable"],
        ),
        stage(
            "RPA-17",
            "rust",
            &["RPA-13", "RPA-16"],
            EvidenceState::NeedsEvidence,
            &[],
            &["atomic, memory-order, race, and schedule evidence unavailable"],
        ),
        stage(
            "RPA-18",
            "rust",
            &["RPA-12", "RPA-15", "RPA-17"],
            EvidenceState::NeedsEvidence,
            &[],
            &["monomorphization, LLVM, object, disassembly, and inline-asm evidence unavailable"],
        ),
        stage(
            "RPA-19",
            "validation",
            &["RPA-06", "RPA-18"],
            EvidenceState::NeedsEvidence,
            &[],
            &["rustfmt, rustc, and Clippy receipts unavailable"],
        ),
        stage(
            "RPA-20",
            "validation",
            &["RPA-19"],
            EvidenceState::NeedsEvidence,
            &[],
            &["test matrix, coverage, and mutation receipts unavailable"],
        ),
        stage(
            "RPA-21",
            "validation",
            &["RPA-14", "RPA-20"],
            EvidenceState::NeedsEvidence,
            &[],
            &["Miri capability and execution receipts unavailable"],
        ),
        stage(
            "RPA-22",
            "validation",
            &["RPA-20", "RPA-21"],
            EvidenceState::NeedsEvidence,
            &[],
            &["fuzz, property, metamorphic, and differential receipts unavailable"],
        ),
        stage(
            "RPA-23",
            "validation",
            &["RPA-17", "RPA-20"],
            EvidenceState::NeedsEvidence,
            &[],
            &["Loom or Shuttle model receipts unavailable"],
        ),
        stage(
            "RPA-24",
            "validation",
            &["RPA-14", "RPA-17", "RPA-20"],
            EvidenceState::NeedsEvidence,
            &[],
            &["formal verifier capability receipts unavailable"],
        ),
        stage(
            "RPA-25",
            "validation",
            &["RPA-06", "RPA-19"],
            EvidenceState::NeedsEvidence,
            &[],
            &["dependency, advisory, license, SemVer, and MSRV receipts unavailable"],
        ),
        stage(
            "RPA-26",
            "validation",
            &["RPA-18", "RPA-19", "RPA-20"],
            EvidenceState::NeedsEvidence,
            &[],
            &["paired size, codegen, and performance receipts unavailable"],
        ),
        stage(
            "RPA-27",
            "validation",
            &[
                "RPA-19", "RPA-20", "RPA-21", "RPA-22", "RPA-23", "RPA-24", "RPA-25", "RPA-26",
            ],
            EvidenceState::NeedsEvidence,
            &[],
            &["human understanding, self-review, and output gate receipts unavailable"],
        ),
        stage(
            "RPA-28",
            "python",
            &["RPA-03", "RPA-04", "RPA-05"],
            EvidenceState::NeedsEvidence,
            &[],
            &["Python frontend receipt unavailable"],
        ),
        stage(
            "RPA-29",
            "python",
            &["RPA-28"],
            EvidenceState::NeedsEvidence,
            &[],
            &["typing specification and checker views unavailable"],
        ),
        stage(
            "RPA-30",
            "python",
            &["RPA-28", "RPA-29"],
            EvidenceState::NeedsEvidence,
            &[],
            &["code object, bytecode, and runtime trace unavailable"],
        ),
        stage(
            "RPA-31",
            "python",
            &["RPA-28", "RPA-29", "RPA-30"],
            EvidenceState::NeedsEvidence,
            &[],
            &["dynamic, async, free-threaded, and native-extension evidence unavailable"],
        ),
        stage(
            "RPA-32",
            "assembly",
            &["RPA-00", "RPA-01", "RPA-02", "RPA-03", "RPA-04", "RPA-05"],
            EvidenceState::NeedsEvidence,
            &[],
            &["Assembly target profile unavailable"],
        ),
        stage(
            "RPA-33",
            "assembly",
            &["RPA-32"],
            EvidenceState::NeedsEvidence,
            &[],
            &["macro, directive, pseudo-op, MC, and encoding evidence unavailable"],
        ),
        stage(
            "RPA-34",
            "assembly",
            &["RPA-33"],
            EvidenceState::NeedsEvidence,
            &[],
            &["instruction state-transition evidence unavailable"],
        ),
        stage(
            "RPA-35",
            "assembly",
            &["RPA-33", "RPA-34"],
            EvidenceState::NeedsEvidence,
            &[],
            &["object, relocation, link, unwind, debug, and loader evidence unavailable"],
        ),
        stage(
            "RPA-36",
            "assembly",
            &["RPA-35"],
            EvidenceState::NeedsEvidence,
            &[],
            &["assembler, decoder, emulator, and native differential evidence unavailable"],
        ),
        stage(
            "RPA-37",
            "cross_language",
            &["RPA-18", "RPA-31", "RPA-36"],
            EvidenceState::NeedsEvidence,
            &[],
            &["Rust inline asm, Python native extension, and GPU-ISA bridge unavailable"],
        ),
        stage(
            "RPA-38",
            "translation_loss",
            &[
                "RPA-21", "RPA-22", "RPA-23", "RPA-24", "RPA-25", "RPA-26", "RPA-27", "RPA-28",
                "RPA-29", "RPA-30", "RPA-31", "RPA-37",
            ],
            EvidenceState::NeedsEvidence,
            &[],
            &["directional roundtrip and corpus adversarial evaluation unavailable"],
        ),
        stage(
            "RPA-39",
            "release",
            &["RPA-38"],
            EvidenceState::NeedsEvidence,
            &[],
            &["host output, package, remote CI, and all language evidence are incomplete"],
        ),
    ];
    for stage in &mut stages {
        let applicable = match stage.group.as_str() {
            "rust" | "validation" => languages.contains(&LanguageKind::Rust),
            "python" => languages.contains(&LanguageKind::Python),
            "assembly" => {
                languages.contains(&LanguageKind::Assembly)
                    || languages.contains(&LanguageKind::Rust)
            }
            "cross_language" => !languages.is_empty(),
            _ => true,
        };
        if !applicable {
            stage.state = EvidenceState::NotApplicable;
            stage.evidence_digests.clear();
            stage.blockers.clear();
        }
    }
    stages
}

fn stage(
    id: &str,
    group: &str,
    dependencies: &[&str],
    state: EvidenceState,
    evidence: &[String],
    blockers: &[&str],
) -> StageReceipt {
    StageReceipt {
        stage_id: id.to_string(),
        group: group.to_string(),
        dependencies: dependencies
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        state,
        closure_state: ClosureState::Open,
        evidence_digests: evidence.to_vec(),
        blockers: blockers.iter().map(|value| (*value).to_string()).collect(),
        creates_authority: false,
        creates_output_commit: false,
    }
}

fn normalize_stages(stages: &mut [StageReceipt]) {
    for stage in stages {
        stage.evidence_digests.sort();
        stage.evidence_digests.dedup();
        stage.blockers.sort();
        stage.blockers.dedup();
        stage.closure_state = match stage.state {
            EvidenceState::NotApplicable => ClosureState::NotApplicable,
            EvidenceState::Refuted | EvidenceState::NotAuthorized => ClosureState::Blocked,
            EvidenceState::Observed if stage.blockers.is_empty() => ClosureState::Satisfied,
            EvidenceState::Observed | EvidenceState::Candidate => ClosureState::Candidate,
            EvidenceState::NeedsEvidence | EvidenceState::Unavailable => ClosureState::Open,
        };
    }
}

fn dependency_order_valid(stages: &[StageReceipt]) -> bool {
    let positions = stages
        .iter()
        .enumerate()
        .map(|(index, stage)| (stage.stage_id.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    stages.iter().enumerate().all(|(index, stage)| {
        stage.dependencies.iter().all(|dependency| {
            positions
                .get(dependency.as_str())
                .is_some_and(|dependency_index| *dependency_index < index)
        })
    })
}

struct LanguageSelection {
    languages: Vec<LanguageKind>,
    owner: String,
    authoritative: bool,
    conflicts: Vec<String>,
}

fn select_languages(prompt: &str, source_name: Option<&str>) -> LanguageSelection {
    let mut from_name = BTreeSet::new();
    if let Some(name) = source_name.map(str::to_ascii_lowercase) {
        if name.ends_with(".rs") {
            from_name.insert(LanguageKind::Rust);
        } else if name.ends_with(".py") {
            from_name.insert(LanguageKind::Python);
        } else if [".s", ".asm", ".S"]
            .iter()
            .any(|suffix| name.ends_with(&suffix.to_ascii_lowercase()))
        {
            from_name.insert(LanguageKind::Assembly);
        }
    }
    let mut from_prompt = BTreeSet::new();
    let lower = prompt.to_ascii_lowercase();
    if contains_marker(&lower, &["rust", "rustc", "cargo"]) {
        from_prompt.insert(LanguageKind::Rust);
    }
    if contains_marker(&lower, &["python", "cpython", "pypy"]) {
        from_prompt.insert(LanguageKind::Python);
    }
    if contains_marker(&lower, &["assembly", "assembler", " asm ", "アセンブリ"]) {
        from_prompt.insert(LanguageKind::Assembly);
    }
    if !from_name.is_empty() {
        let conflicts = from_prompt
            .difference(&from_name)
            .map(|language| format!("prompt_marker_conflicts_with_source_name:{language:?}"))
            .collect();
        LanguageSelection {
            languages: from_name.into_iter().collect(),
            owner: "explicit source-name extension; prompt markers are advisory only".to_string(),
            authoritative: true,
            conflicts,
        }
    } else {
        LanguageSelection {
            languages: from_prompt.into_iter().collect(),
            owner: "prompt markers only; candidate selection without repository or file binding"
                .to_string(),
            authoritative: false,
            conflicts: Vec::new(),
        }
    }
}

fn contains_marker(value: &str, markers: &[&str]) -> bool {
    markers.iter().any(|marker| value.contains(marker))
}
