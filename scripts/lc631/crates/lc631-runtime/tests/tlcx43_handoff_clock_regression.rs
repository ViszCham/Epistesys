//! Local signed test receipts only; no claim of actual host provenance/delivery.
use lc631_core::{stable_sha256, ArtifactId, SourceSpan, TurnId};
use lc631_host::{
    host_output_stage_payload_digest, host_output_stage_scope, host_seed_payload_digest,
    HostOutputCandidate, HostOutputLifecycleState, HostOutputStage, HostOutputStageRequest,
    HostSeedEnvelope,
};
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptIssuer, ReceiptScope, ReceiptVerifier, ReplayGuard, SubjectRevision,
    UntrustedReceipt,
};
use lc631_runtime::handoff::identity::ReportedPackageIdentity;
use lc631_runtime::handoff::{ExternalStage, HandoffExpectation, HandoffSession};
use lc631_substrate::Digest32;

const SEED: &str = "original source";
const OUTPUT: &str = "exact local output";

fn stage_receipt(
    issuer: &ReceiptIssuer,
    seed: &HostSeedEnvelope,
    candidate: &HostOutputCandidate,
    stage: HostOutputStage,
    payload: &str,
    expiry: u64,
) -> UntrustedReceipt {
    let request = HostOutputStageRequest {
        candidate,
        stage,
        callback_payload: payload,
        sink_id: None,
        delivered: None,
        attestation: None,
    };
    issuer
        .issue(
            ReceiptClass::HostOutput,
            SubjectRevision::checked(stable_sha256(&format!(
                "{}:{}:{}",
                candidate.artifact_id.0, candidate.turn_id.0, candidate.candidate_digest
            )))
            .unwrap(),
            ReceiptScope::checked(
                host_output_stage_scope(candidate.artifact_id, candidate.turn_id, stage).unwrap(),
            )
            .unwrap(),
            host_output_stage_payload_digest(seed, &request).unwrap(),
            10,
            Some(expiry),
            None,
        )
        .unwrap()
}

fn stage(stage: HostOutputStage, payload: &str, receipt: UntrustedReceipt) -> ExternalStage<'_> {
    ExternalStage {
        stage,
        callback_payload: payload,
        sink_id: None,
        delivered: None,
        attestation: Some(receipt),
    }
}

#[test]
fn tlcx43_regressed_clock_cannot_revive_expired_postsend_attestation() {
    let issuer = ReceiptIssuer::from_key_bytes("tlcx43-clock-fixture", &[31; 32]).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("tlcx43-clock-fixture", &[31; 32]).unwrap();
    let artifact = ArtifactId(43);
    let turn = TurnId(2);
    let span = SourceSpan::checked(SEED, 0, SEED.len()).unwrap();
    let seed_wire = issuer
        .issue(
            ReceiptClass::HostSeed,
            SubjectRevision::checked(stable_sha256(SEED)).unwrap(),
            ReceiptScope::checked("host/user-span/43/2").unwrap(),
            host_seed_payload_digest(artifact, turn, SEED, span),
            10,
            Some(100),
            None,
        )
        .unwrap();
    let seed = HostSeedEnvelope::verify_attested(
        artifact,
        turn,
        SEED.into(),
        span,
        seed_wire,
        &verifier,
        &mut ReplayGuard::default(),
        10,
    )
    .unwrap();
    let candidate = HostOutputCandidate {
        artifact_id: artifact,
        turn_id: turn,
        output_text: OUTPUT.into(),
        candidate_digest: stable_sha256(OUTPUT),
    };
    let package = ReportedPackageIdentity::new(
        "6.3.2-alpha.3",
        [1, 2, 3, 4, 5].map(|n| Digest32::from_bytes([n; 32])),
    )
    .unwrap();
    let expected = HandoffExpectation::new(package.clone(), SEED, OUTPUT, artifact, turn).unwrap();
    let mut session = HandoffSession::begin(&expected, &package, &seed, &candidate).unwrap();
    let mut replay = ReplayGuard::default();
    let pre = stage_receipt(
        &issuer,
        &seed,
        &candidate,
        HostOutputStage::PreCommitCandidate,
        "pre",
        90,
    );
    session
        .observe_external_stage(
            stage(HostOutputStage::PreCommitCandidate, "pre", pre),
            &verifier,
            &mut replay,
            80,
        )
        .unwrap();
    let expired_at_80 = stage_receipt(
        &issuer,
        &seed,
        &candidate,
        HostOutputStage::PostSendObserved,
        "old-post",
        30,
    );
    let before = replay.observed_count();
    assert!(session
        .observe_external_stage(
            stage(HostOutputStage::PostSendObserved, "old-post", expired_at_80),
            &verifier,
            &mut replay,
            20
        )
        .is_err());
    assert_eq!(replay.observed_count(), before);
    assert_eq!(session.inspect().authenticated_stage_count, 1);
    assert_eq!(
        session.inspect().host_stage_state,
        HostOutputLifecycleState::AwaitingPostSend
    );
    let current = stage_receipt(
        &issuer,
        &seed,
        &candidate,
        HostOutputStage::PostSendObserved,
        "current-post",
        90,
    );
    session
        .observe_external_stage(
            stage(HostOutputStage::PostSendObserved, "current-post", current),
            &verifier,
            &mut replay,
            81,
        )
        .unwrap();
    assert_eq!(session.inspect().authenticated_stage_count, 2);
}
