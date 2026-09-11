use lc631_analysis::{
    analyze, analyze_with_receipts, validation_receipt_payload_digest, validation_receipt_scope,
    AnalysisRequest, EvidenceState, ReceiptBinding, ValidationEvidenceBundle, ValidationKind,
    ValidationReceipt,
};
use lc631_core::stable_sha256;
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptIssuer, ReceiptScope, ReceiptVerifier, SubjectRevision,
};

fn request<'a>(source: &'a str, bundle: &'a ValidationEvidenceBundle) -> AnalysisRequest<'a> {
    AnalysisRequest {
        prompt: "review Rust",
        repo: None,
        validation: Some(bundle),
        release: None,
        source_name: Some("lib.rs"),
        source: Some(source),
    }
}

fn receipt(source_revision: &str) -> ValidationReceipt {
    ValidationReceipt {
        kind: ValidationKind::Rustfmt,
        binding: ReceiptBinding::HostBound,
        state: EvidenceState::Observed,
        source_revision: source_revision.into(),
        tool_revision: "rustfmt.test-v1".into(),
        invocation_digest: stable_sha256("rustfmt --check"),
        output_digest: stable_sha256("ok"),
        scope: "exact-source".into(),
        unsupported: vec![],
        attestation: None,
    }
}

#[test]
fn arc631_06_host_bound_enum_and_digest_syntax_are_not_authenticity() {
    let source = "pub fn answer() -> u32 { 42 }";
    let bundle = ValidationEvidenceBundle {
        bundle_version: "arc631.test-v1".into(),
        receipts: vec![receipt(&stable_sha256(source))],
    };
    let report = analyze(request(source, &bundle));
    assert_eq!(report.validation.as_ref().unwrap().accepted_receipts, 0);
    let envelope = report.canonical_translation.as_ref().unwrap();
    assert!(!envelope.scalar_aggregate_used);
    assert_eq!(envelope.domain_payloads[0].domain, "rpa");
}

#[test]
fn arc631_06_exact_attestation_is_accepted_but_does_not_close_incomplete_stage() {
    let key = [23_u8; 32];
    let issuer = ReceiptIssuer::from_key_bytes("lc631-rpa-root", &key).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("lc631-rpa-root", &key).unwrap();
    let source = "pub fn answer() -> u32 { 42 }";
    let source_revision = stable_sha256(source);
    let mut receipt = receipt(&source_revision);
    let wire = issuer
        .issue(
            ReceiptClass::Validation,
            SubjectRevision::checked(&source_revision).unwrap(),
            ReceiptScope::checked(validation_receipt_scope(&receipt)).unwrap(),
            validation_receipt_payload_digest(&receipt),
            1,
            None,
            None,
        )
        .unwrap();
    receipt.attestation = Some(wire);
    let bundle = ValidationEvidenceBundle {
        bundle_version: "arc631.test-v1".into(),
        receipts: vec![receipt],
    };
    let report = analyze_with_receipts(request(source, &bundle), &verifier, 1);
    assert_eq!(report.validation.as_ref().unwrap().accepted_receipts, 1);
    assert_ne!(
        report.stage("RPA-19").unwrap().state,
        EvidenceState::Observed
    );
}
