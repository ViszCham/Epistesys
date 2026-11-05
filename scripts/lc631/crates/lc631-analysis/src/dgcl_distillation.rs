//! Geometry proposes; the immutable parser's discrete edges validate membership.
//! Structural membership is not condition truth or a runtime completion receipt.
use crate::instruction_grammar::{legacy_instruction_ast_digest, legacy_json_digest};
use crate::{DgclProgramRequirement, DgclProjectionError, InstructionEdgeState};
use lc631_core::{stable_sha256, SourceSpan};
use lc631_substrate::source::{SourceSpan as CheckedSourceSpan, SourceStore};
use lc631_tldg::*;
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DgclConstraintWitness {
    pub requirement_id: String,
    pub ast_digest: String,
    pub constraint_edge_index: usize,
    pub materialization_id: String,
    pub semantic_truth_claim: bool,
    pub authority_created: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DgclStructuralDistillation {
    pub schema_version: &'static str,
    pub input_constraints_digest: String,
    pub run: Option<MutualDistillationRun>,
    pub constraint_witnesses: Vec<DgclConstraintWitness>,
    pub claim_boundary: &'static str,
}

pub fn distill_dgcl_constraints(
    source: &str,
    requirements: &[DgclProgramRequirement],
) -> Result<DgclStructuralDistillation, DgclProjectionError> {
    let source_store = if let Some(store) = requirements
        .iter()
        .flat_map(|requirement| requirement.alternatives.iter())
        .find_map(|alternative| alternative.ast.source_store())
    {
        if store.text() != source {
            return Err(DgclProjectionError::SourceSpanInvalid);
        }
        store.clone()
    } else {
        let max_source_bytes = Dg1Budget::default().max_source_bytes.max(1);
        if source.len() > max_source_bytes {
            return Err(DgclProjectionError::SourceSpanInvalid);
        }
        SourceStore::new(Arc::from(source), max_source_bytes)
            .map_err(|_| DgclProjectionError::SourceSpanInvalid)?
    };
    distill_dgcl_constraints_impl(&source_store, requirements, true)
}

pub(crate) fn distill_dgcl_constraints_store(
    source_store: &SourceStore,
    requirements: &[DgclProgramRequirement],
) -> Result<DgclStructuralDistillation, DgclProjectionError> {
    distill_dgcl_constraints_impl(source_store, requirements, false)
}

fn distill_dgcl_constraints_impl(
    source_store: &SourceStore,
    requirements: &[DgclProgramRequirement],
    allow_explicit_legacy_dto: bool,
) -> Result<DgclStructuralDistillation, DgclProjectionError> {
    let source = source_store.text();
    let mut detached_legacy_dto = false;
    for requirement in requirements {
        for alternative in &requirement.alternatives {
            let ast_id = alternative.ast.canonical_id();
            let Some(ast_store) = alternative.ast.source_store() else {
                if allow_explicit_legacy_dto && ast_id.is_none() {
                    detached_legacy_dto = true;
                    continue;
                }
                return Err(DgclProjectionError::SourceSpanInvalid);
            };
            if ast_store.id() != source_store.id() || ast_store.text() != source {
                return Err(DgclProjectionError::SourceSpanInvalid);
            }
            if let Some(ast_id) = ast_id {
                let expected_anchor = CheckedSourceSpan::checked(
                    source_store,
                    requirement.contract.source_span.start,
                    requirement.contract.source_span.end,
                )
                .map_err(|_| DgclProjectionError::SourceSpanInvalid)?;
                if alternative.ast.root_source_span() != Some(expected_anchor) {
                    return Err(DgclProjectionError::SourceSpanInvalid);
                }
                let Some(contract_report) = requirement.contract.report.as_ref() else {
                    return Err(DgclProjectionError::SourceSpanInvalid);
                };
                if !contract_report
                    .alternatives
                    .iter()
                    .any(|candidate| candidate.canonical_id() == Some(ast_id))
                {
                    return Err(DgclProjectionError::ProgramIrMismatch);
                }
            }
        }
    }
    let claim_boundary = if detached_legacy_dto {
        "legacy DTO structure rechecked against the supplied source spans; canonical AST provenance, semantic truth and authority are not asserted"
    } else {
        "same-artifact controlled AST structural membership, not semantic truth"
    };
    let boundary_roundtrip =
        SourceTextView::full(source_store).map_err(|_| DgclProjectionError::SourceSpanInvalid)?;
    let mut artifact = DeepGrammarArtifact {
        source: SourceRevision::from_store(source_store),
        boundary_roundtrip,
        regions: RegionLattice::default(),
        tokens: TokenLattice::default(),
        grammar: UnifiedGrammarIr::default(),
        syntax: UnifiedSyntaxHypergraph::default(),
        constraints: TypedConstraintGraph::default(),
        claim_boundary: claim_boundary.into(),
    };
    let mut bindings = BTreeMap::new();
    for requirement in requirements {
        let Some(selected) = &requirement.selected_alternative_id else {
            continue;
        };
        let alternative = requirement
            .alternatives
            .iter()
            .find(|alt| &alt.alternative_id == selected)
            .ok_or(DgclProjectionError::ProgramIrMismatch)?;
        let ast = &alternative.ast;
        let actual_ast_digest =
            legacy_instruction_ast_digest(ast).map_err(|_| DgclProjectionError::PayloadEncoding)?;
        if actual_ast_digest != alternative.parser_ast_digest {
            return Err(DgclProjectionError::ProgramIrMismatch);
        }
        let mut ids = Vec::new();
        for action in &ast.actions {
            if artifact.syntax.nodes.len() >= 4096 {
                return Err(DgclProjectionError::PayloadEncoding);
            }
            let start = alternative
                .region_offset
                .checked_add(action.source_span.start)
                .ok_or(DgclProjectionError::SourceSpanInvalid)?;
            let end = alternative
                .region_offset
                .checked_add(action.source_span.end)
                .ok_or(DgclProjectionError::SourceSpanInvalid)?;
            let checked_span = CheckedSourceSpan::checked(source_store, start, end)
                .map_err(|_| DgclProjectionError::SourceSpanInvalid)?;
            if source_store
                .slice(checked_span)
                .map_err(|_| DgclProjectionError::SourceSpanInvalid)?
                != action.text.as_str()
            {
                return Err(DgclProjectionError::SourceSpanInvalid);
            }
            let span = SourceSpan { start, end };
            let id = SyntaxNodeId(artifact.syntax.nodes.len() as u64 + 1);
            artifact.syntax.nodes.push(SyntaxNode {
                id,
                spans: vec![span],
                kind: SyntaxNodeKind::Group,
                profile_candidates: vec![GrammarProfileId(2)],
                origin: alternative.parser_ast_digest.clone(),
            });
            ids.push(id);
        }
        for (index, edge) in ast.constraint_graph.edges.iter().enumerate() {
            if edge.state != InstructionEdgeState::Candidate
                || edge.from_action_indices.len() != 1
                || edge.to_action_indices.len() != 1
            {
                continue;
            }
            let (Some(&from), Some(&to)) = (
                ids.get(edge.from_action_indices.start),
                ids.get(edge.to_action_indices.start),
            ) else {
                continue;
            };
            if from == to {
                continue;
            }
            let evidence = legacy_json_digest(&(
                requirement.requirement_id.as_str(),
                alternative.parser_ast_digest.as_str(),
                index,
                edge,
            ))
            .map_err(|_| DgclProjectionError::PayloadEncoding)?;
            let relation_evidence_digest = stable_sha256(&format!(
                "{}\0{:?}\0{}",
                evidence,
                RelationKind::DependencyCandidate,
                artifact.source.revision
            ));
            // Verified means the edge's existence in this checked AST. It does
            // not assert that an action ran, a predicate holds, or a Grant exists.
            artifact.syntax.relations.push(SyntaxRelation {
                from,
                to,
                kind: RelationKind::DependencyCandidate,
                state: RelationState::Verified,
                evidence,
            });
            bindings.insert(
                (from, to, relation_evidence_digest),
                (
                    requirement.requirement_id.clone(),
                    alternative.parser_ast_digest.clone(),
                    index,
                ),
            );
        }
    }
    let input_constraints_digest =
        legacy_json_digest(&artifact).map_err(|_| DgclProjectionError::PayloadEncoding)?;
    let run = if artifact.syntax.nodes.is_empty() {
        None
    } else {
        let kernel =
            build_structural_kernel(&artifact).map_err(|_| DgclProjectionError::PayloadEncoding)?;
        Some(
            run_mutual_distillation(
                kernel,
                &artifact,
                ParseBudget::reference(),
                &CpuGeometryBackend,
            )
            .map_err(|_| DgclProjectionError::PayloadEncoding)?,
        )
    };
    let mut constraint_witnesses = Vec::new();
    if let Some(run) = &run {
        for edge in &run.final_payload.kernel.materialized_relations {
            let (requirement_id, ast_digest, constraint_edge_index) = bindings
                .get(&(
                    edge.source,
                    edge.target,
                    edge.source_relation_evidence_digest.clone(),
                ))
                .ok_or(DgclProjectionError::ProgramIrMismatch)?;
            constraint_witnesses.push(DgclConstraintWitness {
                requirement_id: requirement_id.clone(),
                ast_digest: ast_digest.clone(),
                constraint_edge_index: *constraint_edge_index,
                materialization_id: edge.materialization_id.clone(),
                semantic_truth_claim: false,
                authority_created: false,
            });
        }
    }
    let output_claim_boundary = if detached_legacy_dto {
        "legacy DTO input was rechecked against source spans; structural suggestions only, with no canonical AST provenance, predicate truth, authority, or runtime evidence"
    } else {
        "discrete AST structure to bounded geometry to checked materializations to Program IR/TL/completion identity; no reparsing, predicate truth, authority, or runtime evidence"
    };
    Ok(DgclStructuralDistillation {
        schema_version: "epistesys-dgcl-structural-distillation.v1",
        input_constraints_digest,
        run,
        constraint_witnesses,
        claim_boundary: output_claim_boundary,
    })
}
