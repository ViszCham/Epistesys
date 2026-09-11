use crate::CorpusReceipt;

pub(crate) fn audited_corpora() -> Vec<CorpusReceipt> {
    vec![
        corpus(
            "assembly-primary-2026-08-25",
            "assembly_vibecoding_primary_source_corpus_2026-08-25.md",
            "sha256:19e779342c27af73ad00dff1fd60a81e9a4040bdad2199f7fccba3278d99a35b",
            884,
        ),
        corpus(
            "rust-primary-2026-08-25",
            "rust_vibecoding_primary_sources_and_analyzers_2026-08-25.md",
            "sha256:ab39077b147fcd60ab8855225c3a220e6597f75242ca15bed8a0d9c0c5ef4e4d",
            406,
        ),
        corpus(
            "python-primary-2026-08-25",
            "python_vibecoding_primary_sources_and_analyzers_2026-08-25.md",
            "sha256:e769840e1b5360f30ca791073843b14179a3442989b61e0dc16fe0f69b0889a0",
            560,
        ),
    ]
}

fn corpus(id: &str, file_name: &str, sha256: &str, explicit_records: usize) -> CorpusReceipt {
    CorpusReceipt {
        corpus_id: id.to_string(),
        file_name: file_name.to_string(),
        sha256: sha256.to_string(),
        explicit_records,
        authority_class: "user_supplied_audit_corpus".to_string(),
        count_is_coverage: false,
        content_packaged: false,
        claim_boundary: "revision identity and audited metadata only; corpus count is not analyzer coverage, normative authority, or proof"
            .to_string(),
    }
}
