use crate::{CodingEvidenceKind, CodingEvidenceProducer, DgclInstructionParseState};
use crate::{
    ConditionEvidenceState, ConditionExpression, ConditionRelation, DgclProgramIrReport,
    DgclProgramResidual, DgclTranslationProjection, InstructionCondition, InstructionModality,
    InstructionPolarity,
};
use lc631_core::stable_sha256;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

pub const DGCL_COMPLETION_PLAN_SCHEMA: &str = "epistesys-dgcl-completion-plan.v1";
const MAX_COMPLETION_TASKS: usize = 8_192;

#[derive(Serialize)]
struct CompletionPlanDigestInput<'a> {
    schema_version: &'static str,
    source_revision: &'a str,
    program_ir_digest: &'a str,
    translation_projection_digest: &'a str,
    status: CompletionPlanStatus,
    requirements: &'a [PlannedCodingRequirement],
    tasks: &'a [PlannedEvidenceTask],
    blockers: &'a [CompletionBlocker],
    nonblocking_residual_count: usize,
    authority_created: bool,
    evidence_issued: bool,
    coding_closure_allowed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompletionPlanStatus {
    ReadyForEvidence,
    ConditionEvidencePending,
    NeedsClarification,
    EmptyRequirementSet,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompletionBlockerKind {
    EmptyRequirementSet,
    LegacyCandidateNeedsGrammar,
    AmbiguousInstructionParse,
    UnresolvedInstructionParse,
    UnsupportedInstructionParse,
    BudgetRejectedInstructionParse,
    NoActionAlternative,
    UnknownPolarityOrModality,
    ConditionEvidenceUnknown,
    ConditionScopeUnresolved,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CompletionBlocker {
    pub blocker_id: String,
    pub requirement_id: Option<String>,
    pub parent_program_requirement_id: Option<String>,
    pub source_span: Option<Range<usize>>,
    pub source_digest: Option<String>,
    pub kind: CompletionBlockerKind,
    pub reason: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskApplicability {
    Required,
    Conditional,
    NotApplicable,
    Unresolved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NotApplicableBasis {
    ForbiddenActionUsesAbsenceCheck,
    PermissionDoesNotRequestExecution,
}

pub type EvidenceProducer = CodingEvidenceProducer;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PlannedCondition {
    pub condition_id: String,
    pub source_span: Range<usize>,
    pub source_digest: String,
    pub marker_span: Range<usize>,
    pub marker_digest: String,
    pub relation: ConditionRelation,
    pub expression: ConditionExpression,
    pub scope_resolved: bool,
    pub truth_state: ConditionEvidenceState,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PlannedTargetCandidate {
    pub kind: crate::InstructionScopeKind,
    pub value: String,
    pub source_span: Range<usize>,
    pub state: crate::InstructionScopeState,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PlannedCodingRequirement {
    pub requirement_id: String,
    pub parent_program_requirement_id: String,
    pub alternative_id: String,
    pub region_id: u32,
    pub action_index: u32,
    pub source_span: Range<usize>,
    pub source_digest: String,
    pub action_text: String,
    pub modality: InstructionModality,
    pub polarity: InstructionPolarity,
    pub target_ref: String,
    pub target_candidates: Vec<PlannedTargetCandidate>,
    pub conditions: Vec<PlannedCondition>,
    pub tasks: Vec<PlannedEvidenceTask>,
    pub ready_for_evidence: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PlannedEvidenceTask {
    pub task_id: String,
    pub requirement_id: String,
    pub evidence_kind: CodingEvidenceKind,
    pub applicability: TaskApplicability,
    pub not_applicable_basis: Option<NotApplicableBasis>,
    pub producer: EvidenceProducer,
    pub producer_revision: String,
    pub target_ref: String,
    pub target_digest: String,
    pub acceptance_criterion: String,
    pub condition_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DgclCompletionPlan {
    pub schema_version: &'static str,
    pub source_revision: String,
    pub program_ir_digest: String,
    pub translation_projection_digest: String,
    pub plan_digest: String,
    pub status: CompletionPlanStatus,
    pub requirements: Vec<PlannedCodingRequirement>,
    pub tasks: Vec<PlannedEvidenceTask>,
    pub blockers: Vec<CompletionBlocker>,
    pub nonblocking_residual_count: usize,
    pub authority_created: bool,
    pub evidence_issued: bool,
    pub coding_closure_allowed: bool,
    pub claim_boundary: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum CompletionPlanError {
    SourceRevisionMismatch,
    ProgramProjectionMismatch,
    SourceSpanInvalid,
    SelectedAlternativeMissing,
    ActionTextMismatch,
    ConditionSpanInvalid,
    TaskBudgetExceeded,
    PlanDigestMismatch,
    PayloadEncoding,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskAssignmentDisposition {
    Scheduled,
    Deferred,
    NotApplicable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CompletionTaskAssignment {
    pub task_id: String,
    pub requirement_id: String,
    pub evidence_kind: CodingEvidenceKind,
    pub target_digest: String,
    pub disposition: TaskAssignmentDisposition,
    pub not_applicable_basis: Option<NotApplicableBasis>,
    pub condition_evidence_state: Option<ConditionEvidenceState>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompletionAuditIssueKind {
    ManifestDigestMismatch,
    MissingAssignment,
    UnknownRequirement,
    UnknownTask,
    DuplicateAssignment,
    UnrelatedTarget,
    ArbitraryNotApplicable,
    RequiredTaskDeferred,
    ConditionalTaskScheduled,
    ConditionEvidenceUnverified,
    UnresolvedTaskScheduled,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CompletionAuditIssue {
    pub task_id: Option<String>,
    pub requirement_id: Option<String>,
    pub kind: CompletionAuditIssueKind,
    pub reason: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompletionAuditState {
    ReadyForEvidence,
    ConditionBlocked,
    NeedsClarification,
    Incomplete,
    Rejected,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CompletionAuditReport {
    pub schema_version: &'static str,
    pub source_revision: String,
    pub plan_digest: String,
    pub state: CompletionAuditState,
    pub scheduled_count: usize,
    pub deferred_count: usize,
    pub not_applicable_count: usize,
    pub missing_assignment_count: usize,
    pub issues: Vec<CompletionAuditIssue>,
    pub evidence_complete: bool,
    pub coding_closure_allowed: bool,
    pub claim_boundary: &'static str,
}

pub fn build_dgcl_completion_plan(
    source: &str,
    program_ir: &DgclProgramIrReport,
    translation: &DgclTranslationProjection,
) -> Result<DgclCompletionPlan, CompletionPlanError> {
    let source_revision = stable_sha256(source);
    if source_revision != program_ir.source_revision
        || source_revision != translation.source_revision
    {
        return Err(CompletionPlanError::SourceRevisionMismatch);
    }
    if translation.program_ir_digest != program_ir.program_digest {
        return Err(CompletionPlanError::ProgramProjectionMismatch);
    }
    let translation_projection_digest = stable_sha256(
        &serde_json::to_string(translation).map_err(|_| CompletionPlanError::PayloadEncoding)?,
    );
    let mut requirements = Vec::new();
    let mut tasks = Vec::new();
    let mut blockers = Vec::new();

    if program_ir.requirements.is_empty() {
        blockers.push(blocker(
            "empty-requirement-set",
            None,
            None,
            None,
            None,
            CompletionBlockerKind::EmptyRequirementSet,
            "no supported instruction requirement or explicit no-op contract exists",
        ));
    }

    for residual in program_ir
        .residuals
        .iter()
        .filter(|residual| residual.blocking_requirement)
    {
        blockers.push(blocker_for_residual(
            residual,
            CompletionBlockerKind::LegacyCandidateNeedsGrammar,
            "legacy candidate has no selected controlled-grammar alternative; keep it open",
        ));
    }

    for program_requirement in &program_ir.requirements {
        if program_requirement.parse_state != DgclInstructionParseState::Parsed {
            let kind = match program_requirement.parse_state {
                DgclInstructionParseState::Ambiguous => {
                    CompletionBlockerKind::AmbiguousInstructionParse
                }
                DgclInstructionParseState::Unresolved => {
                    CompletionBlockerKind::UnresolvedInstructionParse
                }
                DgclInstructionParseState::Unsupported => {
                    CompletionBlockerKind::UnsupportedInstructionParse
                }
                DgclInstructionParseState::BudgetRejected => {
                    CompletionBlockerKind::BudgetRejectedInstructionParse
                }
                DgclInstructionParseState::Parsed => unreachable!(),
            };
            blockers.push(blocker(
                &program_requirement.requirement_id,
                None,
                Some(program_requirement.requirement_id.clone()),
                Some(program_requirement.source_span.clone()),
                Some(program_requirement.source_digest.clone()),
                kind,
                "no unique selected instruction AST is available for evidence planning",
            ));
            continue;
        }
        let selected_id = program_requirement
            .selected_alternative_id
            .as_deref()
            .ok_or(CompletionPlanError::SelectedAlternativeMissing)?;
        let alternative = program_requirement
            .alternatives
            .iter()
            .find(|alternative| alternative.alternative_id == selected_id)
            .ok_or(CompletionPlanError::SelectedAlternativeMissing)?;
        let ast = &alternative.ast;
        if ast.actions.is_empty() {
            blockers.push(blocker(
                &program_requirement.requirement_id,
                None,
                Some(program_requirement.requirement_id.clone()),
                Some(program_requirement.source_span.clone()),
                Some(program_requirement.source_digest.clone()),
                CompletionBlockerKind::NoActionAlternative,
                "selected AST contains no action to schedule",
            ));
            continue;
        }
        for (action_index, action) in ast.actions.iter().enumerate() {
            let action_index_u32 =
                u32::try_from(action_index).map_err(|_| CompletionPlanError::TaskBudgetExceeded)?;
            let relative_span = action.source_span.clone();
            let root_span = alternative.region_offset + relative_span.start
                ..alternative.region_offset + relative_span.end;
            let source_text = source
                .get(root_span.clone())
                .ok_or(CompletionPlanError::SourceSpanInvalid)?;
            if source_text != action.text {
                return Err(CompletionPlanError::ActionTextMismatch);
            }
            let source_digest = stable_sha256(source_text);
            let requirement_id = stable_sha256(&format!(
                "epistesys-dgcl-completion-requirement.v1\0{}\0{}\0{}\0{}\0{}\0{}",
                program_requirement.requirement_id,
                alternative.alternative_id,
                action_index,
                root_span.start,
                root_span.end,
                source_digest
            ));
            let target_ref = format!("dgcl/action/{requirement_id}");
            let condition_sources = ast
                .conditions
                .iter()
                .filter(|condition| condition.applies_to_action_indices.contains(&action_index))
                .map(|condition| planned_condition(source, alternative.region_offset, condition))
                .collect::<Result<Vec<_>, _>>()?;
            let target_candidates = ast
                .scopes
                .iter()
                .filter(|scope| scope.action_indices.contains(&action_index))
                .map(|scope| {
                    let span = alternative.region_offset + scope.source_span.start
                        ..alternative.region_offset + scope.source_span.end;
                    source
                        .get(span.clone())
                        .ok_or(CompletionPlanError::SourceSpanInvalid)?;
                    Ok(PlannedTargetCandidate {
                        kind: scope.kind,
                        value: scope.value.clone(),
                        source_span: span,
                        state: scope.state,
                    })
                })
                .collect::<Result<Vec<_>, CompletionPlanError>>()?;
            let known_action = action.polarity == InstructionPolarity::Positive
                || action.polarity == InstructionPolarity::Forbidden;
            let known_modality = action.modality != InstructionModality::Unknown;
            if !known_action || !known_modality {
                blockers.push(blocker(
                    &requirement_id,
                    Some(requirement_id.clone()),
                    Some(program_requirement.requirement_id.clone()),
                    Some(root_span.clone()),
                    Some(source_digest.clone()),
                    CompletionBlockerKind::UnknownPolarityOrModality,
                    "action polarity or modality is not resolved; do not choose a completion path",
                ));
            }
            for condition in &condition_sources {
                if condition.truth_state == ConditionEvidenceState::Unknown {
                    blockers.push(blocker(
                        &condition.condition_id,
                        Some(requirement_id.clone()),
                        Some(program_requirement.requirement_id.clone()),
                        Some(condition.source_span.clone()),
                        Some(condition.source_digest.clone()),
                        if condition.scope_resolved {
                            CompletionBlockerKind::ConditionEvidenceUnknown
                        } else {
                            CompletionBlockerKind::ConditionScopeUnresolved
                        },
                        "condition truth is not evidenced by its presence in the instruction text",
                    ));
                }
            }
            let mut requirement_tasks = Vec::new();
            for evidence_kind in evidence_kinds() {
                let (applicability, not_applicable_basis) = task_applicability(
                    evidence_kind,
                    action.polarity,
                    action.modality,
                    &condition_sources,
                );
                let producer = evidence_producer(evidence_kind);
                let acceptance_criterion =
                    acceptance_criterion(evidence_kind, action.polarity, &condition_sources);
                let condition_ids = condition_sources
                    .iter()
                    .map(|condition| condition.condition_id.clone())
                    .collect::<Vec<_>>();
                let task_id = stable_sha256(&format!(
                    "epistesys-dgcl-completion-task.v1\0{}\0{:?}\0{:?}\0{}\0{}",
                    requirement_id,
                    evidence_kind,
                    applicability,
                    source_digest,
                    condition_ids.join("\0")
                ));
                requirement_tasks.push(PlannedEvidenceTask {
                    task_id,
                    requirement_id: requirement_id.clone(),
                    evidence_kind,
                    applicability,
                    not_applicable_basis,
                    producer,
                    producer_revision: format!("epia2-13-completion-policy.v1/{producer:?}"),
                    target_ref: target_ref.clone(),
                    target_digest: source_digest.clone(),
                    acceptance_criterion,
                    condition_ids,
                });
            }
            let ready_for_evidence = known_action && known_modality && condition_sources.is_empty();
            requirements.push(PlannedCodingRequirement {
                requirement_id,
                parent_program_requirement_id: program_requirement.requirement_id.clone(),
                alternative_id: alternative.alternative_id.clone(),
                region_id: program_requirement.region_id,
                action_index: action_index_u32,
                source_span: root_span,
                source_digest,
                action_text: action.text.clone(),
                modality: action.modality,
                polarity: action.polarity,
                target_ref,
                target_candidates,
                conditions: condition_sources,
                tasks: requirement_tasks.clone(),
                ready_for_evidence,
            });
            tasks.extend(requirement_tasks);
        }
    }
    if tasks.len() > MAX_COMPLETION_TASKS {
        return Err(CompletionPlanError::TaskBudgetExceeded);
    }
    requirements.sort_by(|left, right| left.requirement_id.cmp(&right.requirement_id));
    tasks.sort_by(|left, right| left.task_id.cmp(&right.task_id));
    blockers.sort_by(|left, right| left.blocker_id.cmp(&right.blocker_id));
    let status = if program_ir.requirements.is_empty() {
        CompletionPlanStatus::EmptyRequirementSet
    } else if blockers.iter().any(|blocker| {
        !matches!(
            blocker.kind,
            CompletionBlockerKind::ConditionEvidenceUnknown
        )
    }) {
        CompletionPlanStatus::NeedsClarification
    } else if blockers
        .iter()
        .any(|blocker| blocker.kind == CompletionBlockerKind::ConditionEvidenceUnknown)
    {
        CompletionPlanStatus::ConditionEvidencePending
    } else {
        CompletionPlanStatus::ReadyForEvidence
    };
    let nonblocking_residual_count = program_ir
        .residuals
        .iter()
        .filter(|residual| !residual.blocking_requirement)
        .count();
    let plan_digest = completion_plan_digest(CompletionPlanDigestInput {
        schema_version: DGCL_COMPLETION_PLAN_SCHEMA,
        source_revision: &source_revision,
        program_ir_digest: &program_ir.program_digest,
        translation_projection_digest: &translation_projection_digest,
        status,
        requirements: &requirements,
        tasks: &tasks,
        blockers: &blockers,
        nonblocking_residual_count,
        authority_created: false,
        evidence_issued: false,
        coding_closure_allowed: false,
    })?;
    Ok(DgclCompletionPlan {
        schema_version: DGCL_COMPLETION_PLAN_SCHEMA,
        source_revision,
        program_ir_digest: program_ir.program_digest.clone(),
        translation_projection_digest,
        plan_digest,
        status,
        requirements,
        tasks,
        blockers,
        nonblocking_residual_count,
        authority_created: false,
        evidence_issued: false,
        coding_closure_allowed: false,
        claim_boundary: "prescriptive per-action evidence plan only; no evidence is issued, no condition is resolved, and no coding closure or authority is granted",
    })
}

pub fn verify_dgcl_completion_plan(
    source: &str,
    program_ir: &DgclProgramIrReport,
    translation: &DgclTranslationProjection,
    observed: &DgclCompletionPlan,
) -> Result<(), CompletionPlanError> {
    let expected = build_dgcl_completion_plan(source, program_ir, translation)?;
    if &expected == observed {
        Ok(())
    } else {
        Err(CompletionPlanError::PlanDigestMismatch)
    }
}

pub fn audit_completion_assignments(
    plan: &DgclCompletionPlan,
    assignments: &[CompletionTaskAssignment],
) -> CompletionAuditReport {
    let known_requirements = plan
        .requirements
        .iter()
        .map(|requirement| requirement.requirement_id.as_str())
        .collect::<BTreeSet<_>>();
    let expected = plan
        .tasks
        .iter()
        .map(|task| (task.task_id.as_str(), task))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    let mut issues = Vec::new();
    let mut scheduled_count = 0;
    let mut deferred_count = 0;
    let mut not_applicable_count = 0;
    let mut rejected = false;
    let mut incomplete = false;
    let manifest_digest_valid = plan.schema_version == DGCL_COMPLETION_PLAN_SCHEMA
        && completion_plan_digest(CompletionPlanDigestInput {
            schema_version: plan.schema_version,
            source_revision: &plan.source_revision,
            program_ir_digest: &plan.program_ir_digest,
            translation_projection_digest: &plan.translation_projection_digest,
            status: plan.status,
            requirements: &plan.requirements,
            tasks: &plan.tasks,
            blockers: &plan.blockers,
            nonblocking_residual_count: plan.nonblocking_residual_count,
            authority_created: plan.authority_created,
            evidence_issued: plan.evidence_issued,
            coding_closure_allowed: plan.coding_closure_allowed,
        })
        .is_ok_and(|expected| expected == plan.plan_digest);
    if !manifest_digest_valid {
        rejected = true;
        issues.push(audit_issue(
            None,
            None,
            CompletionAuditIssueKind::ManifestDigestMismatch,
            "completion plan content does not match its recorded digest",
        ));
    }
    for assignment in assignments {
        if !known_requirements.contains(assignment.requirement_id.as_str()) {
            rejected = true;
            issues.push(audit_issue(
                Some(&assignment.task_id),
                Some(&assignment.requirement_id),
                CompletionAuditIssueKind::UnknownRequirement,
                "assignment does not refer to a requirement in the plan",
            ));
            continue;
        }
        if !seen.insert(assignment.task_id.as_str()) {
            rejected = true;
            issues.push(audit_issue(
                Some(&assignment.task_id),
                Some(&assignment.requirement_id),
                CompletionAuditIssueKind::DuplicateAssignment,
                "one planned task may receive only one accounting response",
            ));
            continue;
        }
        let Some(task) = expected.get(assignment.task_id.as_str()).copied() else {
            rejected = true;
            issues.push(audit_issue(
                Some(&assignment.task_id),
                Some(&assignment.requirement_id),
                CompletionAuditIssueKind::UnknownTask,
                "task identifier is not declared by the completion manifest",
            ));
            continue;
        };
        if task.requirement_id != assignment.requirement_id
            || task.evidence_kind != assignment.evidence_kind
            || task.target_digest != assignment.target_digest
        {
            rejected = true;
            issues.push(audit_issue(
                Some(&assignment.task_id),
                Some(&assignment.requirement_id),
                CompletionAuditIssueKind::UnrelatedTarget,
                "assignment does not match the declared requirement, evidence kind, and target digest",
            ));
            continue;
        }
        if assignment
            .condition_evidence_state
            .is_some_and(|state| state != ConditionEvidenceState::Unknown)
        {
            rejected = true;
            issues.push(audit_issue(
                Some(&assignment.task_id),
                Some(&assignment.requirement_id),
                CompletionAuditIssueKind::ConditionEvidenceUnverified,
                "condition state cannot be promoted without a separate verified evidence producer",
            ));
        }
        match task.applicability {
            TaskApplicability::Required => match assignment.disposition {
                TaskAssignmentDisposition::Scheduled
                    if assignment.not_applicable_basis.is_none() =>
                {
                    scheduled_count += 1
                }
                TaskAssignmentDisposition::Scheduled => {
                    rejected = true;
                    issues.push(audit_issue(
                        Some(&assignment.task_id),
                        Some(&assignment.requirement_id),
                        CompletionAuditIssueKind::ArbitraryNotApplicable,
                        "a required task cannot carry a not-applicable basis",
                    ));
                }
                TaskAssignmentDisposition::NotApplicable => {
                    rejected = true;
                    issues.push(audit_issue(
                        Some(&assignment.task_id),
                        Some(&assignment.requirement_id),
                        CompletionAuditIssueKind::ArbitraryNotApplicable,
                        "required evidence cannot be relabeled as not applicable by the submitter",
                    ));
                }
                _ => {
                    incomplete = true;
                    issues.push(audit_issue(
                        Some(&assignment.task_id),
                        Some(&assignment.requirement_id),
                        CompletionAuditIssueKind::RequiredTaskDeferred,
                        "required task is not scheduled",
                    ));
                }
            },
            TaskApplicability::Conditional => match assignment.disposition {
                TaskAssignmentDisposition::Deferred
                    if assignment.not_applicable_basis.is_none() =>
                {
                    deferred_count += 1
                }
                TaskAssignmentDisposition::Deferred => {
                    rejected = true;
                    issues.push(audit_issue(
                        Some(&assignment.task_id),
                        Some(&assignment.requirement_id),
                        CompletionAuditIssueKind::ArbitraryNotApplicable,
                        "conditional task cannot carry a not-applicable basis",
                    ));
                }
                TaskAssignmentDisposition::Scheduled => {
                    rejected = true;
                    issues.push(audit_issue(
                        Some(&assignment.task_id),
                        Some(&assignment.requirement_id),
                        CompletionAuditIssueKind::ConditionalTaskScheduled,
                        "conditional task is deferred while condition truth remains unknown",
                    ));
                }
                TaskAssignmentDisposition::NotApplicable => {
                    rejected = true;
                    issues.push(audit_issue(
                        Some(&assignment.task_id),
                        Some(&assignment.requirement_id),
                        CompletionAuditIssueKind::ArbitraryNotApplicable,
                        "conditional evidence cannot be omitted while its condition is unknown",
                    ));
                }
            },
            TaskApplicability::Unresolved => match assignment.disposition {
                TaskAssignmentDisposition::Deferred
                    if assignment.not_applicable_basis.is_none() =>
                {
                    deferred_count += 1
                }
                TaskAssignmentDisposition::Deferred => {
                    rejected = true;
                    issues.push(audit_issue(
                        Some(&assignment.task_id),
                        Some(&assignment.requirement_id),
                        CompletionAuditIssueKind::ArbitraryNotApplicable,
                        "unresolved task cannot carry a not-applicable basis",
                    ));
                }
                TaskAssignmentDisposition::NotApplicable => {
                    rejected = true;
                    issues.push(audit_issue(
                        Some(&assignment.task_id),
                        Some(&assignment.requirement_id),
                        CompletionAuditIssueKind::ArbitraryNotApplicable,
                        "unresolved requirement cannot be declared not applicable",
                    ));
                }
                TaskAssignmentDisposition::Scheduled => {
                    rejected = true;
                    issues.push(audit_issue(
                        Some(&assignment.task_id),
                        Some(&assignment.requirement_id),
                        CompletionAuditIssueKind::UnresolvedTaskScheduled,
                        "unresolved requirement must remain deferred for clarification",
                    ));
                }
            },
            TaskApplicability::NotApplicable => {
                if assignment.disposition == TaskAssignmentDisposition::NotApplicable
                    && assignment.not_applicable_basis == task.not_applicable_basis
                    && task.not_applicable_basis.is_some()
                {
                    not_applicable_count += 1;
                } else {
                    rejected = true;
                    issues.push(audit_issue(
                        Some(&assignment.task_id),
                        Some(&assignment.requirement_id),
                        CompletionAuditIssueKind::ArbitraryNotApplicable,
                        "not-applicable disposition must use the exact predeclared policy basis",
                    ));
                }
            }
        }
    }
    let mut missing_assignment_count = 0;
    for task in &plan.tasks {
        if !seen.contains(task.task_id.as_str()) {
            missing_assignment_count += 1;
            incomplete = true;
            issues.push(audit_issue(
                Some(&task.task_id),
                Some(&task.requirement_id),
                CompletionAuditIssueKind::MissingAssignment,
                "declared evidence task has no accounting response",
            ));
        }
    }
    let state = if rejected {
        CompletionAuditState::Rejected
    } else if incomplete || missing_assignment_count > 0 {
        CompletionAuditState::Incomplete
    } else if matches!(
        plan.status,
        CompletionPlanStatus::NeedsClarification | CompletionPlanStatus::EmptyRequirementSet
    ) {
        CompletionAuditState::NeedsClarification
    } else if plan.status == CompletionPlanStatus::ConditionEvidencePending || deferred_count > 0 {
        CompletionAuditState::ConditionBlocked
    } else {
        CompletionAuditState::ReadyForEvidence
    };
    CompletionAuditReport {
        schema_version: "epistesys-dgcl-completion-audit.v1",
        source_revision: plan.source_revision.clone(),
        plan_digest: plan.plan_digest.clone(),
        state,
        scheduled_count,
        deferred_count,
        not_applicable_count,
        missing_assignment_count,
        issues,
        evidence_complete: false,
        coding_closure_allowed: false,
        claim_boundary: "assignment accounting only; scheduling is not evidence, validation, authority, or coding closure",
    }
}

fn planned_condition(
    source: &str,
    region_offset: usize,
    condition: &InstructionCondition,
) -> Result<PlannedCondition, CompletionPlanError> {
    let source_span =
        region_offset + condition.source_span.start..region_offset + condition.source_span.end;
    let condition_text = source
        .get(source_span.clone())
        .ok_or(CompletionPlanError::ConditionSpanInvalid)?;
    let marker_span =
        region_offset + condition.marker_span.start..region_offset + condition.marker_span.end;
    let marker_text = source
        .get(marker_span.clone())
        .ok_or(CompletionPlanError::ConditionSpanInvalid)?;
    let expression = rebase_condition_expression(source, region_offset, &condition.expression)?;
    let condition_id = stable_sha256(&format!(
        "epistesys-dgcl-completion-condition.v1\0{}\0{}\0{:?}",
        source_span.start, source_span.end, condition.relation
    ));
    Ok(PlannedCondition {
        condition_id,
        source_span,
        source_digest: stable_sha256(condition_text),
        marker_span,
        marker_digest: stable_sha256(marker_text),
        relation: condition.relation,
        expression,
        scope_resolved: condition.scope_state == crate::ConditionScopeState::Resolved,
        truth_state: condition.evidence_state,
    })
}

fn rebase_condition_expression(
    source: &str,
    region_offset: usize,
    expression: &ConditionExpression,
) -> Result<ConditionExpression, CompletionPlanError> {
    let rebase_predicate = |predicate: &crate::ConditionPredicate| {
        let span =
            region_offset + predicate.source_span.start..region_offset + predicate.source_span.end;
        let text = source
            .get(span.clone())
            .ok_or(CompletionPlanError::ConditionSpanInvalid)?;
        if text != predicate.text {
            return Err(CompletionPlanError::ConditionSpanInvalid);
        }
        Ok(crate::ConditionPredicate {
            source_span: span,
            text: text.into(),
        })
    };
    Ok(match expression {
        ConditionExpression::Predicate(predicate) => {
            ConditionExpression::Predicate(rebase_predicate(predicate)?)
        }
        ConditionExpression::Unresolved(predicate) => {
            ConditionExpression::Unresolved(rebase_predicate(predicate)?)
        }
        ConditionExpression::All(children) => ConditionExpression::All(
            children
                .iter()
                .map(|child| rebase_condition_expression(source, region_offset, child))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        ConditionExpression::Any(children) => ConditionExpression::Any(
            children
                .iter()
                .map(|child| rebase_condition_expression(source, region_offset, child))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        ConditionExpression::Not(child) => ConditionExpression::Not(Box::new(
            rebase_condition_expression(source, region_offset, child)?,
        )),
    })
}

fn task_applicability(
    kind: CodingEvidenceKind,
    polarity: InstructionPolarity,
    modality: InstructionModality,
    conditions: &[PlannedCondition],
) -> (TaskApplicability, Option<NotApplicableBasis>) {
    if polarity == InstructionPolarity::Unknown
        || polarity == InstructionPolarity::Conflict
        || modality == InstructionModality::Unknown
    {
        return (TaskApplicability::Unresolved, None);
    }
    if kind == CodingEvidenceKind::RuntimeValidation {
        if polarity == InstructionPolarity::Forbidden {
            return (
                TaskApplicability::NotApplicable,
                Some(NotApplicableBasis::ForbiddenActionUsesAbsenceCheck),
            );
        }
        if modality == InstructionModality::Permitted {
            return (
                TaskApplicability::NotApplicable,
                Some(NotApplicableBasis::PermissionDoesNotRequestExecution),
            );
        }
    }
    if !conditions.is_empty()
        && matches!(
            kind,
            CodingEvidenceKind::RuntimeValidation | CodingEvidenceKind::AcceptanceTest
        )
    {
        (TaskApplicability::Conditional, None)
    } else {
        (TaskApplicability::Required, None)
    }
}

fn evidence_kinds() -> [CodingEvidenceKind; 5] {
    [
        CodingEvidenceKind::ImplementationBinding,
        CodingEvidenceKind::ProductionConnection,
        CodingEvidenceKind::StaticValidation,
        CodingEvidenceKind::RuntimeValidation,
        CodingEvidenceKind::AcceptanceTest,
    ]
}

fn evidence_producer(kind: CodingEvidenceKind) -> CodingEvidenceProducer {
    match kind {
        CodingEvidenceKind::ImplementationBinding => CodingEvidenceProducer::StructuralObservation,
        CodingEvidenceKind::ProductionConnection => {
            CodingEvidenceProducer::DeclaredConnectionObserver
        }
        CodingEvidenceKind::StaticValidation => CodingEvidenceProducer::CompilerObserved,
        CodingEvidenceKind::RuntimeValidation => CodingEvidenceProducer::RuntimeObserved,
        CodingEvidenceKind::AcceptanceTest => CodingEvidenceProducer::AcceptanceOracleObserved,
    }
}

fn acceptance_criterion(
    kind: CodingEvidenceKind,
    polarity: InstructionPolarity,
    conditions: &[PlannedCondition],
) -> String {
    let condition_clause = if conditions.is_empty() {
        "no unresolved condition scope".to_string()
    } else {
        "condition source and truth evidence must be independently resolved before conditional execution".to_string()
    };
    if polarity == InstructionPolarity::Forbidden {
        let criterion = match kind {
            CodingEvidenceKind::ImplementationBinding => {
                "bind a change or explicit absence target to the forbidden action span"
            }
            CodingEvidenceKind::ProductionConnection => {
                "show the declared path cannot perform the forbidden side effect"
            }
            CodingEvidenceKind::StaticValidation => {
                "run a bounded static absence/guard check; do not execute the forbidden action"
            }
            CodingEvidenceKind::RuntimeValidation => {
                "not applicable: observe absence through non-mutating guard evidence only"
            }
            CodingEvidenceKind::AcceptanceTest => {
                "independent negative oracle asserts the forbidden effect is absent"
            }
        };
        return if conditions.is_empty() || kind == CodingEvidenceKind::RuntimeValidation {
            criterion.into()
        } else {
            format!("{criterion}; {condition_clause}")
        };
    }
    match kind {
        CodingEvidenceKind::ImplementationBinding => "bind the implementation target and source revision to this action requirement".into(),
        CodingEvidenceKind::ProductionConnection => "observe the declared producer-to-consumer route for this target".into(),
        CodingEvidenceKind::StaticValidation => "run the declared current static/compiler check for the bound target".into(),
        CodingEvidenceKind::RuntimeValidation => format!("bind a bounded runtime trace to the exact executable, input, and target; {condition_clause}"),
        CodingEvidenceKind::AcceptanceTest => format!("check an independently authored acceptance oracle against this requirement; {condition_clause}"),
    }
}

fn blocker_for_residual(
    residual: &DgclProgramResidual,
    kind: CompletionBlockerKind,
    reason: &str,
) -> CompletionBlocker {
    blocker(
        &residual.residual_id,
        None,
        None,
        Some(residual.source_span.clone()),
        Some(residual.source_digest.clone()),
        kind,
        reason,
    )
}

fn blocker(
    id: &str,
    requirement_id: Option<String>,
    parent_program_requirement_id: Option<String>,
    source_span: Option<Range<usize>>,
    source_digest: Option<String>,
    kind: CompletionBlockerKind,
    reason: &str,
) -> CompletionBlocker {
    CompletionBlocker {
        blocker_id: stable_sha256(&format!("epistesys-dgcl-plan-blocker.v1\0{id}\0{kind:?}")),
        requirement_id,
        parent_program_requirement_id,
        source_span,
        source_digest,
        kind,
        reason: reason.into(),
    }
}

fn completion_plan_digest(
    input: CompletionPlanDigestInput<'_>,
) -> Result<String, CompletionPlanError> {
    let payload =
        serde_json::to_string(&input).map_err(|_| CompletionPlanError::PayloadEncoding)?;
    Ok(stable_sha256(&payload))
}

fn audit_issue(
    task_id: Option<&str>,
    requirement_id: Option<&str>,
    kind: CompletionAuditIssueKind,
    reason: &'static str,
) -> CompletionAuditIssue {
    CompletionAuditIssue {
        task_id: task_id.map(str::to_owned),
        requirement_id: requirement_id.map(str::to_owned),
        kind,
        reason,
    }
}
