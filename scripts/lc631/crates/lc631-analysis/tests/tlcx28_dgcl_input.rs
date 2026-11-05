use lc631_analysis::{
    build_dgcl_pipeline,
    context_lowering::{lower_dgcl_context, LoweredContext},
    logical_lowering::{lower_dgcl_logical, LogicProfile, LogicalLoweringBudget},
    solver_coding_bridge::{dgcl_input::*, *},
    DgclBackendSet, DgclPipelineReport, TaskApplicability,
};
use lc631_core::{context::*, AuthorityRevision};

fn pipeline(source: &str) -> DgclPipelineReport {
    build_dgcl_pipeline(
        source,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap()
}

fn context(report: &DgclPipelineReport) -> LoweredContext {
    let pin = SourceRevision::of("TLCX28 declared input scope").digest();
    lower_dgcl_context(
        report,
        ContextScope::declared(pin, pin, pin, pin),
        AuthorityRevision(7),
        ContextLimits {
            max_contexts: 4,
            max_premises: 1024,
            max_edits: 4,
            max_statement_bytes: 262_144,
        },
    )
    .unwrap()
}

#[test]
fn tlcx28_dgcl_input_rejects_cut_or_replaced_legacy_task() {
    let mut report = pipeline("Please test the package.");
    let lowered = context(&report);
    assert_eq!(report.completion_plan.tasks.len(), 5);
    report.completion_plan.tasks.pop();
    assert!(matches!(
        DgclCodingInput::lower(&report, &lowered, LogicalLoweringBudget::default()),
        Err(CodingInputError::PipelineIdentity)
    ));
}

#[test]
fn tlcx28_dgcl_input_rejects_foreign_context_and_non_coding_task() {
    let report = pipeline("Please test the package.");
    let other = pipeline("Please verify the result.");
    let foreign = context(&other);
    assert!(DgclCodingInput::lower(&report, &foreign, LogicalLoweringBudget::default()).is_err());
    let own = context(&report);
    let mathematics = lower_dgcl_logical(
        &report,
        &own,
        TaskKind::Mathematics,
        LogicProfile::ClassicalPropositionalV1,
        LogicalLoweringBudget::default(),
    )
    .unwrap();
    assert!(matches!(
        DgclCodingInput::from_lowered(&report, mathematics),
        Err(CodingInputError::WrongTaskKind)
    ));
}

#[test]
fn tlcx28_dgcl_input_pairs_real_lowering_with_all_five_existing_tasks() {
    let report = pipeline("Please test the package.");
    let lowered = context(&report);
    let input =
        DgclCodingInput::lower(&report, &lowered, LogicalLoweringBudget::default()).unwrap();
    let bound = input.bind_requirement(0).unwrap();
    assert_eq!(bound.planned_tasks().len(), 5);
    for planned in bound.planned_tasks() {
        assert!(report
            .completion_plan
            .tasks
            .iter()
            .any(|original| original.task_id == planned.task_id && original == planned));
        assert_eq!(planned.applicability, TaskApplicability::Required);
        assert!(!planned.producer_revision.is_empty());
        assert!(!planned.acceptance_criterion.is_empty());
    }
    assert!(std::ptr::eq(
        bound.requirement(),
        &report.completion_plan.requirements[0]
    ));
    assert_eq!(bound.readiness(), CodingInputReadiness::ReadyForEvidence);
    assert!(matches!(
        input.bind_requirement(1),
        Err(CodingInputError::MissingRequirement)
    ));
    assert!(!report.completion_plan.coding_closure_allowed);
}

#[test]
fn tlcx28_condition_review_is_consumed_without_opening_legacy_validation() {
    let report = pipeline("Please test the package if ready.");
    let lowered = context(&report);
    let input =
        DgclCodingInput::lower(&report, &lowered, LogicalLoweringBudget::default()).unwrap();
    let bound = input.bind_requirement(0).unwrap();
    assert_eq!(
        bound.readiness(),
        CodingInputReadiness::NeedsConditionEvidence
    );
    let index = bound.condition_indices()[0];
    let request = ConditionRequest::new(input.logical_task(), index).unwrap();
    let reported = ConditionReport::reported(&request, ReportedRelation::UniversalSupport);
    let premises = [(request.predicate(), true)];
    let observation = check_condition(&request, &premises, &reported, 100_000).unwrap();
    let consumed = bound.review_condition(&observation).unwrap();
    assert_eq!(consumed.model_support, FiniteSupport::Supported);
    assert_eq!(consumed.report_check, ReportCheck::Agrees);
    assert_eq!(consumed.source_state, SourceConditionState::NeedsEvidence);
    assert_eq!(
        consumed.validation_readiness,
        CodingInputReadiness::NeedsConditionEvidence
    );
    let cut = check_condition(&request, &[], &reported, 100_000).unwrap();
    assert_eq!(
        bound.review_condition(&cut).unwrap().model_support,
        FiniteSupport::Undetermined
    );
    let unavailable = ConditionReport::reported(&request, ReportedRelation::Unavailable);
    let missing = check_condition(&request, &[], &unavailable, 100_000).unwrap();
    assert_eq!(
        bound.review_condition(&missing).unwrap().report_check,
        ReportCheck::Unavailable
    );
}

#[test]
fn tlcx28_foreign_condition_observation_cannot_enter_bound_requirement() {
    let report = pipeline("Please test the package if ready.");
    let lowered = context(&report);
    let first =
        DgclCodingInput::lower(&report, &lowered, LogicalLoweringBudget::default()).unwrap();
    let second =
        DgclCodingInput::lower(&report, &lowered, LogicalLoweringBudget::default()).unwrap();
    let request = ConditionRequest::new(second.logical_task(), 0).unwrap();
    let reported = ConditionReport::reported(&request, ReportedRelation::Sat);
    let observation = check_condition(&request, &[], &reported, 100_000).unwrap();
    assert!(matches!(
        first
            .bind_requirement(0)
            .unwrap()
            .review_condition(&observation),
        Err(CodingInputError::ForeignConditionObservation)
    ));
}

#[test]
fn tlcx28_alternative_action_is_not_ready_for_automatic_validation() {
    let report = pipeline("Please build or test.");
    let lowered = context(&report);
    let input =
        DgclCodingInput::lower(&report, &lowered, LogicalLoweringBudget::default()).unwrap();
    assert!(!report.completion_plan.requirements.is_empty());
    for index in 0..report.completion_plan.requirements.len() {
        assert_eq!(
            input.bind_requirement(index).unwrap().readiness(),
            CodingInputReadiness::NeedsInterpretation
        );
    }
}
