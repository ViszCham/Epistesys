#![forbid(unsafe_code)]

use lc631_analysis::{
    build_dgcl_pipeline, ConditionEvidenceState, DgclBackendSet, DgclInstructionParseState,
    InstructionParseState,
};

#[test]
fn controlled_grammar_runs_on_each_candidate_and_legacy_only_surface_stays_visible() {
    let source = "Please delete files.\n\nテストを実行し";
    let report = build_dgcl_pipeline(
        source,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    assert!(!report.authority_created);
    assert_eq!(report.instruction_parses.len(), 2);
    assert_eq!(
        report.instruction_parses[0].state,
        DgclInstructionParseState::Parsed
    );
    assert_eq!(
        report.instruction_parses[1].state,
        DgclInstructionParseState::Unsupported
    );
    assert_eq!(report.instruction_parses[0].legacy_requirement_ids, vec![1]);
    assert_eq!(report.instruction_parses[1].legacy_requirement_ids, vec![2]);
    for observation in &report.instruction_parses {
        assert_eq!(observation.root_source_revision, report.source_revision);
        let fragment = source
            .get(observation.source_span.clone())
            .expect("candidate span remains source-bound");
        if let Some(parse) = &observation.report {
            assert_eq!(parse.source_revision, lc631_core::stable_sha256(fragment));
        }
    }
}

#[test]
fn controlled_parser_visits_operative_prose_even_without_a_legacy_marker_candidate() {
    let report = build_dgcl_pipeline(
        "This is background context.",
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    assert!(report.dg1.requirement_candidates.is_empty());
    assert_eq!(report.instruction_parses.len(), 1);
    assert_eq!(
        report.instruction_parses[0].state,
        DgclInstructionParseState::Unsupported
    );
    assert!(report.instruction_parses[0]
        .legacy_requirement_ids
        .is_empty());
}

#[test]
fn unresolved_action_coordination_propagates_as_unresolved_not_parsed() {
    let report = build_dgcl_pipeline(
        "Please test and",
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    assert_eq!(report.instruction_parses.len(), 1);
    assert_eq!(
        report.instruction_parses[0].state,
        DgclInstructionParseState::Unresolved
    );
    assert_eq!(
        report.instruction_parses[0].report.as_ref().unwrap().state,
        InstructionParseState::Unresolved
    );
    assert!(report.instruction_parses[0]
        .report
        .as_ref()
        .unwrap()
        .selected
        .is_none());
}

#[test]
fn condition_truth_remains_unknown_after_pipeline_projection() {
    let report = build_dgcl_pipeline(
        "Please publish if tests pass.",
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    let parser_report = report.instruction_parses[0].report.as_ref().unwrap();
    assert_eq!(parser_report.state, InstructionParseState::Parsed);
    assert_eq!(
        parser_report.selected.as_ref().unwrap().conditions[0].evidence_state,
        ConditionEvidenceState::Unknown
    );
    assert!(!report.authority_created);
}
