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
    pub kernel_digest: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StructuralKernelError {
    NoAnchoredNodes,
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
    let digest_surface = anchors
        .iter()
        .map(|anchor| {
            format!(
                "{}:{}:{}:{}:{:?}",
                anchor.node_id.0,
                anchor.order,
                anchor.relation_degree,
                anchor.profile_count,
                anchor.source_spans
            )
        })
        .collect::<Vec<_>>()
        .join("|");
    Ok(StructuralKernel {
        source_revision: artifact.source.revision.clone(),
        relation_count: artifact.syntax.relations.len(),
        legacy_translation_loss_consumed: false,
        kernel_digest: stable_sha256(&digest_surface),
        anchors,
    })
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
}
