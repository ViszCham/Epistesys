use lc631_core::stable_sha256;
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptIssuer, ReceiptScope, ReceiptVerifier, ReplayGuard, SubjectRevision,
};
use lc631_wire::{
    alpha2_gate_payload_digest, alpha2_gate_scope, alpha2_required_features, assess_alpha2_release,
    Alpha2GateAttestation, Alpha2HostObservation, Alpha2ReleaseProfile, Alpha2ResearchStatus,
    Alpha2SourceReadiness,
};

fn roots() -> (ReceiptIssuer, ReceiptVerifier) {
    let key = [83_u8; 32];
    (
        ReceiptIssuer::from_key_bytes("alpha2-test-root", &key).unwrap(),
        ReceiptVerifier::from_key_bytes("alpha2-test-root", &key).unwrap(),
    )
}

fn attestations(
    profile: &Alpha2ReleaseProfile,
    source: &str,
    issuer: &ReceiptIssuer,
) -> Vec<Alpha2GateAttestation> {
    alpha2_required_features()
        .iter()
        .map(|feature| {
            let evidence_digest = stable_sha256(feature.as_str());
            let receipt = issuer
                .issue(
                    ReceiptClass::StageCompletion,
                    SubjectRevision::checked(source).unwrap(),
                    ReceiptScope::checked(alpha2_gate_scope(profile, *feature)).unwrap(),
                    alpha2_gate_payload_digest(
                        &profile.identity(),
                        source,
                        *feature,
                        &evidence_digest,
                    ),
                    10,
                    None,
                    None,
                )
                .unwrap();
            Alpha2GateAttestation {
                feature: *feature,
                evidence_digest,
                receipt,
            }
        })
        .collect()
}

#[test]
fn alpha2_missing_functional_receipts_remain_held() {
    let (issuer, verifier) = roots();
    let profile = Alpha2ReleaseProfile::standalone();
    let source = stable_sha256("alpha2-source");
    let report = assess_alpha2_release(
        &profile,
        &source,
        &[],
        Some(&verifier),
        &mut ReplayGuard::default(),
        10,
        Alpha2ResearchStatus::PendingNoCorpus,
    );
    assert_eq!(report.source_readiness, Alpha2SourceReadiness::Held);
    assert_eq!(
        report.blocked_features.len(),
        alpha2_required_features().len()
    );
    assert_eq!(
        report.research_status,
        Alpha2ResearchStatus::PendingNoCorpus
    );
    assert_eq!(report.host_observation, Alpha2HostObservation::NotRequested);
    drop(issuer);
}

#[test]
fn alpha2_standalone_gate_closes_without_promoting_pending_research() {
    let (issuer, verifier) = roots();
    let profile = Alpha2ReleaseProfile::standalone();
    let source = stable_sha256("alpha2-source");
    let evidence = attestations(&profile, &source, &issuer);
    let report = assess_alpha2_release(
        &profile,
        &source,
        &evidence,
        Some(&verifier),
        &mut ReplayGuard::default(),
        10,
        Alpha2ResearchStatus::PendingNoCorpus,
    );
    assert_eq!(
        report.source_readiness,
        Alpha2SourceReadiness::SourceReleaseReady
    );
    assert!(report.blocked_features.is_empty());
    assert_eq!(
        report.research_status,
        Alpha2ResearchStatus::PendingNoCorpus
    );
    assert_eq!(report.host_observation, Alpha2HostObservation::NotRequested);
}

#[test]
fn alpha2_required_host_profile_remains_blocked_without_host_output() {
    let (issuer, verifier) = roots();
    let profile = Alpha2ReleaseProfile::host_integrated();
    let source = stable_sha256("alpha2-source");
    let evidence = attestations(&profile, &source, &issuer);
    let report = assess_alpha2_release(
        &profile,
        &source,
        &evidence,
        Some(&verifier),
        &mut ReplayGuard::default(),
        10,
        Alpha2ResearchStatus::PendingNoCorpus,
    );
    assert_eq!(report.source_readiness, Alpha2SourceReadiness::Held);
    assert_eq!(report.blocked_features.len(), 0);
    assert_eq!(report.blocked_external_requirements, vec!["host_output"]);
}

#[test]
fn alpha2_stale_and_duplicate_receipts_do_not_close_gate() {
    let (issuer, verifier) = roots();
    let profile = Alpha2ReleaseProfile::standalone();
    let source = stable_sha256("alpha2-source");
    let mut evidence = attestations(&profile, &source, &issuer);
    evidence.pop();
    evidence.push(attestations(&profile, &stable_sha256("other-source"), &issuer).remove(0));
    evidence.push(attestations(&profile, &source, &issuer).remove(0));
    let report = assess_alpha2_release(
        &profile,
        &source,
        &evidence,
        Some(&verifier),
        &mut ReplayGuard::default(),
        10,
        Alpha2ResearchStatus::PendingNoCorpus,
    );
    assert_eq!(report.source_readiness, Alpha2SourceReadiness::Held);
    assert!(!report.rejected_receipts.is_empty());
}

#[test]
fn alpha2_wrong_receipt_class_does_not_close_feature() {
    let (issuer, verifier) = roots();
    let profile = Alpha2ReleaseProfile::standalone();
    let source = stable_sha256("alpha2-source");
    let mut evidence = attestations(&profile, &source, &issuer);
    let first_feature = evidence[0].feature;
    let first_digest = evidence[0].evidence_digest.clone();
    evidence[0].receipt = issuer
        .issue(
            ReceiptClass::Validation,
            SubjectRevision::checked(&source).unwrap(),
            ReceiptScope::checked(alpha2_gate_scope(&profile, first_feature)).unwrap(),
            alpha2_gate_payload_digest(&profile.identity(), &source, first_feature, &first_digest),
            10,
            None,
            None,
        )
        .unwrap();
    let report = assess_alpha2_release(
        &profile,
        &source,
        &evidence,
        Some(&verifier),
        &mut ReplayGuard::default(),
        10,
        Alpha2ResearchStatus::PendingNoCorpus,
    );
    assert_eq!(report.source_readiness, Alpha2SourceReadiness::Held);
    assert!(report.blocked_features.contains(&first_feature));
    assert!(report
        .rejected_receipts
        .iter()
        .any(|reason| reason.contains("ClassMismatch")));
}
