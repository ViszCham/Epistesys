use crate::{
    coding_evidence_payload_digest, coding_evidence_scope, verify_dgcl_completion_plan,
    CodingEvidenceClaim, CodingEvidenceKind, CompletionBlocker, CompletionBlockerKind,
    CompletionPlanStatus, DgclCompletionPlan, DgclExecutionSnapshot, DgclProgramIrReport,
    DgclTranslationProjection, NotApplicableBasis, PlannedCodingRequirement, PlannedEvidenceTask,
    TaskApplicability,
};
use lc631_core::stable_sha256;
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptPolicy, ReceiptScope, ReceiptVerifier, ReplayGuard, SubjectRevision,
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

pub const DGCL_IMPLEMENTATION_CLOSURE_SCHEMA: &str = "epistesys-dgcl-implementation-closure.v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DgclImplementationStatus {
    ImplementationClosed,
    NoAction,
    Hold,
    Clarify,
    Conflict,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImplementationGapState {
    VerifiedReceipt,
    NotApplicable,
    Unavailable,
    Rejected,
    Stale,
    ConditionalPending,
    Unresolved,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DgclImplementationGap {
    pub gap_id: String,
    pub requirement_id: Option<String>,
    pub evidence_task_id: Option<String>,
    pub evidence_kind: Option<CodingEvidenceKind>,
    pub state: ImplementationGapState,
    pub applicability: Option<TaskApplicability>,
    pub not_applicable_basis: Option<NotApplicableBasis>,
    pub target_ref: Option<String>,
    pub target_digest: Option<String>,
    pub evidence_digest: Option<String>,
    pub reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DgclRequirementClosure {
    pub requirement_id: String,
    pub parent_program_requirement_id: String,
    pub source_span: std::ops::Range<usize>,
    pub status: DgclImplementationStatus,
    pub gap_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DgclEvidenceRejection {
    pub claim_index: usize,
    pub coding_requirement_id: Option<String>,
    pub coding_task_id: Option<String>,
    pub evidence_kind: CodingEvidenceKind,
    pub reason: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DgclImplementationClosureReport {
    pub schema_version: &'static str,
    pub source_revision: String,
    pub program_ir_digest: String,
    pub completion_plan_digest: String,
    pub requirement_count: usize,
    pub gaps: Vec<DgclImplementationGap>,
    pub blockers: Vec<CompletionBlocker>,
    pub rejected_claim_count: usize,
    pub rejected_claims: Vec<DgclEvidenceRejection>,
    pub duplicate_claim_count: usize,
    pub requirements: Vec<DgclRequirementClosure>,
    pub status: DgclImplementationStatus,
    pub evidence_complete_for_declared_requirements: bool,
    pub implementation_complete_candidate: bool,
    pub authority_created: bool,
    pub output_commit_allowed: bool,
    pub claim_boundary: &'static str,
}

pub struct DgclImplementationClosureContext<'a> {
    pub current_lifecycle: Option<&'a DgclExecutionSnapshot>,
    pub verifier: Option<&'a ReceiptVerifier>,
    pub replay: &'a mut ReplayGuard,
    pub now_epoch: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum DgclImplementationClosureError {
    SourceRevisionMismatch,
    ProgramProjectionMismatch,
    CompletionPlanMismatch,
    DuplicateRequirementId,
    DuplicateTaskId,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct EvidenceKey(String);

pub fn build_dgcl_implementation_closure(
    source: &str,
    program_ir: &DgclProgramIrReport,
    translation: &DgclTranslationProjection,
    plan: &DgclCompletionPlan,
    claims: &[CodingEvidenceClaim],
    context: DgclImplementationClosureContext<'_>,
) -> Result<DgclImplementationClosureReport, DgclImplementationClosureError> {
    let DgclImplementationClosureContext {
        current_lifecycle,
        verifier,
        replay,
        now_epoch,
    } = context;
    let source_revision = stable_sha256(source);
    if source_revision != program_ir.source_revision || source_revision != plan.source_revision {
        return Err(DgclImplementationClosureError::SourceRevisionMismatch);
    }
    if translation.program_ir_digest != program_ir.program_digest
        || translation.source_revision != source_revision
        || plan.program_ir_digest != program_ir.program_digest
    {
        return Err(DgclImplementationClosureError::ProgramProjectionMismatch);
    }
    verify_dgcl_completion_plan(source, program_ir, translation, plan)
        .map_err(|_| DgclImplementationClosureError::CompletionPlanMismatch)?;

    let mut requirements = BTreeMap::<String, &PlannedCodingRequirement>::new();
    for requirement in &plan.requirements {
        if requirements
            .insert(requirement.requirement_id.clone(), requirement)
            .is_some()
        {
            return Err(DgclImplementationClosureError::DuplicateRequirementId);
        }
    }
    let mut tasks = BTreeMap::<String, &PlannedEvidenceTask>::new();
    for task in &plan.tasks {
        if tasks.insert(task.task_id.clone(), task).is_some() {
            return Err(DgclImplementationClosureError::DuplicateTaskId);
        }
    }

    let mut claim_counts = BTreeMap::<EvidenceKey, usize>::new();
    for claim in claims {
        if let Some(task_id) = &claim.coding_task_id {
            *claim_counts
                .entry(EvidenceKey(task_id.clone()))
                .or_default() += 1;
        }
    }
    let duplicate_claim_count = claim_counts.values().filter(|count| **count > 1).count();
    let mut accepted = BTreeMap::<String, (String, String)>::new();
    let mut rejected_tasks = BTreeSet::<String>::new();
    let mut stale_tasks = BTreeSet::<String>::new();
    let mut rejected_claims = Vec::new();
    for (claim_index, claim) in claims.iter().enumerate() {
        let task_id = claim.coding_task_id.as_deref();
        let requirement_id = claim.coding_requirement_id.as_deref();
        let known_requirement = requirement_id.is_some_and(|id| requirements.contains_key(id));
        let unknown_task = task_id.is_none_or(|id| !tasks.contains_key(id));
        let reason = if !known_requirement {
            Some("unknown_coding_requirement_id")
        } else if unknown_task {
            Some("unknown_coding_task_id")
        } else if claim_counts.get(&EvidenceKey(task_id.unwrap().to_string())) != Some(&1) {
            Some("duplicate_coding_task_claim")
        } else {
            let task = tasks
                .get(task_id.unwrap())
                .copied()
                .expect("task id checked");
            if task.requirement_id != requirement_id.unwrap()
                || task.evidence_kind != claim.kind
                || task.target_ref != claim.target
                || task.target_digest != claim.target_digest
            {
                Some("coding_task_binding_mismatch")
            } else if task.applicability == TaskApplicability::NotApplicable {
                Some("receipt_supplied_for_not_applicable_task")
            } else if task.applicability == TaskApplicability::Unresolved {
                Some("receipt_cannot_resolve_unparsed_task")
            } else if claim.producer != Some(task.producer)
                || !claim.producer_run_digest.as_deref().is_some_and(is_digest)
                || !claim.observation_digest.as_deref().is_some_and(is_digest)
            {
                Some("producer_policy_or_observation_binding_mismatch")
            } else if claim.source_revision != source_revision {
                Some("source_revision_mismatch")
            } else {
                lifecycle_rejection(
                    claim,
                    current_lifecycle,
                    &claim.target,
                    &claim.target_digest,
                    now_epoch,
                )
            }
        };
        if let Some(reason) = reason {
            if let Some(task_id) = task_id {
                if tasks.contains_key(task_id) {
                    if is_stale_lifecycle_reason(reason) {
                        stale_tasks.insert(task_id.to_string());
                    } else {
                        rejected_tasks.insert(task_id.to_string());
                    }
                }
            }
            rejected_claims.push(DgclEvidenceRejection {
                claim_index,
                coding_requirement_id: claim.coding_requirement_id.clone(),
                coding_task_id: claim.coding_task_id.clone(),
                evidence_kind: claim.kind,
                reason,
            });
            continue;
        }
        let task_id = task_id.expect("task id checked");
        let (Some(verifier), Some(attestation)) = (verifier, claim.attestation.clone()) else {
            rejected_tasks.insert(task_id.to_string());
            rejected_claims.push(DgclEvidenceRejection {
                claim_index,
                coding_requirement_id: claim.coding_requirement_id.clone(),
                coding_task_id: claim.coding_task_id.clone(),
                evidence_kind: claim.kind,
                reason: "trusted_verifier_or_receipt_unavailable",
            });
            continue;
        };
        let Ok(subject) = SubjectRevision::checked(source_revision.clone()) else {
            rejected_tasks.insert(task_id.to_string());
            rejected_claims.push(DgclEvidenceRejection {
                claim_index,
                coding_requirement_id: claim.coding_requirement_id.clone(),
                coding_task_id: claim.coding_task_id.clone(),
                evidence_kind: claim.kind,
                reason: "source_subject_invalid",
            });
            continue;
        };
        let Ok(scope) = ReceiptScope::checked(coding_evidence_scope(claim)) else {
            rejected_tasks.insert(task_id.to_string());
            rejected_claims.push(DgclEvidenceRejection {
                claim_index,
                coding_requirement_id: claim.coding_requirement_id.clone(),
                coding_task_id: claim.coding_task_id.clone(),
                evidence_kind: claim.kind,
                reason: "receipt_scope_invalid",
            });
            continue;
        };
        match verifier.verify(
            attestation,
            &ReceiptPolicy::exact(
                ReceiptClass::Closure,
                subject,
                scope,
                coding_evidence_payload_digest(claim),
                now_epoch,
            ),
            replay,
        ) {
            Ok(receipt) => {
                accepted.insert(
                    task_id.to_string(),
                    (receipt.receipt_digest(), claim.target.clone()),
                );
            }
            Err(_) => {
                rejected_tasks.insert(task_id.to_string());
                rejected_claims.push(DgclEvidenceRejection {
                    claim_index,
                    coding_requirement_id: claim.coding_requirement_id.clone(),
                    coding_task_id: claim.coding_task_id.clone(),
                    evidence_kind: claim.kind,
                    reason: "receipt_rejected_replayed_or_expired",
                });
            }
        }
    }

    let mut gaps = Vec::new();
    let mut requirement_closures = Vec::new();
    let mut requirement_blocker_ids = BTreeSet::new();
    for requirement in &plan.requirements {
        let mut gap_ids = Vec::new();
        let mut states = Vec::new();
        for task in requirement
            .tasks
            .iter()
            .filter(|task| task.requirement_id == requirement.requirement_id)
        {
            let (state, evidence_digest, reason) = match task.applicability {
                TaskApplicability::NotApplicable if !rejected_tasks.contains(&task.task_id) => (
                    ImplementationGapState::NotApplicable,
                    None,
                    "task is N/A under the completion plan's exact engine-declared basis".into(),
                ),
                TaskApplicability::Unresolved => (
                    ImplementationGapState::Unresolved,
                    None,
                    "task remains unresolved and cannot be declared N/A".into(),
                ),
                TaskApplicability::Conditional => (
                    ImplementationGapState::ConditionalPending,
                    accepted.get(&task.task_id).map(|(digest, _)| digest.clone()),
                    "condition truth remains unknown; accepted receipt is not sufficient to close this task".into(),
                ),
                TaskApplicability::Required if stale_tasks.contains(&task.task_id) => (
                    ImplementationGapState::Stale,
                    None,
                    "evidence lease is stale for the current source/build/profile/validator/authority/target snapshot".into(),
                ),
                TaskApplicability::Required if rejected_tasks.contains(&task.task_id) => (
                    ImplementationGapState::Rejected,
                    None,
                    "one or more claims for this task were rejected".into(),
                ),
                TaskApplicability::Required => match accepted.get(&task.task_id) {
                    Some((digest, _)) => (
                        ImplementationGapState::VerifiedReceipt,
                        Some(digest.clone()),
                        "producer receipt verifies for this exact planned task, target, and source revision".into(),
                    ),
                    None => (
                        ImplementationGapState::Unavailable,
                        None,
                        "required task has no verified producer receipt".into(),
                    ),
                },
                TaskApplicability::NotApplicable => (
                    ImplementationGapState::Rejected,
                    None,
                    "an unexpected claim was attached to an N/A task".into(),
                ),
            };
            let gap_id = task.task_id.clone();
            gap_ids.push(gap_id.clone());
            states.push(state);
            gaps.push(DgclImplementationGap {
                gap_id,
                requirement_id: Some(requirement.requirement_id.clone()),
                evidence_task_id: Some(task.task_id.clone()),
                evidence_kind: Some(task.evidence_kind),
                state,
                applicability: Some(task.applicability),
                not_applicable_basis: task.not_applicable_basis,
                target_ref: Some(task.target_ref.clone()),
                target_digest: Some(task.target_digest.clone()),
                evidence_digest,
                reason,
            });
        }
        for blocker in plan.blockers.iter().filter(|blocker| {
            blocker.requirement_id.as_deref() == Some(requirement.requirement_id.as_str())
        }) {
            let state = if blocker.kind == CompletionBlockerKind::ConditionEvidenceUnknown {
                ImplementationGapState::ConditionalPending
            } else {
                ImplementationGapState::Unresolved
            };
            requirement_blocker_ids.insert(blocker.blocker_id.clone());
            gap_ids.push(blocker.blocker_id.clone());
            states.push(state);
            gaps.push(DgclImplementationGap {
                gap_id: blocker.blocker_id.clone(),
                requirement_id: blocker.requirement_id.clone(),
                evidence_task_id: None,
                evidence_kind: None,
                state,
                applicability: None,
                not_applicable_basis: None,
                target_ref: None,
                target_digest: blocker.source_digest.clone(),
                evidence_digest: None,
                reason: blocker.reason.clone(),
            });
        }
        let condition_pending = !requirement.conditions.is_empty();
        let status = if states.contains(&ImplementationGapState::Rejected) {
            DgclImplementationStatus::Conflict
        } else if states.contains(&ImplementationGapState::Unresolved) {
            DgclImplementationStatus::Clarify
        } else if condition_pending
            || states.iter().any(|state| {
                matches!(
                    state,
                    ImplementationGapState::ConditionalPending
                        | ImplementationGapState::Unavailable
                        | ImplementationGapState::Stale
                )
            })
        {
            DgclImplementationStatus::Hold
        } else if states.iter().all(|state| {
            matches!(
                state,
                ImplementationGapState::VerifiedReceipt | ImplementationGapState::NotApplicable
            )
        }) {
            DgclImplementationStatus::ImplementationClosed
        } else {
            DgclImplementationStatus::Hold
        };
        requirement_closures.push(DgclRequirementClosure {
            requirement_id: requirement.requirement_id.clone(),
            parent_program_requirement_id: requirement.parent_program_requirement_id.clone(),
            source_span: requirement.source_span.clone(),
            status,
            gap_ids,
        });
    }
    for blocker in plan
        .blockers
        .iter()
        .filter(|blocker| !requirement_blocker_ids.contains(&blocker.blocker_id))
    {
        gaps.push(DgclImplementationGap {
            gap_id: blocker.blocker_id.clone(),
            requirement_id: blocker.requirement_id.clone(),
            evidence_task_id: None,
            evidence_kind: None,
            state: if blocker.kind == CompletionBlockerKind::ConditionEvidenceUnknown {
                ImplementationGapState::ConditionalPending
            } else {
                ImplementationGapState::Unresolved
            },
            applicability: None,
            not_applicable_basis: None,
            target_ref: None,
            target_digest: blocker.source_digest.clone(),
            evidence_digest: None,
            reason: blocker.reason.clone(),
        });
    }
    gaps.sort_by(|left, right| left.gap_id.cmp(&right.gap_id));
    requirement_closures.sort_by(|left, right| left.requirement_id.cmp(&right.requirement_id));
    rejected_claims.sort_by_key(|claim| claim.claim_index);
    let status = if requirement_closures.is_empty()
        && plan.status == CompletionPlanStatus::EmptyRequirementSet
        && !source.trim().is_empty()
        && program_ir.residuals.is_empty()
    {
        DgclImplementationStatus::NoAction
    } else if requirement_closures.is_empty()
        || plan.status == CompletionPlanStatus::EmptyRequirementSet
        || plan.status == CompletionPlanStatus::NeedsClarification
    {
        DgclImplementationStatus::Clarify
    } else if requirement_closures
        .iter()
        .any(|requirement| requirement.status == DgclImplementationStatus::Conflict)
    {
        DgclImplementationStatus::Conflict
    } else if plan.status == CompletionPlanStatus::ConditionEvidencePending
        || requirement_closures
            .iter()
            .any(|requirement| requirement.status != DgclImplementationStatus::ImplementationClosed)
    {
        DgclImplementationStatus::Hold
    } else {
        DgclImplementationStatus::ImplementationClosed
    };
    Ok(DgclImplementationClosureReport {
        schema_version: DGCL_IMPLEMENTATION_CLOSURE_SCHEMA,
        source_revision,
        program_ir_digest: program_ir.program_digest.clone(),
        completion_plan_digest: plan.plan_digest.clone(),
        requirement_count: requirement_closures.len(),
        gaps,
        blockers: plan.blockers.clone(),
        rejected_claim_count: rejected_claims.len(),
        rejected_claims,
        duplicate_claim_count,
        requirements: requirement_closures,
        status,
        evidence_complete_for_declared_requirements: status == DgclImplementationStatus::ImplementationClosed,
        implementation_complete_candidate: status == DgclImplementationStatus::ImplementationClosed,
        authority_created: false,
        output_commit_allowed: false,
        claim_boundary: "per-action closure candidate over exact planned evidence tasks; no semantic correctness, host output, authority, or mutation permission is established",
    })
}

fn is_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn lifecycle_rejection(
    claim: &CodingEvidenceClaim,
    current: Option<&DgclExecutionSnapshot>,
    target_ref: &str,
    target_digest: &str,
    now_epoch: u64,
) -> Option<&'static str> {
    let Some(lease) = claim.lifecycle_lease.as_ref() else {
        return Some("evidence_lifecycle_lease_missing");
    };
    let Some(current) = current else {
        return Some("current_lifecycle_snapshot_unavailable");
    };
    if current.source_revision != claim.source_revision {
        return Some("current_lifecycle_source_revision_mismatch");
    }
    let observation =
        crate::validate_dgcl_target_lease(lease, current, target_ref, target_digest, now_epoch);
    match observation.state {
        crate::DgclLifecycleState::Current => None,
        crate::DgclLifecycleState::Expired => Some("evidence_lifecycle_lease_expired"),
        crate::DgclLifecycleState::StaleSource => Some("evidence_lifecycle_source_stale"),
        crate::DgclLifecycleState::StaleBuild => Some("evidence_lifecycle_build_stale"),
        crate::DgclLifecycleState::StaleProfile => Some("evidence_lifecycle_profile_stale"),
        crate::DgclLifecycleState::StaleValidator => Some("evidence_lifecycle_validator_stale"),
        crate::DgclLifecycleState::StaleAuthority => Some("evidence_lifecycle_authority_stale"),
        crate::DgclLifecycleState::StaleTargets => Some("evidence_lifecycle_targets_stale"),
        crate::DgclLifecycleState::TargetNotBound => Some("evidence_lifecycle_target_unbound"),
        crate::DgclLifecycleState::InvalidLease => Some("evidence_lifecycle_lease_invalid"),
    }
}

fn is_stale_lifecycle_reason(reason: &str) -> bool {
    reason.starts_with("evidence_lifecycle_")
        && (reason.contains("stale") || reason.contains("expired"))
        || reason == "current_lifecycle_source_revision_mismatch"
}
