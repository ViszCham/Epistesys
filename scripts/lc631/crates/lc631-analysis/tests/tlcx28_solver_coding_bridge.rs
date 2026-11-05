use lc631_analysis::{
    build_dgcl_pipeline,
    context_lowering::lower_dgcl_context,
    logical_lowering::{lower_dgcl_logical, LogicProfile, LogicalLoweringBudget, LogicalTask},
    solver_coding_bridge::*,
    DgclBackendSet,
};
use lc631_core::{context::*, AuthorityRevision};

fn task(source: &str, kind: TaskKind) -> LogicalTask {
    let report = build_dgcl_pipeline(
        source,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    let pin = SourceRevision::of("TLCX28 fixed formal model scope").digest();
    let context = lower_dgcl_context(
        &report,
        ContextScope::declared(pin, pin, pin, pin),
        AuthorityRevision(7),
        ContextLimits {
            max_contexts: 4,
            max_premises: 1024,
            max_edits: 4,
            max_statement_bytes: 262_144,
        },
    )
    .unwrap();
    lower_dgcl_logical(
        &report,
        &context,
        kind,
        LogicProfile::ClassicalPropositionalV1,
        LogicalLoweringBudget::default(),
    )
    .unwrap()
}

#[test]
fn tlcx28_sat_and_existential_reports_never_discharge_conditions() {
    let task = task("Please verify the result if ready.", TaskKind::Coding);
    assert!(!task.conditions().is_empty());
    let request = ConditionRequest::new(&task, 0).unwrap();
    for result in [ReportedRelation::Sat, ReportedRelation::ExistentialSupport] {
        let report = ConditionReport::reported(&request, result);
        let checked = check_condition(&request, &[], &report, 100_000).unwrap();
        assert_eq!(checked.support(), FiniteSupport::Undetermined);
        assert_eq!(checked.report_relation(), ReportCheck::WrongResultKind);
        assert_eq!(
            checked.source_condition_state(),
            SourceConditionState::NeedsEvidence
        );
    }
}

#[test]
fn tlcx28_foreign_predicate_context_and_result_are_rejected() {
    let first = task("Please verify the result if ready.", TaskKind::Coding);
    let second = task("Please verify the result if complete.", TaskKind::Coding);
    let request = ConditionRequest::new(&first, 0).unwrap();
    let other = ConditionRequest::new(&second, 0).unwrap();
    let report = ConditionReport::reported(&other, ReportedRelation::UniversalSupport);
    assert!(matches!(
        check_condition(&request, &[], &report, 100_000),
        Err(ConditionBridgeError::ForeignReport)
    ));
    let report = ConditionReport::reported(&request, ReportedRelation::UniversalSupport);
    assert!(matches!(
        check_condition(&request, &[(other.predicate(), true)], &report, 100_000),
        Err(ConditionBridgeError::ForeignFormula)
    ));
}

#[test]
fn tlcx28_inconsistent_context_budget_and_missing_result_stay_visible() {
    let task = task("Please verify the result if ready.", TaskKind::Reasoning);
    let request = ConditionRequest::new(&task, 0).unwrap();
    let report = ConditionReport::reported(&request, ReportedRelation::UniversalSupport);
    let contradictory = [(request.predicate(), true), (request.predicate(), false)];
    let checked = check_condition(&request, &contradictory, &report, 100_000).unwrap();
    assert_eq!(checked.support(), FiniteSupport::ContextInconsistent);
    assert_eq!(checked.report_relation(), ReportCheck::Disagrees);
    assert!(matches!(
        check_condition(&request, &[], &report, 0),
        Err(ConditionBridgeError::BudgetExhausted)
    ));
    assert!(matches!(
        check_condition(&request, &[], &report, MAX_CONDITION_CHECK_STEPS + 1),
        Err(ConditionBridgeError::InvalidBudget)
    ));
    let missing = ConditionReport::reported(&request, ReportedRelation::Unavailable);
    assert_eq!(
        check_condition(&request, &[], &missing, 100_000)
            .unwrap()
            .report_relation(),
        ReportCheck::Unavailable
    );
}

#[test]
fn tlcx28_refutation_is_distinct_from_inconsistent_context() {
    let task = task("Please verify the result if ready.", TaskKind::Reasoning);
    let request = ConditionRequest::new(&task, 0).unwrap();
    let report = ConditionReport::reported(&request, ReportedRelation::UniversalRefutation);
    let premises = [(request.predicate(), false)];
    let checked = check_condition(&request, &premises, &report, 100_000).unwrap();
    assert_eq!(checked.support(), FiniteSupport::Refuted);
    assert_eq!(checked.report_relation(), ReportCheck::Agrees);
    assert!(checked.satisfying_assignments() > 0);
    assert_eq!(
        checked.source_condition_state(),
        SourceConditionState::NeedsEvidence
    );
}

#[test]
fn tlcx28_finite_check_matches_explicit_premise_and_cut_reopens_model_support() {
    let task = task("Please verify the result if ready.", TaskKind::Reasoning);
    let request = ConditionRequest::new(&task, 0).unwrap();
    let report = ConditionReport::reported(&request, ReportedRelation::UniversalSupport);
    let premises = [(request.predicate(), true)];
    let checked = check_condition(&request, &premises, &report, 100_000).unwrap();
    assert_eq!(checked.support(), FiniteSupport::Supported);
    assert_eq!(checked.report_relation(), ReportCheck::Agrees);
    assert!(checked.satisfying_assignments() > 0);
    // Explicit premises define this finite model; they are not source facts.
    assert_eq!(
        checked.source_condition_state(),
        SourceConditionState::NeedsEvidence
    );
    let cut = check_condition(&request, &[], &report, 100_000).unwrap();
    assert_eq!(cut.support(), FiniteSupport::Undetermined);
    assert_eq!(cut.report_relation(), ReportCheck::Disagrees);
}

#[test]
fn tlcx28_task_routing_keeps_five_coding_kinds_and_no_cargo_for_math() {
    let math = task("Please verify the result.", TaskKind::Mathematics);
    let coding = task("Please verify the result.", TaskKind::Coding);
    let math_route = task_validation_route(&math).unwrap();
    let coding_route = task_validation_route(&coding).unwrap();
    assert_eq!(math_route, TaskValidationRoute::TaskSpecificEvidence);
    assert_eq!(
        coding_route,
        TaskValidationRoute::ExistingFiveKindCodingValidation
    );
    assert_eq!(coding.obligations().len(), 5);
    assert!(math
        .obligations()
        .iter()
        .all(|o| !o.kind().is_coding_validation()));
}
