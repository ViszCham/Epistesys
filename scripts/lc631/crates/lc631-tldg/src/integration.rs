use crate::{
    analyze_with_dg1, build_semantic_views, build_structural_kernel, default_backend_registry,
    evaluate_geometry_execution, execute_builtin_backend_receipts, report_for_artifact,
    run_mutual_distillation, BackendExecutionValidationReport, BackflowReport,
    CalibrationPartition, CpuGeometryBackend, Dg1Report, EmpiricalRiskOverlay,
    GeometryExecutionReceipt, ParseBudget, SemanticView, TldgError, UnifiedParseReport,
};
use lc631_core::{stable_sha256, ObligationId, SourceSpan};
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptIssuer, ReceiptPolicy, ReceiptScope, ReceiptVerifier, ReplayGuard,
    SubjectRevision, UntrustedReceipt,
};
use lc631_tl::{
    Obligation, ObligationKind, ObligationPolarity, ObligationStrength, PreservationState,
    ProjectionDefectGraphV3, ProjectionEdgeId, ProjectionEdgeV3, ProjectionGateDecision,
    ProjectionStage, SourceLedgerId, TargetClaim, TargetClaimId, VerificationReceipt,
    VerificationStatus, VerifierKind,
};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum PipelineError {
    Calibration(String),
    Distillation(String),
    Parse(TldgError),
    Dg1(String),
    Projection(String),
    Structural(String),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DistillationSummary {
    pub accepted: usize,
    pub materialized_relation_count: usize,
    pub rejected: usize,
    pub needs_evidence: usize,
    pub incomparable: usize,
    pub geometry_direct_promotions: usize,
    pub backflow: BackflowReport,
    pub epochs_executed: usize,
    pub reprojection_request_count: usize,
    pub epoch_revisions: Vec<String>,
    pub state_change_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct OutputClosure {
    pub binding_state: String,
    pub requirement_projection_state: String,
    pub defect_gate: ProjectionGateDecision,
    pub output_commit_allowed: bool,
    pub authority_created: bool,
    pub exact_host_output_bound: bool,
    pub requirement_candidate_count: usize,
    pub projected_obligation_count: usize,
    pub projection_edge_count: usize,
    pub unexplained_prose_count: usize,
    pub empty_requirement_set_held: bool,
}

#[derive(Serialize)]
struct RequirementProgramIr<'a> {
    schema_version: &'static str,
    requirement_id: u32,
    source_revision: &'a str,
    source_span: SourceSpan,
    source_text: &'a str,
    language: crate::Dg1Language,
    strength: crate::Dg1RequirementStrength,
    polarity: crate::Dg1RequirementPolarity,
    conditional: bool,
    exception_present: bool,
    condition_kind: Option<crate::Dg1ConditionKind>,
    scope: &'a Option<String>,
    unresolved_reference: bool,
    authority_grant: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TldgPipelineReport {
    pub source_revision: String,
    pub parse: UnifiedParseReport,
    pub dg1: Dg1Report,
    pub backend_validation: BackendExecutionValidationReport,
    pub semantic_views: Vec<SemanticView>,
    pub distillation: DistillationSummary,
    pub empirical_risk: EmpiricalRiskOverlay,
    pub output: OutputClosure,
    pub geometry_execution: GeometryExecutionReceipt,
    pub local_source_ready: bool,
    pub external_backends_observed: bool,
    pub builtin_profiles_validated: bool,
    pub residuals: Vec<String>,
    pub claim_boundary: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TldgReleaseEvidence {
    pub source_revision: String,
    pub gpu_execution_digest: String,
    pub gpu_parity_digest: String,
    pub gpu_lane_parity: bool,
    pub host_binder_digest: String,
    pub adversarial_digest: String,
    pub gpu_attestation: Option<UntrustedReceipt>,
    pub host_attestation: Option<UntrustedReceipt>,
    pub adversarial_attestation: Option<UntrustedReceipt>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TldgReleaseGate {
    pub predicates: BTreeMap<String, bool>,
    pub local_source_implemented: bool,
    pub release_complete: bool,
    pub deployment_complete: bool,
    pub blocked_reasons: Vec<String>,
    pub residuals: Vec<String>,
    pub automatic_promotion: bool,
    pub claim_boundary: String,
}

pub fn build_tldg_pipeline(
    source: &str,
    candidate_output: Option<&str>,
    partition: &CalibrationPartition,
) -> Result<TldgPipelineReport, PipelineError> {
    build_tldg_pipeline_inner(source, candidate_output, partition, None)
}

pub fn build_tldg_pipeline_with_receipts(
    source: &str,
    candidate_output: Option<&str>,
    partition: &CalibrationPartition,
    issuer: &ReceiptIssuer,
    verifier: &ReceiptVerifier,
    replay: &mut ReplayGuard,
    now_epoch: u64,
) -> Result<TldgPipelineReport, PipelineError> {
    build_tldg_pipeline_inner(
        source,
        candidate_output,
        partition,
        Some((issuer, verifier, replay, now_epoch)),
    )
}

fn build_tldg_pipeline_inner(
    source: &str,
    candidate_output: Option<&str>,
    partition: &CalibrationPartition,
    receipt_context: Option<(&ReceiptIssuer, &ReceiptVerifier, &mut ReplayGuard, u64)>,
) -> Result<TldgPipelineReport, PipelineError> {
    let (artifact, dg1) = analyze_with_dg1(source).map_err(PipelineError::Parse)?;
    let parse = report_for_artifact(&artifact);
    let mut registry = default_backend_registry();
    let backend_validation = if let Some((issuer, verifier, replay, now_epoch)) = receipt_context {
        let receipts = execute_builtin_backend_receipts(&artifact, issuer, now_epoch)
            .map_err(PipelineError::Projection)?;
        registry.validate_executions_detailed(
            &receipts,
            &artifact.source.revision,
            verifier,
            replay,
            now_epoch,
        )
    } else {
        BackendExecutionValidationReport::default()
    };
    let semantic_views = build_semantic_views(&artifact, &registry);
    let kernel = build_structural_kernel(&artifact)
        .map_err(|error| PipelineError::Structural(format!("{error:?}")))?;
    let run = run_mutual_distillation(
        kernel,
        &artifact,
        ParseBudget::reference(),
        &CpuGeometryBackend,
    )
    .map_err(|error| PipelineError::Distillation(format!("{error:?}")))?;
    let distillation = DistillationSummary {
        accepted: run.final_payload.accepted.len(),
        materialized_relation_count: run.final_payload.kernel.materialized_relations.len(),
        rejected: run.final_payload.rejected.len(),
        needs_evidence: run.final_payload.needs_evidence.len(),
        incomparable: run.final_payload.incomparable.len(),
        geometry_direct_promotions: run.final_payload.geometry_direct_promotions,
        backflow: run.final_backflow.clone(),
        epochs_executed: run.epochs.len(),
        reprojection_request_count: run.reprojection_requests.len(),
        epoch_revisions: run
            .epochs
            .iter()
            .map(|epoch| epoch.output_revision.clone())
            .collect(),
        state_change_count: run
            .epochs
            .iter()
            .filter(|epoch| epoch.state_changed)
            .count(),
    };
    let empirical_risk = EmpiricalRiskOverlay::from_defects(&artifact.syntax.defects, partition);
    let output = build_output_closure(source, candidate_output, &dg1)?;
    let geometry_execution = evaluate_geometry_execution(None, None);
    let external_backends_observed = semantic_views.iter().any(|view| {
        view.state == crate::BackendState::ValidatedObservation
            && !matches!(
                view.backend,
                crate::BackendFamily::BuiltinLossless
                    | crate::BackendFamily::BuiltinOpenDiscourse
                    | crate::BackendFamily::BuiltinExecutableSymbolic
            )
    });
    let builtin_profiles_validated = registry.builtin_profiles_validated();
    let local_source_ready = parse.source_roundtrip == source
        && distillation.geometry_direct_promotions == 0
        && !output.authority_created
        && builtin_profiles_validated;
    let mut residuals = Vec::new();
    if !external_backends_observed {
        residuals.push("external_grammar_and_semantic_backends_unobserved".into());
    }
    residuals.push(
        "japanese_english_linguistic_backend_unavailable_rule_candidates_are_not_full_parse".into(),
    );
    if !geometry_execution.real_gpu_observed {
        residuals.push("real_gpu_geometry_unobserved".into());
    }
    if !output.exact_host_output_bound {
        residuals.push("exact_host_output_unbound".into());
    }
    residuals.push("remote_tldg_ci_unobserved".into());
    Ok(TldgPipelineReport {
        source_revision: artifact.source.revision.clone(),
        parse,
        dg1,
        backend_validation,
        semantic_views,
        distillation,
        empirical_risk,
        output,
        geometry_execution,
        local_source_ready,
        external_backends_observed,
        builtin_profiles_validated,
        residuals,
        claim_boundary:
            "local source readiness with external backend, GPU, calibration, and host residuals"
                .into(),
    })
}

pub fn build_tldg_release_gate(
    report: &TldgPipelineReport,
    adversarial: &crate::AdversarialEvaluationReport,
) -> TldgReleaseGate {
    build_tldg_release_gate_with_receipts(report, adversarial, false, false, false)
}

pub fn build_tldg_release_gate_with_receipts(
    report: &TldgPipelineReport,
    adversarial: &crate::AdversarialEvaluationReport,
    _real_gpu_observed: bool,
    _gpu_lane_parity: bool,
    _host_binder_source_connected: bool,
) -> TldgReleaseGate {
    let mut gate = assemble_release_gate(report, adversarial, false, false, false, false);
    gate.blocked_reasons
        .push("legacy_boolean_release_evidence_rejected".into());
    gate.blocked_reasons.sort();
    gate.blocked_reasons.dedup();
    gate.release_complete = false;
    gate
}

pub fn build_tldg_release_gate_verified(
    report: &TldgPipelineReport,
    adversarial: &crate::AdversarialEvaluationReport,
    evidence: &TldgReleaseEvidence,
    verifier: &ReceiptVerifier,
    replay: &mut ReplayGuard,
    now_epoch: u64,
) -> TldgReleaseGate {
    let source_matches = evidence.source_revision == report.source_revision;
    let gpu_payload = tldg_gpu_release_payload_digest(evidence);
    let gpu = source_matches
        && verify_release_receipt(
            verifier,
            replay,
            evidence.gpu_attestation.clone(),
            ReceiptClass::Accelerator,
            &report.source_revision,
            "tldg/release/gpu",
            &gpu_payload,
            now_epoch,
        );
    let host = source_matches
        && verify_release_receipt(
            verifier,
            replay,
            evidence.host_attestation.clone(),
            ReceiptClass::HostOutput,
            &report.source_revision,
            "tldg/release/host",
            &evidence.host_binder_digest,
            now_epoch,
        );
    let adversarial_receipt = source_matches
        && verify_release_receipt(
            verifier,
            replay,
            evidence.adversarial_attestation.clone(),
            ReceiptClass::ReleaseEvaluation,
            &report.source_revision,
            "tldg/release/adversarial",
            &evidence.adversarial_digest,
            now_epoch,
        );
    assemble_release_gate(
        report,
        adversarial,
        gpu,
        gpu && evidence.gpu_lane_parity && valid_digest(&evidence.gpu_parity_digest),
        host,
        adversarial_receipt,
    )
}

fn assemble_release_gate(
    report: &TldgPipelineReport,
    adversarial: &crate::AdversarialEvaluationReport,
    real_gpu_observed: bool,
    gpu_lane_parity: bool,
    host_binder_source_connected: bool,
    adversarial_receipt: bool,
) -> TldgReleaseGate {
    let predicates: BTreeMap<String, bool> = BTreeMap::from([
        ("local_source_implemented".into(), report.local_source_ready),
        (
            "builtin_profiles_validated".into(),
            report.builtin_profiles_validated,
        ),
        ("real_gpu_geometry_observed".into(), real_gpu_observed),
        ("gpu_numeric_lane_parity".into(), gpu_lane_parity),
        (
            "host_binder_source_connected".into(),
            host_binder_source_connected,
        ),
        (
            "adversarial_local_gates".into(),
            adversarial_receipt
                && adversarial.source_roundtrip_pass
                && adversarial.geometry_non_authority_pass
                && adversarial.unknown_retention_pass,
        ),
    ]);
    let blocked_reasons = predicates
        .iter()
        .filter_map(|(name, passed)| (!passed).then_some(name.clone()))
        .collect::<Vec<_>>();
    let mut residuals = Vec::new();
    if !report.external_backends_observed {
        residuals.push("optional_external_precision_backends_unobserved".into());
    }
    if !report.output.exact_host_output_bound {
        residuals.push("current_run_exact_host_output_unbound".into());
    }
    residuals.push("remote_tldg_ci_unobserved".into());
    residuals.push("statistical_coverage_unobserved".into());
    let release_complete = blocked_reasons.is_empty();
    TldgReleaseGate {
        local_source_implemented: report.local_source_ready,
        release_complete,
        deployment_complete: false,
        predicates,
        blocked_reasons,
        residuals,
        automatic_promotion: false,
        claim_boundary:
            "source package completion uses validated builtin profiles, real GPU numeric parity, and host-binder source connectivity; deployment and general semantic claims remain separate"
                .into(),
    }
}

#[allow(clippy::too_many_arguments)]
fn verify_release_receipt(
    verifier: &ReceiptVerifier,
    replay: &mut ReplayGuard,
    attestation: Option<UntrustedReceipt>,
    class: ReceiptClass,
    source_revision: &str,
    scope: &str,
    payload: &str,
    now_epoch: u64,
) -> bool {
    let Some(attestation) = attestation else {
        return false;
    };
    let Ok(subject) = SubjectRevision::checked(source_revision) else {
        return false;
    };
    let Ok(scope) = ReceiptScope::checked(scope) else {
        return false;
    };
    verifier
        .verify(
            attestation,
            &ReceiptPolicy::exact(class, subject, scope, payload, now_epoch),
            replay,
        )
        .is_ok()
}

pub fn tldg_gpu_release_payload_digest(evidence: &TldgReleaseEvidence) -> String {
    stable_sha256(&format!(
        "{}\0{}\0{}\0{}",
        evidence.source_revision,
        evidence.gpu_execution_digest,
        evidence.gpu_parity_digest,
        evidence.gpu_lane_parity
    ))
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn build_output_closure(
    source: &str,
    candidate_output: Option<&str>,
    dg1: &Dg1Report,
) -> Result<OutputClosure, PipelineError> {
    let source_revision = stable_sha256(source);
    let mut graph = ProjectionDefectGraphV3::new(SourceLedgerId(631_022), &source_revision, source)
        .map_err(|error| PipelineError::Projection(format!("{error:?}")))?;
    let mut next_obligation_id = 1u64;
    let mut next_edge_id = 1u64;
    let mut projected_obligation_count = 0usize;
    let mut projection_edge_count = 0usize;
    for requirement in &dg1.requirement_candidates {
        let span = SourceSpan::checked(
            source,
            requirement.source_span.start,
            requirement.source_span.end,
        )
        .map_err(|error| PipelineError::Projection(format!("{error:?}")))?;
        let mut dimensions = vec![(ObligationKind::Semantic, "semantic")];
        if requirement.polarity != crate::Dg1RequirementPolarity::Positive {
            dimensions.push((ObligationKind::Polarity, "polarity"));
        }
        if requirement.conditional || requirement.exception_present {
            dimensions.push((ObligationKind::Scope, "scope"));
        }
        for (kind, dimension) in dimensions {
            let obligation_id = ObligationId(next_obligation_id);
            next_obligation_id = next_obligation_id.saturating_add(1);
            let obligation = Obligation {
                id: obligation_id,
                kind,
                strength: match requirement.strength {
                    crate::Dg1RequirementStrength::Must => ObligationStrength::Must,
                    crate::Dg1RequirementStrength::Should => ObligationStrength::Should,
                    crate::Dg1RequirementStrength::May => ObligationStrength::May,
                },
                polarity: match requirement.polarity {
                    crate::Dg1RequirementPolarity::Positive => ObligationPolarity::Positive,
                    crate::Dg1RequirementPolarity::Negative => ObligationPolarity::Negative,
                    crate::Dg1RequirementPolarity::Unknown => ObligationPolarity::Unknown,
                    crate::Dg1RequirementPolarity::Conflict => ObligationPolarity::Conflict,
                },
                scope: format!(
                    "dg1-requirement-{}:{dimension}:conditional={}:exception={}",
                    requirement.id, requirement.conditional, requirement.exception_present
                ),
                source_span: span,
                source_text: requirement.source_text.clone(),
            };
            graph
                .add_obligation(obligation)
                .map_err(|error| PipelineError::Projection(format!("{error:?}")))?;
            let anchor = graph
                .obligation_anchor(obligation_id)
                .cloned()
                .ok_or_else(|| PipelineError::Projection("requirement_anchor_missing".into()))?;

            let contract = serde_json::to_string(requirement).map_err(|error| {
                PipelineError::Projection(format!("requirement_encode:{error}"))
            })?;
            let program = serde_json::to_string(&RequirementProgramIr {
                schema_version: "epistesys-dgcl-program-ir-requirement.v1",
                requirement_id: requirement.id,
                source_revision: &source_revision,
                source_span: span,
                source_text: &requirement.source_text,
                language: requirement.language,
                strength: requirement.strength,
                polarity: requirement.polarity,
                conditional: requirement.conditional,
                exception_present: requirement.exception_present,
                condition_kind: requirement.condition_kind,
                scope: &requirement.scope,
                unresolved_reference: requirement.unresolved_reference,
                authority_grant: false,
            })
            .map_err(|error| {
                PipelineError::Projection(format!("program_requirement_encode:{error}"))
            })?;
            let contract_id = TargetClaimId(obligation_id.0.saturating_mul(10).saturating_add(1));
            let program_id = TargetClaimId(obligation_id.0.saturating_mul(10).saturating_add(2));
            for (id, artifact, path, content) in [
                (
                    contract_id,
                    "task-contract",
                    format!("requirements/{}/{}", requirement.id, dimension),
                    contract.as_str(),
                ),
                (
                    program_id,
                    "program-ir",
                    format!("requirements/{}/{}", requirement.id, dimension),
                    program.as_str(),
                ),
            ] {
                graph
                    .add_target_claim(
                        TargetClaim::checked(id, artifact, path, stable_sha256(content), content)
                            .map_err(|error| PipelineError::Projection(format!("{error:?}")))?,
                    )
                    .map_err(|error| PipelineError::Projection(format!("{error:?}")))?;
            }
            let mut stage_targets = vec![
                (ProjectionStage::SeedToContract, vec![contract_id]),
                (ProjectionStage::ContractToProgram, vec![program_id]),
            ];
            if let Some(candidate) = candidate_output.filter(|value| !value.trim().is_empty()) {
                let candidate_id =
                    TargetClaimId(obligation_id.0.saturating_mul(10).saturating_add(3));
                let output_id = TargetClaimId(obligation_id.0.saturating_mul(10).saturating_add(4));
                for (id, artifact, path) in [
                    (candidate_id, "implementation-candidate", "candidate"),
                    (output_id, "candidate-output", "output"),
                ] {
                    graph
                        .add_target_claim(
                            TargetClaim::checked(
                                id,
                                artifact,
                                format!("requirements/{}/{}", requirement.id, path),
                                stable_sha256(candidate),
                                candidate,
                            )
                            .map_err(|error| PipelineError::Projection(format!("{error:?}")))?,
                        )
                        .map_err(|error| PipelineError::Projection(format!("{error:?}")))?;
                }
                stage_targets.push((ProjectionStage::ProgramToCandidate, vec![candidate_id]));
                stage_targets.push((ProjectionStage::CandidateToOutput, vec![output_id]));
            } else {
                stage_targets.push((ProjectionStage::ProgramToCandidate, Vec::new()));
                stage_targets.push((ProjectionStage::CandidateToOutput, Vec::new()));
            }
            for (stage, targets) in stage_targets {
                graph
                    .add_edge(ProjectionEdgeV3 {
                        id: ProjectionEdgeId(next_edge_id),
                        obligation_id: Some(obligation_id),
                        source: Some(anchor.clone()),
                        targets,
                        stage,
                        state: PreservationState::Unresolved,
                        rule_id: format!("dg1-requirement-projection.v1:{dimension}"),
                        verifier: VerificationReceipt {
                            verifier: VerifierKind::ModelAdvisory,
                            status: VerificationStatus::NeedsEvidence,
                            revision: "unverified-requirement-projection.v1".into(),
                            evidence_digest: None,
                        },
                    })
                    .map_err(|error| PipelineError::Projection(format!("{error:?}")))?;
                next_edge_id = next_edge_id.saturating_add(1);
                projection_edge_count += 1;
            }
            projected_obligation_count += 1;
        }
    }
    let empty_requirement_set_held = dg1.requirement_candidates.is_empty();
    let defect_gate = if empty_requirement_set_held {
        ProjectionGateDecision::Clarify
    } else {
        graph.gate()
    };
    Ok(OutputClosure {
        binding_state: "host_output_unbound".into(),
        requirement_projection_state: if empty_requirement_set_held {
            "no_supported_requirement_candidate_clarify".into()
        } else {
            "requirement_candidates_projected_but_unverified_host_output_unbound".into()
        },
        defect_gate,
        output_commit_allowed: false,
        authority_created: false,
        exact_host_output_bound: false,
        requirement_candidate_count: dg1.requirement_candidates.len(),
        projected_obligation_count,
        projection_edge_count,
        unexplained_prose_count: dg1.instruction_residuals.len(),
        empty_requirement_set_held,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_pipeline_is_source_ready_but_not_host_bound() {
        let report =
            build_tldg_pipeline("answer", None, &CalibrationPartition::reference()).unwrap();
        assert!(!report.local_source_ready);
        assert!(!report.builtin_profiles_validated);
        assert!(!report.output.output_commit_allowed);
        assert!(!report.external_backends_observed);
    }

    #[test]
    fn legacy_boolean_release_gate_cannot_close() {
        let report =
            build_tldg_pipeline("answer", None, &CalibrationPartition::reference()).unwrap();
        let adversarial = crate::run_adversarial_evaluation().unwrap();
        let gate = build_tldg_release_gate_with_receipts(&report, &adversarial, true, true, true);
        assert!(!gate.release_complete);
        assert!(gate
            .blocked_reasons
            .contains(&"legacy_boolean_release_evidence_rejected".to_string()));
    }

    #[test]
    fn typed_attested_release_gate_closes_source_only() {
        let key = [43_u8; 32];
        let issuer = ReceiptIssuer::from_key_bytes("lc631-tldg-root", &key).unwrap();
        let verifier = ReceiptVerifier::from_key_bytes("lc631-tldg-root", &key).unwrap();
        let mut replay = ReplayGuard::default();
        let report = build_tldg_pipeline_with_receipts(
            "answer",
            None,
            &CalibrationPartition::reference(),
            &issuer,
            &verifier,
            &mut replay,
            1,
        )
        .unwrap();
        assert!(report.local_source_ready);
        assert_eq!(report.backend_validation.accepted_count, 3);
        assert_eq!(report.backend_validation.rejected_count, 0);
        let adversarial = crate::run_adversarial_evaluation().unwrap();
        let mut evidence = TldgReleaseEvidence {
            source_revision: report.source_revision.clone(),
            gpu_execution_digest: stable_sha256("gpu-execution"),
            gpu_parity_digest: stable_sha256("gpu-parity"),
            gpu_lane_parity: true,
            host_binder_digest: stable_sha256("host-binder"),
            adversarial_digest: stable_sha256(&format!("{adversarial:?}")),
            gpu_attestation: None,
            host_attestation: None,
            adversarial_attestation: None,
        };
        let issue = |class, scope: &str, payload: String| {
            issuer
                .issue(
                    class,
                    SubjectRevision::checked(&report.source_revision).unwrap(),
                    ReceiptScope::checked(scope).unwrap(),
                    payload,
                    1,
                    None,
                    None,
                )
                .unwrap()
        };
        evidence.gpu_attestation = Some(issue(
            ReceiptClass::Accelerator,
            "tldg/release/gpu",
            tldg_gpu_release_payload_digest(&evidence),
        ));
        evidence.host_attestation = Some(issue(
            ReceiptClass::HostOutput,
            "tldg/release/host",
            evidence.host_binder_digest.clone(),
        ));
        evidence.adversarial_attestation = Some(issue(
            ReceiptClass::ReleaseEvaluation,
            "tldg/release/adversarial",
            evidence.adversarial_digest.clone(),
        ));
        let mut release_replay = ReplayGuard::default();
        let gate = build_tldg_release_gate_verified(
            &report,
            &adversarial,
            &evidence,
            &verifier,
            &mut release_replay,
            1,
        );
        assert!(gate.release_complete);
        assert!(gate.blocked_reasons.is_empty());
        assert!(!gate.deployment_complete);
    }

    #[test]
    fn dgcl_12_projects_each_supported_requirement_into_tl_without_claiming_closure() {
        let report = build_tldg_pipeline(
            "Do not edit this file. If tests pass, document the result.",
            Some("candidate answer"),
            &CalibrationPartition::reference(),
        )
        .unwrap();
        assert_eq!(report.dg1.requirement_candidates.len(), 2);
        assert_eq!(report.output.requirement_candidate_count, 2);
        assert!(report.output.projected_obligation_count >= 4);
        assert_eq!(
            report.output.projection_edge_count,
            report.output.projected_obligation_count * 4
        );
        assert_eq!(report.output.defect_gate, ProjectionGateDecision::Clarify);
        assert!(!report.output.output_commit_allowed);
        assert!(!report.output.authority_created);
    }

    #[test]
    fn dgcl_12_empty_extraction_cannot_become_a_noop_commit() {
        let report = build_tldg_pipeline(
            "An observation without a supported directive.",
            None,
            &CalibrationPartition::reference(),
        )
        .unwrap();
        assert!(report.dg1.requirement_candidates.is_empty());
        assert!(report.output.empty_requirement_set_held);
        assert_eq!(report.output.defect_gate, ProjectionGateDecision::Clarify);
        assert!(!report.output.output_commit_allowed);
    }
}
