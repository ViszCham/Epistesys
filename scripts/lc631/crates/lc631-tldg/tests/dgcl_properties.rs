#![forbid(unsafe_code)]

use lc631_tldg::{analyze_dg1, Dg1Budget, Dg1Language, Dg1RequirementPolarity, Dg1ResidualReason};

#[test]
fn dgcl_26_generated_10k_bilingual_source_cases_preserve_spans_and_quote_role() {
    for index in 0..10_000_u32 {
        let source = format!("Implement requirement {index}.\n引用:『削除してください。』\n");
        let report = analyze_dg1(&source, Dg1Budget::default()).unwrap();
        assert!(report.exact_source_roundtrip);
        assert_eq!(report.requirement_candidates.len(), 1);
        let candidate = &report.requirement_candidates[0];
        assert_eq!(candidate.polarity, Dg1RequirementPolarity::Positive);
        assert_eq!(candidate.language, Dg1Language::English);
        assert_eq!(
            source.get(candidate.source_span.clone()),
            Some(candidate.source_text.as_str())
        );
        assert!(report
            .instruction_residuals
            .iter()
            .any(|residual| residual.reason == Dg1ResidualReason::QuotedText));
        assert!(report
            .requirement_candidates
            .iter()
            .all(|candidate| !candidate.authority_grant));
    }
}

#[test]
fn dgcl_26_generated_10k_rust_cst_cases_ignore_brackets_inside_raw_strings() {
    for index in 0..10_000_u32 {
        let source = format!("fn f_{index}() {{ let s = r#\"({{}})\"#; }}");
        let report = analyze_dg1(&source, Dg1Budget::default()).unwrap();
        assert_eq!(report.rust_trees.len(), 1);
        let tree = &report.rust_trees[0];
        assert!(!tree.root_has_error);
        assert!(tree
            .nodes
            .iter()
            .any(|node| { node.kind == "string_literal" || node.kind == "raw_string_literal" }));
        assert!(tree.nodes.iter().all(|node| {
            source.is_char_boundary(node.span.start) && source.is_char_boundary(node.span.end)
        }));
    }
}
