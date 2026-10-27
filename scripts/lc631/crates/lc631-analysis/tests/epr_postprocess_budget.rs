use lc631_analysis::{
    build_dgcl_pipeline, parse_controlled_instruction, ConditionEvidenceState, DgclBackendSet,
    DgclImplementationStatus, DgclInstructionParseState, InstructionGrammarError,
    InstructionParseBudget,
};

fn small_work_budget() -> InstructionParseBudget {
    InstructionParseBudget {
        max_chart_items: 32,
        ..InstructionParseBudget::default()
    }
}

#[test]
fn scope_word_scans_and_materialization_share_the_requested_budget() {
    let source = format!("対象 {}確認してください", "file x ".repeat(12));
    assert_eq!(
        parse_controlled_instruction(&source, small_work_budget()),
        Err(InstructionGrammarError::PostprocessBudgetExceeded)
    );
    let positive =
        parse_controlled_instruction(&source, InstructionParseBudget::default()).unwrap();
    assert_eq!(positive.selected.unwrap().scopes.len(), 12);
}

#[test]
fn condition_terms_cannot_bypass_the_budget_via_an_opaque_japanese_body() {
    let source = format!("もし{}xなら編集してください", "xかつ".repeat(20));
    assert_eq!(
        parse_controlled_instruction(&source, small_work_budget()),
        Err(InstructionGrammarError::PostprocessBudgetExceeded)
    );
    let positive =
        parse_controlled_instruction(&source, InstructionParseBudget::default()).unwrap();
    assert!(positive
        .selected
        .unwrap()
        .conditions
        .iter()
        .all(|condition| condition.evidence_state == ConditionEvidenceState::Unknown));
}

#[test]
fn action_graph_work_is_not_an_unmetered_second_pass() {
    let source = format!("{}編集しないでください", "編集、".repeat(12));
    assert_eq!(
        parse_controlled_instruction(&source, small_work_budget()),
        Err(InstructionGrammarError::PostprocessBudgetExceeded)
    );
    assert!(parse_controlled_instruction(&source, InstructionParseBudget::default()).is_ok());
}

#[test]
fn ordinary_three_clause_input_preserves_exact_action_spans() {
    let source = "編集し、検証し、保存してください";
    let report = parse_controlled_instruction(source, InstructionParseBudget::default()).unwrap();
    let selected = report.selected.unwrap();
    assert_eq!(selected.actions.len(), 3);
    for action in selected.actions {
        assert_eq!(source.get(action.source_span), Some(action.text.as_str()));
    }
}

#[test]
fn budget_rejection_stays_visible_through_program_ir_and_completion() {
    let source = format!("対象 {}確認してください", "file x ".repeat(8000));
    let pipeline = build_dgcl_pipeline(
        &source,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    assert!(pipeline
        .instruction_parses
        .iter()
        .any(|parse| parse.state == DgclInstructionParseState::BudgetRejected));
    assert!(!pipeline.completion_plan.blockers.is_empty());
    assert_eq!(
        pipeline.implementation_closure.status,
        DgclImplementationStatus::Clarify
    );
    assert!(!pipeline.authority_created);
}
