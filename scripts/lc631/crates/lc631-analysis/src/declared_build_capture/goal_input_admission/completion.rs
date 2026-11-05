//! Explicit borrowed completion fields; no Serialize and no wildcard field omission.
use super::{boolean, number, Field, Node, Object, Sequence};

pub(super) fn completion(value: &crate::DgclCompletionPlan, index: usize) -> Option<Field<'_>> {
    let crate::DgclCompletionPlan {
        schema_version,
        source_revision,
        program_ir_digest,
        translation_projection_digest,
        plan_digest,
        status,
        requirements,
        tasks,
        blockers,
        nonblocking_residual_count,
        authority_created,
        evidence_issued,
        coding_closure_allowed,
        claim_boundary,
    } = value;
    match index {
        0 => Some(Field {
            key: "schema_version",
            value: Node::Str(schema_version),
        }),
        1 => Some(Field {
            key: "source_revision",
            value: Node::Str(source_revision),
        }),
        2 => Some(Field {
            key: "program_ir_digest",
            value: Node::Str(program_ir_digest),
        }),
        3 => Some(Field {
            key: "translation_projection_digest",
            value: Node::Str(translation_projection_digest),
        }),
        4 => Some(Field {
            key: "plan_digest",
            value: Node::Str(plan_digest),
        }),
        5 => Some(Field {
            key: "status",
            value: Node::Str(super::tags_completion::planstatus(status)),
        }),
        6 => Some(Field {
            key: "requirements",
            value: Node::Seq(Sequence::PlannedRequirement(requirements)),
        }),
        7 => Some(Field {
            key: "tasks",
            value: Node::Seq(Sequence::Task(tasks)),
        }),
        8 => Some(Field {
            key: "blockers",
            value: Node::Seq(Sequence::Blocker(blockers)),
        }),
        9 => Some(Field {
            key: "nonblocking_residual_count",
            value: number(nonblocking_residual_count),
        }),
        10 => Some(Field {
            key: "authority_created",
            value: boolean(authority_created),
        }),
        11 => Some(Field {
            key: "evidence_issued",
            value: boolean(evidence_issued),
        }),
        12 => Some(Field {
            key: "coding_closure_allowed",
            value: boolean(coding_closure_allowed),
        }),
        13 => Some(Field {
            key: "claim_boundary",
            value: Node::Str(claim_boundary),
        }),
        _ => None,
    }
}

pub(super) fn plannedrequirement(
    value: &crate::PlannedCodingRequirement,
    index: usize,
) -> Option<Field<'_>> {
    let crate::PlannedCodingRequirement {
        requirement_id,
        parent_program_requirement_id,
        alternative_id,
        region_id,
        action_index,
        source_span,
        source_digest,
        action_text,
        modality,
        polarity,
        target_ref,
        target_candidates,
        conditions,
        tasks,
        ready_for_evidence,
    } = value;
    match index {
        0 => Some(Field {
            key: "requirement_id",
            value: Node::Str(requirement_id),
        }),
        1 => Some(Field {
            key: "parent_program_requirement_id",
            value: Node::Str(parent_program_requirement_id),
        }),
        2 => Some(Field {
            key: "alternative_id",
            value: Node::Str(alternative_id),
        }),
        3 => Some(Field {
            key: "region_id",
            value: number(region_id),
        }),
        4 => Some(Field {
            key: "action_index",
            value: number(action_index),
        }),
        5 => Some(Field {
            key: "source_span",
            value: Node::Object(Object::Range(source_span)),
        }),
        6 => Some(Field {
            key: "source_digest",
            value: Node::Str(source_digest),
        }),
        7 => Some(Field {
            key: "action_text",
            value: Node::Str(action_text),
        }),
        8 => Some(Field {
            key: "modality",
            value: Node::Str(super::tags_grammar::modality(modality)),
        }),
        9 => Some(Field {
            key: "polarity",
            value: Node::Str(super::tags_grammar::polarity(polarity)),
        }),
        10 => Some(Field {
            key: "target_ref",
            value: Node::Str(target_ref),
        }),
        11 => Some(Field {
            key: "target_candidates",
            value: Node::Seq(Sequence::PlannedTarget(target_candidates)),
        }),
        12 => Some(Field {
            key: "conditions",
            value: Node::Seq(Sequence::PlannedCondition(conditions)),
        }),
        13 => Some(Field {
            key: "tasks",
            value: Node::Seq(Sequence::Task(tasks)),
        }),
        14 => Some(Field {
            key: "ready_for_evidence",
            value: boolean(ready_for_evidence),
        }),
        _ => None,
    }
}

pub(super) fn plannedtarget(
    value: &crate::PlannedTargetCandidate,
    index: usize,
) -> Option<Field<'_>> {
    let crate::PlannedTargetCandidate {
        kind,
        value,
        source_span,
        state,
    } = value;
    match index {
        0 => Some(Field {
            key: "kind",
            value: Node::Str(super::tags_grammar::scopekind(kind)),
        }),
        1 => Some(Field {
            key: "value",
            value: Node::Str(value),
        }),
        2 => Some(Field {
            key: "source_span",
            value: Node::Object(Object::Range(source_span)),
        }),
        3 => Some(Field {
            key: "state",
            value: Node::Str(super::tags_grammar::scopestate(state)),
        }),
        _ => None,
    }
}

pub(super) fn plannedcondition(value: &crate::PlannedCondition, index: usize) -> Option<Field<'_>> {
    let crate::PlannedCondition {
        condition_id,
        source_span,
        source_digest,
        marker_span,
        marker_digest,
        relation,
        expression,
        scope_resolved,
        truth_state,
    } = value;
    match index {
        0 => Some(Field {
            key: "condition_id",
            value: Node::Str(condition_id),
        }),
        1 => Some(Field {
            key: "source_span",
            value: Node::Object(Object::Range(source_span)),
        }),
        2 => Some(Field {
            key: "source_digest",
            value: Node::Str(source_digest),
        }),
        3 => Some(Field {
            key: "marker_span",
            value: Node::Object(Object::Range(marker_span)),
        }),
        4 => Some(Field {
            key: "marker_digest",
            value: Node::Str(marker_digest),
        }),
        5 => Some(Field {
            key: "relation",
            value: Node::Str(super::tags_grammar::conditionrelation(relation)),
        }),
        6 => Some(Field {
            key: "expression",
            value: Node::Object(Object::Expression(expression)),
        }),
        7 => Some(Field {
            key: "scope_resolved",
            value: boolean(scope_resolved),
        }),
        8 => Some(Field {
            key: "truth_state",
            value: Node::Str(super::tags_grammar::conditionevidence(truth_state)),
        }),
        _ => None,
    }
}

pub(super) fn task(value: &crate::PlannedEvidenceTask, index: usize) -> Option<Field<'_>> {
    let crate::PlannedEvidenceTask {
        task_id,
        requirement_id,
        evidence_kind,
        applicability,
        not_applicable_basis,
        producer,
        producer_revision,
        target_ref,
        target_digest,
        acceptance_criterion,
        condition_ids,
    } = value;
    match index {
        0 => Some(Field {
            key: "task_id",
            value: Node::Str(task_id),
        }),
        1 => Some(Field {
            key: "requirement_id",
            value: Node::Str(requirement_id),
        }),
        2 => Some(Field {
            key: "evidence_kind",
            value: Node::Str(super::tags_completion::evidencekind(evidence_kind)),
        }),
        3 => Some(Field {
            key: "applicability",
            value: Node::Str(super::tags_completion::applicability(applicability)),
        }),
        4 => Some(Field {
            key: "not_applicable_basis",
            value: not_applicable_basis
                .as_ref()
                .map(|v| Node::Str(super::tags_completion::notapplicable(v)))
                .unwrap_or(Node::Null),
        }),
        5 => Some(Field {
            key: "producer",
            value: Node::Str(super::tags_completion::producer(producer)),
        }),
        6 => Some(Field {
            key: "producer_revision",
            value: Node::Str(producer_revision),
        }),
        7 => Some(Field {
            key: "target_ref",
            value: Node::Str(target_ref),
        }),
        8 => Some(Field {
            key: "target_digest",
            value: Node::Str(target_digest),
        }),
        9 => Some(Field {
            key: "acceptance_criterion",
            value: Node::Str(acceptance_criterion),
        }),
        10 => Some(Field {
            key: "condition_ids",
            value: Node::Seq(Sequence::String(condition_ids)),
        }),
        _ => None,
    }
}

pub(super) fn blocker(value: &crate::CompletionBlocker, index: usize) -> Option<Field<'_>> {
    let crate::CompletionBlocker {
        blocker_id,
        requirement_id,
        parent_program_requirement_id,
        source_span,
        source_digest,
        kind,
        reason,
    } = value;
    match index {
        0 => Some(Field {
            key: "blocker_id",
            value: Node::Str(blocker_id),
        }),
        1 => Some(Field {
            key: "requirement_id",
            value: requirement_id
                .as_deref()
                .map(Node::Str)
                .unwrap_or(Node::Null),
        }),
        2 => Some(Field {
            key: "parent_program_requirement_id",
            value: parent_program_requirement_id
                .as_deref()
                .map(Node::Str)
                .unwrap_or(Node::Null),
        }),
        3 => Some(Field {
            key: "source_span",
            value: source_span
                .as_ref()
                .map(|v| Node::Object(Object::Range(v)))
                .unwrap_or(Node::Null),
        }),
        4 => Some(Field {
            key: "source_digest",
            value: source_digest
                .as_deref()
                .map(Node::Str)
                .unwrap_or(Node::Null),
        }),
        5 => Some(Field {
            key: "kind",
            value: Node::Str(super::tags_completion::blockerkind(kind)),
        }),
        6 => Some(Field {
            key: "reason",
            value: Node::Str(reason),
        }),
        _ => None,
    }
}

pub(super) fn closure(
    value: &crate::DgclImplementationClosureReport,
    index: usize,
) -> Option<Field<'_>> {
    let crate::DgclImplementationClosureReport {
        schema_version,
        source_revision,
        program_ir_digest,
        completion_plan_digest,
        requirement_count,
        gaps,
        blockers,
        rejected_claim_count,
        rejected_claims,
        duplicate_claim_count,
        requirements,
        status,
        evidence_complete_for_declared_requirements,
        implementation_complete_candidate,
        authority_created,
        output_commit_allowed,
        claim_boundary,
    } = value;
    match index {
        0 => Some(Field {
            key: "schema_version",
            value: Node::Str(schema_version),
        }),
        1 => Some(Field {
            key: "source_revision",
            value: Node::Str(source_revision),
        }),
        2 => Some(Field {
            key: "program_ir_digest",
            value: Node::Str(program_ir_digest),
        }),
        3 => Some(Field {
            key: "completion_plan_digest",
            value: Node::Str(completion_plan_digest),
        }),
        4 => Some(Field {
            key: "requirement_count",
            value: number(requirement_count),
        }),
        5 => Some(Field {
            key: "gaps",
            value: Node::Seq(Sequence::Gap(gaps)),
        }),
        6 => Some(Field {
            key: "blockers",
            value: Node::Seq(Sequence::Blocker(blockers)),
        }),
        7 => Some(Field {
            key: "rejected_claim_count",
            value: number(rejected_claim_count),
        }),
        8 => Some(Field {
            key: "rejected_claims",
            value: Node::Seq(Sequence::Rejection(rejected_claims)),
        }),
        9 => Some(Field {
            key: "duplicate_claim_count",
            value: number(duplicate_claim_count),
        }),
        10 => Some(Field {
            key: "requirements",
            value: Node::Seq(Sequence::RequirementClosure(requirements)),
        }),
        11 => Some(Field {
            key: "status",
            value: Node::Str(super::tags_completion::implementationstatus(status)),
        }),
        12 => Some(Field {
            key: "evidence_complete_for_declared_requirements",
            value: boolean(evidence_complete_for_declared_requirements),
        }),
        13 => Some(Field {
            key: "implementation_complete_candidate",
            value: boolean(implementation_complete_candidate),
        }),
        14 => Some(Field {
            key: "authority_created",
            value: boolean(authority_created),
        }),
        15 => Some(Field {
            key: "output_commit_allowed",
            value: boolean(output_commit_allowed),
        }),
        16 => Some(Field {
            key: "claim_boundary",
            value: Node::Str(claim_boundary),
        }),
        _ => None,
    }
}

pub(super) fn gap(value: &crate::DgclImplementationGap, index: usize) -> Option<Field<'_>> {
    let crate::DgclImplementationGap {
        gap_id,
        requirement_id,
        evidence_task_id,
        evidence_kind,
        state,
        applicability,
        not_applicable_basis,
        target_ref,
        target_digest,
        evidence_digest,
        reason,
    } = value;
    match index {
        0 => Some(Field {
            key: "gap_id",
            value: Node::Str(gap_id),
        }),
        1 => Some(Field {
            key: "requirement_id",
            value: requirement_id
                .as_deref()
                .map(Node::Str)
                .unwrap_or(Node::Null),
        }),
        2 => Some(Field {
            key: "evidence_task_id",
            value: evidence_task_id
                .as_deref()
                .map(Node::Str)
                .unwrap_or(Node::Null),
        }),
        3 => Some(Field {
            key: "evidence_kind",
            value: evidence_kind
                .as_ref()
                .map(|v| Node::Str(super::tags_completion::evidencekind(v)))
                .unwrap_or(Node::Null),
        }),
        4 => Some(Field {
            key: "state",
            value: Node::Str(super::tags_completion::gapstate(state)),
        }),
        5 => Some(Field {
            key: "applicability",
            value: applicability
                .as_ref()
                .map(|v| Node::Str(super::tags_completion::applicability(v)))
                .unwrap_or(Node::Null),
        }),
        6 => Some(Field {
            key: "not_applicable_basis",
            value: not_applicable_basis
                .as_ref()
                .map(|v| Node::Str(super::tags_completion::notapplicable(v)))
                .unwrap_or(Node::Null),
        }),
        7 => Some(Field {
            key: "target_ref",
            value: target_ref.as_deref().map(Node::Str).unwrap_or(Node::Null),
        }),
        8 => Some(Field {
            key: "target_digest",
            value: target_digest
                .as_deref()
                .map(Node::Str)
                .unwrap_or(Node::Null),
        }),
        9 => Some(Field {
            key: "evidence_digest",
            value: evidence_digest
                .as_deref()
                .map(Node::Str)
                .unwrap_or(Node::Null),
        }),
        10 => Some(Field {
            key: "reason",
            value: Node::Str(reason),
        }),
        _ => None,
    }
}

pub(super) fn rejection(value: &crate::DgclEvidenceRejection, index: usize) -> Option<Field<'_>> {
    let crate::DgclEvidenceRejection {
        claim_index,
        coding_requirement_id,
        coding_task_id,
        evidence_kind,
        reason,
    } = value;
    match index {
        0 => Some(Field {
            key: "claim_index",
            value: number(claim_index),
        }),
        1 => Some(Field {
            key: "coding_requirement_id",
            value: coding_requirement_id
                .as_deref()
                .map(Node::Str)
                .unwrap_or(Node::Null),
        }),
        2 => Some(Field {
            key: "coding_task_id",
            value: coding_task_id
                .as_deref()
                .map(Node::Str)
                .unwrap_or(Node::Null),
        }),
        3 => Some(Field {
            key: "evidence_kind",
            value: Node::Str(super::tags_completion::evidencekind(evidence_kind)),
        }),
        4 => Some(Field {
            key: "reason",
            value: Node::Str(reason),
        }),
        _ => None,
    }
}

pub(super) fn requirementclosure(
    value: &crate::DgclRequirementClosure,
    index: usize,
) -> Option<Field<'_>> {
    let crate::DgclRequirementClosure {
        requirement_id,
        parent_program_requirement_id,
        source_span,
        status,
        gap_ids,
    } = value;
    match index {
        0 => Some(Field {
            key: "requirement_id",
            value: Node::Str(requirement_id),
        }),
        1 => Some(Field {
            key: "parent_program_requirement_id",
            value: Node::Str(parent_program_requirement_id),
        }),
        2 => Some(Field {
            key: "source_span",
            value: Node::Object(Object::Range(source_span)),
        }),
        3 => Some(Field {
            key: "status",
            value: Node::Str(super::tags_completion::implementationstatus(status)),
        }),
        4 => Some(Field {
            key: "gap_ids",
            value: Node::Seq(Sequence::String(gap_ids)),
        }),
        _ => None,
    }
}
