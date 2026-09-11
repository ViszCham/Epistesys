use crate::{BackendFamily, BackendRegistry, BackendState, DeepGrammarArtifact, SyntaxNodeKind};
use serde::Serialize;

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
    pub canonical_owner: bool,
    pub evidence: String,
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
    }
}
