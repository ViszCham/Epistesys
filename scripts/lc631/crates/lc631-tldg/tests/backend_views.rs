#![forbid(unsafe_code)]

use lc631_tldg::{
    analyze, build_semantic_views, default_backend_registry, BackendFamily, BackendState,
    SemanticViewKind,
};

#[test]
fn tldg_13_natural_language_backends_are_revision_bound_and_fail_visible() {
    let registry = default_backend_registry();
    let erg = registry.get(BackendFamily::EnglishHpsg).unwrap();
    assert_eq!(erg.state, BackendState::Unavailable);
    assert!(!erg.revision.is_empty());
    assert!(!erg.can_authorize());
}

#[test]
fn tldg_14_program_backends_decode_to_the_same_shared_schema() {
    let registry = default_backend_registry();
    let rust = registry.get(BackendFamily::RustSyntaxHir).unwrap();
    let japanese = registry.get(BackendFamily::JapaneseHpsg).unwrap();
    assert_eq!(rust.target_schema, japanese.target_schema);
    assert_eq!(rust.target_schema, "lc631-unified-syntax-hypergraph.v1");
}

#[test]
fn tldg_15_semantic_views_remain_independent_and_noncanonical() {
    let artifact = analyze("if x { return y }").unwrap();
    let registry = default_backend_registry();
    let views = build_semantic_views(&artifact, &registry);
    assert!(views.iter().any(|view| view.kind == SemanticViewKind::Mrs));
    assert!(views
        .iter()
        .any(|view| view.kind == SemanticViewKind::CompilerHir));
    assert!(views.iter().all(|view| !view.canonical_owner));
    assert!(views
        .iter()
        .any(|view| view.state == BackendState::Unavailable));
}
