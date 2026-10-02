//! Geometry proposes; the immutable parser's discrete edges validate membership.
//! Structural membership is not condition truth or a runtime completion receipt.
use crate::{DgclProgramRequirement, DgclProjectionError, InstructionEdgeState};
use lc631_core::{stable_sha256, SourceSpan};
use lc631_tldg::*;
use serde::Serialize;
use std::collections::BTreeMap;

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
    let mut artifact = DeepGrammarArtifact {
        source: SourceRevision::from_source(source),
        boundary_roundtrip: source.into(),
        regions: RegionLattice::default(),
        tokens: TokenLattice::default(),
        grammar: UnifiedGrammarIr::default(),
        syntax: UnifiedSyntaxHypergraph::default(),
        constraints: TypedConstraintGraph::default(),
        claim_boundary: "same-artifact controlled AST structural membership, not semantic truth"
            .into(),
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
        let actual_ast_digest = stable_sha256(
            &serde_json::to_string(ast).map_err(|_| DgclProjectionError::PayloadEncoding)?,
        );
        if actual_ast_digest != alternative.parser_ast_digest {
            return Err(DgclProjectionError::ProgramIrMismatch);
        }
        let mut ids = Vec::new();
        for action in &ast.actions {
            if artifact.syntax.nodes.len() >= 4096 {
                return Err(DgclProjectionError::PayloadEncoding);
            }
            let span = SourceSpan {
                start: alternative.region_offset + action.source_span.start,
                end: alternative.region_offset + action.source_span.end,
            };
            if source.get(span.start..span.end) != Some(action.text.as_str()) {
                return Err(DgclProjectionError::SourceSpanInvalid);
            }
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
            let evidence = stable_sha256(
                &serde_json::to_string(&(
                    requirement.requirement_id.as_str(),
                    alternative.parser_ast_digest.as_str(),
                    index,
                    edge,
                ))
                .map_err(|_| DgclProjectionError::PayloadEncoding)?,
            );
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
    let input_constraints_digest = stable_sha256(
        &serde_json::to_string(&artifact).map_err(|_| DgclProjectionError::PayloadEncoding)?,
    );
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
    Ok(DgclStructuralDistillation { schema_version: "epistesys-dgcl-structural-distillation.v1",
        input_constraints_digest, run, constraint_witnesses,
        claim_boundary: "discrete AST structure to bounded geometry to checked materializations to Program IR/TL/completion identity; no reparsing, predicate truth, authority, or runtime evidence" })
}
