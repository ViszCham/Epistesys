#![forbid(unsafe_code)]

use lc631_analysis::{
    parse_controlled_instruction, InstructionConstraintKind, InstructionEdgeState,
    InstructionParseBudget, InstructionReferenceState, InstructionScopeKind, InstructionScopeState,
};

#[test]
fn environment_scopes_remain_action_local_and_do_not_transfer() {
    let report = parse_controlled_instruction(
        "Please deploy to staging and deploy to production.",
        InstructionParseBudget::default(),
    )
    .unwrap();
    let ast = report.selected.unwrap();
    let environments = ast
        .scopes
        .iter()
        .filter(|scope| scope.kind == InstructionScopeKind::Environment)
        .collect::<Vec<_>>();
    assert_eq!(environments.len(), 2);
    assert_eq!(environments[0].value, "staging");
    assert_eq!(environments[1].value, "production");
    assert_eq!(environments[0].action_indices, 0..1);
    assert_eq!(environments[1].action_indices, 1..2);
    assert!(environments
        .iter()
        .all(|scope| scope.state == InstructionScopeState::Candidate));
}

#[test]
fn pronoun_references_are_not_fabricated_into_a_unique_target() {
    let report =
        parse_controlled_instruction("Please update it.", InstructionParseBudget::default())
            .unwrap();
    let reference = &report.selected.unwrap().references[0];
    assert_eq!(reference.text, "it.");
    assert!(reference.candidate_action_indices.is_empty());
    assert_eq!(reference.state, InstructionReferenceState::Unresolved);

    let prior_action = parse_controlled_instruction(
        "Please test the file and update it.",
        InstructionParseBudget::default(),
    )
    .unwrap();
    let references = prior_action.selected.unwrap().references;
    assert!(references.iter().any(|reference| {
        reference.text == "it."
            && reference.candidate_action_indices == vec![0]
            && reference.state == InstructionReferenceState::Candidate
    }));
}

#[test]
fn resource_branch_and_time_scopes_keep_their_own_values_and_spans() {
    let report = parse_controlled_instruction(
        "Please update file README.md on branch feature/epia2.",
        InstructionParseBudget::default(),
    )
    .unwrap();
    let scopes = report.selected.unwrap().scopes;
    assert!(scopes.iter().any(|scope| {
        scope.kind == InstructionScopeKind::Resource && scope.value == "readme.md"
    }));
    assert!(scopes.iter().any(|scope| {
        scope.kind == InstructionScopeKind::Branch && scope.value == "feature/epia2"
    }));

    let timed = parse_controlled_instruction(
        "Please publish until tests pass.",
        InstructionParseBudget::default(),
    )
    .unwrap();
    assert!(timed
        .selected
        .unwrap()
        .scopes
        .iter()
        .any(|scope| scope.kind == InstructionScopeKind::Time));
}

#[test]
fn temporal_requires_and_conflict_edges_are_candidates_not_action_permits() {
    let temporal = parse_controlled_instruction(
        "Please test then publish.",
        InstructionParseBudget::default(),
    )
    .unwrap();
    let before = temporal
        .selected
        .unwrap()
        .constraint_graph
        .edges
        .into_iter()
        .find(|edge| edge.kind == InstructionConstraintKind::Before)
        .unwrap();
    assert_eq!(before.from_action_indices, 0..1);
    assert_eq!(before.to_action_indices, 1..2);
    assert_eq!(before.state, InstructionEdgeState::Candidate);

    let required = parse_controlled_instruction(
        "Please publish only if tests pass.",
        InstructionParseBudget::default(),
    )
    .unwrap();
    assert!(required
        .selected
        .unwrap()
        .constraint_graph
        .edges
        .iter()
        .any(|edge| edge.kind == InstructionConstraintKind::Requires
            && edge.state == InstructionEdgeState::Candidate
            && edge.target_span.is_some()));

    let conflict = parse_controlled_instruction(
        "Please publish but do not publish.",
        InstructionParseBudget::default(),
    )
    .unwrap();
    assert!(conflict
        .selected
        .unwrap()
        .constraint_graph
        .edges
        .iter()
        .any(|edge| edge.kind == InstructionConstraintKind::Conflicts
            && edge.state == InstructionEdgeState::Conflict));
}
