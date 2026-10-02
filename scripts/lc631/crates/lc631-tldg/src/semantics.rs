use crate::{BackendFamily, BackendRegistry, BackendState, DeepGrammarArtifact, SyntaxNodeKind};
use lc631_core::stable_sha256;
use serde::Serialize;
use std::fmt::Write as _;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticViewKind {
    SurfaceDiscourse,
    SurfaceExecutable,
    Mrs,
    EnhancedUd,
    Ucca,
    Amr,
    PropBank,
    TimeMl,
    CompilerAst,
    CompilerHir,
    ControlFlow,
    TypeEffect,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SemanticView {
    pub kind: SemanticViewKind,
    pub backend: BackendFamily,
    pub state: BackendState,
    pub source_revision: String,
    pub anchored_node_count: usize,
    pub payload: Option<SemanticViewPayload>,
    pub canonical_owner: bool,
    pub evidence: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SemanticViewPayload {
    pub schema_version: &'static str,
    pub projection_kind: &'static str,
    pub payload_digest: String,
    pub nodes: Vec<SemanticViewNode>,
    pub edges: Vec<SemanticViewEdge>,
    pub semantic_claim: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SemanticViewNode {
    pub source_node_id: u64,
    pub kind: SyntaxNodeKind,
    pub spans: Vec<lc631_core::SourceSpan>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SemanticViewEdge {
    pub from: u64,
    pub to: u64,
    pub kind: crate::RelationKind,
    pub state: crate::RelationState,
}

pub fn build_semantic_views(
    artifact: &DeepGrammarArtifact,
    registry: &BackendRegistry,
) -> Vec<SemanticView> {
    let anchored_node_count = artifact
        .syntax
        .nodes
        .iter()
        .filter(|node| node.kind != SyntaxNodeKind::Implicit && !node.spans.is_empty())
        .count();
    [
        (
            SemanticViewKind::SurfaceDiscourse,
            BackendFamily::BuiltinOpenDiscourse,
        ),
        (
            SemanticViewKind::SurfaceExecutable,
            BackendFamily::BuiltinExecutableSymbolic,
        ),
        (SemanticViewKind::Mrs, BackendFamily::EnglishHpsg),
        (SemanticViewKind::EnhancedUd, BackendFamily::EnhancedUd),
        (SemanticViewKind::Ucca, BackendFamily::Ucca),
        (SemanticViewKind::Amr, BackendFamily::Amr),
        (SemanticViewKind::PropBank, BackendFamily::PropBank),
        (SemanticViewKind::TimeMl, BackendFamily::TimeMl),
        (SemanticViewKind::CompilerAst, BackendFamily::TreeSitter),
        (SemanticViewKind::CompilerHir, BackendFamily::RustSyntaxHir),
        (SemanticViewKind::ControlFlow, BackendFamily::CppCompiler),
        (
            SemanticViewKind::TypeEffect,
            BackendFamily::TypeScriptCompiler,
        ),
    ]
    .into_iter()
    .map(|(kind, backend)| {
        let state = registry
            .get(backend)
            .map_or(BackendState::Unavailable, |descriptor| descriptor.state);
        SemanticView {
            kind,
            backend,
            state,
            source_revision: artifact.source.revision.clone(),
            anchored_node_count,
            payload: if matches!(
                kind,
                SemanticViewKind::SurfaceDiscourse | SemanticViewKind::SurfaceExecutable
            ) {
                Some(surface_payload(artifact))
            } else {
                None
            },
            canonical_owner: false,
            evidence: if state == BackendState::Unavailable {
                "backend_unavailable".into()
            } else {
                "backend_receipt_required_before_semantic_use".into()
            },
        }
    })
    .collect()
}

fn surface_payload(artifact: &DeepGrammarArtifact) -> SemanticViewPayload {
    let nodes = artifact
        .syntax
        .nodes
        .iter()
        .map(|node| SemanticViewNode {
            source_node_id: node.id.0,
            kind: node.kind,
            spans: node.spans.clone(),
        })
        .collect::<Vec<_>>();
    let edges = artifact
        .syntax
        .relations
        .iter()
        .map(|edge| SemanticViewEdge {
            from: edge.from.0,
            to: edge.to.0,
            kind: edge.kind,
            state: edge.state,
        })
        .collect::<Vec<_>>();
    let mut canonical = String::new();
    canonical.push_str("surface-structure-v1\0");
    canonical.push_str(&nodes.len().to_string());
    canonical.push('\0');
    for node in &nodes {
        let _ = write!(
            canonical,
            "{}:{}:{}",
            node.source_node_id,
            node_kind_tag(node.kind),
            node.spans.len()
        );
        for span in &node.spans {
            let _ = write!(canonical, ":{}:{}", span.start, span.end);
        }
        canonical.push('\0');
    }
    canonical.push_str(&edges.len().to_string());
    canonical.push('\0');
    for edge in &edges {
        let _ = write!(
            canonical,
            "{}:{}:{}:{}\0",
            edge.from,
            edge.to,
            relation_kind_tag(edge.kind),
            relation_state_tag(edge.state)
        );
    }
    SemanticViewPayload {
        schema_version: "epistesys-surface-structure.v1",
        projection_kind: "source_anchored_structural_view",
        payload_digest: stable_sha256(&canonical),
        nodes,
        edges,
        semantic_claim: false,
    }
}

fn node_kind_tag(kind: SyntaxNodeKind) -> &'static str {
    match kind {
        SyntaxNodeKind::Document => "document",
        SyntaxNodeKind::Token => "token",
        SyntaxNodeKind::Group => "group",
        SyntaxNodeKind::Error => "error",
        SyntaxNodeKind::Implicit => "implicit",
    }
}

fn relation_kind_tag(kind: crate::RelationKind) -> &'static str {
    match kind {
        crate::RelationKind::Contains => "contains",
        crate::RelationKind::Sequence => "sequence",
        crate::RelationKind::Scope => "scope",
        crate::RelationKind::BindingCandidate => "binding_candidate",
        crate::RelationKind::DependencyCandidate => "dependency_candidate",
        crate::RelationKind::EmbeddedIn => "embedded_in",
    }
}

fn relation_state_tag(state: crate::RelationState) -> &'static str {
    match state {
        crate::RelationState::Verified => "verified",
        crate::RelationState::Candidate => "candidate",
        crate::RelationState::Unknown => "unknown",
        crate::RelationState::Incomparable => "incomparable",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{analyze, default_backend_registry};

    #[test]
    fn views_do_not_claim_canonical_ownership() {
        let artifact = analyze("fn main() {}").unwrap();
        let views = build_semantic_views(&artifact, &default_backend_registry());
        assert_eq!(views.len(), 12);
        assert!(views.iter().all(|view| !view.canonical_owner));
        assert!(views
            .iter()
            .filter(|view| matches!(
                view.kind,
                SemanticViewKind::SurfaceDiscourse | SemanticViewKind::SurfaceExecutable
            ))
            .all(|view| view
                .payload
                .as_ref()
                .is_some_and(|payload| !payload.semantic_claim)));
        assert!(views
            .iter()
            .filter(|view| matches!(
                view.kind,
                SemanticViewKind::Mrs | SemanticViewKind::Ucca | SemanticViewKind::Amr
            ))
            .all(|view| view.payload.is_none()));
    }
}
