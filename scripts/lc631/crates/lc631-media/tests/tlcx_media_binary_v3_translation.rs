#![forbid(unsafe_code)]

use lc631_core::{ObligationId, SourceSpan};
use lc631_media::attestation_v3::{translation::write_translation_v3, CanonicalWriter};
use lc631_tl::{
    CanonicalTranslationEnvelope, DomainTranslationPayload, PreservationState, ProjectionEdgeId,
    ProjectionEdgeV3, ProjectionGateDecision, ProjectionStage, SourceAnchorV3, SourceLedgerId,
    TargetClaim, TargetClaimId, VerificationReceipt, VerificationStatus, VerifierKind,
};

fn envelope() -> CanonicalTranslationEnvelope {
    CanonicalTranslationEnvelope {
        schema_version: "fixture-v1",
        source_ledger_id: SourceLedgerId(1),
        source_revision: "rev".into(),
        source_digest: "source".into(),
        target_claims: vec![TargetClaim {
            id: TargetClaimId(2),
            artifact: "a".into(),
            path: "p".into(),
            revision: "r".into(),
            content_digest: "c".into(),
            span: Some(SourceSpan { start: 0, end: 1 }),
        }],
        edges: vec![ProjectionEdgeV3 {
            id: ProjectionEdgeId(3),
            obligation_id: Some(ObligationId(4)),
            source: Some(SourceAnchorV3 {
                ledger_id: SourceLedgerId(1),
                revision: "r".into(),
                span: SourceSpan { start: 0, end: 1 },
                surface_digest: "surface".into(),
            }),
            targets: vec![TargetClaimId(2)],
            stage: ProjectionStage::AudioToTranscript,
            state: PreservationState::Unresolved,
            rule_id: "rule".into(),
            verifier: VerificationReceipt {
                verifier: VerifierKind::RuntimeObservation,
                status: VerificationStatus::NeedsEvidence,
                revision: "v".into(),
                evidence_digest: Some("e".into()),
            },
        }],
        domain_payloads: vec![DomainTranslationPayload {
            domain: "media".into(),
            schema_version: "1".into(),
            payload_digest: "d".into(),
            unknowns: vec!["u".into()],
        }],
        gate: ProjectionGateDecision::Clarify,
        scalar_aggregate_used: false,
    }
}
fn encode(value: &CanonicalTranslationEnvelope) -> Vec<u8> {
    let mut bytes = Vec::new();
    write_translation_v3(&mut CanonicalWriter::new(&mut bytes, 4096), value).unwrap();
    bytes
}
#[test]
fn translation_top_level_and_nested_changes_are_not_omitted() {
    let original = envelope();
    let baseline = encode(&original);
    let mut changes = Vec::new();
    let mut v = original.clone();
    v.schema_version = "other";
    changes.push(v);
    let mut v = original.clone();
    v.source_ledger_id = SourceLedgerId(2);
    changes.push(v);
    let mut v = original.clone();
    v.source_revision.push('x');
    changes.push(v);
    let mut v = original.clone();
    v.source_digest.push('x');
    changes.push(v);
    let mut v = original.clone();
    v.target_claims[0].id = TargetClaimId(9);
    changes.push(v);
    let mut v = original.clone();
    v.target_claims[0].artifact.push('x');
    changes.push(v);
    let mut v = original.clone();
    v.target_claims[0].path.push('x');
    changes.push(v);
    let mut v = original.clone();
    v.target_claims[0].revision.push('x');
    changes.push(v);
    let mut v = original.clone();
    v.target_claims[0].content_digest.push('x');
    changes.push(v);
    let mut v = original.clone();
    v.target_claims[0].span = None;
    changes.push(v);
    let mut v = original.clone();
    v.target_claims[0].span.as_mut().unwrap().start = 1;
    changes.push(v);
    let mut v = original.clone();
    v.target_claims[0].span.as_mut().unwrap().end = 2;
    changes.push(v);
    let mut v = original.clone();
    v.edges[0].id = ProjectionEdgeId(8);
    changes.push(v);
    let mut v = original.clone();
    v.edges[0].obligation_id = None;
    changes.push(v);
    let mut v = original.clone();
    v.edges[0].source = None;
    changes.push(v);
    let mut v = original.clone();
    v.edges[0].source.as_mut().unwrap().ledger_id = SourceLedgerId(8);
    changes.push(v);
    let mut v = original.clone();
    v.edges[0].source.as_mut().unwrap().revision.push('x');
    changes.push(v);
    let mut v = original.clone();
    v.edges[0].source.as_mut().unwrap().span.start = 1;
    changes.push(v);
    let mut v = original.clone();
    v.edges[0].source.as_mut().unwrap().span.end = 2;
    changes.push(v);
    let mut v = original.clone();
    v.edges[0].source.as_mut().unwrap().surface_digest.push('x');
    changes.push(v);
    let mut v = original.clone();
    v.edges[0].targets.push(TargetClaimId(3));
    changes.push(v);
    let mut v = original.clone();
    v.edges[0].stage = ProjectionStage::FrameToObservation;
    changes.push(v);
    let mut v = original.clone();
    v.edges[0].state = PreservationState::Exact;
    changes.push(v);
    let mut v = original.clone();
    v.edges[0].rule_id.push('x');
    changes.push(v);
    let mut v = original.clone();
    v.edges[0].verifier.verifier = VerifierKind::Schema;
    changes.push(v);
    let mut v = original.clone();
    v.edges[0].verifier.status = VerificationStatus::Passed;
    changes.push(v);
    let mut v = original.clone();
    v.edges[0].verifier.revision.push('x');
    changes.push(v);
    let mut v = original.clone();
    v.edges[0].verifier.evidence_digest = None;
    changes.push(v);
    let mut v = original.clone();
    v.domain_payloads[0].domain.push('x');
    changes.push(v);
    let mut v = original.clone();
    v.domain_payloads[0].schema_version.push('x');
    changes.push(v);
    let mut v = original.clone();
    v.domain_payloads[0].payload_digest.push('x');
    changes.push(v);
    let mut v = original.clone();
    v.domain_payloads[0].unknowns.push("u2".into());
    changes.push(v);
    let mut v = original.clone();
    v.gate = ProjectionGateDecision::HardHold;
    changes.push(v);
    let mut v = original.clone();
    v.scalar_aggregate_used = true;
    changes.push(v);
    for (i, value) in changes.iter().enumerate() {
        assert_ne!(encode(value), baseline, "translation field {i} omitted");
    }
}

#[test]
fn translation_byte_limit_failure_is_returned() {
    let mut bytes = Vec::new();
    assert!(write_translation_v3(&mut CanonicalWriter::new(&mut bytes, 1), &envelope()).is_err());
}

#[test]
fn empty_envelope_literal_golden_keeps_unknown_gate_distinct() {
    let mut value = envelope();
    value.schema_version = "";
    value.source_ledger_id = SourceLedgerId(0);
    value.source_revision.clear();
    value.source_digest.clear();
    value.target_claims.clear();
    value.edges.clear();
    value.domain_payloads.clear();
    let mut expected = vec![1, 0, 0, 0, 0, 0, 0, 0, 0];
    expected.extend_from_slice(&[0x10, 0, 0, 0, 0, 0, 0, 0, 0]);
    expected.extend_from_slice(&[1, 0, 0, 0, 0, 0, 0, 0, 0]);
    expected.extend_from_slice(&[1, 0, 0, 0, 0, 0, 0, 0, 0]);
    for _ in 0..3 {
        expected.extend_from_slice(&[0x10, 0, 0, 0, 0, 0, 0, 0, 0]);
    }
    expected.extend_from_slice(&[0x14, 3, 0, 0, 0, 0x13, 0]);
    assert_eq!(encode(&value), expected);
}
