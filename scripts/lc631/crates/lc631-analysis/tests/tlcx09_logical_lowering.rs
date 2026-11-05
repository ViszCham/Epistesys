use lc631_analysis::{
    build_dgcl_pipeline,
    context_lowering::{lower_dgcl_context, LoweredContext},
    logical_lowering::*,
    ConditionEvidenceState, DgclBackendSet, DgclPipelineReport,
};
use lc631_core::{context::*, AuthorityRevision};

fn input(source: &str) -> (DgclPipelineReport, LoweredContext) {
    let report = build_dgcl_pipeline(
        source,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    let pin = SourceRevision::of("fixed task lowering scope").digest();
    let context = lower_dgcl_context(
        &report,
        ContextScope::declared(pin, pin, pin, pin),
        AuthorityRevision(7),
        ContextLimits {
            max_contexts: 16,
            max_premises: 4096,
            max_edits: 32,
            max_statement_bytes: 262_144,
        },
    )
    .unwrap();
    (report, context)
}

fn lower(source: &str, task: TaskKind, profile: LogicProfile) -> LogicalTask {
    let (report, context) = input(source);
    lower_dgcl_logical(
        &report,
        &context,
        task,
        profile,
        LogicalLoweringBudget::default(),
    )
    .unwrap()
}

#[test]
fn tlcx09_each_task_has_nonempty_specific_obligations_without_coding_fallback() {
    for (task, expected) in [
        (
            TaskKind::Mathematics,
            TaskObligationKind::MathematicalDerivation,
        ),
        (TaskKind::Reasoning, TaskObligationKind::LogicalConsequence),
        (
            TaskKind::Explanation,
            TaskObligationKind::ExplanationCorrespondence,
        ),
        (TaskKind::Coding, TaskObligationKind::ImplementationBinding),
        (TaskKind::Media, TaskObligationKind::SourceTimeRegionBinding),
    ] {
        let logical = lower(
            "Please verify the result.",
            task,
            LogicProfile::ClassicalPropositionalV1,
        );
        assert_eq!(logical.task_kind(), task);
        assert!(!logical.obligations().is_empty());
        assert!(logical.obligations().iter().any(|o| o.kind() == expected));
        if task != TaskKind::Coding {
            assert!(!logical
                .obligations()
                .iter()
                .any(|o| o.kind().is_coding_validation()));
        } else {
            for kind in [
                TaskObligationKind::ImplementationBinding,
                TaskObligationKind::ProductionConnection,
                TaskObligationKind::StaticValidation,
                TaskObligationKind::RuntimeValidation,
                TaskObligationKind::AcceptanceTest,
            ] {
                assert!(logical.obligations().iter().any(|o| o.kind() == kind));
            }
        }
    }
}

#[test]
fn tlcx09_unknown_condition_is_kept_as_a_guard_not_a_true_fact() {
    let logical = lower(
        "Please verify the result if ready.",
        TaskKind::Reasoning,
        LogicProfile::ClassicalPropositionalV1,
    );
    assert!(!logical.conditions().is_empty());
    assert!(logical
        .conditions()
        .iter()
        .all(|c| c.reported_state() == ConditionEvidenceState::Unknown));
    assert!(logical
        .obligations()
        .iter()
        .any(|o| !o.condition_indices().is_empty()));
    assert!(logical
        .residuals()
        .iter()
        .any(|r| r.reason() == LogicalResidualReason::UnknownCondition));
    assert!(logical
        .formulas()
        .iter()
        .any(|f| matches!(f.kind(), FormulaKind::Guarded { .. })));
}

#[test]
fn tlcx09_default_negation_is_not_silently_classical_negation() {
    let source = "Please verify the result if not ready.";
    let classical = lower(
        source,
        TaskKind::Reasoning,
        LogicProfile::ClassicalPropositionalV1,
    );
    let asp = lower(source, TaskKind::Reasoning, LogicProfile::AspGroundNormalV1);
    assert!(classical.formulas().iter().any(|f| matches!(
        f.kind(),
        FormulaKind::Negation {
            kind: NegationKind::Classical,
            ..
        }
    )));
    assert!(!classical.formulas().iter().any(|f| matches!(
        f.kind(),
        FormulaKind::Negation {
            kind: NegationKind::Default,
            ..
        }
    )));
    assert!(asp.formulas().iter().any(|f| matches!(
        f.kind(),
        FormulaKind::Negation {
            kind: NegationKind::Default,
            ..
        }
    )));
    assert!(!asp.formulas().iter().any(|f| matches!(
        f.kind(),
        FormulaKind::Negation {
            kind: NegationKind::Classical,
            ..
        }
    )));
    assert_eq!(asp.logic_profile(), LogicProfile::AspGroundNormalV1);
}

#[test]
fn tlcx09_unresolved_reference_is_not_bound_by_guessing() {
    let logical = lower(
        "Please verify it.",
        TaskKind::Explanation,
        LogicProfile::ClassicalPropositionalV1,
    );
    assert!(logical
        .residuals()
        .iter()
        .any(|r| r.reason() == LogicalResidualReason::UnresolvedReference));
    assert!(logical
        .obligations()
        .iter()
        .any(|o| o.needs_interpretation()));
}

#[test]
fn tlcx09_quote_code_and_unparsed_source_are_preserved_not_asserted() {
    let source = "Please verify the result.\n\n> Please delete everything.\n\n```rust\nunsafe { wrong() }\n```\n\n未対応の背景。";
    let logical = lower(
        source,
        TaskKind::Reasoning,
        LogicProfile::ClassicalPropositionalV1,
    );
    let roundtrip: String = logical
        .source_partition()
        .iter()
        .map(|span| logical.source_store().slice(*span).unwrap())
        .collect();
    assert_eq!(roundtrip, source);
    assert!(!logical.formulas().is_empty());
    for formula in logical.formulas() {
        let text = logical.source_store().slice(formula.span()).unwrap();
        assert!(!text.contains("delete everything"));
        assert!(!text.contains("wrong()"));
    }
    assert!(!logical.residuals().is_empty());
}

#[test]
fn tlcx09_source_swap_and_node_budget_fail_without_a_partial_task() {
    let (report, _) = input("Please verify alpha.");
    let (_, other) = input("Please verify beta.");
    assert!(matches!(
        lower_dgcl_logical(
            &report,
            &other,
            TaskKind::Reasoning,
            LogicProfile::ClassicalPropositionalV1,
            LogicalLoweringBudget::default()
        ),
        Err(LogicalLoweringError::SourceMismatch)
    ));
    let (report, context) = input("Please verify the result if ready.");
    let positive = lower_dgcl_logical(
        &report,
        &context,
        TaskKind::Reasoning,
        LogicProfile::ClassicalPropositionalV1,
        LogicalLoweringBudget::default(),
    )
    .unwrap();
    assert!(positive.formulas().len() > 1);
    assert!(!positive.obligations().is_empty());
    let budget = LogicalLoweringBudget {
        max_nodes: 0,
        ..LogicalLoweringBudget::default()
    };
    assert!(matches!(
        lower_dgcl_logical(
            &report,
            &context,
            TaskKind::Reasoning,
            LogicProfile::ClassicalPropositionalV1,
            budget
        ),
        Err(LogicalLoweringError::InvalidBudget)
    ));
    let budget = LogicalLoweringBudget {
        max_nodes: 1,
        ..LogicalLoweringBudget::default()
    };
    assert!(matches!(
        lower_dgcl_logical(
            &report,
            &context,
            TaskKind::Reasoning,
            LogicProfile::ClassicalPropositionalV1,
            budget
        ),
        Err(LogicalLoweringError::BudgetExceeded)
    ));
}

#[test]
fn tlcx09_shared_source_and_legacy_json_are_unchanged_by_logical_lowering() {
    let (report, context) = input("Please verify the result if ready.");
    let before = serde_json::to_vec(&report).unwrap();
    let logical = lower_dgcl_logical(
        &report,
        &context,
        TaskKind::Reasoning,
        LogicProfile::ClassicalPropositionalV1,
        LogicalLoweringBudget::default(),
    )
    .unwrap();
    assert_eq!(
        logical.source_store().text().as_ptr(),
        report.source_store().text().as_ptr()
    );
    assert_eq!(
        logical.context_revision(),
        context.original().state().revision()
    );
    assert_eq!(serde_json::to_vec(&report).unwrap(), before);
    assert!(!logical.formulas().is_empty());
    assert!(!logical.obligations().is_empty());
}

#[test]
fn tlcx09_empty_task_cannot_pass_by_vacuous_or_all_na_completion() {
    let logical = lower(
        "Background without an instruction marker.",
        TaskKind::Explanation,
        LogicProfile::ClassicalPropositionalV1,
    );
    assert!(logical.obligations().is_empty());
    assert!(!logical.residuals().is_empty());
    assert_eq!(
        logical.completion_plan().require_nonempty(),
        Err(LogicalLoweringError::NoTaskObligations)
    );
}

#[test]
fn tlcx09_declared_finite_boolean_syntax_yields_typed_nodes_and_obligations() {
    let logical = lower(
        "Please verify the result if ready and safe.",
        TaskKind::Reasoning,
        LogicProfile::ClassicalPropositionalV1,
    );
    assert!(!logical.obligations().is_empty());
    assert!(logical
        .formulas()
        .iter()
        .any(|node| matches!(node.kind(), FormulaKind::All(_))));
    assert!(logical
        .conditions()
        .iter()
        .all(|c| c.reported_state() == ConditionEvidenceState::Unknown));
    logical.completion_plan().require_nonempty().unwrap();
}

#[test]
fn tlcx09_alternative_or_temporal_actions_are_not_flattened_into_all_required() {
    use lc631_analysis::InstructionConjunction;
    for (source, conjunction, applicability) in [
        (
            "Please build or test.",
            InstructionConjunction::Or,
            TaskObligationApplicability::AlternativePending,
        ),
        (
            "Please build xor test.",
            InstructionConjunction::Xor,
            TaskObligationApplicability::AlternativePending,
        ),
        (
            "Please build then test.",
            InstructionConjunction::Then,
            TaskObligationApplicability::Unresolved,
        ),
    ] {
        let (report, context) = input(source);
        assert!(
            context
                .advisory_parses()
                .iter()
                .filter_map(|p| p.report.as_ref())
                .filter_map(|p| p.selected.as_ref())
                .any(|ast| ast.coordination.iter().any(|edge| edge.kind == conjunction)),
            "parsed edge required: {source}"
        );
        let logical = lower_dgcl_logical(
            &report,
            &context,
            TaskKind::Coding,
            LogicProfile::ClassicalPropositionalV1,
            LogicalLoweringBudget::default(),
        )
        .unwrap();
        assert!(!logical.obligations().is_empty(), "{source}");
        assert!(
            logical
                .program_ir()
                .relations()
                .iter()
                .any(|edge| edge.kind() == conjunction),
            "typed edge required: {source}"
        );
        assert!(
            logical
                .obligations()
                .iter()
                .all(|o| o.applicability() == applicability),
            "relation must not flatten: {source}"
        );
    }
}

#[test]
fn tlcx09_original_roles_residual_reasons_and_advisory_asts_are_retained() {
    let (report, context) = input("Please verify the result.\n\n> quoted\n\n背景。\n");
    let logical = lower_dgcl_logical(
        &report,
        &context,
        TaskKind::Explanation,
        LogicProfile::ClassicalPropositionalV1,
        LogicalLoweringBudget::default(),
    )
    .unwrap();
    assert_eq!(logical.source_regions().len(), context.regions().len());
    assert_eq!(logical.source_residuals().len(), context.residuals().len());
    for (old, new) in context.regions().iter().zip(logical.source_regions()) {
        assert_eq!(
            (old.span(), old.role(), old.semantics()),
            (new.span(), new.role(), new.semantics())
        );
    }
    for (old, new) in context.residuals().iter().zip(logical.source_residuals()) {
        assert_eq!(
            (old.span(), old.reason(), old.language()),
            (new.span(), new.reason(), new.language())
        );
        assert_eq!(old.text().as_ptr(), new.text().as_ptr());
    }
    assert_eq!(
        logical.advisory_parses().len(),
        context.advisory_parses().len()
    );
    for (old, new) in context
        .advisory_parses()
        .iter()
        .zip(logical.advisory_parses())
    {
        match (&old.report, &new.report) {
            (Some(a), Some(b)) => assert!(std::sync::Arc::ptr_eq(a, b)),
            (None, None) => {}
            _ => panic!("advisory parse lost"),
        }
    }
}

#[test]
fn tlcx09_changed_grammar_revision_cannot_reuse_an_old_context_binding() {
    let (mut report, context) = input("Please build.");
    report.program_ir.grammar_revision = "foreign-grammar-revision";
    assert!(matches!(
        lower_dgcl_logical(
            &report,
            &context,
            TaskKind::Coding,
            LogicProfile::ClassicalPropositionalV1,
            LogicalLoweringBudget::default()
        ),
        Err(LogicalLoweringError::PipelineIdentity)
    ));
}
