#![forbid(unsafe_code)]

use lc631_analysis::{
    audit_completion_assignments, build_dgcl_pipeline, CompletionAuditIssueKind,
    CompletionAuditState, DgclBackendSet, DgclInstructionParseState, NotApplicableBasis,
    TaskApplicability, TaskAssignmentDisposition,
};

fn report(source: &str) -> lc631_analysis::DgclPipelineReport {
    build_dgcl_pipeline(
        source,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap()
}

fn planned_assignments(
    plan: &lc631_analysis::DgclCompletionPlan,
) -> Vec<lc631_analysis::CompletionTaskAssignment> {
    plan.tasks
        .iter()
        .map(|task| lc631_analysis::CompletionTaskAssignment {
            task_id: task.task_id.clone(),
            requirement_id: task.requirement_id.clone(),
            evidence_kind: task.evidence_kind,
            target_digest: task.target_digest.clone(),
            disposition: match task.applicability {
                TaskApplicability::Required => TaskAssignmentDisposition::Scheduled,
                TaskApplicability::Conditional | TaskApplicability::Unresolved => {
                    TaskAssignmentDisposition::Deferred
                }
                TaskApplicability::NotApplicable => TaskAssignmentDisposition::NotApplicable,
            },
            not_applicable_basis: task.not_applicable_basis,
            condition_evidence_state: None,
        })
        .collect()
}

#[test]
fn completion_plan_freezes_all_evidence_kinds_producers_targets_and_acceptance_rules() {
    let report = report("Please test the package.");
    let plan = &report.completion_plan;
    assert_eq!(plan.requirements.len(), 1);
    assert_eq!(plan.tasks.len(), 5);
    assert!(plan.tasks.iter().all(|task| {
        !task.producer_revision.is_empty()
            && !task.acceptance_criterion.is_empty()
            && task.target_digest.starts_with("sha256:")
            && task.target_ref.starts_with("dgcl/action/")
    }));
    assert_eq!(
        plan.status,
        lc631_analysis::CompletionPlanStatus::ReadyForEvidence
    );
    assert!(!plan.evidence_issued);
    assert!(!plan.coding_closure_allowed);
    assert!(!plan.authority_created);

    let audit = audit_completion_assignments(plan, &planned_assignments(plan));
    assert_eq!(audit.state, CompletionAuditState::ReadyForEvidence);
    assert!(!audit.coding_closure_allowed);
    assert!(audit.issues.is_empty());
}

#[test]
fn nonblocking_context_residual_does_not_poison_an_independent_action_plan() {
    let report = report("Please test the package.\n\nThis is background context only.");
    assert_eq!(report.program_ir.requirements.len(), 1);
    assert_eq!(report.completion_plan.nonblocking_residual_count, 1);
    assert_eq!(report.completion_plan.blockers.len(), 0);
    assert_eq!(
        report.completion_plan.status,
        lc631_analysis::CompletionPlanStatus::ReadyForEvidence
    );
    assert_eq!(
        audit_completion_assignments(
            &report.completion_plan,
            &planned_assignments(&report.completion_plan)
        )
        .state,
        CompletionAuditState::ReadyForEvidence
    );
}

#[test]
fn forbidden_and_permission_only_actions_use_declared_absence_not_arbitrary_na() {
    let forbidden = report("Do not delete the database.");
    let runtime = forbidden
        .completion_plan
        .tasks
        .iter()
        .find(|task| task.evidence_kind == lc631_analysis::CodingEvidenceKind::RuntimeValidation)
        .unwrap();
    assert_eq!(runtime.applicability, TaskApplicability::NotApplicable);
    assert_eq!(
        runtime.not_applicable_basis,
        Some(NotApplicableBasis::ForbiddenActionUsesAbsenceCheck)
    );
    assert!(forbidden.completion_plan.tasks.iter().any(|task| {
        task.evidence_kind == lc631_analysis::CodingEvidenceKind::StaticValidation
            && task.applicability == TaskApplicability::Required
    }));
    assert!(forbidden.completion_plan.tasks.iter().any(|task| {
        task.evidence_kind == lc631_analysis::CodingEvidenceKind::AcceptanceTest
            && task.acceptance_criterion.contains("absent")
    }));
    assert_eq!(
        audit_completion_assignments(
            &forbidden.completion_plan,
            &planned_assignments(&forbidden.completion_plan)
        )
        .state,
        CompletionAuditState::ReadyForEvidence
    );

    let conditional_forbidden = report("Do not publish unless tests pass.");
    let conditional_runtime = conditional_forbidden
        .completion_plan
        .tasks
        .iter()
        .find(|task| task.evidence_kind == lc631_analysis::CodingEvidenceKind::RuntimeValidation)
        .unwrap();
    assert_eq!(
        conditional_runtime.applicability,
        TaskApplicability::NotApplicable
    );
    let conditional_acceptance = conditional_forbidden
        .completion_plan
        .tasks
        .iter()
        .find(|task| task.evidence_kind == lc631_analysis::CodingEvidenceKind::AcceptanceTest)
        .unwrap();
    assert_eq!(
        conditional_acceptance.applicability,
        TaskApplicability::Conditional
    );
    assert!(conditional_acceptance
        .acceptance_criterion
        .contains("condition source and truth evidence"));
    assert!(!conditional_forbidden.completion_plan.coding_closure_allowed);

    let permission = report("May publish the package.");
    let permission_runtime = permission
        .completion_plan
        .tasks
        .iter()
        .find(|task| task.evidence_kind == lc631_analysis::CodingEvidenceKind::RuntimeValidation)
        .unwrap();
    assert_eq!(
        permission_runtime.applicability,
        TaskApplicability::NotApplicable
    );
    assert_eq!(
        permission_runtime.not_applicable_basis,
        Some(NotApplicableBasis::PermissionDoesNotRequestExecution)
    );
}

#[test]
fn missing_duplicate_unknown_or_unrelated_assignments_cannot_complete_the_plan() {
    let baseline = report("Please test this package.");
    let plan = &baseline.completion_plan;
    let mut missing = planned_assignments(plan);
    missing.pop();
    let audit = audit_completion_assignments(plan, &missing);
    assert_eq!(audit.state, CompletionAuditState::Incomplete);
    assert!(audit
        .issues
        .iter()
        .any(|issue| issue.kind == CompletionAuditIssueKind::MissingAssignment));

    let mut arbitrary_na = planned_assignments(plan);
    let runtime = arbitrary_na
        .iter_mut()
        .find(|answer| {
            answer.evidence_kind == lc631_analysis::CodingEvidenceKind::RuntimeValidation
        })
        .unwrap();
    runtime.disposition = TaskAssignmentDisposition::NotApplicable;
    runtime.not_applicable_basis = Some(NotApplicableBasis::ForbiddenActionUsesAbsenceCheck);
    assert!(audit_completion_assignments(plan, &arbitrary_na)
        .issues
        .iter()
        .any(|issue| issue.kind == CompletionAuditIssueKind::ArbitraryNotApplicable));

    let mut forged_plan = plan.clone();
    let runtime_task = forged_plan
        .tasks
        .iter_mut()
        .find(|task| task.evidence_kind == lc631_analysis::CodingEvidenceKind::RuntimeValidation)
        .unwrap();
    runtime_task.applicability = TaskApplicability::NotApplicable;
    runtime_task.not_applicable_basis = Some(NotApplicableBasis::ForbiddenActionUsesAbsenceCheck);
    assert_eq!(
        lc631_analysis::verify_dgcl_completion_plan(
            "Please test this package.",
            &baseline.program_ir,
            &baseline.translation_projection,
            &forged_plan,
        ),
        Err(lc631_analysis::CompletionPlanError::PlanDigestMismatch)
    );
    let forged_audit =
        audit_completion_assignments(&forged_plan, &planned_assignments(&forged_plan));
    assert_eq!(forged_audit.state, CompletionAuditState::Rejected);
    assert!(forged_audit
        .issues
        .iter()
        .any(|issue| issue.kind == CompletionAuditIssueKind::ManifestDigestMismatch));

    let mut unknown = planned_assignments(plan);
    unknown[0].requirement_id =
        "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".into();
    assert!(audit_completion_assignments(plan, &unknown)
        .issues
        .iter()
        .any(|issue| issue.kind == CompletionAuditIssueKind::UnknownRequirement));

    let mut unrelated = planned_assignments(plan);
    unrelated[0].target_digest =
        "sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".into();
    assert!(audit_completion_assignments(plan, &unrelated)
        .issues
        .iter()
        .any(|issue| issue.kind == CompletionAuditIssueKind::UnrelatedTarget));

    let mut duplicate = planned_assignments(plan);
    duplicate.push(duplicate[0].clone());
    assert!(audit_completion_assignments(plan, &duplicate)
        .issues
        .iter()
        .any(|issue| issue.kind == CompletionAuditIssueKind::DuplicateAssignment));
}

#[test]
fn unknown_condition_is_deferred_and_cannot_be_claimed_satisfied() {
    let source = "Context only.\n\nPlease publish if tests pass.";
    let report = report(source);
    let plan = &report.completion_plan;
    assert_eq!(
        plan.status,
        lc631_analysis::CompletionPlanStatus::ConditionEvidencePending
    );
    let runtime = plan
        .tasks
        .iter()
        .find(|task| task.evidence_kind == lc631_analysis::CodingEvidenceKind::RuntimeValidation)
        .unwrap();
    assert_eq!(runtime.applicability, TaskApplicability::Conditional);
    let condition = &plan.requirements[0].conditions[0];
    assert!(condition.source_span.start >= "Context only.\n\n".len());
    assert!(source.get(condition.source_span.clone()).is_some());
    let predicate = condition.expression.predicates()[0];
    assert!(predicate.source_span.start >= condition.source_span.start);
    assert_eq!(
        source.get(predicate.source_span.clone()),
        Some(predicate.text.as_str())
    );
    let deferred = planned_assignments(plan);
    let audit = audit_completion_assignments(plan, &deferred);
    assert_eq!(audit.state, CompletionAuditState::ConditionBlocked);
    assert!(!audit.coding_closure_allowed);

    let mut claimed_supported = deferred;
    let conditional = claimed_supported
        .iter_mut()
        .find(|answer| {
            answer.evidence_kind == lc631_analysis::CodingEvidenceKind::RuntimeValidation
        })
        .unwrap();
    conditional.condition_evidence_state = Some(lc631_analysis::ConditionEvidenceState::Supported);
    assert!(audit_completion_assignments(plan, &claimed_supported)
        .issues
        .iter()
        .any(|issue| issue.kind == CompletionAuditIssueKind::ConditionEvidenceUnverified));
}

#[test]
fn ambiguous_or_legacy_only_requirements_remain_blockers_without_invented_tasks() {
    let ambiguous = report("Please testしてください");
    assert!(ambiguous.completion_plan.tasks.is_empty());
    assert!(!ambiguous.completion_plan.blockers.is_empty());
    assert_eq!(
        audit_completion_assignments(&ambiguous.completion_plan, &[]).state,
        CompletionAuditState::NeedsClarification
    );

    let legacy = report("Please delete files.");
    let mut projection = legacy.clone();
    projection.instruction_parses[0].report = None;
    projection.instruction_parses[0].state = DgclInstructionParseState::BudgetRejected;
    projection.instruction_parses[0].error =
        Some(lc631_analysis::InstructionGrammarError::TokenBudgetExceeded);
    let (program_ir, translation_projection) = lc631_analysis::build_dgcl_projection(
        "Please delete files.",
        &projection.dg1,
        &projection.language_regions,
        &projection.instruction_parses,
    )
    .unwrap();
    let plan = lc631_analysis::build_dgcl_completion_plan(
        "Please delete files.",
        &program_ir,
        &translation_projection,
    )
    .unwrap();
    assert!(plan.tasks.is_empty());
    assert!(plan.blockers.iter().any(|blocker| blocker.kind
        == lc631_analysis::CompletionBlockerKind::LegacyCandidateNeedsGrammar));
}
