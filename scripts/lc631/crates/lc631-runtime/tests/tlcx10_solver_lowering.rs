use lc631_analysis::{
    build_dgcl_pipeline,
    context_lowering::{lower_dgcl_context, LoweredContext},
    logical_lowering::{lower_dgcl_logical, LogicProfile, LogicalLoweringBudget, LogicalTask},
    ConditionEvidenceState, DgclBackendSet, DgclPipelineReport,
};
use lc631_core::{
    context::{ContextLimits, ContextScope, SourceRevision, TaskKind},
    AuthorityRevision,
};
use lc631_runtime::solver_lowering::{
    lower_logical_formula, FormulaHoldReason, FormulaLoweringDecision,
};
use lc631_solvers::runner::QuerySourceIdentity;

fn input(source: &str) -> (DgclPipelineReport, LoweredContext) {
    let report = build_dgcl_pipeline(
        source,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    let pin = SourceRevision::of("TLCX-10 adapter test scope").digest();
    let context = lower_dgcl_context(
        &report,
        ContextScope::declared(pin, pin, pin, pin),
        AuthorityRevision(10),
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

fn lower(source: &str, profile: LogicProfile) -> LogicalTask {
    let (report, context) = input(source);
    lower_dgcl_logical(
        &report,
        &context,
        TaskKind::Reasoning,
        profile,
        LogicalLoweringBudget::default(),
    )
    .unwrap()
}

#[test]
fn tlcx10_unknown_condition_evidence_is_held_with_source_and_formula_identity() {
    let task = lower(
        "Please verify the result if ready.",
        LogicProfile::ClassicalPropositionalV1,
    );
    assert!(!task.conditions().is_empty());
    assert!(task
        .conditions()
        .iter()
        .all(|condition| condition.reported_state() == ConditionEvidenceState::Unknown));

    let formula = task.formulas()[0].id();
    let decision = lower_logical_formula(&task, formula, Default::default());
    let FormulaLoweringDecision::Hold(held) = decision else {
        panic!("unobserved condition evidence must not become an SMT query");
    };

    assert_eq!(held.reason(), FormulaHoldReason::UnknownConditionEvidence);
    assert_eq!(held.binding().formula_id(), formula);
    assert_eq!(held.binding().context_revision(), task.context_revision());
    assert_eq!(held.binding().task_kind(), TaskKind::Reasoning);
    assert_eq!(
        held.binding().logic_profile(),
        LogicProfile::ClassicalPropositionalV1
    );
    assert_eq!(
        held.binding().profile_revision(),
        "controlled-classical-conditions.v1"
    );
    assert_eq!(
        held.binding().source_identity(),
        QuerySourceIdentity::from_bytes(*task.source_store().id().digest().as_bytes())
    );
    assert_eq!(
        held.binding().program_digest(),
        task.program_ir().legacy_program_digest()
    );
    assert_eq!(
        held.binding().grammar_revision(),
        task.program_ir().parser_grammar_revision()
    );
}

#[test]
fn tlcx10_residuals_are_held_without_discarding_the_binding() {
    let task = lower("Please verify it.", LogicProfile::ClassicalPropositionalV1);
    assert!(!task.formulas().is_empty());
    assert!(task.conditions().is_empty());
    assert!(!task.residuals().is_empty());
    assert!(task.residuals().iter().any(|residual| residual.reason()
        != lc631_analysis::logical_lowering::LogicalResidualReason::UninterpretedPredicate));
    let formula = task.formulas()[0].id();

    let FormulaLoweringDecision::Hold(held) =
        lower_logical_formula(&task, formula, Default::default())
    else {
        panic!("unresolved task residuals must not be lowered");
    };
    assert_eq!(held.reason(), FormulaHoldReason::UnsupportedTaskResidual);
    assert_eq!(held.binding().formula_id(), formula);
}

#[test]
fn tlcx10_asp_profile_is_not_reinterpreted_as_classical_smt() {
    let task = lower(
        "Please verify the result if ready.",
        LogicProfile::AspGroundNormalV1,
    );
    let formula = task.formulas()[0].id();

    let FormulaLoweringDecision::Hold(held) =
        lower_logical_formula(&task, formula, Default::default())
    else {
        panic!("ASP must not be projected into classical SMT");
    };
    assert_eq!(held.reason(), FormulaHoldReason::UnsupportedLogicProfile);
}

#[test]
fn tlcx10_foreign_formula_owner_is_rejected_before_other_holds() {
    let left = lower(
        "Please verify alpha if ready.",
        LogicProfile::ClassicalPropositionalV1,
    );
    let right = lower(
        "Please verify beta if ready.",
        LogicProfile::ClassicalPropositionalV1,
    );
    let foreign_id = right.formulas()[0].id();

    let FormulaLoweringDecision::Hold(held) =
        lower_logical_formula(&left, foreign_id, Default::default())
    else {
        panic!("a formula owned by another program must not be rebound");
    };
    assert_eq!(held.reason(), FormulaHoldReason::ForeignFormula);
    assert_eq!(held.binding().formula_id(), foreign_id);
    assert_eq!(held.binding().context_revision(), left.context_revision());
}
