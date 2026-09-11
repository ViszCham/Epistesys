use lc631_core::{stable_sha256, ObligationId, SourceSpan};
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptIssuer, ReceiptScope, ReceiptVerifier, ReplayGuard, SubjectRevision,
};
use lc631_tl::{
    verifier_capability_payload_digest, CanonicalTranslationEnvelope, CapabilityDescriptorV3,
    Obligation, ObligationKind, ObligationPolarity, ObligationStrength, PreservationState,
    ProjectionDefectGraphV3, ProjectionEdgeId, ProjectionEdgeV3, ProjectionGateDecision,
    ProjectionStage, SourceAnchorV3, SourceLedgerId, TargetClaim, TargetClaimId,
    VerificationReceipt, VerificationStatus, VerifierKind,
};

fn obligation(source: &str) -> Obligation {
    Obligation {
        id: ObligationId(1),
        kind: ObligationKind::Semantic,
        strength: ObligationStrength::Must,
        polarity: ObligationPolarity::Positive,
        scope: "answer".into(),
        source_span: SourceSpan::checked(source, 0, source.len()).unwrap(),
        source_text: source.into(),
    }
}

#[test]
fn arc631_07_anchor_from_another_ledger_is_rejected() {
    let source = "answer exactly";
    let mut graph = ProjectionDefectGraphV3::new(SourceLedgerId(1), "source-r1", source).unwrap();
    graph.add_obligation(obligation(source)).unwrap();
    graph
        .add_target_claim(
            TargetClaim::checked(
                TargetClaimId(1),
                "candidate",
                "answer",
                "candidate-r1",
                "answer exactly",
            )
            .unwrap(),
        )
        .unwrap();
    let swapped =
        SourceAnchorV3::checked(SourceLedgerId(2), "source-r1", source, 0, source.len()).unwrap();
    assert!(graph
        .add_edge(ProjectionEdgeV3 {
            id: ProjectionEdgeId(1),
            obligation_id: Some(ObligationId(1)),
            source: Some(swapped),
            targets: vec![TargetClaimId(1)],
            stage: ProjectionStage::SeedToContract,
            state: PreservationState::Exact,
            rule_id: "copy".into(),
            verifier: VerificationReceipt {
                verifier: VerifierKind::SourceSpan,
                status: VerificationStatus::Passed,
                revision: "span-r1".into(),
                evidence_digest: Some(stable_sha256(source)),
            },
        })
        .is_err());
}

#[test]
fn arc631_08_candidate_commit_requires_authenticated_capability_and_every_stage() {
    let source = "answer exactly";
    let mut graph = ProjectionDefectGraphV3::new(SourceLedgerId(1), "source-r1", source).unwrap();
    graph.add_obligation(obligation(source)).unwrap();
    let anchor = graph.obligation_anchor(ObligationId(1)).unwrap().clone();
    graph
        .add_target_claim(
            TargetClaim::checked(
                TargetClaimId(1),
                "candidate",
                "answer",
                "candidate-r1",
                "answer exactly",
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(graph.gate(), ProjectionGateDecision::Clarify);

    let descriptor = CapabilityDescriptorV3 {
        verifier: VerifierKind::SourceSpan,
        obligation_kind: ObligationKind::Semantic,
        revision: "span-r1".into(),
        stages: ProjectionDefectGraphV3::required_output_stages(),
        exact_allowed: true,
    };
    let key = [37_u8; 32];
    let issuer = ReceiptIssuer::from_key_bytes("lc631-tl-root", &key).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("lc631-tl-root", &key).unwrap();
    let wire = issuer
        .issue(
            ReceiptClass::VerifierCapability,
            SubjectRevision::checked("source-r1").unwrap(),
            ReceiptScope::checked("tl/verifier/source_span/semantic").unwrap(),
            verifier_capability_payload_digest(&descriptor),
            1,
            None,
            None,
        )
        .unwrap();
    graph
        .register_capability(descriptor, wire, &verifier, &mut ReplayGuard::default(), 1)
        .unwrap();
    for (index, stage) in ProjectionDefectGraphV3::required_output_stages()
        .into_iter()
        .enumerate()
    {
        graph
            .add_edge(ProjectionEdgeV3 {
                id: ProjectionEdgeId(index as u64 + 1),
                obligation_id: Some(ObligationId(1)),
                source: Some(anchor.clone()),
                targets: vec![TargetClaimId(1)],
                stage,
                state: PreservationState::Exact,
                rule_id: format!("copy-{index}"),
                verifier: VerificationReceipt {
                    verifier: VerifierKind::SourceSpan,
                    status: VerificationStatus::Passed,
                    revision: "span-r1".into(),
                    evidence_digest: Some(stable_sha256(&format!("edge-{index}"))),
                },
            })
            .unwrap();
    }
    assert_eq!(graph.gate(), ProjectionGateDecision::CandidateCommit);
    let envelope = CanonicalTranslationEnvelope::from_graph(&graph, vec![]);
    assert_eq!(envelope.gate, ProjectionGateDecision::CandidateCommit);
}

#[test]
fn arc631_07_target_only_claim_is_retained_without_fake_obligation() {
    let source = "answer";
    let mut graph = ProjectionDefectGraphV3::new(SourceLedgerId(1), "source-r1", source).unwrap();
    graph.add_obligation(obligation(source)).unwrap();
    graph
        .add_target_claim(
            TargetClaim::checked(
                TargetClaimId(9),
                "candidate",
                "extra",
                "candidate-r1",
                "unsupported claim",
            )
            .unwrap(),
        )
        .unwrap();
    graph
        .add_edge(ProjectionEdgeV3 {
            id: ProjectionEdgeId(9),
            obligation_id: None,
            source: None,
            targets: vec![TargetClaimId(9)],
            stage: ProjectionStage::ProgramToCandidate,
            state: PreservationState::IntroducedWithoutSource,
            rule_id: "introduced".into(),
            verifier: VerificationReceipt {
                verifier: VerifierKind::Schema,
                status: VerificationStatus::Failed,
                revision: "schema-r1".into(),
                evidence_digest: Some(stable_sha256("introduced")),
            },
        })
        .unwrap();
    assert_eq!(graph.introduced_target_claims(), vec![TargetClaimId(9)]);
    assert_eq!(graph.gate(), ProjectionGateDecision::HardHold);
}
