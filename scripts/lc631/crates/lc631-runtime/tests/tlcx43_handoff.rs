//! Synthetic material is used only for negative API tests, never real-host delivery evidence.
use lc631_core::{stable_sha256, ArtifactId, SourceSpan, TurnId};
use lc631_host::{
    HostError, HostOutputCandidate, HostOutputLifecycleState, HostOutputStage, HostSeedEnvelope,
};
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptIssuer, ReceiptScope, ReceiptVerifier, ReplayGuard, SubjectRevision,
};
use lc631_runtime::handoff::identity::{
    compare_reported_identity, IdentityError, PackageComponent, ReportedPackageIdentity,
};
use lc631_runtime::handoff::{
    ExternalStage, HandoffDisposition, HandoffError, HandoffExpectation, HandoffSession,
    PendingInput,
};
use lc631_substrate::Digest32;

const SEED: &str = "同じseed / same seed";
const OUTPUT: &str = "条件付き回答 / conditional answer";

fn pins() -> [Digest32; 5] {
    [1, 2, 3, 4, 5].map(|n| Digest32::from_bytes([n; 32]))
}
fn package() -> ReportedPackageIdentity {
    ReportedPackageIdentity::new("6.3.2-alpha.3", pins()).unwrap()
}
fn expectation() -> HandoffExpectation {
    HandoffExpectation::new(package(), SEED, OUTPUT, ArtifactId(43), TurnId(2)).unwrap()
}
fn candidate() -> HostOutputCandidate {
    HostOutputCandidate {
        artifact_id: ArtifactId(43),
        turn_id: TurnId(2),
        candidate_digest: stable_sha256(OUTPUT),
        output_text: OUTPUT.into(),
    }
}
fn unverified_seed() -> HostSeedEnvelope {
    HostSeedEnvelope::self_attested(ArtifactId(43), TurnId(2), SEED.into())
}
fn verifier() -> ReceiptVerifier {
    ReceiptVerifier::from_key_bytes("tlcx43-negative-fixture", &[7; 32]).unwrap()
}
fn negative_test_seed() -> HostSeedEnvelope {
    let issuer = ReceiptIssuer::from_key_bytes("tlcx43-negative-fixture", &[7; 32]).unwrap();
    let span = SourceSpan::checked(SEED, 0, SEED.len()).unwrap();
    let receipt = issuer
        .issue(
            ReceiptClass::HostSeed,
            SubjectRevision::checked(stable_sha256(SEED)).unwrap(),
            ReceiptScope::checked("host/user-span/43/2").unwrap(),
            lc631_host::host_seed_payload_digest(ArtifactId(43), TurnId(2), SEED, span),
            10,
            Some(20),
            None,
        )
        .unwrap();
    HostSeedEnvelope::verify_attested(
        ArtifactId(43),
        TurnId(2),
        SEED.into(),
        span,
        receipt,
        &verifier(),
        &mut ReplayGuard::default(),
        10,
    )
    .unwrap()
}
fn absent_precommit() -> ExternalStage<'static> {
    ExternalStage {
        stage: HostOutputStage::PreCommitCandidate,
        callback_payload: "negative fixture: no stage attestation",
        sink_id: None,
        delivered: None,
        attestation: None,
    }
}

#[test]
fn tlcx43_matching_reported_identity_remains_pending_without_host() {
    let seed = unverified_seed();
    let output = candidate();
    let session = HandoffSession::begin(&expectation(), &package(), &seed, &output).unwrap();
    let report = session.inspect();
    assert_eq!(
        report.host_stage_state,
        HostOutputLifecycleState::AwaitingPreCommit
    );
    assert_eq!(report.authenticated_stage_count, 0);
    assert_eq!(report.reported_package_label, package().content_label());
    let HandoffDisposition::Pending(inputs) = report.disposition;
    assert!(inputs.contains(&PendingInput::PackageIngressProvenance));
    assert!(inputs.contains(&PendingInput::FreshSessionProvenance));
    assert!(inputs.contains(&PendingInput::DurableReplayConsumer));
    assert!(inputs.contains(&PendingInput::ResumeConsumer));
    assert!(inputs.contains(&PendingInput::RequiredEdgesAndGates));
    assert!(inputs.contains(&PendingInput::HostStage(
        HostOutputLifecycleState::AwaitingPreCommit
    )));
}

#[test]
fn tlcx43_each_component_swap_is_rejected_before_handoff() {
    let components = [
        PackageComponent::Source,
        PackageComponent::Binary,
        PackageComponent::Plugin,
        PackageComponent::Schema,
        PackageComponent::Documentation,
    ];
    for (index, component) in components.into_iter().enumerate() {
        let mut altered = pins();
        altered[index] = Digest32::from_bytes([99; 32]);
        let changed = ReportedPackageIdentity::new(package().version(), altered).unwrap();
        assert_eq!(
            compare_reported_identity(&package(), &changed),
            Err(IdentityError::ComponentMismatch(component))
        );
        assert_ne!(changed.content_label(), package().content_label());
        assert_eq!(package().pin(component), pins()[index]);
    }
}

#[test]
fn tlcx43_version_and_component_order_are_bound_without_authenticity_claim() {
    let other = ReportedPackageIdentity::new("6.3.2-alpha.2", pins()).unwrap();
    assert_eq!(
        compare_reported_identity(&package(), &other),
        Err(IdentityError::VersionMismatch)
    );
    assert_eq!(
        ReportedPackageIdentity::new("", pins()),
        Err(IdentityError::InvalidVersion)
    );
    assert_eq!(
        ReportedPackageIdentity::new("bad\nversion", pins()),
        Err(IdentityError::InvalidVersion)
    );
    let mut swapped = pins();
    swapped.swap(0, 1);
    assert_ne!(
        ReportedPackageIdentity::new(package().version(), swapped)
            .unwrap()
            .content_label(),
        package().content_label()
    );
}

#[test]
fn tlcx43_exact_seed_and_output_swaps_are_rejected() {
    let mut seed = unverified_seed();
    let mut output = candidate();
    seed.raw_user_message.push('!');
    assert!(matches!(
        HandoffSession::begin(&expectation(), &package(), &seed, &output),
        Err(HandoffError::SeedMismatch)
    ));
    let seed = unverified_seed();
    output.output_text.push('!');
    output.candidate_digest = stable_sha256(&output.output_text);
    assert!(matches!(
        HandoffSession::begin(&expectation(), &package(), &seed, &output),
        Err(HandoffError::OutputMismatch)
    ));
}

#[test]
fn tlcx43_artifact_turn_and_public_digest_tampering_are_rejected() {
    let seed = unverified_seed();
    let mut output = candidate();
    output.turn_id = TurnId(3);
    assert!(matches!(
        HandoffSession::begin(&expectation(), &package(), &seed, &output),
        Err(HandoffError::ArtifactTurnMismatch)
    ));
    output.turn_id = TurnId(2);
    output.candidate_digest = stable_sha256("different output");
    assert!(matches!(
        HandoffSession::begin(&expectation(), &package(), &seed, &output),
        Err(HandoffError::OutputMismatch)
    ));
}

#[test]
fn tlcx43_out_of_order_stage_is_rejected_before_nonce_consumption() {
    let seed = unverified_seed();
    let output = candidate();
    let mut session = HandoffSession::begin(&expectation(), &package(), &seed, &output).unwrap();
    let mut replay = ReplayGuard::default();
    let mut input = absent_precommit();
    input.stage = HostOutputStage::PostSendObserved;
    assert_eq!(
        session.observe_external_stage(input, &verifier(), &mut replay, 10),
        Err(HandoffError::Host(HostError::HostOutputStageConflict))
    );
    assert_eq!(replay.observed_count(), 0);
    assert_eq!(session.inspect().authenticated_stage_count, 0);
}

#[test]
fn tlcx43_self_attested_seed_cannot_observe_a_host_stage() {
    let seed = unverified_seed();
    let output = candidate();
    let mut session = HandoffSession::begin(&expectation(), &package(), &seed, &output).unwrap();
    assert_eq!(
        session.observe_external_stage(
            absent_precommit(),
            &verifier(),
            &mut ReplayGuard::default(),
            10
        ),
        Err(HandoffError::Host(HostError::HostSeedUnbound))
    );
    assert_eq!(session.inspect().authenticated_stage_count, 0);
}

#[test]
fn tlcx43_valid_test_seed_without_stage_receipt_stays_pending() {
    let seed = negative_test_seed();
    let output = candidate();
    let mut session = HandoffSession::begin(&expectation(), &package(), &seed, &output).unwrap();
    assert_eq!(
        session.observe_external_stage(
            absent_precommit(),
            &verifier(),
            &mut ReplayGuard::default(),
            10
        ),
        Err(HandoffError::Host(HostError::MissingHostReceipt))
    );
    assert_eq!(
        session.inspect().host_stage_state,
        HostOutputLifecycleState::AwaitingPreCommit
    );
}

#[test]
fn tlcx43_expired_and_wrong_key_seed_checks_remain_real_host_errors() {
    let seed = negative_test_seed();
    let output = candidate();
    let mut session = HandoffSession::begin(&expectation(), &package(), &seed, &output).unwrap();
    assert_eq!(
        session.observe_external_stage(
            absent_precommit(),
            &verifier(),
            &mut ReplayGuard::default(),
            20
        ),
        Err(HandoffError::Host(HostError::HostSeedUnbound))
    );
    let wrong = ReceiptVerifier::from_key_bytes("tlcx43-negative-fixture", &[8; 32]).unwrap();
    assert_eq!(
        session.observe_external_stage(absent_precommit(), &wrong, &mut ReplayGuard::default(), 10),
        Err(HandoffError::Host(HostError::AuthorityHostMismatch))
    );
    assert_eq!(session.inspect().authenticated_stage_count, 0);
}

#[test]
fn tlcx43_empty_and_oversized_inputs_have_no_handoff() {
    assert!(matches!(
        HandoffExpectation::new(package(), "", OUTPUT, ArtifactId(43), TurnId(2)),
        Err(HandoffError::SeedSize)
    ));
    assert!(matches!(
        HandoffExpectation::new(package(), SEED, "", ArtifactId(43), TurnId(2)),
        Err(HandoffError::OutputSize)
    ));
    assert!(matches!(
        HandoffExpectation::new(
            package(),
            SEED,
            &"x".repeat(65537),
            ArtifactId(43),
            TurnId(2)
        ),
        Err(HandoffError::OutputSize)
    ));
}
