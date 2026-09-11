#![forbid(unsafe_code)]

use lc631_tldg::analyze_document;

#[test]
fn tldg_06_profiles_do_not_use_a_natural_program_binary_split() {
    let report = analyze_document("Explain fn main() { println!(\"hi\"); }").unwrap();
    assert!(!report.natural_program_binary_split);
    assert!(report.profile_count >= 2);
}

#[test]
fn tldg_07_region_lattice_retains_embedded_code_and_prose() {
    let report = analyze_document("説明:\n~~~rust\nfn main() {}\n~~~\n").unwrap();
    assert!(report.region_count >= 2);
    assert!(report.embedded_region_count >= 1);
}

#[test]
fn tldg_08_token_lattice_roundtrips_utf8_exactly() {
    let source = "日本語 + Rust::main();\n";
    let report = analyze_document(source).unwrap();
    assert_eq!(report.source_roundtrip, source);
    assert!(report.token_count > 4);
}

#[test]
fn tldg_09_10_unified_grammar_keeps_packed_derivations_and_errors() {
    let ambiguous = analyze_document("call(x) or call x").unwrap();
    assert!(ambiguous.derivation_count >= 2);

    let malformed = analyze_document("fn main( {").unwrap();
    assert!(malformed.error_node_count > 0);
}

#[test]
fn tldg_11_constraint_graph_is_materialized() {
    let report = analyze_document("if user says yes, return value").unwrap();
    assert!(report.relation_count >= report.token_count.saturating_sub(1));
    assert!(report.constraint_count >= report.token_count.saturating_sub(1));
}

#[test]
fn tldg_12_nested_dialects_do_not_drop_source_bytes() {
    let source = "sql!(\"SELECT * FROM t\"); // SQL query";
    let report = analyze_document(source).unwrap();
    assert_eq!(report.source_roundtrip, source);
    assert!(report.embedded_region_count >= 1);
}
