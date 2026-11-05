//! Local signed fixtures exercise the registered source contract only.
//! They are not real host callbacks, delivery observations, or authority.
use lc631_core::{stable_sha256, ArtifactId, SourceSpan, TurnId};
use lc631_host::tlcx_output_lifecycle::{
    CallbackObservation, DurableReplayState, ExactOutputLifecycle, HostIntegrationState,
    LifecycleError,
};
use lc631_host::{
    host_output_stage_payload_digest, host_output_stage_scope, host_seed_payload_digest,
    HostOutputLifecycleState, HostOutputStage, HostOutputStageRequest, HostSeedEnvelope,
};
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptIssuer, ReceiptScope, ReceiptVerifier, ReplayGuard, SubjectRevision,
    UntrustedReceipt,
};

fn fixture(seed_text: &str) -> (ReceiptIssuer, ReceiptVerifier, HostSeedEnvelope) {
    let key = [135; 32];
    let issuer = ReceiptIssuer::from_key_bytes("tlcx35-contract-fixture", &key).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("tlcx35-contract-fixture", &key).unwrap();
    let artifact = ArtifactId(35);
    let turn = TurnId(2);
    let span = SourceSpan::checked(seed_text, 0, seed_text.len()).unwrap();
    let wire = issuer
        .issue(
            ReceiptClass::HostSeed,
            SubjectRevision::checked(stable_sha256(seed_text)).unwrap(),
            ReceiptScope::checked("host/user-span/35/2").unwrap(),
            host_seed_payload_digest(artifact, turn, seed_text, span),
            10,
            Some(100),
            None,
        )
        .unwrap();
    let seed = HostSeedEnvelope::verify_attested(
        artifact,
        turn,
        seed_text.into(),
        span,
        wire,
        &verifier,
        &mut ReplayGuard::default(),
        10,
    )
    .unwrap();
    (issuer, verifier, seed)
}

fn fixture_attestation(
    issuer: &ReceiptIssuer,
    seed: &HostSeedEnvelope,
    lifecycle: &ExactOutputLifecycle<'_>,
    stage: HostOutputStage,
    payload: &str,
    delivered: Option<bool>,
) -> UntrustedReceipt {
    let candidate = lifecycle.candidate();
    let sink = delivered.map(|_| "fixture-sink");
    let request = HostOutputStageRequest {
        candidate,
        stage,
        callback_payload: payload,
        sink_id: sink,
        delivered,
        attestation: None,
    };
    let payload_digest = host_output_stage_payload_digest(seed, &request).unwrap();
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
            payload_digest,
            20,
            Some(90),
            None,
        )
        .unwrap()
}

fn callback<'a>(
    stage: HostOutputStage,
    output: &'a str,
    payload: &'a str,
    delivered: Option<bool>,
    attestation: Option<UntrustedReceipt>,
) -> CallbackObservation<'a> {
    CallbackObservation {
        stage,
        observed_output: output,
        callback_payload: payload,
        sink_id: delivered.map(|_| "fixture-sink"),
        delivered,
        attestation,
    }
}

#[test]
fn tlcx35_missing_callbacks_stay_pending_without_issuing_receipts() {
    let (_, _, seed) = fixture("render this decision");
    let lifecycle = ExactOutputLifecycle::new(&seed, "日本語\nEnglish\n", 1024).unwrap();
    assert_eq!(lifecycle.exact_output(), "日本語\nEnglish\n");
    assert_eq!(
        lifecycle.expected_stage(),
        Some(HostOutputStage::PreCommitCandidate)
    );
    assert_eq!(
        lifecycle.state(),
        HostOutputLifecycleState::AwaitingPreCommit
    );
    assert_eq!(
        lifecycle.host_integration(),
        HostIntegrationState::HostIntegrationPending
    );
    assert_eq!(lifecycle.durable_replay(), DurableReplayState::Unobserved);
    assert!(lifecycle.receipts().is_empty());
}

#[test]
fn tlcx35_local_contract_callbacks_are_ordered_but_never_real_host_evidence() {
    let (issuer, verifier, seed) = fixture("render this decision");
    let output = "日本語\nEnglish\n";
    let mut lifecycle = ExactOutputLifecycle::new(&seed, output, 1024).unwrap();
    let mut replay = ReplayGuard::default();
    for (stage, payload, delivered, expected) in [
        (
            HostOutputStage::PreCommitCandidate,
            "fixture/precommit",
            None,
            HostOutputLifecycleState::AwaitingPostSend,
        ),
        (
            HostOutputStage::PostSendObserved,
            "fixture/postsend",
            None,
            HostOutputLifecycleState::AwaitingSinkDelivery,
        ),
        (
            HostOutputStage::SinkDeliveryObserved,
            "fixture/sink",
            Some(true),
            HostOutputLifecycleState::Delivered,
        ),
    ] {
        let wire = fixture_attestation(&issuer, &seed, &lifecycle, stage, payload, delivered);
        lifecycle
            .accept_callback(
                callback(stage, output, payload, delivered, Some(wire)),
                &verifier,
                &mut replay,
                21,
            )
            .unwrap();
        assert_eq!(lifecycle.state(), expected);
        assert_eq!(
            lifecycle.host_integration(),
            HostIntegrationState::HostIntegrationPending
        );
        assert_eq!(lifecycle.durable_replay(), DurableReplayState::Unobserved);
    }
    assert_eq!(lifecycle.receipts().len(), 3);
    assert_eq!(lifecycle.expected_stage(), None);
}

#[test]
fn tlcx35_byte_substitution_is_rejected_before_nonce_consumption() {
    let (issuer, verifier, seed) = fixture("render this decision");
    let mut lifecycle = ExactOutputLifecycle::new(&seed, "exact\n", 1024).unwrap();
    let stage = HostOutputStage::PreCommitCandidate;
    let wire = fixture_attestation(&issuer, &seed, &lifecycle, stage, "callback", None);
    let mut replay = ReplayGuard::default();
    for altered in ["exact", "exact\r\n", "exact\n ", "Exact\n"] {
        assert_eq!(
            lifecycle.accept_callback(
                callback(stage, altered, "callback", None, Some(wire.clone())),
                &verifier,
                &mut replay,
                21
            ),
            Err(LifecycleError::OutputMismatch)
        );
        assert!(lifecycle.receipts().is_empty());
    }
    lifecycle
        .accept_callback(
            callback(stage, "exact\n", "callback", None, Some(wire)),
            &verifier,
            &mut replay,
            21,
        )
        .unwrap();
}

#[test]
fn tlcx35_stage_swap_is_rejected_before_nonce_consumption() {
    let (issuer, verifier, seed) = fixture("render this decision");
    let mut lifecycle = ExactOutputLifecycle::new(&seed, "exact", 1024).unwrap();
    let post = HostOutputStage::PostSendObserved;
    let wire = fixture_attestation(&issuer, &seed, &lifecycle, post, "post", None);
    let mut replay = ReplayGuard::default();
    assert_eq!(
        lifecycle.accept_callback(
            callback(post, "exact", "post", None, Some(wire.clone())),
            &verifier,
            &mut replay,
            21
        ),
        Err(LifecycleError::StageOrder)
    );
    let pre = HostOutputStage::PreCommitCandidate;
    let pre_wire = fixture_attestation(&issuer, &seed, &lifecycle, pre, "pre", None);
    lifecycle
        .accept_callback(
            callback(pre, "exact", "pre", None, Some(pre_wire)),
            &verifier,
            &mut replay,
            21,
        )
        .unwrap();
    lifecycle
        .accept_callback(
            callback(post, "exact", "post", None, Some(wire)),
            &verifier,
            &mut replay,
            22,
        )
        .unwrap();
}

#[test]
fn tlcx35_seed_swap_missing_attestation_and_expiry_never_advance() {
    let (issuer, verifier, first) = fixture("first source");
    let (_, _, second) = fixture("second source");
    let mut lifecycle = ExactOutputLifecycle::new(&first, "exact", 1024).unwrap();
    let alien = ExactOutputLifecycle::new(&second, "exact", 1024).unwrap();
    let stage = HostOutputStage::PreCommitCandidate;
    let wire = fixture_attestation(&issuer, &second, &alien, stage, "event", None);
    let mut replay = ReplayGuard::default();
    assert!(lifecycle
        .accept_callback(
            callback(stage, "exact", "event", None, Some(wire)),
            &verifier,
            &mut replay,
            21
        )
        .is_err());
    assert!(lifecycle
        .accept_callback(
            callback(stage, "exact", "event", None, None),
            &verifier,
            &mut replay,
            21
        )
        .is_err());
    let valid = fixture_attestation(&issuer, &first, &lifecycle, stage, "event", None);
    assert!(lifecycle
        .accept_callback(
            callback(stage, "exact", "event", None, Some(valid)),
            &verifier,
            &mut replay,
            100
        )
        .is_err());
    assert!(lifecycle.receipts().is_empty());
}

#[test]
fn tlcx35_failed_delivery_is_terminal_and_clock_regression_is_visible() {
    let (issuer, verifier, seed) = fixture("render this decision");
    let mut lifecycle = ExactOutputLifecycle::new(&seed, "exact", 1024).unwrap();
    let mut replay = ReplayGuard::default();
    for (stage, payload, delivered, time) in [
        (HostOutputStage::PreCommitCandidate, "pre", None, 22),
        (HostOutputStage::PostSendObserved, "post", None, 23),
        (
            HostOutputStage::SinkDeliveryObserved,
            "sink",
            Some(false),
            24,
        ),
    ] {
        let wire = fixture_attestation(&issuer, &seed, &lifecycle, stage, payload, delivered);
        if stage == HostOutputStage::PostSendObserved {
            assert_eq!(
                lifecycle.accept_callback(
                    callback(stage, "exact", payload, delivered, Some(wire.clone())),
                    &verifier,
                    &mut replay,
                    21
                ),
                Err(LifecycleError::ClockRegression)
            );
        }
        lifecycle
            .accept_callback(
                callback(stage, "exact", payload, delivered, Some(wire)),
                &verifier,
                &mut replay,
                time,
            )
            .unwrap();
    }
    assert_eq!(lifecycle.state(), HostOutputLifecycleState::DeliveryFailed);
    assert_eq!(lifecycle.expected_stage(), None);
    assert_eq!(
        lifecycle.accept_callback(
            callback(
                HostOutputStage::SinkDeliveryObserved,
                "exact",
                "retry",
                Some(true),
                None
            ),
            &verifier,
            &mut replay,
            25
        ),
        Err(LifecycleError::StageOrder)
    );
}

#[test]
fn tlcx35_output_budget_is_checked_before_capture() {
    let (_, _, seed) = fixture("render this decision");
    for (text, cap) in [("", 1024), ("a", 0), ("a", 65_537), ("日本", 5)] {
        assert!(matches!(
            ExactOutputLifecycle::new(&seed, text, cap),
            Err(LifecycleError::OutputBudget)
        ));
    }
    assert!(ExactOutputLifecycle::new(&seed, "日本", 6).is_ok());
}
