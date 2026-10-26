use lc631_tldg::{analyze, ConstraintState, SyntaxNodeKind};
use std::collections::BTreeMap;

#[test]
fn epr_13_constraint_endpoints_are_real_token_nodes_not_ordinal_aliases() {
    for source in [
        "",
        "one",
        "if user says yes, return value",
        "日本語と Rust::main() を確認します。",
        "> quoted instructions\n\n```rust\nfn main() { let x = 1; }\n```\n",
    ] {
        let artifact = analyze(source).unwrap();
        let nodes = artifact
            .syntax
            .nodes
            .iter()
            .map(|node| (node.id, node))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            artifact.constraints.edges.len(),
            artifact.tokens.tokens.len().saturating_sub(1)
        );
        for (pair, edge) in artifact
            .tokens
            .tokens
            .windows(2)
            .zip(&artifact.constraints.edges)
        {
            for (token, endpoint) in [(&pair[0], edge.from), (&pair[1], edge.to)] {
                let node = nodes
                    .get(&endpoint)
                    .expect("constraint endpoint must exist");
                assert_eq!(node.kind, SyntaxNodeKind::Token);
                assert_eq!(node.spans.as_slice(), &[token.span]);
                assert_eq!(
                    source.get(token.span.start..token.span.end),
                    Some(token.surface.as_str())
                );
            }
            assert_ne!(
                edge.state,
                ConstraintState::Verified,
                "lexical candidates are not semantic proof"
            );
        }
    }
}

#[test]
fn epr_13_binding_tampering_is_rejected_by_the_downstream_kernel() {
    use lc631_tldg::{build_structural_kernel, StructuralKernelError, SyntaxNodeId};
    let original = analyze("alpha + beta").unwrap();
    assert_eq!(
        original.constraints.token_bindings.len(),
        original.tokens.tokens.len()
    );
    assert!(build_structural_kernel(&original).is_ok());
    let mut variants = Vec::new();

    let mut missing = original.clone();
    missing.constraints.token_bindings.pop();
    variants.push(missing);
    let mut alias = original.clone();
    alias.constraints.token_bindings[0].syntax_node = SyntaxNodeId(0);
    variants.push(alias);
    let mut span = original.clone();
    span.constraints.token_bindings[0].source_span.end += 1;
    variants.push(span);
    let mut endpoint = original.clone();
    endpoint.constraints.edges[0].from = SyntaxNodeId(0);
    variants.push(endpoint);
    let mut duplicate = original.clone();
    duplicate.syntax.nodes[1].id = duplicate.syntax.nodes[0].id;
    variants.push(duplicate);
    let mut revision = original.clone();
    revision.source.revision = "sha256:unrelated-source".into();
    variants.push(revision);
    let mut promoted = original.clone();
    promoted.constraints.edges[0].state = ConstraintState::Verified;
    variants.push(promoted);
    let mut dangling = original.clone();
    dangling.syntax.relations[0].to = SyntaxNodeId(u64::MAX);
    variants.push(dangling);
    let mut erased_projection = original.clone();
    erased_projection.tokens.tokens.clear();
    erased_projection.constraints = Default::default();
    variants.push(erased_projection);

    for changed in variants {
        assert_eq!(
            build_structural_kernel(&changed),
            Err(StructuralKernelError::InvalidConstraintBinding)
        );
    }
}
