#![forbid(unsafe_code)]

use lc631_analysis::{
    parse_controlled_instruction, ConditionEvidenceState, ConditionExpression, ConditionRelation,
    ConditionScopeState, InstructionConjunction, InstructionCoordinationState,
    InstructionGrammarError, InstructionMarker, InstructionModality, InstructionParseBudget,
    InstructionParseState, InstructionPolarity, InstructionPolarityScopeState, TemporalRelation,
};

#[test]
fn english_and_japanese_controlled_directives_produce_source_bound_asts() {
    let english =
        parse_controlled_instruction("Please test the parser.", InstructionParseBudget::default())
            .unwrap();
    assert_eq!(english.state, InstructionParseState::Parsed);
    let english_ast = english.selected.as_ref().unwrap();
    assert_eq!(english_ast.marker, InstructionMarker::Please);
    assert_eq!(english_ast.body, "test the parser.");
    assert_eq!(
        "Please test the parser.".get(english_ast.body_span.start..english_ast.body_span.end),
        Some(english_ast.body.as_str())
    );
    assert!(!english.forest.nodes.is_empty());
    assert_eq!(english.forest.roots.len(), 1);
    assert!(english
        .forest
        .nodes
        .iter()
        .all(|node| { node.children.iter().all(|child_id| *child_id < node.id) }));

    let japanese =
        parse_controlled_instruction("公開しないでください。", InstructionParseBudget::default())
            .unwrap();
    assert_eq!(japanese.state, InstructionParseState::Parsed);
    assert_eq!(
        japanese.selected.as_ref().unwrap().marker,
        InstructionMarker::JapaneseForbidden
    );
}

#[test]
fn mixed_surface_with_two_valid_routes_preserves_both_and_selects_neither() {
    let report =
        parse_controlled_instruction("Please testしてください", InstructionParseBudget::default())
            .unwrap();
    assert_eq!(report.state, InstructionParseState::Ambiguous);
    assert!(report.selected.is_none());
    assert_eq!(report.alternatives.len(), 2);
    assert_eq!(report.forest.roots.len(), 2);
    for root in &report.forest.roots {
        assert_eq!(report.forest.nodes[*root as usize].symbol, "Start");
    }
}

#[test]
fn declared_modality_and_request_markers_retain_their_exact_surface_class() {
    for (source, marker) in [
        ("Must test the parser.", InstructionMarker::Must),
        ("MUST NOT modify schema.", InstructionMarker::MustNot),
        ("Should not publish.", InstructionMarker::ShouldNot),
        ("Should run local tests.", InstructionMarker::Should),
        ("May run local tests.", InstructionMarker::May),
        ("Do not publish.", InstructionMarker::DoNot),
        ("設定が必須", InstructionMarker::JapaneseMust),
        ("公開禁止", InstructionMarker::JapaneseForbidden),
        ("公開許可", InstructionMarker::JapanesePermission),
    ] {
        let report =
            parse_controlled_instruction(source, InstructionParseBudget::default()).unwrap();
        assert_eq!(report.state, InstructionParseState::Parsed, "{source}");
        assert_eq!(report.selected.unwrap().marker, marker, "{source}");
    }
}

#[test]
fn conjunctions_keep_action_specific_modality_and_forbidden_polarity() {
    let source = "Please test the package and do not publish.";
    let report = parse_controlled_instruction(source, InstructionParseBudget::default()).unwrap();
    let ast = report.selected.unwrap();
    assert_eq!(ast.actions.len(), 2);
    assert_eq!(ast.actions[0].text, "test the package");
    assert_eq!(ast.actions[0].modality, InstructionModality::Requested);
    assert_eq!(ast.actions[0].polarity, InstructionPolarity::Positive);
    assert_eq!(ast.actions[1].text, "publish.");
    assert_eq!(ast.actions[1].modality, InstructionModality::Required);
    assert_eq!(ast.actions[1].polarity, InstructionPolarity::Forbidden);
    assert_eq!(
        ast.actions[1].connector_before,
        Some(InstructionConjunction::And)
    );
    assert_eq!(ast.coordination.len(), 1);
    assert_eq!(ast.coordination[0].kind, InstructionConjunction::And);
    assert_eq!(
        ast.coordination_state,
        InstructionCoordinationState::Resolved
    );
    assert_eq!(
        ast.shared_argument_state,
        lc631_analysis::SharedArgumentState::UnresolvedCandidate
    );
    assert_eq!(ast.polarity_scopes.len(), 1);
    assert_eq!(ast.polarity_scopes[0].action_indices, 1..2);
    assert_eq!(
        ast.polarity_scopes[0].state,
        InstructionPolarityScopeState::LocalAction
    );
    assert!(source
        .get(ast.coordination[0].source_span.clone())
        .is_some());
}

#[test]
fn dangling_coordination_remains_unresolved_instead_of_dropping_the_connector() {
    let report =
        parse_controlled_instruction("Please test and", InstructionParseBudget::default()).unwrap();
    assert_eq!(report.state, InstructionParseState::Unresolved);
    assert!(report.selected.is_none());
    assert_eq!(report.alternatives.len(), 1);
    assert_eq!(
        report.alternatives[0].coordination_state,
        InstructionCoordinationState::Unresolved
    );
}

#[test]
fn conditional_expression_shape_does_not_promote_its_truth_and_preserves_modal_relation() {
    let source = "Please publish if tests pass and approval exists.";
    let report = parse_controlled_instruction(source, InstructionParseBudget::default()).unwrap();
    let ast = report.selected.unwrap();
    assert_eq!(ast.actions.len(), 1);
    assert_eq!(ast.actions[0].text, "publish");
    assert!(ast.body.contains("if tests pass and approval exists"));
    assert_eq!(ast.conditions.len(), 1);
    let condition = &ast.conditions[0];
    assert_eq!(condition.relation, ConditionRelation::Sufficient);
    assert_eq!(condition.evidence_state, ConditionEvidenceState::Unknown);
    assert_eq!(condition.applies_to_action_indices, 0..1);
    match &condition.expression {
        ConditionExpression::All(predicates) => assert_eq!(predicates.len(), 2),
        other => panic!("expected conjunction condition, got {other:?}"),
    }
    for predicate in condition.expression.predicates() {
        assert!(source.get(predicate.source_span.clone()).is_some());
    }
}

#[test]
fn postposed_condition_over_multiple_actions_keeps_action_scope_unresolved() {
    let report = parse_controlled_instruction(
        "Please test and publish if tests pass.",
        InstructionParseBudget::default(),
    )
    .unwrap();
    let ast = report.selected.unwrap();
    assert_eq!(ast.actions.len(), 2);
    assert_eq!(ast.conditions.len(), 1);
    assert_eq!(
        ast.conditions[0].scope_state,
        ConditionScopeState::Unresolved
    );
    assert_eq!(ast.conditions[0].applies_to_action_indices, 0..2);
    assert_eq!(
        ast.conditions[0].evidence_state,
        ConditionEvidenceState::Unknown
    );
}

#[test]
fn necessary_exception_and_temporal_conditions_remain_distinct_and_unproven() {
    let necessary = parse_controlled_instruction(
        "Please publish only if tests pass.",
        InstructionParseBudget::default(),
    )
    .unwrap();
    assert_eq!(
        necessary.selected.unwrap().conditions[0].relation,
        ConditionRelation::Necessary
    );

    let exception = parse_controlled_instruction(
        "Please publish unless tests fail.",
        InstructionParseBudget::default(),
    )
    .unwrap();
    let condition = &exception.selected.unwrap().conditions[0];
    assert_eq!(condition.relation, ConditionRelation::Exception);
    assert!(matches!(condition.expression, ConditionExpression::Not(_)));
    assert_eq!(condition.evidence_state, ConditionEvidenceState::Unknown);

    let japanese_exception = parse_controlled_instruction(
        "テストが成功しない限り公開してください。",
        InstructionParseBudget::default(),
    )
    .unwrap();
    let condition = &japanese_exception.selected.unwrap().conditions[0];
    assert_eq!(condition.relation, ConditionRelation::Exception);
    assert!(matches!(condition.expression, ConditionExpression::Not(_)));
    assert_eq!(condition.evidence_state, ConditionEvidenceState::Unknown);
    assert_eq!(condition.exception_precedence, Some(1));

    let temporal = parse_controlled_instruction(
        "Do not publish until tests pass.",
        InstructionParseBudget::default(),
    )
    .unwrap();
    let condition = &temporal.selected.unwrap().conditions[0];
    assert_eq!(condition.relation, ConditionRelation::Temporal);
    assert_eq!(condition.temporal_relation, Some(TemporalRelation::Until));
    assert_eq!(condition.evidence_state, ConditionEvidenceState::Unknown);
}

#[test]
fn mixed_boolean_precedence_stays_unresolved_and_explicit_condition_negation_is_noted() {
    let mixed = parse_controlled_instruction(
        "Please publish if tests pass and owner agrees or reviewer signs.",
        InstructionParseBudget::default(),
    )
    .unwrap();
    assert_eq!(mixed.state, InstructionParseState::Unresolved);
    assert!(mixed.selected.is_none());
    assert!(matches!(
        mixed.alternatives[0].conditions[0].expression,
        ConditionExpression::Unresolved(_)
    ));
    assert_eq!(
        mixed.alternatives[0].conditions[0].evidence_state,
        ConditionEvidenceState::Unknown
    );

    let explicit_not = parse_controlled_instruction(
        "Please publish if not blocked.",
        InstructionParseBudget::default(),
    )
    .unwrap();
    assert!(matches!(
        explicit_not.selected.as_ref().unwrap().conditions[0].expression,
        ConditionExpression::Not(_)
    ));
}

#[test]
fn nested_condition_operators_are_retained_without_flattening_parentheses() {
    let source = "Please publish if (tests pass or approval exists) and owner signs.";
    let report = parse_controlled_instruction(source, InstructionParseBudget::default()).unwrap();
    let ast = report.selected.unwrap();
    match &ast.conditions[0].expression {
        ConditionExpression::All(terms) => {
            assert_eq!(terms.len(), 2);
            assert!(matches!(terms[0], ConditionExpression::Any(_)));
        }
        other => panic!("expected nested All/Any expression, got {other:?}"),
    }
    assert_eq!(ast.actions.len(), 1);
    assert_eq!(
        ast.conditions[0].evidence_state,
        ConditionEvidenceState::Unknown
    );
}

#[test]
fn negation_scope_obeys_explicit_parentheses_without_absorbing_following_conjuncts() {
    let source = "Please publish if not tests pass and approval exists.";
    let report = parse_controlled_instruction(source, InstructionParseBudget::default()).unwrap();
    let expression = &report.selected.unwrap().conditions[0].expression;
    match expression {
        ConditionExpression::All(terms) => {
            assert_eq!(terms.len(), 2);
            assert!(matches!(terms[0], ConditionExpression::Not(_)));
            assert!(matches!(terms[1], ConditionExpression::Predicate(_)));
        }
        other => panic!("negation must not absorb the later conjunct: {other:?}"),
    }

    let explicit = parse_controlled_instruction(
        "Please publish if not (tests pass and approval exists).",
        InstructionParseBudget::default(),
    )
    .unwrap();
    assert!(matches!(
        explicit.selected.unwrap().conditions[0].expression,
        ConditionExpression::Not(ref inner) if matches!(**inner, ConditionExpression::All(_))
    ));
}

#[test]
fn quoted_condition_markers_are_not_promoted_into_live_condition_nodes() {
    let report = parse_controlled_instruction(
        "Please repeat \" if tests pass \" exactly.",
        InstructionParseBudget::default(),
    )
    .unwrap();
    let ast = report.selected.unwrap();
    assert!(ast.conditions.is_empty());
    assert_eq!(ast.actions.len(), 1);
    assert!(ast.actions[0].text.contains("if tests pass"));

    let quoted_connector = parse_controlled_instruction(
        "Please preserve \" and \" literally.",
        InstructionParseBudget::default(),
    )
    .unwrap();
    let quoted_connector_ast = quoted_connector.selected.unwrap();
    assert_eq!(quoted_connector_ast.actions.len(), 1);
    assert!(quoted_connector_ast.coordination.is_empty());

    let unterminated = parse_controlled_instruction(
        "Please repeat \" if tests pass exactly.",
        InstructionParseBudget::default(),
    )
    .unwrap();
    assert!(unterminated.selected.unwrap().conditions.is_empty());
}

#[test]
fn leading_context_and_japanese_sufficiency_conditions_remain_source_bound() {
    let leading = parse_controlled_instruction(
        "If tests pass, please publish.",
        InstructionParseBudget::default(),
    )
    .unwrap();
    assert_eq!(leading.state, InstructionParseState::Parsed);
    assert_eq!(
        leading.selected.as_ref().unwrap().conditions[0].relation,
        ConditionRelation::Sufficient
    );
    assert_eq!(
        leading.selected.as_ref().unwrap().conditions[0].evidence_state,
        ConditionEvidenceState::Unknown
    );

    let japanese = parse_controlled_instruction(
        "テストが成功した場合のみ公開してください。",
        InstructionParseBudget::default(),
    )
    .unwrap();
    let japanese_ast = japanese.selected.unwrap();
    assert_eq!(japanese_ast.conditions.len(), 1);
    let condition = &japanese_ast.conditions[0];
    assert_eq!(condition.relation, ConditionRelation::Necessary);
    assert_eq!(condition.evidence_state, ConditionEvidenceState::Unknown);
    assert_eq!(japanese_ast.actions[0].text, "公開");
}

#[test]
fn before_after_and_japanese_until_are_temporal_not_truth_claims() {
    for (source, expected) in [
        (
            "Please publish before tests expire.",
            TemporalRelation::Before,
        ),
        ("Please publish after tests pass.", TemporalRelation::After),
        (
            "テストが終わるまで公開してください。",
            TemporalRelation::Until,
        ),
    ] {
        let report =
            parse_controlled_instruction(source, InstructionParseBudget::default()).unwrap();
        assert_eq!(report.state, InstructionParseState::Parsed, "{source}");
        let condition = &report.selected.unwrap().conditions[0];
        assert_eq!(condition.temporal_relation, Some(expected), "{source}");
        assert_eq!(condition.evidence_state, ConditionEvidenceState::Unknown);
        assert!(source.get(condition.source_span.clone()).is_some());
    }
}

#[test]
fn postposed_modality_and_japanese_te_coordination_do_not_leak_scope() {
    let english = parse_controlled_instruction(
        "You may test but must not publish.",
        InstructionParseBudget::default(),
    )
    .unwrap();
    assert_eq!(english.state, InstructionParseState::Parsed);
    let actions = english.selected.unwrap().actions;
    assert_eq!(actions.len(), 2);
    assert_eq!(actions[0].modality, InstructionModality::Permitted);
    assert_eq!(actions[0].polarity, InstructionPolarity::Positive);
    assert_eq!(actions[1].modality, InstructionModality::Required);
    assert_eq!(actions[1].polarity, InstructionPolarity::Forbidden);

    let japanese = parse_controlled_instruction(
        "テストを実行し、公開しないでください。",
        InstructionParseBudget::default(),
    )
    .unwrap();
    assert_eq!(japanese.state, InstructionParseState::Parsed);
    let japanese_ast = japanese.selected.unwrap();
    assert_eq!(japanese_ast.actions.len(), 2);
    assert_eq!(
        japanese_ast.actions[0].polarity,
        InstructionPolarity::Positive
    );
    assert_eq!(
        japanese_ast.actions[1].polarity,
        InstructionPolarity::Forbidden
    );
    assert_eq!(
        japanese_ast.coordination[0].kind,
        InstructionConjunction::JapaneseTe
    );
    assert_eq!(japanese_ast.polarity_scopes.len(), 1);
    assert_eq!(japanese_ast.polarity_scopes[0].action_indices, 1..2);
    assert_eq!(
        japanese_ast.polarity_scopes[0].state,
        InstructionPolarityScopeState::LocalAction
    );
}

#[test]
fn outer_negation_is_not_distributed_across_a_coordinated_group() {
    let report = parse_controlled_instruction(
        "Do not delete or publish.",
        InstructionParseBudget::default(),
    )
    .unwrap();
    let ast = report.selected.unwrap();
    assert_eq!(ast.actions.len(), 2);
    assert!(ast
        .actions
        .iter()
        .all(|action| action.polarity == InstructionPolarity::Unknown));
    assert_eq!(ast.polarity_scopes.len(), 1);
    assert_eq!(
        ast.polarity_scopes[0].state,
        InstructionPolarityScopeState::UnresolvedGroup
    );
    assert_eq!(ast.polarity_scopes[0].action_indices, 0..2);
    assert_eq!(
        ast.polarity_scopes[0].polarity,
        InstructionPolarity::Forbidden
    );
}

#[test]
fn ordinary_prose_is_explicitly_unsupported_and_parser_budgets_are_fail_visible() {
    let unsupported = parse_controlled_instruction(
        "This is background context.",
        InstructionParseBudget::default(),
    )
    .unwrap();
    assert_eq!(unsupported.state, InstructionParseState::Unsupported);
    assert!(unsupported.selected.is_none());
    assert!(unsupported.alternatives.is_empty());

    assert!(parse_controlled_instruction(
        "Please test.",
        InstructionParseBudget {
            max_source_bytes: 1,
            ..InstructionParseBudget::default()
        }
    )
    .is_err());
    assert!(parse_controlled_instruction(
        "Please test.",
        InstructionParseBudget {
            max_tokens: 1,
            ..InstructionParseBudget::default()
        }
    )
    .is_err());
    assert!(matches!(
        parse_controlled_instruction(
            "Please test parser.",
            InstructionParseBudget {
                max_chart_items: 1,
                ..InstructionParseBudget::default()
            }
        ),
        Err(InstructionGrammarError::ChartItemBudgetExceeded)
    ));
    assert!(matches!(
        parse_controlled_instruction(
            "Please testしてください",
            InstructionParseBudget {
                max_forest_nodes: 1,
                ..InstructionParseBudget::default()
            }
        ),
        Err(InstructionGrammarError::ForestBudgetExceeded)
    ));
    assert!(matches!(
        parse_controlled_instruction(
            "Please test parser.",
            InstructionParseBudget {
                max_derivation_depth: 1,
                ..InstructionParseBudget::default()
            }
        ),
        Err(InstructionGrammarError::DerivationDepthExceeded)
    ));
}
