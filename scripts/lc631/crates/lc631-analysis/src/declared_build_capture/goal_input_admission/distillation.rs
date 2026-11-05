//! Explicit borrowed distillation fields; no Serialize and no wildcard field omission.
use super::{boolean, number, Field, Node, Object, Sequence};

pub(super) fn run(value: &lc631_tldg::MutualDistillationRun, index: usize) -> Option<Field<'_>> {
    let lc631_tldg::MutualDistillationRun {
        epochs,
        reprojection_requests,
        final_payload,
        final_backflow,
        max_epochs_respected,
        stop_reason,
    } = value;
    match index {
        0 => Some(Field {
            key: "epochs",
            value: Node::Seq(Sequence::Epoch(epochs)),
        }),
        1 => Some(Field {
            key: "reprojection_requests",
            value: Node::Seq(Sequence::Reprojection(reprojection_requests)),
        }),
        2 => Some(Field {
            key: "final_payload",
            value: Node::Object(Object::Payload(final_payload)),
        }),
        3 => Some(Field {
            key: "final_backflow",
            value: Node::Object(Object::Backflow(final_backflow)),
        }),
        4 => Some(Field {
            key: "max_epochs_respected",
            value: boolean(max_epochs_respected),
        }),
        5 => Some(Field {
            key: "stop_reason",
            value: Node::Str(super::tags_state::stopreason(stop_reason)),
        }),
        _ => None,
    }
}

pub(super) fn epoch(
    value: &lc631_tldg::DistillationEpochRecord,
    index: usize,
) -> Option<Field<'_>> {
    let lc631_tldg::DistillationEpochRecord {
        epoch,
        input_revision,
        output_revision,
        accepted,
        materialized_relations_added,
        rejected,
        needs_evidence,
        incomparable,
        grammar_mutated,
        thresholds_mutated,
        state_digest,
        state_changed,
    } = value;
    match index {
        0 => Some(Field {
            key: "epoch",
            value: number(epoch),
        }),
        1 => Some(Field {
            key: "input_revision",
            value: Node::Str(input_revision),
        }),
        2 => Some(Field {
            key: "output_revision",
            value: Node::Str(output_revision),
        }),
        3 => Some(Field {
            key: "accepted",
            value: number(accepted),
        }),
        4 => Some(Field {
            key: "materialized_relations_added",
            value: number(materialized_relations_added),
        }),
        5 => Some(Field {
            key: "rejected",
            value: number(rejected),
        }),
        6 => Some(Field {
            key: "needs_evidence",
            value: number(needs_evidence),
        }),
        7 => Some(Field {
            key: "incomparable",
            value: number(incomparable),
        }),
        8 => Some(Field {
            key: "grammar_mutated",
            value: boolean(grammar_mutated),
        }),
        9 => Some(Field {
            key: "thresholds_mutated",
            value: boolean(thresholds_mutated),
        }),
        10 => Some(Field {
            key: "state_digest",
            value: Node::Str(state_digest),
        }),
        11 => Some(Field {
            key: "state_changed",
            value: boolean(state_changed),
        }),
        _ => None,
    }
}

pub(super) fn reprojection(
    value: &lc631_tldg::ReprojectionRequest,
    index: usize,
) -> Option<Field<'_>> {
    let lc631_tldg::ReprojectionRequest {
        after_epoch,
        from_revision,
        next_input_revision,
        residual_classes,
        authority_created,
    } = value;
    match index {
        0 => Some(Field {
            key: "after_epoch",
            value: number(after_epoch),
        }),
        1 => Some(Field {
            key: "from_revision",
            value: Node::Str(from_revision),
        }),
        2 => Some(Field {
            key: "next_input_revision",
            value: Node::Str(next_input_revision),
        }),
        3 => Some(Field {
            key: "residual_classes",
            value: Node::Seq(Sequence::String(residual_classes)),
        }),
        4 => Some(Field {
            key: "authority_created",
            value: boolean(authority_created),
        }),
        _ => None,
    }
}

pub(super) fn payload(value: &lc631_tldg::EpochPayload, index: usize) -> Option<Field<'_>> {
    let lc631_tldg::EpochPayload {
        kernel,
        geometry,
        decoded,
        accepted,
        rejected,
        needs_evidence,
        incomparable,
        geometry_direct_promotions,
    } = value;
    match index {
        0 => Some(Field {
            key: "kernel",
            value: Node::Object(Object::Kernel(kernel)),
        }),
        1 => Some(Field {
            key: "geometry",
            value: geometry
                .as_ref()
                .map(|v| Node::Object(Object::Geometry(v)))
                .unwrap_or(Node::Null),
        }),
        2 => Some(Field {
            key: "decoded",
            value: Node::Seq(Sequence::Decoded(decoded)),
        }),
        3 => Some(Field {
            key: "accepted",
            value: Node::Seq(Sequence::Accepted(accepted)),
        }),
        4 => Some(Field {
            key: "rejected",
            value: Node::Seq(Sequence::Rejected(rejected)),
        }),
        5 => Some(Field {
            key: "needs_evidence",
            value: Node::Seq(Sequence::Validation(needs_evidence)),
        }),
        6 => Some(Field {
            key: "incomparable",
            value: Node::Seq(Sequence::Validation(incomparable)),
        }),
        7 => Some(Field {
            key: "geometry_direct_promotions",
            value: number(geometry_direct_promotions),
        }),
        _ => None,
    }
}

pub(super) fn kernel(value: &lc631_tldg::StructuralKernel, index: usize) -> Option<Field<'_>> {
    let lc631_tldg::StructuralKernel {
        source_revision,
        anchors,
        relation_count,
        legacy_translation_loss_consumed,
        structural_revision,
        materialized_relations,
        kernel_digest,
    } = value;
    match index {
        0 => Some(Field {
            key: "source_revision",
            value: Node::Str(source_revision),
        }),
        1 => Some(Field {
            key: "anchors",
            value: Node::Seq(Sequence::Anchor(anchors)),
        }),
        2 => Some(Field {
            key: "relation_count",
            value: number(relation_count),
        }),
        3 => Some(Field {
            key: "legacy_translation_loss_consumed",
            value: boolean(legacy_translation_loss_consumed),
        }),
        4 => Some(Field {
            key: "structural_revision",
            value: Node::Str(structural_revision),
        }),
        5 => Some(Field {
            key: "materialized_relations",
            value: Node::Seq(Sequence::Materialized(materialized_relations)),
        }),
        6 => Some(Field {
            key: "kernel_digest",
            value: Node::Str(kernel_digest),
        }),
        _ => None,
    }
}

pub(super) fn anchor(value: &lc631_tldg::StructuralAnchor, index: usize) -> Option<Field<'_>> {
    let lc631_tldg::StructuralAnchor {
        node_id,
        source_spans,
        order,
        relation_degree,
        profile_count,
        state,
    } = value;
    match index {
        0 => Some(Field {
            key: "node_id",
            value: number(node_id),
        }),
        1 => Some(Field {
            key: "source_spans",
            value: Node::Seq(Sequence::SourceSpan(source_spans)),
        }),
        2 => Some(Field {
            key: "order",
            value: number(order),
        }),
        3 => Some(Field {
            key: "relation_degree",
            value: number(relation_degree),
        }),
        4 => Some(Field {
            key: "profile_count",
            value: number(profile_count),
        }),
        5 => Some(Field {
            key: "state",
            value: Node::Str(super::tags_state::anchorstate(state)),
        }),
        _ => None,
    }
}

pub(super) fn materialized(
    value: &lc631_tldg::MaterializedRelationEvidence,
    index: usize,
) -> Option<Field<'_>> {
    let lc631_tldg::MaterializedRelationEvidence {
        materialization_id,
        source,
        target,
        kind,
        source_relation_evidence_digest,
        first_materialized_epoch,
        claim_boundary,
    } = value;
    match index {
        0 => Some(Field {
            key: "materialization_id",
            value: Node::Str(materialization_id),
        }),
        1 => Some(Field {
            key: "source",
            value: number(source),
        }),
        2 => Some(Field {
            key: "target",
            value: number(target),
        }),
        3 => Some(Field {
            key: "kind",
            value: Node::Str(super::tags_state::relationkind(kind)),
        }),
        4 => Some(Field {
            key: "source_relation_evidence_digest",
            value: Node::Str(source_relation_evidence_digest),
        }),
        5 => Some(Field {
            key: "first_materialized_epoch",
            value: number(first_materialized_epoch),
        }),
        6 => Some(Field {
            key: "claim_boundary",
            value: Node::Str(claim_boundary),
        }),
        _ => None,
    }
}

pub(super) fn decoded(value: &lc631_tldg::DecodedProposal, index: usize) -> Option<Field<'_>> {
    let lc631_tldg::DecodedProposal {
        proposal,
        decoded_from_geometry,
    } = value;
    match index {
        0 => Some(Field {
            key: "proposal",
            value: Node::Object(Object::Proposal(proposal)),
        }),
        1 => Some(Field {
            key: "decoded_from_geometry",
            value: boolean(decoded_from_geometry),
        }),
        _ => None,
    }
}

pub(super) fn accepted(value: &lc631_tldg::AcceptedProposal, index: usize) -> Option<Field<'_>> {
    let lc631_tldg::AcceptedProposal { proposal, receipt } = value;
    match index {
        0 => Some(Field {
            key: "proposal",
            value: Node::Object(Object::Proposal(proposal)),
        }),
        1 => Some(Field {
            key: "receipt",
            value: Node::Object(Object::Validation(receipt)),
        }),
        _ => None,
    }
}

pub(super) fn rejected(value: &lc631_tldg::RejectedProposal, index: usize) -> Option<Field<'_>> {
    let lc631_tldg::RejectedProposal {
        proposal_id,
        reason,
        receipt,
    } = value;
    match index {
        0 => Some(Field {
            key: "proposal_id",
            value: number(proposal_id),
        }),
        1 => Some(Field {
            key: "reason",
            value: Node::Str(reason),
        }),
        2 => Some(Field {
            key: "receipt",
            value: Node::Object(Object::Validation(receipt)),
        }),
        _ => None,
    }
}

pub(super) fn validation(value: &lc631_tldg::ValidationReceipt, index: usize) -> Option<Field<'_>> {
    let lc631_tldg::ValidationReceipt {
        proposal_id,
        verifier,
        status,
        canonical_edge_authorized,
    } = value;
    match index {
        0 => Some(Field {
            key: "proposal_id",
            value: number(proposal_id),
        }),
        1 => Some(Field {
            key: "verifier",
            value: Node::Str(verifier),
        }),
        2 => Some(Field {
            key: "status",
            value: Node::Str(super::tags_state::validationstate(status)),
        }),
        3 => Some(Field {
            key: "canonical_edge_authorized",
            value: boolean(canonical_edge_authorized),
        }),
        _ => None,
    }
}

pub(super) fn backflow(value: &lc631_tldg::BackflowReport, index: usize) -> Option<Field<'_>> {
    let lc631_tldg::BackflowReport {
        grammar_mutated,
        thresholds_mutated,
        accepted_candidates,
        rejected_residuals,
        retained_unknowns,
        retained_incomparable,
        claim_boundary,
    } = value;
    match index {
        0 => Some(Field {
            key: "grammar_mutated",
            value: boolean(grammar_mutated),
        }),
        1 => Some(Field {
            key: "thresholds_mutated",
            value: boolean(thresholds_mutated),
        }),
        2 => Some(Field {
            key: "accepted_candidates",
            value: number(accepted_candidates),
        }),
        3 => Some(Field {
            key: "rejected_residuals",
            value: number(rejected_residuals),
        }),
        4 => Some(Field {
            key: "retained_unknowns",
            value: number(retained_unknowns),
        }),
        5 => Some(Field {
            key: "retained_incomparable",
            value: number(retained_incomparable),
        }),
        6 => Some(Field {
            key: "claim_boundary",
            value: Node::Str(claim_boundary),
        }),
        _ => None,
    }
}
