//! 日本語: v3固定hashと独立domain差替え。backend真正性や移行完了ではない。
//! English: Fixed v3 hash and isolated domain swaps, not backend truth or migration.
#![forbid(unsafe_code)]

use lc631_core::stable_sha256;
use lc631_media::attestation_v3::{
    media_backend_payload_digest_v3, media_backend_scope_v3, verify_media_backend_attestation_v3,
    write_media_report_v3, CanonicalWriter, MediaAttestationV3Error,
};
use lc631_media::{
    MediaBackendExecutionReceipt, MediaExecutionPlan, MediaProcessReceipt, MediaResourceBudget,
    MultimodalEventGraph, MEDIA_BACKEND_SCHEMA,
};
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptIssuer, ReceiptScope, ReceiptVerifier, ReplayGuard, SubjectRevision,
};
use lc631_tl::{
    CanonicalTranslationEnvelope, ProjectionGateDecision, SourceLedgerId,
    TRANSLATION_ENVELOPE_V3_SCHEMA,
};
use std::collections::BTreeMap;

// Explicit empty payload values pin the golden independently of Default budgets
// and graph constructors. The source/id below serve only local receipt bindings.
fn fixture() -> MediaBackendExecutionReceipt {
    MediaBackendExecutionReceipt {
        schema_version: MEDIA_BACKEND_SCHEMA,
        source_sha256: stable_sha256("independent-v3-fixture"),
        backend_id: "fixture".into(),
        backend_version: String::new(),
        model_revision: String::new(),
        model_cache_manifest_sha256: String::new(),
        model_card_license_hint: String::new(),
        backend_receipt_sha256: String::new(),
        execution_plan: MediaExecutionPlan {
            source_snapshot_sha256: String::new(),
            source_bytes: 0,
            python_sha256: String::new(),
            backend_script_sha256: String::new(),
            ffmpeg_sha256: String::new(),
            model_cache_manifest_sha256: String::new(),
            configuration_digest: String::new(),
            resource_budget: MediaResourceBudget {
                max_stdout_bytes: 0,
                max_stderr_bytes: 0,
                max_temporary_bytes: 0,
                max_decoded_frames: 0,
                max_pcm_samples: 0,
                max_observations: 0,
            },
            network_policy: String::new(),
            network_isolation_os_enforced: false,
        },
        process: MediaProcessReceipt {
            exit_code: None,
            stdout_digest: String::new(),
            stderr_digest: String::new(),
            stdout_truncated: false,
            stderr_truncated: false,
            process_tree_kill_attempted: false,
            process_tree_kill_verified: false,
            memory_limit_bytes: None,
            memory_limit_kind: String::new(),
        },
        executed_family_states: BTreeMap::new(),
        graph: MultimodalEventGraph::default(),
        diagnostics: vec![],
        source_authority_preserved: false,
        identity_inference_committed: false,
        internal_state_committed: false,
        family_validation_count: 0,
        authenticity_receipt_digest: None,
        authenticity_attestation: None,
        translation_envelope: CanonicalTranslationEnvelope {
            schema_version: TRANSLATION_ENVELOPE_V3_SCHEMA,
            source_ledger_id: SourceLedgerId(0),
            source_revision: String::new(),
            source_digest: String::new(),
            target_claims: vec![],
            edges: vec![],
            domain_payloads: vec![],
            gate: ProjectionGateDecision::Clarify,
            scalar_aggregate_used: false,
        },
        claim_boundary: "",
    }
}

fn sign(
    value: &mut MediaBackendExecutionReceipt,
    scope: &str,
    payload: String,
    issued: u64,
) -> String {
    let parent = stable_sha256("independent-v3-parent");
    let issuer = ReceiptIssuer::from_key_bytes("independent-v3-test", &[44; 32]).unwrap();
    let receipt = issuer
        .issue(
            ReceiptClass::MediaBackend,
            SubjectRevision::checked(&value.source_sha256).unwrap(),
            ReceiptScope::checked(scope).unwrap(),
            payload,
            issued,
            Some(20),
            Some(parent.clone()),
        )
        .unwrap();
    value.authenticity_receipt_digest = Some(receipt.receipt_digest());
    value.authenticity_attestation = Some(receipt);
    parent
}

fn verify(
    value: &MediaBackendExecutionReceipt,
    parent: &str,
) -> Result<(), MediaAttestationV3Error> {
    let verifier = ReceiptVerifier::from_key_bytes("independent-v3-test", &[44; 32]).unwrap();
    verify_media_backend_attestation_v3(value, &verifier, &mut ReplayGuard::default(), 12, parent)
}

#[test]
fn full_report_hash_matches_a_fixed_independent_454_byte_golden() {
    let mut value = fixture();
    value.source_sha256.clear();
    value.backend_id.clear();
    let mut bytes = Vec::new();
    write_media_report_v3(&mut CanonicalWriter::new(&mut bytes, 454), &value).unwrap();
    assert_eq!(bytes.len(), 454);
    // Computed outside Rust with Python hashlib over independently assembled
    // literal-tag/LE-length field bytes. Never regenerate from this encoder.
    assert_eq!(
        media_backend_payload_digest_v3(&value).unwrap(),
        "sha256:4a060bf738eb4a4027d3d9500071085daa4adb70a4cb3ee90005bde3fe22f739"
    );
}

#[test]
fn legacy_scope_and_payload_are_each_rejected_independently() {
    for version in 1..=2 {
        let original = fixture();
        let current_payload = media_backend_payload_digest_v3(&original).unwrap();
        let current_scope = media_backend_scope_v3(&original.backend_id);
        let (old_scope, old_payload) = if version == 1 {
            (
                format!("media/backend/{}", original.backend_id),
                lc631_media::media_backend_payload_digest(&original),
            )
        } else {
            (
                lc631_media::attestation_v2::media_backend_scope_v2(&original.backend_id),
                lc631_media::attestation_v2::media_backend_payload_digest_v2(&original).unwrap(),
            )
        };
        assert_ne!(current_scope, old_scope);
        assert_ne!(current_payload, old_payload);
        let mut correct = original.clone();
        let parent = sign(&mut correct, &current_scope, current_payload.clone(), 10);
        assert!(verify(&correct, &parent).is_ok());
        let mut wrong_scope = original.clone();
        let parent = sign(&mut wrong_scope, &old_scope, current_payload, 10);
        assert_eq!(
            verify(&wrong_scope, &parent),
            Err(MediaAttestationV3Error::VerificationRejected)
        );
        let mut wrong_payload = original;
        let parent = sign(&mut wrong_payload, &current_scope, old_payload, 10);
        assert_eq!(
            verify(&wrong_payload, &parent),
            Err(MediaAttestationV3Error::VerificationRejected)
        );
    }
}

#[test]
fn replacing_excluded_wire_changes_credential_binding_but_not_report_hash() {
    let mut first = fixture();
    let digest = media_backend_payload_digest_v3(&first).unwrap();
    let scope = media_backend_scope_v3(&first.backend_id);
    let parent = sign(&mut first, &scope, digest.clone(), 10);
    let mut second = first.clone();
    let second_parent = sign(&mut second, &scope, digest.clone(), 11);
    assert_eq!(parent, second_parent);
    assert_ne!(
        first.authenticity_attestation,
        second.authenticity_attestation
    );
    assert_ne!(
        first.authenticity_receipt_digest,
        second.authenticity_receipt_digest
    );
    assert_eq!(media_backend_payload_digest_v3(&first).unwrap(), digest);
    assert_eq!(media_backend_payload_digest_v3(&second).unwrap(), digest);
    assert!(verify(&first, &parent).is_ok());
    assert!(verify(&second, &parent).is_ok());
    // The two excluded self-reference fields must still agree at verification.
    second.authenticity_receipt_digest = first.authenticity_receipt_digest;
    assert_eq!(
        verify(&second, &parent),
        Err(MediaAttestationV3Error::InvalidBinding)
    );
}
