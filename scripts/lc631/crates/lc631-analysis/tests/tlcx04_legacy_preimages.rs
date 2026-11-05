use lc631_analysis::{build_dgcl_pipeline, verify_dgcl_projection, DgclBackendSet};
use lc631_core::stable_sha256;

#[test]
fn tlcx04_existing_ast_consumer_hashes_the_legacy_struct_order_not_sorted_json() {
    for source in ["Please test the parser.", "公開しないでください。"] {
        let report = build_dgcl_pipeline(
            source,
            DgclBackendSet {
                english: None,
                japanese: None,
            },
        )
        .unwrap();
        assert!(!report.program_ir.requirements.is_empty());
        for requirement in &report.program_ir.requirements {
            assert!(!requirement.alternatives.is_empty());
            for alternative in &requirement.alternatives {
                let legacy_bytes = serde_json::to_string(&*alternative.ast).unwrap();
                assert_eq!(alternative.parser_ast_digest, stable_sha256(&legacy_bytes));
                let sorted_json =
                    serde_json::to_string(&serde_json::to_value(&*alternative.ast).unwrap())
                        .unwrap();
                assert_ne!(legacy_bytes, sorted_json);
                assert_ne!(alternative.parser_ast_digest, stable_sha256(&sorted_json));
            }
        }
        assert!(!report.program_ir.authority_created);
        assert!(!report.translation_projection.output_commit_allowed);
    }
}

#[test]
fn tlcx04_ast_rehash_under_frame_domain_does_not_pass_existing_consumer() {
    let source = "Please test the parser.";
    let mut report = build_dgcl_pipeline(
        source,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    let alternative = &mut report.program_ir.requirements[0].alternatives[0];
    let legacy_bytes = serde_json::to_string(&*alternative.ast).unwrap();
    alternative.parser_ast_digest = stable_sha256(&format!(
        "Epistesys/TLCX/FrameDigest/sha256/v1\0{legacy_bytes}"
    ));
    assert!(verify_dgcl_projection(
        source,
        &report.dg1,
        &report.language_regions,
        &report.instruction_parses,
        &report.program_ir,
        &report.translation_projection,
    )
    .is_err());
    assert!(!report.program_ir.authority_created);
    assert!(!report.translation_projection.output_commit_allowed);
}
