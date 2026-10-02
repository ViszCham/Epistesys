use crate::{DeepGrammarArtifact, RelationState, SyntaxNodeId};
use lc631_core::{stable_sha256, SourceSpan};
use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AnchorState {
    Verified,
    Unknown,
    Incomparable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StructuralAnchor {
    pub node_id: SyntaxNodeId,
    pub source_spans: Vec<SourceSpan>,
    pub order: u32,
    pub relation_degree: u32,
    pub profile_count: u16,
    pub state: AnchorState,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StructuralKernel {
    pub source_revision: String,
    pub anchors: Vec<StructuralAnchor>,
    pub relation_count: usize,
    pub legacy_translation_loss_consumed: bool,
    pub structural_revision: String,
    pub materialized_relations: Vec<MaterializedRelationEvidence>,
    pub kernel_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MaterializedRelationEvidence {
    pub materialization_id: String,
    pub source: SyntaxNodeId,
    pub target: SyntaxNodeId,
    pub kind: crate::RelationKind,
    pub source_relation_evidence_digest: String,
    pub first_materialized_epoch: u16,
    pub claim_boundary: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StructuralKernelError {
    NoAnchoredNodes,
    CanonicalEncodingUnavailable,
}

pub fn build_structural_kernel(
    artifact: &DeepGrammarArtifact,
) -> Result<StructuralKernel, StructuralKernelError> {
    let anchors = artifact
        .syntax
        .nodes
        .iter()
        .filter(|node| !node.spans.is_empty())
        .enumerate()
        .map(|(order, node)| {
            let relation_degree = artifact
                .syntax
                .relations
                .iter()
                .filter(|edge| edge.from == node.id || edge.to == node.id)
                .count() as u32;
            let state = if artifact.syntax.relations.iter().any(|edge| {
                (edge.from == node.id || edge.to == node.id)
                    && edge.state == RelationState::Verified
            }) {
                AnchorState::Verified
            } else {
                AnchorState::Unknown
            };
            StructuralAnchor {
                node_id: node.id,
                source_spans: node.spans.clone(),
                order: order as u32,
                relation_degree,
                profile_count: node.profile_candidates.len().min(u16::MAX as usize) as u16,
                state,
            }
        })
        .collect::<Vec<_>>();
    if anchors.is_empty() {
        return Err(StructuralKernelError::NoAnchoredNodes);
    }
    let digest_surface = serde_json::to_string(&(
        &artifact.source.revision,
        &artifact.syntax.nodes,
        &artifact.syntax.relations,
        &artifact.syntax.derivations,
        &artifact.syntax.defects,
        &artifact.constraints,
        &anchors,
    ))
    .map_err(|_| StructuralKernelError::CanonicalEncodingUnavailable)?;
    let structural_revision = stable_sha256(&digest_surface);
    let materialized_relations = Vec::new();
    let kernel_digest = distillation_kernel_digest(&structural_revision, &materialized_relations)?;
    Ok(StructuralKernel {
        source_revision: artifact.source.revision.clone(),
        relation_count: artifact.syntax.relations.len(),
        legacy_translation_loss_consumed: false,
        structural_revision,
        materialized_relations,
        kernel_digest,
        anchors,
    })
}

pub(crate) fn distillation_kernel_digest(
    structural_revision: &str,
    materialized_relations: &[MaterializedRelationEvidence],
) -> Result<String, StructuralKernelError> {
    let canonical = serde_json::to_string(&(
        "epistesys-distillation-kernel.v1",
        structural_revision,
        materialized_relations,
    ))
    .map_err(|_| StructuralKernelError::CanonicalEncodingUnavailable)?;
    Ok(stable_sha256(&canonical))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyze;

    #[test]
    fn kernel_is_source_anchored_and_tl_independent() {
        let artifact = analyze("a + b").unwrap();
        let kernel = build_structural_kernel(&artifact).unwrap();
        assert!(!kernel.legacy_translation_loss_consumed);
        assert!(kernel
            .anchors
            .iter()
            .all(|anchor| !anchor.source_spans.is_empty()));
    }

    #[test]
    fn kernel_digest_binds_relation_kind_and_source_revision_not_just_node_counts() {
        let original = analyze("alpha + beta").unwrap();
        let mut changed = original.clone();
        assert!(!changed.syntax.relations.is_empty());
        changed.syntax.relations[0].kind = crate::RelationKind::DependencyCandidate;
        assert_eq!(original.syntax.nodes.len(), changed.syntax.nodes.len());
        assert_eq!(
            original.syntax.relations.len(),
            changed.syntax.relations.len()
        );
        let original_kernel = build_structural_kernel(&original).unwrap();
        let changed_kernel = build_structural_kernel(&changed).unwrap();
        assert_ne!(original_kernel.kernel_digest, changed_kernel.kernel_digest);
    }
}
