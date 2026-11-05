use lc631_analysis::{
    build_dgcl_pipeline,
    context_lowering::lower_dgcl_context,
    logical_lowering::{
        lower_dgcl_logical, AtomRole, FormulaKind, LogicProfile, LogicalLoweringBudget,
        LogicalResidualReason, LogicalTask,
    },
    ConditionEvidenceState, ConditionRelation, DgclBackendSet,
};
use lc631_core::{
    context::{ContextLimits, ContextScope, SourceRevision, TaskKind},
    AuthorityRevision,
};
use lc631_runtime::solver_lowering::{
    lower_logical_formula, prepare_abstract_condition_satisfiability, AbstractSatisfiabilityBudget,
    AbstractSatisfiabilityDecision, AbstractSatisfiabilityHoldReason,
    AbstractSatisfiabilityInterpretation, FormulaHoldReason, FormulaLoweringDecision,
};
use lc631_solvers::{runner::QuerySourceIdentity, typed_query::SmtEmitLimits};

fn lower(source: &str, profile: LogicProfile) -> LogicalTask {
    let report = build_dgcl_pipeline(
        source,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    let pin = SourceRevision::of("TLCX-10 abstract SAT scope").digest();
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
    lower_dgcl_logical(
        &report,
        &context,
        TaskKind::Reasoning,
        profile,
        LogicalLoweringBudget::default(),
    )
    .unwrap()
}

fn hold_reason(decision: AbstractSatisfiabilityDecision<'_>) -> AbstractSatisfiabilityHoldReason {
    let AbstractSatisfiabilityDecision::Hold(held) = decision else {
        panic!("unsupported abstract condition query must Hold");
    };
    held.reason()
}

#[test]
fn tlcx10_public_unknown_condition_is_prepared_only_as_free_boolean_atoms() {
    let task = lower(
        "Please verify the result if ready.",
        LogicProfile::ClassicalPropositionalV1,
    );
    let condition = task.conditions().first().unwrap();
    assert_eq!(condition.reported_state(), ConditionEvidenceState::Unknown);

    let residuals_before: Vec<_> = task
        .residuals()
        .iter()
        .map(|residual| (residual.span(), residual.reason()))
        .collect();
    let obligations_before: Vec<_> = task
        .obligations()
        .iter()
        .map(|obligation| {
            (
                obligation.target(),
                obligation.applicability(),
                obligation.condition_indices().to_vec(),
            )
        })
        .collect();

    let decision = prepare_abstract_condition_satisfiability(
        &task,
        condition.formula(),
        AbstractSatisfiabilityInterpretation::FreeBooleanAtoms,
        SmtEmitLimits::default(),
        AbstractSatisfiabilityBudget::default(),
    );
    let AbstractSatisfiabilityDecision::Prepared(prepared) = decision else {
        panic!("a supported condition tree may be prepared under the explicit abstract mode");
    };

    assert_eq!(
        prepared.interpretation(),
        AbstractSatisfiabilityInterpretation::FreeBooleanAtoms
    );
    assert_eq!(prepared.binding().formula_id(), condition.formula());
    assert_eq!(
        prepared.binding().context_revision(),
        task.context_revision()
    );
    assert_eq!(
        prepared.binding().source_identity(),
        QuerySourceIdentity::from_bytes(*task.source_store().id().digest().as_bytes())
    );
    assert_eq!(prepared.binding().task_kind(), TaskKind::Reasoning);
    assert_eq!(
        prepared.binding().logic_profile(),
        LogicProfile::ClassicalPropositionalV1
    );
    assert_eq!(
        prepared.binding().program_digest(),
        task.program_ir().legacy_program_digest()
    );
    assert_eq!(
        prepared.binding().grammar_revision(),
        task.program_ir().parser_grammar_revision()
    );
    assert_eq!(
        prepared.binding().formula_span(),
        task.program_ir()
            .formula(condition.formula())
            .ok()
            .map(|formula| formula.span())
    );
    assert_eq!(prepared.condition_span(), condition.span());
    assert_eq!(
        prepared.condition_reported_state(),
        ConditionEvidenceState::Unknown
    );
    assert_eq!(prepared.relation(), ConditionRelation::Sufficient);
    assert!(!prepared.guard_uses().is_empty());
    assert!(prepared
        .guard_uses()
        .iter()
        .all(|guard| guard.relation() == ConditionRelation::Sufficient));
    assert!(!prepared.atom_bindings().is_empty());
    assert!(prepared
        .atom_bindings()
        .iter()
        .all(|atom| atom.condition_reported_state() == ConditionEvidenceState::Unknown));
    assert!(prepared
        .selected_condition_residuals()
        .iter()
        .any(
            |residual| residual.reason() == LogicalResidualReason::UnknownCondition
                && residual.source_span() == condition.span()
        ));
    assert!(prepared
        .selected_condition_residuals()
        .iter()
        .any(|residual| residual.reason() == LogicalResidualReason::UninterpretedPredicate));

    assert_eq!(
        prepared.unvalidated_task_residual_count(),
        prepared.unvalidated_task_residuals().len()
    );
    assert_eq!(
        prepared.unvalidated_task_residual_count(),
        residuals_before.len() - prepared.selected_condition_residuals().len()
    );
    assert!(prepared.unvalidated_task_residual_count() > 0);
    assert!(prepared
        .unvalidated_task_residuals()
        .iter()
        .all(
            |residual| residual.source_span().end() <= condition.span().start()
                || residual.source_span().start() >= condition.span().end()
        ));
    assert!(prepared
        .claim_boundary()
        .contains("unvalidated task residuals"));

    let payload = std::str::from_utf8(prepared.smt_payload()).unwrap();
    assert!(payload.contains("(declare-const b"));
    let assertions: Vec<_> = payload
        .lines()
        .filter(|line| line.starts_with("(assert "))
        .collect();
    assert_eq!(assertions.len(), 1);
    let symbol = prepared.atom_bindings()[0].symbol().get();
    assert_eq!(assertions[0], format!("(assert b{symbol})"));
    assert!(!payload.contains("verify"));
    assert!(!payload.contains("result"));
    assert!(!payload.contains("(assert true)"));
    assert!(!payload.contains("(assert false)"));
    assert!(prepared
        .claim_boundary()
        .contains("not source truth, evidence, entailment, obligation completion, authority"));
    assert!(prepared
        .claim_boundary()
        .contains("unvalidated task residuals"));

    // Query preparation is immutable and does not resolve task obligations.
    assert_eq!(condition.reported_state(), ConditionEvidenceState::Unknown);
    assert_eq!(
        task.residuals()
            .iter()
            .map(|residual| (residual.span(), residual.reason()))
            .collect::<Vec<_>>(),
        residuals_before
    );
    assert_eq!(
        task.obligations()
            .iter()
            .map(|obligation| (
                obligation.target(),
                obligation.applicability(),
                obligation.condition_indices().to_vec(),
            ))
            .collect::<Vec<_>>(),
        obligations_before
    );
}

#[test]
fn tlcx10_evidence_gated_lowering_still_holds_public_unknown_conditions() {
    let task = lower(
        "Please verify the result if ready.",
        LogicProfile::ClassicalPropositionalV1,
    );
    let formula = task.conditions()[0].formula();
    let FormulaLoweringDecision::Hold(held) =
        lower_logical_formula(&task, formula, SmtEmitLimits::default())
    else {
        panic!("the evidence-gated API must not be weakened by the abstract API");
    };
    assert_eq!(held.reason(), FormulaHoldReason::UnknownConditionEvidence);
}

#[test]
fn tlcx10_abstract_sat_holds_non_condition_formula_roots() {
    let task = lower(
        "Please verify the result if ready.",
        LogicProfile::ClassicalPropositionalV1,
    );
    let action = task
        .formulas()
        .iter()
        .find(|formula| {
            matches!(
                formula.kind(),
                FormulaKind::Atom {
                    role: AtomRole::UninterpretedAction
                }
            )
        })
        .unwrap();
    assert_eq!(
        hold_reason(prepare_abstract_condition_satisfiability(
            &task,
            action.id(),
            AbstractSatisfiabilityInterpretation::FreeBooleanAtoms,
            SmtEmitLimits::default(),
            AbstractSatisfiabilityBudget::default(),
        )),
        AbstractSatisfiabilityHoldReason::UnsupportedActionAtom
    );

    let directive = task
        .formulas()
        .iter()
        .find(|formula| matches!(formula.kind(), FormulaKind::Directive { .. }))
        .unwrap();
    assert_eq!(
        hold_reason(prepare_abstract_condition_satisfiability(
            &task,
            directive.id(),
            AbstractSatisfiabilityInterpretation::FreeBooleanAtoms,
            SmtEmitLimits::default(),
            AbstractSatisfiabilityBudget::default(),
        )),
        AbstractSatisfiabilityHoldReason::UnsupportedDirective
    );

    let guard = task
        .formulas()
        .iter()
        .find(|formula| matches!(formula.kind(), FormulaKind::Guarded { .. }))
        .unwrap();
    assert_eq!(
        hold_reason(prepare_abstract_condition_satisfiability(
            &task,
            guard.id(),
            AbstractSatisfiabilityInterpretation::FreeBooleanAtoms,
            SmtEmitLimits::default(),
            AbstractSatisfiabilityBudget::default(),
        )),
        AbstractSatisfiabilityHoldReason::UnsupportedGuard
    );
}

#[test]
fn tlcx10_abstract_sat_holds_foreign_asp_and_temporal_inputs() {
    let local = lower(
        "Please verify alpha if ready.",
        LogicProfile::ClassicalPropositionalV1,
    );
    let foreign = lower(
        "Please verify beta if ready.",
        LogicProfile::ClassicalPropositionalV1,
    );
    assert_eq!(
        hold_reason(prepare_abstract_condition_satisfiability(
            &local,
            foreign.conditions()[0].formula(),
            AbstractSatisfiabilityInterpretation::FreeBooleanAtoms,
            SmtEmitLimits::default(),
            AbstractSatisfiabilityBudget::default(),
        )),
        AbstractSatisfiabilityHoldReason::ForeignFormula
    );

    let asp = lower(
        "Please verify the result if ready.",
        LogicProfile::AspGroundNormalV1,
    );
    assert_eq!(
        hold_reason(prepare_abstract_condition_satisfiability(
            &asp,
            asp.conditions()[0].formula(),
            AbstractSatisfiabilityInterpretation::FreeBooleanAtoms,
            SmtEmitLimits::default(),
            AbstractSatisfiabilityBudget::default(),
        )),
        AbstractSatisfiabilityHoldReason::UnsupportedLogicProfile
    );

    let temporal = lower(
        "Please verify the result when ready.",
        LogicProfile::ClassicalPropositionalV1,
    );
    assert_eq!(
        hold_reason(prepare_abstract_condition_satisfiability(
            &temporal,
            temporal.conditions()[0].formula(),
            AbstractSatisfiabilityInterpretation::FreeBooleanAtoms,
            SmtEmitLimits::default(),
            AbstractSatisfiabilityBudget::default(),
        )),
        AbstractSatisfiabilityHoldReason::UnsupportedConditionRelation
    );
}

#[test]
fn tlcx10_abstract_sat_holds_unresolved_condition_scope_and_budget_exhaustion() {
    let unresolved = lower(
        "Please test and publish if tests pass.",
        LogicProfile::ClassicalPropositionalV1,
    );
    assert_eq!(
        hold_reason(prepare_abstract_condition_satisfiability(
            &unresolved,
            unresolved.conditions()[0].formula(),
            AbstractSatisfiabilityInterpretation::FreeBooleanAtoms,
            SmtEmitLimits::default(),
            AbstractSatisfiabilityBudget::default(),
        )),
        AbstractSatisfiabilityHoldReason::UnresolvedConditionScope
    );

    let bounded = lower(
        "Please verify the result if ready.",
        LogicProfile::ClassicalPropositionalV1,
    );
    let limits = SmtEmitLimits::checked(1024, 0, 0, 1, 16, 16).unwrap();
    assert_eq!(
        hold_reason(prepare_abstract_condition_satisfiability(
            &bounded,
            bounded.conditions()[0].formula(),
            AbstractSatisfiabilityInterpretation::FreeBooleanAtoms,
            limits,
            AbstractSatisfiabilityBudget::default(),
        )),
        AbstractSatisfiabilityHoldReason::BooleanSymbolLimitExceeded
    );
}

#[test]
fn tlcx10_abstract_sat_holds_when_scope_work_budget_is_exhausted() {
    let task = lower(
        "Please verify the result if ready.",
        LogicProfile::ClassicalPropositionalV1,
    );
    let decision = prepare_abstract_condition_satisfiability(
        &task,
        task.conditions()[0].formula(),
        AbstractSatisfiabilityInterpretation::FreeBooleanAtoms,
        SmtEmitLimits::default(),
        AbstractSatisfiabilityBudget::with_max_scope_work(1),
    );
    assert_eq!(
        hold_reason(decision),
        AbstractSatisfiabilityHoldReason::ScopeWorkLimitExceeded
    );
}

#[test]
fn tlcx10_abstract_sat_rejects_a_budget_above_the_profile_cap() {
    let task = lower(
        "Please verify the result if ready.",
        LogicProfile::ClassicalPropositionalV1,
    );
    let decision = prepare_abstract_condition_satisfiability(
        &task,
        task.conditions()[0].formula(),
        AbstractSatisfiabilityInterpretation::FreeBooleanAtoms,
        SmtEmitLimits::default(),
        AbstractSatisfiabilityBudget::with_max_scope_work(usize::MAX),
    );
    assert_eq!(
        hold_reason(decision),
        AbstractSatisfiabilityHoldReason::InvalidScopeBudget
    );
}
