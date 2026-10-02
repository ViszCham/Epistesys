use crate::{
    DgclInstructionParse, DgclInstructionParseState, DgclLanguageRegion, InstructionAst,
    InstructionParseState,
};
use lc631_core::{stable_sha256, ObligationId, SourceSpan};
use lc631_tl::{
    CanonicalTranslationEnvelope, DomainTranslationPayload, Obligation, ObligationKind,
    ObligationPolarity, ObligationStrength, PreservationState, ProjectionDefectGraphV3,
    ProjectionEdgeId, ProjectionEdgeV3, ProjectionGateDecision, ProjectionStage, SourceLedgerId,
    TargetClaim, TargetClaimId, VerificationReceipt, VerificationStatus, VerifierKind,
};
use lc631_tldg::{Dg1ContentRole, Dg1RegionKind, Dg1Report};
use serde::Serialize;
use std::collections::BTreeSet;
use std::ops::Range;

pub const DGCL_PROGRAM_IR_SCHEMA: &str = "epistesys-dgcl-program-ir.v1";
pub const DGCL_TRANSLATION_PROJECTION_SCHEMA: &str = "epistesys-dgcl-translation-projection.v1";

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DgclProgramAlternative {
    pub alternative_id: String,
    pub production_id: u32,
    pub language: crate::InstructionLanguage,
    pub parser_ast_digest: String,
    pub region_offset: usize,
    pub root_source_span: Range<usize>,
    /// Parser spans inside `ast` remain region-relative; `region_offset` maps them to root bytes.
    pub ast: InstructionAst,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DgclProgramDependency {
    pub language_span_id: u32,
    pub source_span: Range<usize>,
    pub backend_revision: String,
    pub model_manifest_digest: String,
    pub runtime_manifest_digest: String,
    pub requirements_lock_digest: String,
    pub payload_binding_revision: String,
    pub payload_digest: String,
    pub semantic_truth_claim: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DgclProgramRequirement {
    pub requirement_id: String,
    pub region_id: u32,
    pub root_source_revision: String,
    pub source_span: Range<usize>,
    pub source_digest: String,
    pub parse_state: DgclInstructionParseState,
    pub legacy_candidate_references: Vec<u32>,
    pub contract_digest: String,
    pub contract: DgclInstructionParse,
    pub selected_alternative_id: Option<String>,
    pub alternatives: Vec<DgclProgramAlternative>,
    pub dependencies: Vec<DgclProgramDependency>,
    pub semantic_truth_claim: bool,
    pub authority_grant: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DgclProgramResidual {
    pub residual_id: String,
    pub region_id: u32,
    pub source_span: Range<usize>,
    pub source_digest: String,
    pub state: DgclInstructionParseState,
    pub legacy_candidate_references: Vec<u32>,
    pub reason: String,
    pub blocking_requirement: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DgclProgramIrReport {
    pub schema_version: &'static str,
    pub source_revision: String,
    pub grammar_revision: &'static str,
    pub program_digest: String,
    pub requirements: Vec<DgclProgramRequirement>,
    pub residuals: Vec<DgclProgramResidual>,
    pub structural_distillation: crate::DgclStructuralDistillation,
    pub semantic_truth_claim: bool,
    pub authority_created: bool,
    pub claim_boundary: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TranslationLossKind {
    SemanticPreservationUnverified,
    UnsupportedInputRetained,
    DownstreamCandidateAbsent,
    FinalOutputUnbound,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionDefectDimension {
    Requirement,
    SourceAnchor,
    Alternative,
    Polarity,
    Condition,
    ScopeDependency,
    EvidenceBinding,
    SemanticStructure,
    UnsupportedResidual,
    ProgramIdentity,
    TranslationGraph,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TranslationLossEvent {
    pub requirement_id: String,
    pub stage: ProjectionStage,
    pub input_digest: Option<String>,
    pub output_digest: Option<String>,
    pub stage_event_digest: String,
    pub state: PreservationState,
    pub kind: TranslationLossKind,
    pub reason: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DgclTranslationProjection {
    pub schema_version: &'static str,
    pub source_revision: String,
    pub program_ir_digest: String,
    pub envelope: CanonicalTranslationEnvelope,
    pub loss_events: Vec<TranslationLossEvent>,
    pub gate: ProjectionGateDecision,
    pub empty_requirement_set_held: bool,
    pub scalar_aggregate_used: bool,
    pub authority_created: bool,
    pub output_commit_allowed: bool,
    pub claim_boundary: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum DgclProjectionError {
    SourceRevisionMismatch,
    SourceSpanInvalid,
    ParseRevisionMismatch,
    ParseStateMismatch,
    LegacyRequirementUnknown,
    DuplicateProgramRequirement,
    ProgramIrMismatch,
    ProjectionDefect(ProjectionDefectDimension),
    TlProjection(String),
    PayloadEncoding,
}

pub fn build_dgcl_projection(
    source: &str,
    dg1: &Dg1Report,
    language_regions: &[DgclLanguageRegion],
    instruction_parses: &[DgclInstructionParse],
) -> Result<(DgclProgramIrReport, DgclTranslationProjection), DgclProjectionError> {
    let source_revision = stable_sha256(source);
    if dg1.source_revision != source_revision || !dg1.exact_source_roundtrip {
        return Err(DgclProjectionError::SourceRevisionMismatch);
    }
    let operative_regions = dg1
        .regions
        .iter()
        .filter(|region| {
            region.kind == Dg1RegionKind::Prose
                && region.content_role == Dg1ContentRole::OperativeProse
        })
        .collect::<Vec<_>>();
    if operative_regions.len() != instruction_parses.len() {
        return Err(DgclProjectionError::ParseStateMismatch);
    }
    let mut seen_language_spans = BTreeSet::new();
    for view in language_regions {
        let language_span = dg1
            .language_span_lattice
            .iter()
            .find(|span| span.id == view.language_span_id)
            .ok_or(DgclProjectionError::SourceSpanInvalid)?;
        let region = dg1
            .regions
            .iter()
            .find(|region| region.id == view.region_id)
            .ok_or(DgclProjectionError::SourceSpanInvalid)?;
        let view_text = source
            .get(view.source_span.clone())
            .ok_or(DgclProjectionError::SourceSpanInvalid)?;
        let payload = serde_json::to_string(&(
            "epistesys-dgcl-language-region-payload.v2",
            &view.root_source_revision,
            view.region_id,
            view.language_span_id,
            view.source_span.clone(),
            view.language,
            &view.region_source_revision,
            &view.backend_revision,
            &view.model_manifest_digest,
            &view.runtime_manifest_digest,
            &view.requirements_lock_digest,
            &view.python_version,
            &view.sentences,
        ))
        .map_err(|_| DgclProjectionError::PayloadEncoding)?;
        if !seen_language_spans.insert(view.language_span_id)
            || view.root_source_revision != source_revision
            || view.source_span != language_span.source_span
            || language_span.region_id != view.region_id
            || region.span.start > view.source_span.start
            || region.span.end < view.source_span.end
            || language_span.language != view.language
            || view.region_source_revision != stable_sha256(view_text)
            || view.payload_binding_revision != "epistesys-dgcl-language-region-payload.v2"
            || view.payload_digest != stable_sha256(&payload)
            || view.semantic_truth_claim
            || !valid_digest(&view.model_manifest_digest)
            || !valid_digest(&view.runtime_manifest_digest)
            || !valid_digest(&view.requirements_lock_digest)
            || view.backend_revision.trim().is_empty()
        {
            return Err(DgclProjectionError::SourceRevisionMismatch);
        }
    }
    let mut requirements = Vec::new();
    let mut residuals = Vec::new();
    let mut seen_regions = BTreeSet::new();
    let mut seen_requirement_ids = BTreeSet::new();
    let mut referenced_legacy_ids = BTreeSet::new();

    for parse in instruction_parses {
        if !seen_regions.insert(parse.region_id) || parse.root_source_revision != source_revision {
            return Err(DgclProjectionError::SourceRevisionMismatch);
        }
        let operative_region = operative_regions
            .iter()
            .find(|region| region.id == parse.region_id)
            .ok_or(DgclProjectionError::ParseStateMismatch)?;
        if operative_region.span != parse.source_span {
            return Err(DgclProjectionError::SourceSpanInvalid);
        }
        let region_text = source
            .get(parse.source_span.clone())
            .ok_or(DgclProjectionError::SourceSpanInvalid)?;
        for legacy_id in &parse.legacy_requirement_ids {
            let candidate = dg1
                .requirement_candidates
                .iter()
                .find(|candidate| candidate.id == *legacy_id)
                .ok_or(DgclProjectionError::LegacyRequirementUnknown)?;
            if candidate.source_span.start < parse.source_span.start
                || candidate.source_span.end > parse.source_span.end
            {
                return Err(DgclProjectionError::SourceSpanInvalid);
            }
            referenced_legacy_ids.insert(*legacy_id);
        }

        let mut alternatives = Vec::new();
        let mut selected_alternative_id = None;
        let contract_digest = serde_json::to_string(parse)
            .map(|payload| stable_sha256(&payload))
            .map_err(|_| DgclProjectionError::PayloadEncoding)?;
        if let Some(report) = &parse.report {
            if report.source_revision != stable_sha256(region_text)
                || report.grammar_revision != crate::INSTRUCTION_GRAMMAR_REVISION
            {
                return Err(DgclProjectionError::ParseRevisionMismatch);
            }
            if map_parse_state(report.state) != parse.state {
                return Err(DgclProjectionError::ParseStateMismatch);
            }
            let mut alternative_ids = BTreeSet::new();
            for ast in &report.alternatives {
                validate_ast_spans(ast, region_text)?;
                let ast_json =
                    serde_json::to_string(ast).map_err(|_| DgclProjectionError::PayloadEncoding)?;
                let parser_ast_digest = stable_sha256(&ast_json);
                let alternative_id = stable_sha256(&format!(
                    "epistesys-dgcl-program-alternative.v1\0{source_revision}\0{}\0{}\0{}\0{}",
                    parse.region_id,
                    parse.source_span.start,
                    parse.source_span.end,
                    parser_ast_digest
                ));
                if !alternative_ids.insert(alternative_id.clone()) {
                    return Err(DgclProjectionError::ProgramIrMismatch);
                }
                alternatives.push(DgclProgramAlternative {
                    alternative_id,
                    production_id: ast.production_id,
                    language: ast.language,
                    parser_ast_digest,
                    region_offset: parse.source_span.start,
                    root_source_span: parse.source_span.start + ast.source_span.start
                        ..parse.source_span.start + ast.source_span.end,
                    ast: ast.clone(),
                });
            }
            match (&report.selected, report.state) {
                (Some(selected), InstructionParseState::Parsed) => {
                    let selected_digest = stable_sha256(
                        &serde_json::to_string(selected)
                            .map_err(|_| DgclProjectionError::PayloadEncoding)?,
                    );
                    let matches = alternatives
                        .iter()
                        .filter(|alternative| alternative.parser_ast_digest == selected_digest)
                        .collect::<Vec<_>>();
                    if matches.len() != 1 {
                        return Err(DgclProjectionError::ProgramIrMismatch);
                    }
                    selected_alternative_id = Some(matches[0].alternative_id.clone());
                }
                (None, InstructionParseState::Parsed) | (Some(_), _) => {
                    return Err(DgclProjectionError::ParseStateMismatch);
                }
                (None, _) => {}
            }
        } else if parse.state != DgclInstructionParseState::BudgetRejected {
            return Err(DgclProjectionError::ParseStateMismatch);
        }

        let is_requirement = !alternatives.is_empty();
        if is_requirement {
            let requirement_id = stable_sha256(&format!(
                "epistesys-dgcl-program-requirement.v1\0{source_revision}\0{}\0{}\0{}\0{}",
                parse.region_id, parse.source_span.start, parse.source_span.end, contract_digest
            ));
            if !seen_requirement_ids.insert(requirement_id.clone()) {
                return Err(DgclProjectionError::DuplicateProgramRequirement);
            }
            let dependencies = language_regions
                .iter()
                .filter(|view| {
                    view.source_span.start < parse.source_span.end
                        && parse.source_span.start < view.source_span.end
                })
                .map(|view| {
                    Ok(DgclProgramDependency {
                        language_span_id: view.language_span_id,
                        source_span: view.source_span.clone(),
                        backend_revision: view.backend_revision.clone(),
                        model_manifest_digest: view.model_manifest_digest.clone(),
                        runtime_manifest_digest: view.runtime_manifest_digest.clone(),
                        requirements_lock_digest: view.requirements_lock_digest.clone(),
                        payload_binding_revision: view.payload_binding_revision.into(),
                        payload_digest: view.payload_digest.clone(),
                        semantic_truth_claim: false,
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            requirements.push(DgclProgramRequirement {
                requirement_id,
                region_id: parse.region_id,
                root_source_revision: source_revision.clone(),
                source_span: parse.source_span.clone(),
                source_digest: stable_sha256(region_text),
                parse_state: parse.state,
                legacy_candidate_references: parse.legacy_requirement_ids.clone(),
                contract_digest,
                contract: parse.clone(),
                selected_alternative_id,
                alternatives,
                dependencies,
                semantic_truth_claim: false,
                authority_grant: false,
            });
        } else {
            let residual_id = stable_sha256(&format!(
                "epistesys-dgcl-projection-residual.v1\0{source_revision}\0{}\0{}\0{}\0{:?}",
                parse.region_id, parse.source_span.start, parse.source_span.end, parse.state
            ));
            residuals.push(DgclProgramResidual {
                residual_id,
                region_id: parse.region_id,
                source_span: parse.source_span.clone(),
                source_digest: stable_sha256(region_text),
                state: parse.state,
                legacy_candidate_references: parse.legacy_requirement_ids.clone(),
                reason: if parse.legacy_requirement_ids.is_empty() {
                    "operative prose was retained without inventing a supported instruction or action".into()
                } else {
                    "legacy candidate has no controlled-grammar alternative and remains a blocking residual".into()
                },
                blocking_requirement: !parse.legacy_requirement_ids.is_empty(),
            });
        }
    }

    for candidate in &dg1.requirement_candidates {
        if !referenced_legacy_ids.contains(&candidate.id) {
            let source_text = source
                .get(candidate.source_span.clone())
                .ok_or(DgclProjectionError::SourceSpanInvalid)?;
            let residual_id = stable_sha256(&format!(
                "epistesys-dgcl-unbound-legacy-candidate.v1\0{source_revision}\0{}\0{}\0{}",
                candidate.id, candidate.source_span.start, candidate.source_span.end
            ));
            residuals.push(DgclProgramResidual {
                residual_id,
                region_id: 0,
                source_span: candidate.source_span.clone(),
                source_digest: stable_sha256(source_text),
                state: DgclInstructionParseState::Unresolved,
                legacy_candidate_references: vec![candidate.id],
                reason: "legacy candidate has no matching controlled-grammar contract".into(),
                blocking_requirement: true,
            });
        }
    }

    requirements.sort_by_key(|requirement| (requirement.source_span.start, requirement.region_id));
    residuals.sort_by_key(|residual| (residual.source_span.start, residual.region_id));
    let structural_distillation = crate::distill_dgcl_constraints(source, &requirements)?;
    let program_digest = stable_sha256(
        &serde_json::to_string(&(
            DGCL_PROGRAM_IR_SCHEMA,
            &source_revision,
            crate::INSTRUCTION_GRAMMAR_REVISION,
            &requirements,
            &residuals,
            &structural_distillation,
            false,
            false,
        ))
        .map_err(|_| DgclProjectionError::PayloadEncoding)?,
    );
    let program_ir = DgclProgramIrReport {
        schema_version: DGCL_PROGRAM_IR_SCHEMA,
        source_revision: source_revision.clone(),
        grammar_revision: crate::INSTRUCTION_GRAMMAR_REVISION,
        program_digest,
        requirements,
        residuals,
        structural_distillation,
        semantic_truth_claim: false,
        authority_created: false,
        claim_boundary: "lossless source-anchored projection of controlled grammar candidates; not semantic validation or execution authority",
    };
    let translation = build_translation_projection(source, &program_ir)?;
    Ok((program_ir, translation))
}

pub fn verify_dgcl_projection(
    source: &str,
    dg1: &Dg1Report,
    language_regions: &[DgclLanguageRegion],
    instruction_parses: &[DgclInstructionParse],
    program_ir: &DgclProgramIrReport,
    translation: &DgclTranslationProjection,
) -> Result<(), DgclProjectionError> {
    let (expected_program, expected_translation) =
        build_dgcl_projection(source, dg1, language_regions, instruction_parses)?;
    if &expected_program != program_ir {
        return Err(DgclProjectionError::ProjectionDefect(
            classify_program_difference(&expected_program, program_ir),
        ));
    }
    if &expected_translation != translation {
        return Err(DgclProjectionError::ProjectionDefect(
            ProjectionDefectDimension::TranslationGraph,
        ));
    }
    Ok(())
}

fn classify_program_difference(
    expected: &DgclProgramIrReport,
    observed: &DgclProgramIrReport,
) -> ProjectionDefectDimension {
    if expected.source_revision != observed.source_revision
        || expected.schema_version != observed.schema_version
        || expected.grammar_revision != observed.grammar_revision
    {
        return ProjectionDefectDimension::SourceAnchor;
    }
    if expected.requirements.len() != observed.requirements.len() {
        return ProjectionDefectDimension::Requirement;
    }
    for expected_requirement in &expected.requirements {
        let Some(observed_requirement) = observed
            .requirements
            .iter()
            .find(|requirement| requirement.requirement_id == expected_requirement.requirement_id)
        else {
            return ProjectionDefectDimension::Requirement;
        };
        if expected_requirement.root_source_revision != observed_requirement.root_source_revision
            || expected_requirement.source_span != observed_requirement.source_span
            || expected_requirement.source_digest != observed_requirement.source_digest
        {
            return ProjectionDefectDimension::SourceAnchor;
        }
        if expected_requirement.parse_state != observed_requirement.parse_state
            || expected_requirement.legacy_candidate_references
                != observed_requirement.legacy_candidate_references
        {
            return ProjectionDefectDimension::Requirement;
        }
        if expected_requirement.alternatives.len() != observed_requirement.alternatives.len()
            || expected_requirement.selected_alternative_id
                != observed_requirement.selected_alternative_id
        {
            return ProjectionDefectDimension::Alternative;
        }
        for expected_alternative in &expected_requirement.alternatives {
            let Some(observed_alternative) =
                observed_requirement
                    .alternatives
                    .iter()
                    .find(|alternative| {
                        alternative.alternative_id == expected_alternative.alternative_id
                    })
            else {
                return ProjectionDefectDimension::Alternative;
            };
            if expected_alternative.region_offset != observed_alternative.region_offset
                || expected_alternative.root_source_span != observed_alternative.root_source_span
                || expected_alternative.language != observed_alternative.language
            {
                return ProjectionDefectDimension::SourceAnchor;
            }
            if let Some(dimension) =
                classify_ast_difference(&expected_alternative.ast, &observed_alternative.ast)
            {
                return dimension;
            }
        }
        if expected_requirement.dependencies != observed_requirement.dependencies {
            return ProjectionDefectDimension::EvidenceBinding;
        }
        if expected_requirement.contract_digest != observed_requirement.contract_digest
            || expected_requirement.contract != observed_requirement.contract
        {
            return ProjectionDefectDimension::Alternative;
        }
        if expected_requirement != observed_requirement {
            return ProjectionDefectDimension::ProgramIdentity;
        }
    }
    if expected.residuals != observed.residuals {
        return ProjectionDefectDimension::UnsupportedResidual;
    }
    if expected.semantic_truth_claim != observed.semantic_truth_claim
        || expected.authority_created != observed.authority_created
        || expected.program_digest != observed.program_digest
    {
        return ProjectionDefectDimension::ProgramIdentity;
    }
    ProjectionDefectDimension::ProgramIdentity
}

fn classify_ast_difference(
    expected: &InstructionAst,
    observed: &InstructionAst,
) -> Option<ProjectionDefectDimension> {
    if expected.source_span != observed.source_span
        || expected.marker_span != observed.marker_span
        || expected.body_span != observed.body_span
    {
        return Some(ProjectionDefectDimension::SourceAnchor);
    }
    if expected.conditions != observed.conditions {
        return Some(ProjectionDefectDimension::Condition);
    }
    if expected.scopes != observed.scopes
        || expected.references != observed.references
        || expected.constraint_graph != observed.constraint_graph
        || expected.coordination != observed.coordination
        || expected.coordination_state != observed.coordination_state
        || expected.polarity_scopes != observed.polarity_scopes
    {
        return Some(ProjectionDefectDimension::ScopeDependency);
    }
    if expected.actions.len() != observed.actions.len() {
        return Some(ProjectionDefectDimension::SemanticStructure);
    }
    if expected
        .actions
        .iter()
        .map(|action| action.polarity)
        .ne(observed.actions.iter().map(|action| action.polarity))
    {
        return Some(ProjectionDefectDimension::Polarity);
    }
    if expected.actions != observed.actions {
        return Some(ProjectionDefectDimension::SemanticStructure);
    }
    (expected != observed).then_some(ProjectionDefectDimension::SemanticStructure)
}

fn build_translation_projection(
    source: &str,
    program_ir: &DgclProgramIrReport,
) -> Result<DgclTranslationProjection, DgclProjectionError> {
    if program_ir.source_revision != stable_sha256(source) {
        return Err(DgclProjectionError::SourceRevisionMismatch);
    }
    let mut graph = ProjectionDefectGraphV3::new(
        SourceLedgerId(632_012),
        program_ir.source_revision.clone(),
        source,
    )
    .map_err(|error| DgclProjectionError::TlProjection(format!("graph:{error:?}")))?;
    let mut next_obligation = 1_u64;
    let mut next_target = 1_u64;
    let mut next_edge = 1_u64;
    let mut loss_events = Vec::new();
    let mut domain_payloads = vec![DomainTranslationPayload {
        domain: "program-ir".into(),
        schema_version: program_ir.schema_version.into(),
        payload_digest: program_ir.program_digest.clone(),
        unknowns: vec!["semantic_equivalence_unverified".into()],
    }];
    for residual in &program_ir.residuals {
        domain_payloads.push(DomainTranslationPayload {
            domain: "unclassified-operative-span".into(),
            schema_version: "epistesys-dgcl-residual.v1".into(),
            payload_digest: residual.residual_id.clone(),
            unknowns: vec![residual.reason.clone()],
        });
        loss_events.push(translation_loss_event(
            residual.residual_id.clone(),
            ProjectionStage::SeedToContract,
            Some(residual.source_digest.clone()),
            None,
            PreservationState::Unresolved,
            TranslationLossKind::UnsupportedInputRetained,
            "source is retained as a residual instead of being promoted to a requirement",
        ));
    }

    let mut blocking_residuals = program_ir
        .residuals
        .iter()
        .filter(|residual| residual.blocking_requirement)
        .cloned()
        .collect::<Vec<_>>();
    if program_ir.requirements.is_empty() && blocking_residuals.is_empty() {
        blocking_residuals.push(DgclProgramResidual {
            residual_id: stable_sha256(&format!(
                "epistesys-dgcl-empty-requirement-set.v1\0{}",
                program_ir.source_revision
            )),
            region_id: 0,
            source_span: 0..source.len(),
            source_digest: stable_sha256(source),
            state: DgclInstructionParseState::Unsupported,
            legacy_candidate_references: Vec::new(),
            reason: "no supported instruction contract or explicit no-op contract was supplied"
                .into(),
            blocking_requirement: true,
        });
    }
    for residual in blocking_residuals {
        let span =
            SourceSpan::checked(source, residual.source_span.start, residual.source_span.end)
                .map_err(|_| DgclProjectionError::SourceSpanInvalid)?;
        let source_text = span
            .slice(source)
            .map_err(|_| DgclProjectionError::SourceSpanInvalid)?;
        let obligation_id = ObligationId(next_obligation);
        next_obligation = next_obligation
            .checked_add(1)
            .ok_or(DgclProjectionError::ProgramIrMismatch)?;
        graph
            .add_obligation(Obligation {
                id: obligation_id,
                kind: ObligationKind::Semantic,
                strength: if source_text.trim().is_empty() {
                    ObligationStrength::Should
                } else {
                    ObligationStrength::Must
                },
                polarity: ObligationPolarity::Unknown,
                scope: format!("dgcl/residual/{}", residual.residual_id),
                source_span: span,
                source_text: source_text.into(),
            })
            .map_err(|error| DgclProjectionError::TlProjection(format!("residual:{error:?}")))?;
        let anchor = graph
            .obligation_anchor(obligation_id)
            .cloned()
            .ok_or(DgclProjectionError::ProgramIrMismatch)?;
        let target_id = TargetClaimId(next_target);
        next_target = next_target
            .checked_add(1)
            .ok_or(DgclProjectionError::ProgramIrMismatch)?;
        let content =
            serde_json::to_string(&residual).map_err(|_| DgclProjectionError::PayloadEncoding)?;
        let residual_target_digest = target_content_digest(&residual.residual_id, &content);
        graph
            .add_target_claim(
                TargetClaim::checked(
                    target_id,
                    "unresolved-instruction-residual",
                    format!("residuals/{}", residual.residual_id),
                    &residual.residual_id,
                    &content,
                )
                .map_err(|error| {
                    DgclProjectionError::TlProjection(format!("residual:{error:?}"))
                })?,
            )
            .map_err(|error| DgclProjectionError::TlProjection(format!("residual:{error:?}")))?;
        graph
            .add_edge(ProjectionEdgeV3 {
                id: ProjectionEdgeId(next_edge),
                obligation_id: Some(obligation_id),
                source: Some(anchor),
                targets: vec![target_id],
                stage: ProjectionStage::SeedToContract,
                state: PreservationState::Unresolved,
                rule_id: "dgcl-unresolved-instruction-residual.v1".into(),
                verifier: VerificationReceipt {
                    verifier: VerifierKind::ModelAdvisory,
                    status: VerificationStatus::NeedsEvidence,
                    revision: "dgcl-residual-projection.v1".into(),
                    evidence_digest: None,
                },
            })
            .map_err(|error| DgclProjectionError::TlProjection(format!("residual:{error:?}")))?;
        next_edge = next_edge
            .checked_add(1)
            .ok_or(DgclProjectionError::ProgramIrMismatch)?;
        loss_events.push(translation_loss_event(
            residual.residual_id,
            ProjectionStage::SeedToContract,
            Some(residual.source_digest),
            Some(residual_target_digest),
            PreservationState::Unresolved,
            TranslationLossKind::UnsupportedInputRetained,
            "blocking residual prevents an empty or unbound requirement set from passing",
        ));
    }

    for requirement in &program_ir.requirements {
        let span = SourceSpan::checked(
            source,
            requirement.source_span.start,
            requirement.source_span.end,
        )
        .map_err(|_| DgclProjectionError::SourceSpanInvalid)?;
        let source_text = span
            .slice(source)
            .map_err(|_| DgclProjectionError::SourceSpanInvalid)?;
        if stable_sha256(source_text) != requirement.source_digest {
            return Err(DgclProjectionError::SourceRevisionMismatch);
        }
        let obligation_id = ObligationId(next_obligation);
        next_obligation = next_obligation
            .checked_add(1)
            .ok_or(DgclProjectionError::ProgramIrMismatch)?;
        let obligation = Obligation {
            id: obligation_id,
            kind: ObligationKind::Semantic,
            strength: ObligationStrength::Must,
            polarity: requirement_polarity(requirement),
            scope: format!("dgcl/requirement/{}", requirement.requirement_id),
            source_span: span,
            source_text: source_text.into(),
        };
        graph
            .add_obligation(obligation)
            .map_err(|error| DgclProjectionError::TlProjection(format!("obligation:{error:?}")))?;
        let anchor = graph
            .obligation_anchor(obligation_id)
            .cloned()
            .ok_or(DgclProjectionError::ProgramIrMismatch)?;

        let contract_content = serde_json::to_string(&requirement.contract)
            .map_err(|_| DgclProjectionError::PayloadEncoding)?;
        let program_content =
            serde_json::to_string(requirement).map_err(|_| DgclProjectionError::PayloadEncoding)?;
        let contract_target_digest =
            target_content_digest(&requirement.contract_digest, &contract_content);
        let program_target_digest =
            target_content_digest(&requirement.requirement_id, &program_content);
        let contract_id = TargetClaimId(next_target);
        next_target = next_target
            .checked_add(1)
            .ok_or(DgclProjectionError::ProgramIrMismatch)?;
        let program_id = TargetClaimId(next_target);
        next_target = next_target
            .checked_add(1)
            .ok_or(DgclProjectionError::ProgramIrMismatch)?;
        for (id, artifact, path, revision, content) in [
            (
                contract_id,
                "deep-grammar-contract",
                format!("regions/{}/contract", requirement.region_id),
                requirement.contract_digest.as_str(),
                contract_content.as_str(),
            ),
            (
                program_id,
                "program-ir-requirement",
                format!("requirements/{}", requirement.requirement_id),
                requirement.requirement_id.as_str(),
                program_content.as_str(),
            ),
        ] {
            graph
                .add_target_claim(
                    TargetClaim::checked(id, artifact, path, revision, content).map_err(
                        |error| DgclProjectionError::TlProjection(format!("target:{error:?}")),
                    )?,
                )
                .map_err(|error| DgclProjectionError::TlProjection(format!("target:{error:?}")))?;
        }
        for (stage, input_digest, output_digest, targets, kind, reason) in [
            (
                ProjectionStage::SeedToContract,
                Some(requirement.source_digest.clone()),
                Some(contract_target_digest.clone()),
                vec![contract_id],
                TranslationLossKind::SemanticPreservationUnverified,
                "surface contract is source-bound but its semantic interpretation is unverified",
            ),
            (
                ProjectionStage::ContractToProgram,
                Some(contract_target_digest),
                Some(program_target_digest.clone()),
                vec![program_id],
                TranslationLossKind::SemanticPreservationUnverified,
                "Program IR retains the parser AST and dependencies but semantic equivalence is unverified",
            ),
            (
                ProjectionStage::ProgramToCandidate,
                Some(program_target_digest),
                None,
                Vec::new(),
                TranslationLossKind::DownstreamCandidateAbsent,
                "no implementation candidate is bound to this Program IR requirement",
            ),
            (
                ProjectionStage::CandidateToOutput,
                None,
                None,
                Vec::new(),
                TranslationLossKind::FinalOutputUnbound,
                "no final output candidate or host-output receipt is connected",
            ),
        ] {
            graph
                .add_edge(ProjectionEdgeV3 {
                    id: ProjectionEdgeId(next_edge),
                    obligation_id: Some(obligation_id),
                    source: Some(anchor.clone()),
                    targets,
                    stage,
                    state: PreservationState::Unresolved,
                    rule_id: format!("dgcl-deepgrammar-program-ir-tl.v1:{stage:?}"),
                    verifier: VerificationReceipt {
                        verifier: VerifierKind::ModelAdvisory,
                        status: VerificationStatus::NeedsEvidence,
                        revision: "dgcl-structural-projection.v1".into(),
                        evidence_digest: None,
                    },
                })
                .map_err(|error| DgclProjectionError::TlProjection(format!("edge:{error:?}")))?;
            next_edge = next_edge
                .checked_add(1)
                .ok_or(DgclProjectionError::ProgramIrMismatch)?;
            loss_events.push(translation_loss_event(
                requirement.requirement_id.clone(),
                stage,
                input_digest,
                output_digest,
                PreservationState::Unresolved,
                kind,
                reason,
            ));
        }
        if !requirement.dependencies.is_empty() {
            let payload = serde_json::to_string(&requirement.dependencies)
                .map_err(|_| DgclProjectionError::PayloadEncoding)?;
            domain_payloads.push(DomainTranslationPayload {
                domain: format!("nlp-observations/{}", requirement.requirement_id),
                schema_version: "epistesys-dgcl-nlp-dependency.v1".into(),
                payload_digest: stable_sha256(&payload),
                unknowns: vec!["dependency_parse_is_not_semantic_truth_or_authority".into()],
            });
        }
    }
    let envelope = CanonicalTranslationEnvelope::from_graph(&graph, domain_payloads);
    let empty_requirement_set_held = program_ir.requirements.is_empty();
    let has_blocking_residual = program_ir
        .residuals
        .iter()
        .any(|residual| residual.blocking_requirement);
    let gate = if empty_requirement_set_held || has_blocking_residual {
        ProjectionGateDecision::Clarify
    } else {
        envelope.gate
    };
    Ok(DgclTranslationProjection {
        schema_version: DGCL_TRANSLATION_PROJECTION_SCHEMA,
        source_revision: program_ir.source_revision.clone(),
        program_ir_digest: program_ir.program_digest.clone(),
        envelope,
        loss_events,
        gate,
        empty_requirement_set_held,
        scalar_aggregate_used: false,
        authority_created: false,
        output_commit_allowed: false,
        claim_boundary: "typed unresolved translation-loss events over a single source-bound DeepGrammar-to-Program-IR path; no semantic equivalence, final output, authority, or commit is asserted",
    })
}

fn requirement_polarity(requirement: &DgclProgramRequirement) -> ObligationPolarity {
    if requirement.parse_state != DgclInstructionParseState::Parsed {
        return ObligationPolarity::Unknown;
    }
    let Some(selected) = requirement
        .selected_alternative_id
        .as_deref()
        .and_then(|selected| {
            requirement
                .alternatives
                .iter()
                .find(|alternative| alternative.alternative_id == selected)
        })
    else {
        return ObligationPolarity::Unknown;
    };
    let mut positive = false;
    let mut negative = false;
    let mut unknown = false;
    for action in &selected.ast.actions {
        match action.polarity {
            crate::InstructionPolarity::Positive => positive = true,
            crate::InstructionPolarity::Forbidden => negative = true,
            crate::InstructionPolarity::Conflict => return ObligationPolarity::Conflict,
            crate::InstructionPolarity::Unknown => unknown = true,
        }
    }
    if positive && negative {
        ObligationPolarity::Conflict
    } else if unknown || (!positive && !negative) {
        ObligationPolarity::Unknown
    } else if negative {
        ObligationPolarity::Negative
    } else {
        ObligationPolarity::Positive
    }
}

fn validate_ast_spans(ast: &InstructionAst, region: &str) -> Result<(), DgclProjectionError> {
    let check = |span: &Range<usize>| span.start <= span.end && region.get(span.clone()).is_some();
    if !check(&ast.source_span)
        || !check(&ast.marker_span)
        || !check(&ast.body_span)
        || ast.actions.iter().any(|action| {
            !check(&action.source_span)
                || action.marker_span.as_ref().is_some_and(|span| !check(span))
        })
        || ast
            .coordination
            .iter()
            .any(|edge| !check(&edge.source_span))
        || ast
            .polarity_scopes
            .iter()
            .any(|scope| !check(&scope.marker_span))
        || ast.conditions.iter().any(|condition| {
            !check(&condition.source_span)
                || !check(&condition.marker_span)
                || condition
                    .expression
                    .predicates()
                    .iter()
                    .any(|predicate| !check(&predicate.source_span))
        })
        || ast.scopes.iter().any(|scope| !check(&scope.source_span))
        || ast
            .references
            .iter()
            .any(|reference| !check(&reference.source_span))
        || ast
            .constraint_graph
            .nodes
            .iter()
            .any(|node| !check(&node.source_span))
        || ast.constraint_graph.edges.iter().any(|edge| {
            !check(&edge.source_span) || edge.target_span.as_ref().is_some_and(|span| !check(span))
        })
    {
        return Err(DgclProjectionError::SourceSpanInvalid);
    }
    Ok(())
}

fn map_parse_state(state: InstructionParseState) -> DgclInstructionParseState {
    match state {
        InstructionParseState::Parsed => DgclInstructionParseState::Parsed,
        InstructionParseState::Ambiguous => DgclInstructionParseState::Ambiguous,
        InstructionParseState::Unresolved => DgclInstructionParseState::Unresolved,
        InstructionParseState::Unsupported => DgclInstructionParseState::Unsupported,
    }
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn target_content_digest(revision: &str, content: &str) -> String {
    stable_sha256(&format!("{revision}\0{content}"))
}

fn translation_loss_event(
    requirement_id: String,
    stage: ProjectionStage,
    input_digest: Option<String>,
    output_digest: Option<String>,
    state: PreservationState,
    kind: TranslationLossKind,
    reason: &'static str,
) -> TranslationLossEvent {
    let stage_event_digest = stable_sha256(&format!(
        "epistesys-translation-loss-event.v1\0{requirement_id}\0{stage:?}\0{}\0{}\0{state:?}\0{kind:?}\0{reason}",
        input_digest.as_deref().unwrap_or("<absent>"),
        output_digest.as_deref().unwrap_or("<absent>")
    ));
    TranslationLossEvent {
        requirement_id,
        stage,
        input_digest,
        output_digest,
        stage_event_digest,
        state,
        kind,
        reason,
    }
}
