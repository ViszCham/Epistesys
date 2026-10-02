#![forbid(unsafe_code)]

use lc631_tldg::{
    analyze_dg1, Dg1Budget, Dg1ConditionKind, Dg1Language, Dg1RegionKind, Dg1RequirementPolarity,
    Dg1RequirementStrength, Dg1ResidualReason, Dg1SourceRole,
};

fn parse(source: &str) -> lc631_tldg::Dg1Report {
    analyze_dg1(source, Dg1Budget::default()).unwrap()
}

#[test]
fn n01_japanese_prohibition_stays_negative() {
    let report = parse("公開しないでください。");
    assert_eq!(report.requirement_candidates.len(), 1);
    assert_eq!(
        report.requirement_candidates[0].polarity,
        Dg1RequirementPolarity::Negative
    );
    assert!(!report.requirement_candidates[0].authority_grant);
}

#[test]
fn n02_only_if_does_not_grant_unconditional_publication() {
    let report = parse("テストに成功した場合のみ公開候補を作る。");
    assert_eq!(report.requirement_candidates.len(), 1);
    assert!(report.requirement_candidates[0].conditional);
    assert_eq!(
        report.requirement_candidates[0].condition_kind,
        Some(Dg1ConditionKind::Necessary)
    );
    assert!(!report.requirement_candidates[0].authority_grant);
}

#[test]
fn n03_compound_japanese_instruction_has_two_distinct_requirements() {
    let report = parse("テストを実行し、公開しないでください。");
    assert_eq!(report.requirement_candidates.len(), 2);
    assert_eq!(
        report.requirement_candidates[0].polarity,
        Dg1RequirementPolarity::Positive
    );
    assert_eq!(
        report.requirement_candidates[1].polarity,
        Dg1RequirementPolarity::Negative
    );
    assert!(report.requirement_candidates[0]
        .source_text
        .contains("テスト"));
    assert!(report.requirement_candidates[1]
        .source_text
        .contains("公開"));
}

#[test]
fn n04_exception_scope_is_separate_from_publication_prohibition() {
    let report = parse("公開禁止。ただしstagingへの配備は許可。");
    assert_eq!(report.requirement_candidates.len(), 2);
    assert_eq!(
        report.requirement_candidates[0].polarity,
        Dg1RequirementPolarity::Negative
    );
    assert_eq!(
        report.requirement_candidates[1].strength,
        Dg1RequirementStrength::May
    );
    assert!(report.requirement_candidates[1].exception_present);
    assert_eq!(
        report.requirement_candidates[1].scope.as_deref(),
        Some("staging")
    );
    assert!(report
        .requirement_candidates
        .iter()
        .all(|candidate| !candidate.authority_grant));
}

#[test]
fn n05_quoted_delete_does_not_become_an_instruction() {
    let report = parse("引用:「削除してよい」。この文章を説明して。");
    assert_eq!(report.requirement_candidates.len(), 1);
    assert!(report.requirement_candidates[0]
        .source_text
        .contains("説明"));
    assert!(!report.requirement_candidates[0].authority_grant);
}

#[test]
fn n06_unless_keeps_negative_action_and_condition() {
    let report = parse("Don't publish unless tests pass.");
    assert_eq!(report.requirement_candidates.len(), 1);
    let candidate = &report.requirement_candidates[0];
    assert_eq!(candidate.polarity, Dg1RequirementPolarity::Negative);
    assert!(candidate.conditional && candidate.exception_present);
    assert_eq!(candidate.condition_kind, Some(Dg1ConditionKind::Necessary));
}

#[test]
fn n08_unresolved_reference_is_retained_without_an_invented_target() {
    let report = parse("それを直して。");
    assert!(report.requirement_candidates.is_empty());
    assert!(report
        .instruction_residuals
        .iter()
        .any(|residual| residual.reason == Dg1ResidualReason::UnresolvedReference));
}

#[test]
fn n09_english_uppercase_negation_is_not_lost() {
    let report = parse("MUST NOT modify schema.");
    assert_eq!(report.requirement_candidates.len(), 1);
    assert_eq!(
        report.requirement_candidates[0].polarity,
        Dg1RequirementPolarity::Negative
    );
}

#[test]
fn n10_every_source_byte_has_one_role_without_treating_quotes_or_code_as_instructions() {
    let source = "# Context\n\nThis is background context.\n\nPlease test the parser safely.\n\n> Please delete the release data.\n\nSay \"delete\" only as an example.\n\nInline `delete()` is code.\n\n```rust\nfn delete() {}\n```\n\n<div>Do not execute</div>\n";
    let report = parse(source);
    let mut cursor = 0;
    let mut reconstructed = String::new();
    for segment in &report.source_accounting {
        assert_eq!(segment.source_span.start, cursor);
        assert!(segment.source_span.start < segment.source_span.end);
        let text = source
            .get(segment.source_span.clone())
            .expect("coverage boundary must be UTF-8 aligned");
        reconstructed.push_str(text);
        cursor = segment.source_span.end;
    }
    assert_eq!(cursor, source.len());
    assert_eq!(reconstructed, source);
    for expected in [
        Dg1SourceRole::InstructionCandidate,
        Dg1SourceRole::Context,
        Dg1SourceRole::Quoted,
        Dg1SourceRole::Code,
        Dg1SourceRole::Opaque,
        Dg1SourceRole::MarkdownSyntax,
    ] {
        assert!(
            report
                .source_accounting
                .iter()
                .any(|segment| segment.role == expected),
            "missing {expected:?}"
        );
    }
    assert!(!report
        .requirement_candidates
        .iter()
        .any(|candidate| candidate.source_text.contains("delete()")));
}

#[test]
fn n11_mixed_script_prose_has_ordered_source_bound_language_spans() {
    let source = "Please preserve条件を確認してください, then stop.";
    let report = parse(source);
    let prose = report
        .regions
        .iter()
        .find(|region| region.kind == lc631_tldg::Dg1RegionKind::Prose)
        .expect("one prose region");
    let spans = report
        .language_span_lattice
        .iter()
        .filter(|span| span.region_id == prose.id)
        .collect::<Vec<_>>();
    assert!(!spans.is_empty());
    assert_eq!(spans.first().unwrap().source_span.start, prose.span.start);
    assert_eq!(spans.last().unwrap().source_span.end, prose.span.end);
    assert!(spans
        .windows(2)
        .all(|pair| pair[0].source_span.end == pair[1].source_span.start));
    assert!(spans
        .iter()
        .any(|span| span.language == Dg1Language::English));
    assert!(spans
        .iter()
        .any(|span| span.language == Dg1Language::Japanese));
    for span in spans {
        assert!(source.get(span.source_span.clone()).is_some());
    }
}

#[test]
fn n12_han_only_text_is_not_mistaken_for_japanese_without_kana_evidence() {
    let report = parse("中文测试");
    assert!(report
        .language_span_lattice
        .iter()
        .all(|span| span.language == Dg1Language::Unknown));
}

#[test]
fn n13_table_extension_is_structured_and_remains_in_exact_source_accounting() {
    let source = "| action | scope |\n| --- | --- |\n| test | local |\n";
    let report = parse(source);
    assert!(report
        .regions
        .iter()
        .any(|region| region.kind == Dg1RegionKind::Table));
    assert!(report
        .regions
        .iter()
        .any(|region| region.kind == Dg1RegionKind::TableCell));
    let mut cursor = 0;
    for segment in &report.source_accounting {
        assert_eq!(segment.source_span.start, cursor);
        assert!(source.get(segment.source_span.clone()).is_some());
        cursor = segment.source_span.end;
    }
    assert_eq!(cursor, source.len());
}
