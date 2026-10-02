#![forbid(unsafe_code)]

use lc631_core::{stable_sha256, ArtifactId, TurnId};
use lc631_host::{
    host_output_stage_payload_digest, host_output_stage_scope, host_seed_payload_digest,
    observe_host_output_stage_attested, HostError, HostOutputCandidate, HostOutputLifecycleState,
    HostOutputStage, HostOutputStageLedger, HostOutputStageRequest, HostSeedEnvelope,
};
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptIssuer, ReceiptScope, ReceiptVerifier, ReplayGuard, SubjectRevision,
};

fn trusted_context() -> (ReceiptIssuer, ReceiptVerifier, HostSeedEnvelope) {
    let key = [181_u8; 32];
    let issuer = ReceiptIssuer::from_key_bytes("host-stage-root", &key).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("host-stage-root", &key).unwrap();
    let raw = "send exact candidate output".to_string();
    let span = lc631_core::SourceSpan::checked(&raw, 0, raw.len()).unwrap();
    let artifact = ArtifactId(17);
    let turn = TurnId(4);
    let payload = host_seed_payload_digest(artifact, turn, &raw, span);
    let subject = SubjectRevision::checked(stable_sha256(&raw)).unwrap();
    let scope = ReceiptScope::checked("host/user-span/17/4").unwrap();
    let receipt = issuer
        .issue(
            ReceiptClass::HostSeed,
            subject,
            scope,
            payload,
            10,
            Some(500),
            None,
        )
        .unwrap();
    let seed = HostSeedEnvelope::verify_attested(
        artifact,
        turn,
        raw,
        span,
        receipt,
        &verifier,
        &mut ReplayGuard::default(),
        10,
    )
    .unwrap();
    (issuer, verifier, seed)
}

fn stage_receipt(
    issuer: &ReceiptIssuer,
    verifier: &ReceiptVerifier,
    seed: &HostSeedEnvelope,
    mut request: HostOutputStageRequest<'_>,
) -> lc631_host::HostOutputStageReceipt {
    let payload = host_output_stage_payload_digest(seed, &request).unwrap();
    let candidate = request.candidate;
    let output_digest = stable_sha256(&candidate.output_text);
    let subject = SubjectRevision::checked(stable_sha256(&format!(
        "{}:{}:{}",
        candidate.artifact_id.0, candidate.turn_id.0, output_digest
    )))
    .unwrap();
    let scope = ReceiptScope::checked(
        host_output_stage_scope(candidate.artifact_id, candidate.turn_id, request.stage).unwrap(),
    )
    .unwrap();
    request.attestation = Some(
        issuer
            .issue(
                ReceiptClass::HostOutput,
                subject,
                scope,
                payload,
                20,
                Some(300),
                None,
            )
            .unwrap(),
    );
    observe_host_output_stage_attested(seed, request, verifier, &mut ReplayGuard::default(), 21)
        .unwrap()
}

#[test]
fn host_output_stages_are_exactly_ordered_and_not_collapsed() {
    let (issuer, verifier, seed) = trusted_context();
    let output = "the full candidate";
    let candidate = HostOutputCandidate {
        artifact_id: ArtifactId(17),
        turn_id: TurnId(4),
        candidate_digest: stable_sha256(output),
        output_text: output.into(),
    };
    let mut ledger = HostOutputStageLedger::new(&candidate).unwrap();
    let precommit = stage_receipt(
        &issuer,
        &verifier,
        &seed,
        HostOutputStageRequest {
            candidate: &candidate,
            stage: HostOutputStage::PreCommitCandidate,
            callback_payload: "host precommit callback",
            sink_id: None,
            delivered: None,
            attestation: None,
        },
    );
    ledger.append(precommit).unwrap();
    assert_eq!(ledger.state(), HostOutputLifecycleState::AwaitingPostSend);

    let sink = stage_receipt(
        &issuer,
        &verifier,
        &seed,
        HostOutputStageRequest {
            candidate: &candidate,
            stage: HostOutputStage::SinkDeliveryObserved,
            callback_payload: "host reports sink delivery",
            sink_id: Some("discord-channel-1"),
            delivered: Some(true),
            attestation: None,
        },
    );
    assert_eq!(ledger.append(sink), Err(HostError::HostOutputStageConflict));
    assert_eq!(ledger.stages().len(), 1);

    let post_send = stage_receipt(
        &issuer,
        &verifier,
        &seed,
        HostOutputStageRequest {
            candidate: &candidate,
            stage: HostOutputStage::PostSendObserved,
            callback_payload: "host reports send attempt",
            sink_id: None,
            delivered: None,
            attestation: None,
        },
    );
    ledger.append(post_send).unwrap();
    assert_eq!(
        ledger.state(),
        HostOutputLifecycleState::AwaitingSinkDelivery
    );
    let delivered = stage_receipt(
        &issuer,
        &verifier,
        &seed,
        HostOutputStageRequest {
            candidate: &candidate,
            stage: HostOutputStage::SinkDeliveryObserved,
            callback_payload: "host reports exact sink receipt",
            sink_id: Some("discord-channel-1"),
            delivered: Some(true),
            attestation: None,
        },
    );
    ledger.append(delivered).unwrap();
    assert_eq!(ledger.state(), HostOutputLifecycleState::Delivered);
}

#[test]
fn delivery_failure_is_distinct_and_precommit_receipt_cannot_be_promoted() {
    let (issuer, verifier, seed) = trusted_context();
    let output = "candidate bytes";
    let candidate = HostOutputCandidate {
        artifact_id: ArtifactId(17),
        turn_id: TurnId(4),
        candidate_digest: stable_sha256(output),
        output_text: output.into(),
    };
    let precommit = stage_receipt(
        &issuer,
        &verifier,
        &seed,
        HostOutputStageRequest {
            candidate: &candidate,
            stage: HostOutputStage::PreCommitCandidate,
            callback_payload: "precommit callback",
            sink_id: None,
            delivered: None,
            attestation: None,
        },
    );
    let forged_stage = HostOutputStageRequest {
        candidate: &candidate,
        stage: HostOutputStage::PostSendObserved,
        callback_payload: "precommit callback",
        sink_id: None,
        delivered: None,
        attestation: Some(precommit.authenticity_attestation().clone()),
    };
    assert_eq!(
        observe_host_output_stage_attested(
            &seed,
            forged_stage.clone(),
            &verifier,
            &mut ReplayGuard::default(),
            21,
        ),
        Err(HostError::ReceiptRejected)
    );
    let mut ledger = HostOutputStageLedger::new(&candidate).unwrap();
    ledger.append(precommit).unwrap();
    ledger
        .append(stage_receipt(
            &issuer,
            &verifier,
            &seed,
            HostOutputStageRequest {
                candidate: &candidate,
                stage: HostOutputStage::PostSendObserved,
                callback_payload: "send was attempted",
                sink_id: None,
                delivered: None,
                attestation: None,
            },
        ))
        .unwrap();
    ledger
        .append(stage_receipt(
            &issuer,
            &verifier,
            &seed,
            HostOutputStageRequest {
                candidate: &candidate,
                stage: HostOutputStage::SinkDeliveryObserved,
                callback_payload: "sink rejected exact candidate",
                sink_id: Some("discord-channel-1"),
                delivered: Some(false),
                attestation: None,
            },
        ))
        .unwrap();
    assert_eq!(ledger.state(), HostOutputLifecycleState::DeliveryFailed);
}

#[test]
fn a16_exact_callback_and_candidate_cut_then_restore_do_not_change_turn_identity() {
    let (issuer, verifier, seed) = trusted_context();
    let candidate = HostOutputCandidate {
        artifact_id: ArtifactId(17),
        turn_id: TurnId(4),
        output_text: "exact standalone candidate".into(),
        candidate_digest: stable_sha256("exact standalone candidate"),
    };
    let receipt = stage_receipt(
        &issuer,
        &verifier,
        &seed,
        HostOutputStageRequest {
            candidate: &candidate,
            stage: HostOutputStage::PreCommitCandidate,
            callback_payload: "exact callback bytes",
            sink_id: None,
            delivered: None,
            attestation: None,
        },
    );
    let changed = HostOutputCandidate {
        candidate_digest: stable_sha256("substituted candidate"),
        output_text: "substituted candidate".into(),
        ..candidate.clone()
    };
    for (input, callback) in [
        (&candidate, "wrong callback bytes"),
        (&changed, "exact callback bytes"),
    ] {
        assert!(observe_host_output_stage_attested(
            &seed,
            HostOutputStageRequest {
                candidate: input,
                stage: HostOutputStage::PreCommitCandidate,
                callback_payload: callback,
                sink_id: None,
                delivered: None,
                attestation: Some(receipt.authenticity_attestation().clone())
            },
            &verifier,
            &mut ReplayGuard::default(),
            21
        )
        .is_err());
    }
    let restored = observe_host_output_stage_attested(
        &seed,
        HostOutputStageRequest {
            candidate: &candidate,
            stage: HostOutputStage::PreCommitCandidate,
            callback_payload: "exact callback bytes",
            sink_id: None,
            delivered: None,
            attestation: Some(receipt.authenticity_attestation().clone()),
        },
        &verifier,
        &mut ReplayGuard::default(),
        21,
    )
    .unwrap();
    let mut ledger = HostOutputStageLedger::new(&candidate).unwrap();
    ledger.append(restored).unwrap();
    assert_eq!(candidate.turn_id, TurnId(4));
    assert_eq!(ledger.state(), HostOutputLifecycleState::AwaitingPostSend);
}
