//! Explicit borrowed tl fields; no Serialize and no wildcard field omission.
use super::{boolean, number, Field, Node, Object, Sequence};

pub(super) fn translation(
    value: &crate::DgclTranslationProjection,
    index: usize,
) -> Option<Field<'_>> {
    let crate::DgclTranslationProjection {
        schema_version,
        source_revision,
        program_ir_digest,
        envelope,
        loss_events,
        gate,
        empty_requirement_set_held,
        scalar_aggregate_used,
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
            key: "envelope",
            value: Node::Object(Object::Envelope(envelope)),
        }),
        4 => Some(Field {
            key: "loss_events",
            value: Node::Seq(Sequence::Loss(loss_events)),
        }),
        5 => Some(Field {
            key: "gate",
            value: Node::Str(super::tags_state::gate(gate)),
        }),
        6 => Some(Field {
            key: "empty_requirement_set_held",
            value: boolean(empty_requirement_set_held),
        }),
        7 => Some(Field {
            key: "scalar_aggregate_used",
            value: boolean(scalar_aggregate_used),
        }),
        8 => Some(Field {
            key: "authority_created",
            value: boolean(authority_created),
        }),
        9 => Some(Field {
            key: "output_commit_allowed",
            value: boolean(output_commit_allowed),
        }),
        10 => Some(Field {
            key: "claim_boundary",
            value: Node::Str(claim_boundary),
        }),
        _ => None,
    }
}

pub(super) fn envelope(
    value: &lc631_tl::CanonicalTranslationEnvelope,
    index: usize,
) -> Option<Field<'_>> {
    let lc631_tl::CanonicalTranslationEnvelope {
        schema_version,
        source_ledger_id,
        source_revision,
        source_digest,
        target_claims,
        edges,
        domain_payloads,
        gate,
        scalar_aggregate_used,
    } = value;
    match index {
        0 => Some(Field {
            key: "schema_version",
            value: Node::Str(schema_version),
        }),
        1 => Some(Field {
            key: "source_ledger_id",
            value: number(source_ledger_id),
        }),
        2 => Some(Field {
            key: "source_revision",
            value: Node::Str(source_revision),
        }),
        3 => Some(Field {
            key: "source_digest",
            value: Node::Str(source_digest),
        }),
        4 => Some(Field {
            key: "target_claims",
            value: Node::Seq(Sequence::Target(target_claims)),
        }),
        5 => Some(Field {
            key: "edges",
            value: Node::Seq(Sequence::ProjectionEdge(edges)),
        }),
        6 => Some(Field {
            key: "domain_payloads",
            value: Node::Seq(Sequence::Domain(domain_payloads)),
        }),
        7 => Some(Field {
            key: "gate",
            value: Node::Str(super::tags_state::gate(gate)),
        }),
        8 => Some(Field {
            key: "scalar_aggregate_used",
            value: boolean(scalar_aggregate_used),
        }),
        _ => None,
    }
}

pub(super) fn target(value: &lc631_tl::TargetClaim, index: usize) -> Option<Field<'_>> {
    let lc631_tl::TargetClaim {
        id,
        artifact,
        path,
        revision,
        content_digest,
        span,
    } = value;
    match index {
        0 => Some(Field {
            key: "id",
            value: number(id),
        }),
        1 => Some(Field {
            key: "artifact",
            value: Node::Str(artifact),
        }),
        2 => Some(Field {
            key: "path",
            value: Node::Str(path),
        }),
        3 => Some(Field {
            key: "revision",
            value: Node::Str(revision),
        }),
        4 => Some(Field {
            key: "content_digest",
            value: Node::Str(content_digest),
        }),
        5 => Some(Field {
            key: "span",
            value: span
                .as_ref()
                .map(|v| Node::Object(Object::SourceSpan(v)))
                .unwrap_or(Node::Null),
        }),
        _ => None,
    }
}

pub(super) fn projectionedge(
    value: &lc631_tl::ProjectionEdgeV3,
    index: usize,
) -> Option<Field<'_>> {
    let lc631_tl::ProjectionEdgeV3 {
        id,
        obligation_id,
        source,
        targets,
        stage,
        state,
        rule_id,
        verifier,
    } = value;
    match index {
        0 => Some(Field {
            key: "id",
            value: number(id),
        }),
        1 => Some(Field {
            key: "obligation_id",
            value: obligation_id.as_ref().map(number).unwrap_or(Node::Null),
        }),
        2 => Some(Field {
            key: "source",
            value: source
                .as_ref()
                .map(|v| Node::Object(Object::SourceAnchor(v)))
                .unwrap_or(Node::Null),
        }),
        3 => Some(Field {
            key: "targets",
            value: Node::Seq(Sequence::TargetId(targets)),
        }),
        4 => Some(Field {
            key: "stage",
            value: Node::Str(super::tags_state::projectionstage(stage)),
        }),
        5 => Some(Field {
            key: "state",
            value: Node::Str(super::tags_state::preservation(state)),
        }),
        6 => Some(Field {
            key: "rule_id",
            value: Node::Str(rule_id),
        }),
        7 => Some(Field {
            key: "verifier",
            value: Node::Object(Object::Receipt(verifier)),
        }),
        _ => None,
    }
}

pub(super) fn sourceanchor(value: &lc631_tl::SourceAnchorV3, index: usize) -> Option<Field<'_>> {
    let lc631_tl::SourceAnchorV3 {
        ledger_id,
        revision,
        span,
        surface_digest,
    } = value;
    match index {
        0 => Some(Field {
            key: "ledger_id",
            value: number(ledger_id),
        }),
        1 => Some(Field {
            key: "revision",
            value: Node::Str(revision),
        }),
        2 => Some(Field {
            key: "span",
            value: Node::Object(Object::SourceSpan(span)),
        }),
        3 => Some(Field {
            key: "surface_digest",
            value: Node::Str(surface_digest),
        }),
        _ => None,
    }
}

pub(super) fn receipt(value: &lc631_tl::VerificationReceipt, index: usize) -> Option<Field<'_>> {
    let lc631_tl::VerificationReceipt {
        verifier,
        status,
        revision,
        evidence_digest,
    } = value;
    match index {
        0 => Some(Field {
            key: "verifier",
            value: Node::Str(super::tags_state::verifier(verifier)),
        }),
        1 => Some(Field {
            key: "status",
            value: Node::Str(super::tags_state::verificationstatus(status)),
        }),
        2 => Some(Field {
            key: "revision",
            value: Node::Str(revision),
        }),
        3 => Some(Field {
            key: "evidence_digest",
            value: evidence_digest
                .as_deref()
                .map(Node::Str)
                .unwrap_or(Node::Null),
        }),
        _ => None,
    }
}

pub(super) fn domain(
    value: &lc631_tl::DomainTranslationPayload,
    index: usize,
) -> Option<Field<'_>> {
    let lc631_tl::DomainTranslationPayload {
        domain,
        schema_version,
        payload_digest,
        unknowns,
    } = value;
    match index {
        0 => Some(Field {
            key: "domain",
            value: Node::Str(domain),
        }),
        1 => Some(Field {
            key: "schema_version",
            value: Node::Str(schema_version),
        }),
        2 => Some(Field {
            key: "payload_digest",
            value: Node::Str(payload_digest),
        }),
        3 => Some(Field {
            key: "unknowns",
            value: Node::Seq(Sequence::String(unknowns)),
        }),
        _ => None,
    }
}

pub(super) fn loss(value: &crate::TranslationLossEvent, index: usize) -> Option<Field<'_>> {
    let crate::TranslationLossEvent {
        requirement_id,
        stage,
        input_digest,
        output_digest,
        stage_event_digest,
        state,
        kind,
        reason,
    } = value;
    match index {
        0 => Some(Field {
            key: "requirement_id",
            value: Node::Str(requirement_id),
        }),
        1 => Some(Field {
            key: "stage",
            value: Node::Str(super::tags_state::projectionstage(stage)),
        }),
        2 => Some(Field {
            key: "input_digest",
            value: input_digest.as_deref().map(Node::Str).unwrap_or(Node::Null),
        }),
        3 => Some(Field {
            key: "output_digest",
            value: output_digest
                .as_deref()
                .map(Node::Str)
                .unwrap_or(Node::Null),
        }),
        4 => Some(Field {
            key: "stage_event_digest",
            value: Node::Str(stage_event_digest),
        }),
        5 => Some(Field {
            key: "state",
            value: Node::Str(super::tags_state::preservation(state)),
        }),
        6 => Some(Field {
            key: "kind",
            value: Node::Str(super::tags_state::losskind(kind)),
        }),
        7 => Some(Field {
            key: "reason",
            value: Node::Str(reason),
        }),
        _ => None,
    }
}
