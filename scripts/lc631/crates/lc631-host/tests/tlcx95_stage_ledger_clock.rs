//! Caller-clock ordering tests for a local contract, not real host events.
#![forbid(unsafe_code)]

use lc631_core::{stable_sha256, ArtifactId, SourceSpan, TurnId};
use lc631_host::{
    host_output_stage_payload_digest, host_output_stage_scope, host_seed_payload_digest,
    observe_host_output_stage_attested, HostError, HostOutputCandidate, HostOutputLifecycleState,
    HostOutputStage, HostOutputStageLedger, HostOutputStageReceipt, HostOutputStageRequest,
    HostSeedEnvelope,
};
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptIssuer, ReceiptScope, ReceiptVerifier, ReplayGuard, SubjectRevision,
};

struct Fixture {
    issuer: ReceiptIssuer,
    verifier: ReceiptVerifier,
    seed: HostSeedEnvelope,
    candidate: HostOutputCandidate,
}

impl Fixture {
    fn new() -> Self {
        let issuer = ReceiptIssuer::from_key_bytes("tlcx95-clock-fixture", &[94; 32]).unwrap();
        let verifier = ReceiptVerifier::from_key_bytes("tlcx95-clock-fixture", &[94; 32]).unwrap();
        let raw = "synthetic stage contract";
        let artifact = ArtifactId(95);
        let turn = TurnId(1);
        let span = SourceSpan::checked(raw, 0, raw.len()).unwrap();
        let wire = issuer
            .issue(
                ReceiptClass::HostSeed,
                SubjectRevision::checked(stable_sha256(raw)).unwrap(),
                ReceiptScope::checked("host/user-span/95/1").unwrap(),
                host_seed_payload_digest(artifact, turn, raw, span),
                10,
                Some(100),
                None,
            )
            .unwrap();
        let seed = HostSeedEnvelope::verify_attested(
            artifact,
            turn,
            raw.into(),
            span,
            wire,
            &verifier,
            &mut ReplayGuard::default(),
            10,
        )
        .unwrap();
        let candidate = HostOutputCandidate {
            artifact_id: artifact,
            turn_id: turn,
            candidate_digest: stable_sha256("synthetic exact output"),
            output_text: "synthetic exact output".into(),
        };
        Self {
            issuer,
            verifier,
            seed,
            candidate,
        }
    }

    fn observe(&self, stage: HostOutputStage, now: u64) -> HostOutputStageReceipt {
        let sink = (stage == HostOutputStage::SinkDeliveryObserved).then_some("synthetic-sink");
        let delivered = sink.map(|_| true);
        let mut request = HostOutputStageRequest {
            candidate: &self.candidate,
            stage,
            callback_payload: "synthetic callback",
            sink_id: sink,
            delivered,
            attestation: None,
        };
        let digest = host_output_stage_payload_digest(&self.seed, &request).unwrap();
        request.attestation = Some(
            self.issuer
                .issue(
                    ReceiptClass::HostOutput,
                    SubjectRevision::checked(stable_sha256(&format!(
                        "95:1:{}",
                        self.candidate.candidate_digest
                    )))
                    .unwrap(),
                    ReceiptScope::checked(
                        host_output_stage_scope(ArtifactId(95), TurnId(1), stage).unwrap(),
                    )
                    .unwrap(),
                    digest,
                    20,
                    Some(90),
                    None,
                )
                .unwrap(),
        );
        observe_host_output_stage_attested(
            &self.seed,
            request,
            &self.verifier,
            &mut ReplayGuard::default(),
            now,
        )
        .unwrap()
    }
}

#[test]
fn tlcx95_direct_ledger_rejects_regressing_postsend_observation_without_mutation() {
    let fixture = Fixture::new();
    let mut ledger = HostOutputStageLedger::new(&fixture.candidate).unwrap();
    let pre = fixture.observe(HostOutputStage::PreCommitCandidate, 30);
    ledger.append(pre.clone()).unwrap();
    let before = serde_json::to_value(&ledger).unwrap();
    // Both receipts are individually valid at their supplied times. Appending
    // them through the old public consumer must not bypass session ordering.
    let earlier = fixture.observe(HostOutputStage::PostSendObserved, 21);
    assert_eq!(
        ledger.append(earlier),
        Err(HostError::HostOutputStageConflict)
    );
    assert_eq!(serde_json::to_value(&ledger).unwrap(), before);
    ledger
        .append(fixture.observe(HostOutputStage::PostSendObserved, 31))
        .unwrap();
    assert_eq!(
        ledger.state(),
        HostOutputLifecycleState::AwaitingSinkDelivery
    );
}

#[test]
fn tlcx95_direct_ledger_rejects_regressing_sink_observation_without_delivery() {
    let fixture = Fixture::new();
    let mut ledger = HostOutputStageLedger::new(&fixture.candidate).unwrap();
    ledger
        .append(fixture.observe(HostOutputStage::PreCommitCandidate, 25))
        .unwrap();
    ledger
        .append(fixture.observe(HostOutputStage::PostSendObserved, 30))
        .unwrap();
    let before = serde_json::to_value(&ledger).unwrap();
    let earlier = fixture.observe(HostOutputStage::SinkDeliveryObserved, 29);
    assert_eq!(
        ledger.append(earlier),
        Err(HostError::HostOutputStageConflict)
    );
    assert_eq!(serde_json::to_value(&ledger).unwrap(), before);
    assert_eq!(
        ledger.state(),
        HostOutputLifecycleState::AwaitingSinkDelivery
    );
}

#[test]
fn tlcx95_nondecreasing_direct_ledger_preserves_the_existing_twelve_field_wire() {
    let fixture = Fixture::new();
    let pre = fixture.observe(HostOutputStage::PreCommitCandidate, 30);
    let later = observe_host_output_stage_attested(
        &fixture.seed,
        HostOutputStageRequest {
            candidate: &fixture.candidate,
            stage: HostOutputStage::PreCommitCandidate,
            callback_payload: "synthetic callback",
            sink_id: None,
            delivered: None,
            attestation: Some(pre.authenticity_attestation().clone()),
        },
        &fixture.verifier,
        &mut ReplayGuard::default(),
        31,
    )
    .unwrap();
    assert_eq!(
        pre, later,
        "legacy equality compares claims, not local observation time"
    );
    let wire = serde_json::to_value(&pre).unwrap();
    assert_eq!(serde_json::to_value(&later).unwrap(), wire);
    let mut keys: Vec<_> = wire
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort();
    let mut expected = [
        "schema_version",
        "artifact_id",
        "turn_id",
        "candidate_digest",
        "output_digest",
        "stage",
        "callback_event_digest",
        "sink_id",
        "delivered",
        "host_seed_receipt_digest",
        "authenticity_receipt_digest",
        "authenticity_attestation",
    ];
    expected.sort();
    assert_eq!(keys, expected);
    let mut ledger = HostOutputStageLedger::new(&fixture.candidate).unwrap();
    ledger.append(pre.clone()).unwrap();
    ledger
        .append(fixture.observe(HostOutputStage::PostSendObserved, 30))
        .unwrap();
    ledger
        .append(fixture.observe(HostOutputStage::SinkDeliveryObserved, 31))
        .unwrap();
    assert_eq!(ledger.state(), HostOutputLifecycleState::Delivered);
    assert_eq!(ledger.stages().len(), 3);
    assert_eq!(serde_json::to_value(&ledger.stages()[0]).unwrap(), wire);
    // Delivered is the existing local contract state, never G13/HostBound.
}
