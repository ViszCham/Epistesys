use crate::build_dgcl_projection;
use crate::{
    parse_controlled_instruction, run_language_worker, DependencySentence, InstructionGrammarError,
    InstructionParseBudget, InstructionParseReport, InstructionParseState, LanguageWorkerConfig,
};
use lc631_core::stable_sha256;
use lc631_tldg::{
    analyze_dg1, Dg1Budget, Dg1ContentRole, Dg1Language, Dg1RegionKind, Dg1Report,
    Dg1RequirementPolarity, Dg1ResidualReason,
};
use serde::Serialize;
use std::collections::BTreeMap;
use std::ops::Range;

#[derive(Clone, Copy)]
pub struct DgclBackendSet<'a> {
    pub english: Option<&'a LanguageWorkerConfig>,
    pub japanese: Option<&'a LanguageWorkerConfig>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DgclBindingState {
    ObservedCandidate,
    NeedsEvidence,
    PolarityConflict,
}

#[derive(Clone, Debug, Serialize)]
pub struct DgclLanguageRegion {
    pub region_id: u32,
    pub language_span_id: u32,
    pub source_span: Range<usize>,
    pub root_source_revision: String,
    pub region_source_revision: String,
    pub language: Dg1Language,
    pub backend_revision: String,
    pub model_manifest_digest: String,
    pub runtime_manifest_digest: String,
    pub requirements_lock_digest: String,
    pub python_version: String,
    pub payload_binding_revision: &'static str,
    pub payload_digest: String,
    pub sentences: Vec<DependencySentence>,
    pub semantic_truth_claim: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct DgclRequirementBinding {
    pub requirement_id: u32,
    pub region_id: Option<u32>,
    pub dependency_payload_digest: Option<String>,
    pub dependency_language_span_ids: Vec<u32>,
    pub dependency_payload_digests: Vec<String>,
    pub polarity_cue_observed: bool,
    pub conditional_cue_observed: bool,
    pub state: DgclBindingState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DgclInstructionParseState {
    Parsed,
    Ambiguous,
    Unresolved,
    Unsupported,
    BudgetRejected,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DgclInstructionParse {
    pub region_id: u32,
    pub legacy_requirement_ids: Vec<u32>,
    pub root_source_revision: String,
    pub source_span: Range<usize>,
    pub state: DgclInstructionParseState,
    pub report: Option<InstructionParseReport>,
    pub error: Option<InstructionGrammarError>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DgclPipelineReport {
    pub schema_version: &'static str,
    pub source_revision: String,
    pub dg1: Dg1Report,
    pub language_regions: Vec<DgclLanguageRegion>,
    pub unsupported_operative_region_ids: Vec<u32>,
    pub requirement_bindings: Vec<DgclRequirementBinding>,
    pub instruction_parses: Vec<DgclInstructionParse>,
    pub program_ir: crate::DgclProgramIrReport,
    pub translation_projection: crate::DgclTranslationProjection,
    pub completion_plan: crate::DgclCompletionPlan,
    pub implementation_closure: crate::DgclImplementationClosureReport,
    pub authority_created: bool,
    pub complete_instruction_grammar_claim: bool,
    pub claim_boundary: &'static str,
    #[serde(skip)]
    immutable_core_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum DgclPipelineError {
    Parse(String),
    SourceSpan,
    BatchBudget,
    Backend(String),
    CrossRegionDependency,
    PayloadEncoding,
    Projection(crate::DgclProjectionError),
    CompletionPlan(crate::CompletionPlanError),
    ImplementationClosure(crate::DgclImplementationClosureError),
}

fn pipeline_core_digest(report: &DgclPipelineReport) -> String {
    stable_sha256(
        &serde_json::to_string(&(
            report.schema_version,
            &report.source_revision,
            &report.dg1,
            &report.language_regions,
            &report.unsupported_operative_region_ids,
            &report.requirement_bindings,
            &report.instruction_parses,
            &report.program_ir,
            &report.translation_projection,
            &report.completion_plan,
            report.authority_created,
            report.complete_instruction_grammar_claim,
        ))
        .expect("pipeline core serializes"),
    )
}

/// Local construction seal, not a transferable signature. JSON/report inputs
/// cannot recreate it; mutation of a cloned public core is detected without a
/// second seed parse or another language-worker call.
pub fn verify_dgcl_pipeline_identity(source: &str, report: &DgclPipelineReport) -> bool {
    report.source_revision == stable_sha256(source)
        && report.immutable_core_digest == pipeline_core_digest(report)
        && !report.authority_created
        && !report.complete_instruction_grammar_claim
}

#[derive(Clone, Debug)]
struct BatchRegion {
    region_id: u32,
    language_span_id: u32,
    synthetic_span: Range<usize>,
    source_span: Range<usize>,
    language: Dg1Language,
}

pub fn build_dgcl_pipeline(
    source: &str,
    backends: DgclBackendSet<'_>,
) -> Result<DgclPipelineReport, DgclPipelineError> {
    let dg1 = analyze_dg1(source, Dg1Budget::default())
        .map_err(|error| DgclPipelineError::Parse(format!("{error:?}")))?;
    let instruction_parses = dg1
        .regions
        .iter()
        .filter(|region| {
            region.kind == Dg1RegionKind::Prose
                && region.content_role == Dg1ContentRole::OperativeProse
        })
        .map(|region| {
            let fragment = source
                .get(region.span.clone())
                .ok_or(DgclPipelineError::SourceSpan)?;
            let legacy_requirement_ids = dg1
                .requirement_candidates
                .iter()
                .filter(|requirement| {
                    region.span.start <= requirement.source_span.start
                        && requirement.source_span.end <= region.span.end
                })
                .map(|requirement| requirement.id)
                .collect();
            let (state, report, error) =
                match parse_controlled_instruction(fragment, InstructionParseBudget::default()) {
                    Ok(report) => {
                        let state = match report.state {
                            InstructionParseState::Parsed => DgclInstructionParseState::Parsed,
                            InstructionParseState::Ambiguous => {
                                DgclInstructionParseState::Ambiguous
                            }
                            InstructionParseState::Unresolved => {
                                DgclInstructionParseState::Unresolved
                            }
                            InstructionParseState::Unsupported => {
                                DgclInstructionParseState::Unsupported
                            }
                        };
                        (state, Some(report), None)
                    }
                    Err(error) => (DgclInstructionParseState::BudgetRejected, None, Some(error)),
                };
            Ok(DgclInstructionParse {
                region_id: region.id,
                legacy_requirement_ids,
                root_source_revision: dg1.source_revision.clone(),
                source_span: region.span.clone(),
                state,
                report,
                error,
            })
        })
        .collect::<Result<Vec<_>, DgclPipelineError>>()?;
    let mut language_regions = Vec::new();
    let mut unsupported_operative_region_ids = Vec::new();
    for (language, config) in [
        (Dg1Language::English, backends.english),
        (Dg1Language::Japanese, backends.japanese),
    ] {
        let mut synthetic = String::new();
        let mut maps = Vec::new();
        for language_span in &dg1.language_span_lattice {
            let Some(region) = dg1
                .regions
                .iter()
                .find(|region| region.id == language_span.region_id)
            else {
                return Err(DgclPipelineError::SourceSpan);
            };
            if region.kind != Dg1RegionKind::Prose
                || region.content_role != Dg1ContentRole::OperativeProse
                || language_span.language != language
            {
                continue;
            }
            let text = source
                .get(language_span.source_span.clone())
                .ok_or(DgclPipelineError::SourceSpan)?;
            let start = synthetic.len();
            synthetic.push_str(text);
            let end = synthetic.len();
            synthetic.push_str("\n\n");
            maps.push(BatchRegion {
                region_id: region.id,
                language_span_id: language_span.id,
                synthetic_span: start..end,
                source_span: language_span.source_span.clone(),
                language,
            });
        }
        if maps.is_empty() {
            continue;
        }
        if synthetic.len() > 262_144 {
            return Err(DgclPipelineError::BatchBudget);
        }
        let Some(config) = config else {
            unsupported_operative_region_ids.extend(maps.iter().map(|map| map.region_id));
            continue;
        };
        let language_code = if language == Dg1Language::English {
            "en"
        } else {
            "ja"
        };
        let observed = run_language_worker(&synthetic, language_code, config)
            .map_err(|error| DgclPipelineError::Backend(format!("{error:?}")))?;
        let mut by_language_span = BTreeMap::<u32, Vec<DependencySentence>>::new();
        for sentence in observed.sentences {
            if sentence.tokens.is_empty() {
                continue;
            }
            let mut owner = None;
            let mut mapped_tokens = Vec::new();
            for mut token in sentence.tokens {
                let map = maps
                    .iter()
                    .find(|map| {
                        token.start_byte >= map.synthetic_span.start
                            && token.end_byte <= map.synthetic_span.end
                    })
                    .ok_or(DgclPipelineError::CrossRegionDependency)?;
                if owner.is_some_and(|id| id != map.language_span_id) {
                    return Err(DgclPipelineError::CrossRegionDependency);
                }
                owner = Some(map.language_span_id);
                let relative_start = token.start_byte - map.synthetic_span.start;
                let relative_end = token.end_byte - map.synthetic_span.start;
                token.start_byte = map.source_span.start + relative_start;
                token.end_byte = map.source_span.start + relative_end;
                if source.get(token.start_byte..token.end_byte) != Some(token.text.as_str()) {
                    return Err(DgclPipelineError::SourceSpan);
                }
                mapped_tokens.push(token);
            }
            if let Some(id) = owner {
                by_language_span
                    .entry(id)
                    .or_default()
                    .push(DependencySentence {
                        tokens: mapped_tokens,
                    });
            }
        }
        for map in maps {
            let sentences = by_language_span
                .remove(&map.language_span_id)
                .unwrap_or_default();
            let region_text = source
                .get(map.source_span.clone())
                .ok_or(DgclPipelineError::SourceSpan)?;
            let canonical = serde_json::to_string(&(
                "epistesys-dgcl-language-region-payload.v2",
                &dg1.source_revision,
                map.region_id,
                map.language_span_id,
                map.source_span.clone(),
                map.language,
                stable_sha256(region_text),
                &observed.backend_revision,
                &observed.model_manifest_digest,
                &observed.runtime_manifest_digest,
                &observed.requirements_lock_digest,
                &observed.python_version,
                &sentences,
            ))
            .map_err(|_| DgclPipelineError::PayloadEncoding)?;
            language_regions.push(DgclLanguageRegion {
                region_id: map.region_id,
                language_span_id: map.language_span_id,
                source_span: map.source_span,
                root_source_revision: dg1.source_revision.clone(),
                region_source_revision: stable_sha256(region_text),
                language: map.language,
                backend_revision: observed.backend_revision.clone(),
                model_manifest_digest: observed.model_manifest_digest.clone(),
                runtime_manifest_digest: observed.runtime_manifest_digest.clone(),
                requirements_lock_digest: observed.requirements_lock_digest.clone(),
                python_version: observed.python_version.clone(),
                payload_binding_revision: "epistesys-dgcl-language-region-payload.v2",
                payload_digest: stable_sha256(&canonical),
                sentences,
                semantic_truth_claim: false,
            });
        }
    }
    for requirement in &dg1.requirement_candidates {
        let observed_span_views = language_regions
            .iter()
            .map(|view| view.source_span.clone())
            .collect::<Vec<_>>();
        if !source_range_is_covered(&observed_span_views, &requirement.source_span) {
            if let Some(region) = dg1.regions.iter().find(|region| {
                region.kind == Dg1RegionKind::Prose
                    && region.content_role == Dg1ContentRole::OperativeProse
                    && region.span.start <= requirement.source_span.start
                    && requirement.source_span.end <= region.span.end
            }) {
                unsupported_operative_region_ids.push(region.id);
            }
        }
    }
    for residual in dg1
        .instruction_residuals
        .iter()
        .filter(|residual| residual.reason == Dg1ResidualReason::MixedOrUnknownLanguage)
    {
        if let Some(region) = dg1.regions.iter().find(|region| {
            region.kind == Dg1RegionKind::Prose
                && region.content_role == Dg1ContentRole::OperativeProse
                && region.span.start <= residual.source_span.start
                && residual.source_span.end <= region.span.end
        }) {
            unsupported_operative_region_ids.push(region.id);
        }
    }
    unsupported_operative_region_ids.sort_unstable();
    unsupported_operative_region_ids.dedup();
    language_regions.sort_by_key(|region| (region.region_id, region.language_span_id));
    let requirement_bindings = dg1
        .requirement_candidates
        .iter()
        .map(|requirement| {
            let view = language_regions.iter().find(|view| {
                requirement.source_span.start >= view.source_span.start
                    && requirement.source_span.end <= view.source_span.end
            });
            let observed_views = language_regions
                .iter()
                .filter(|view| {
                    view.source_span.start < requirement.source_span.end
                        && requirement.source_span.start < view.source_span.end
                })
                .collect::<Vec<_>>();
            let mut polarity_cue_observed = false;
            let mut conditional_cue_observed = false;
            if let Some(view) = view {
                for word in view
                    .sentences
                    .iter()
                    .flat_map(|sentence| &sentence.tokens)
                    .filter(|token| {
                        token.start_byte >= requirement.source_span.start
                            && token.end_byte <= requirement.source_span.end
                    })
                    .flat_map(|token| &token.words)
                {
                    let lower = word.text.to_lowercase();
                    let lemma = word.lemma.as_deref().unwrap_or("").to_lowercase();
                    polarity_cue_observed |=
                        matches!(lower.as_str(), "not" | "never" | "ない" | "ぬ" | "禁止")
                            || matches!(lemma.as_str(), "not" | "never" | "ない" | "ぬ");
                    conditional_cue_observed |=
                        matches!(
                            lower.as_str(),
                            "if" | "unless" | "until" | "when" | "場合" | "まで"
                        ) || matches!(word.deprel.as_str(), "advcl" | "mark");
                }
            }
            let state = match view {
                None => DgclBindingState::NeedsEvidence,
                Some(_) if requirement.unresolved_reference => DgclBindingState::NeedsEvidence,
                Some(_)
                    if requirement.polarity == Dg1RequirementPolarity::Negative
                        && !polarity_cue_observed =>
                {
                    DgclBindingState::PolarityConflict
                }
                Some(_)
                    if requirement.polarity == Dg1RequirementPolarity::Positive
                        && polarity_cue_observed =>
                {
                    DgclBindingState::PolarityConflict
                }
                Some(_) if requirement.conditional && !conditional_cue_observed => {
                    DgclBindingState::NeedsEvidence
                }
                Some(_) => DgclBindingState::ObservedCandidate,
            };
            DgclRequirementBinding {
                requirement_id: requirement.id,
                region_id: view.map(|view| view.region_id),
                dependency_payload_digest: view.map(|view| view.payload_digest.clone()),
                dependency_language_span_ids: observed_views
                    .iter()
                    .map(|view| view.language_span_id)
                    .collect(),
                dependency_payload_digests: observed_views
                    .iter()
                    .map(|view| view.payload_digest.clone())
                    .collect(),
                polarity_cue_observed,
                conditional_cue_observed,
                state,
            }
        })
        .collect();
    let (program_ir, translation_projection) =
        build_dgcl_projection(source, &dg1, &language_regions, &instruction_parses)
            .map_err(DgclPipelineError::Projection)?;
    let completion_plan =
        crate::build_dgcl_completion_plan(source, &program_ir, &translation_projection)
            .map_err(DgclPipelineError::CompletionPlan)?;
    let implementation_closure = crate::build_dgcl_implementation_closure(
        source,
        &program_ir,
        &translation_projection,
        &completion_plan,
        &[],
        crate::DgclImplementationClosureContext {
            current_lifecycle: None,
            verifier: None,
            replay: &mut lc631_receipt_kernel::ReplayGuard::default(),
            now_epoch: 0,
        },
    )
    .map_err(DgclPipelineError::ImplementationClosure)?;
    let mut report = DgclPipelineReport {
        schema_version: "epistesys-dgcl-pipeline.v1",
        source_revision: dg1.source_revision.clone(),
        dg1,
        language_regions,
        unsupported_operative_region_ids,
        requirement_bindings,
        instruction_parses,
        program_ir,
        translation_projection,
        completion_plan,
        implementation_closure,
        authority_created: false,
        complete_instruction_grammar_claim: false,
        claim_boundary: "real JA/EN dependency observations are source mapped candidate evidence, not validated instruction semantics or authority",
        immutable_core_digest: String::new(),
    };
    report.immutable_core_digest = pipeline_core_digest(&report);
    Ok(report)
}

fn source_range_is_covered(views: &[Range<usize>], target: &Range<usize>) -> bool {
    if target.start >= target.end {
        return false;
    }
    let mut spans = views
        .iter()
        .filter(|span| span.start < target.end && target.start < span.end)
        .cloned()
        .collect::<Vec<_>>();
    spans.sort_by_key(|span| (span.start, span.end));
    let mut cursor = target.start;
    for span in spans {
        if span.start > cursor {
            return false;
        }
        cursor = cursor.max(span.end);
        if cursor >= target.end {
            return true;
        }
    }
    false
}
