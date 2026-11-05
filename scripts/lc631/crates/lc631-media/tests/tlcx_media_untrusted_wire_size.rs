//! Untrusted attestation wire is a separate bound from signed report bytes.
//! No media process, external issuer, filesystem, allocator instrumentation or Cargo here.
use lc631_core::stable_sha256;
use lc631_media::{
    attestation_v2::{
        media_backend_payload_digest_v2, media_backend_scope_v2,
        verify_media_backend_attestation_v2, MediaAttestationV2Error,
        MAX_MEDIA_ATTESTATION_V2_BYTES,
    },
    MediaBackendExecutionReceipt, MediaExecutionPlan, MediaProcessReceipt, MediaResourceBudget,
    MultimodalEventGraph, MEDIA_BACKEND_SCHEMA,
};
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptIssuer, ReceiptScope, ReceiptVerifier, ReplayGuard, SubjectRevision,
    UntrustedReceipt,
};
use lc631_tl::{CanonicalTranslationEnvelope, ProjectionDefectGraphV3, SourceLedgerId};
use std::collections::BTreeMap;

fn report() -> MediaBackendExecutionReceipt {
    let digest = stable_sha256("small synthetic report");
    let projection = ProjectionDefectGraphV3::new(SourceLedgerId(1), &digest, "fixture").unwrap();
    MediaBackendExecutionReceipt {
        schema_version: MEDIA_BACKEND_SCHEMA,
        source_sha256: digest.clone(),
        backend_id: "fixture".into(),
        backend_version: "fixture".into(),
        model_revision: digest.clone(),
        model_cache_manifest_sha256: digest.clone(),
        model_card_license_hint: "fixture".into(),
        backend_receipt_sha256: digest.clone(),
        execution_plan: MediaExecutionPlan {
            source_snapshot_sha256: digest.clone(),
            source_bytes: 0,
            python_sha256: digest.clone(),
            backend_script_sha256: digest.clone(),
            ffmpeg_sha256: digest.clone(),
            model_cache_manifest_sha256: digest.clone(),
            configuration_digest: digest.clone(),
            resource_budget: MediaResourceBudget::default(),
            network_policy: "none".into(),
            network_isolation_os_enforced: false,
        },
        process: MediaProcessReceipt {
            exit_code: Some(0),
            stdout_digest: digest.clone(),
            stderr_digest: digest,
            stdout_truncated: false,
            stderr_truncated: false,
            process_tree_kill_attempted: false,
            process_tree_kill_verified: false,
            memory_limit_bytes: None,
            memory_limit_kind: "fixture".into(),
        },
        executed_family_states: BTreeMap::new(),
        graph: MultimodalEventGraph::default(),
        diagnostics: Vec::new(),
        source_authority_preserved: false,
        identity_inference_committed: false,
        internal_state_committed: false,
        family_validation_count: 0,
        authenticity_receipt_digest: None,
        authenticity_attestation: None,
        translation_envelope: CanonicalTranslationEnvelope::from_graph(&projection, vec![]),
        claim_boundary: "synthetic integrity fixture, not execution or authority",
    }
}

#[test]
fn tlcx_media_oversized_unsigned_wire_is_rejected_before_digest_binding_not_as_small_report() {
    let mut report = report();
    let issuer = ReceiptIssuer::from_key_bytes("fixture", &[91; 32]).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("fixture", &[91; 32]).unwrap();
    let parent = stable_sha256("parent");
    let wire = issuer
        .issue(
            ReceiptClass::MediaBackend,
            SubjectRevision::checked(&report.source_sha256).unwrap(),
            ReceiptScope::checked(media_backend_scope_v2(&report.backend_id)).unwrap(),
            media_backend_payload_digest_v2(&report).unwrap(),
            10,
            None,
            Some(parent.clone()),
        )
        .unwrap();
    let original_digest = wire.receipt_digest();
    let value = serde_json::to_value(&wire).unwrap();
    let mut replay = ReplayGuard::default();
    // Receipt ingress now bounds these fields before constructing a typed
    // wire. Preserve this earlier rejection and the untouched replay state.
    for path in [
        "schema_version",
        "nonce_hex",
        "payload_digest",
        "parent_receipt_digest",
        "mac_sha256",
    ] {
        let mut mutant = value.clone();
        let length = if path == "schema_version" {
            MAX_MEDIA_ATTESTATION_V2_BYTES + 1
        } else {
            4096
        };
        if path == "mac_sha256" {
            mutant[path] = serde_json::Value::String("x".repeat(length));
        } else {
            mutant["claims"][path] = serde_json::Value::String("x".repeat(length));
        }
        assert!(
            serde_json::from_value::<UntrustedReceipt>(mutant).is_err(),
            "oversized wire field {path}"
        );
        assert_eq!(replay.observed_count(), 0);
    }
    report.authenticity_attestation = Some(wire.clone());
    report.authenticity_receipt_digest = Some(original_digest);
    assert!(media_backend_payload_digest_v2(&report).is_ok());
    // The expected parent is a separate input: it is not covered by either
    // signed-report or untrusted-wire size checks. Reject malformed digests
    // before to_owned(), without treating a digest as current source authority.
    for invalid_parent in [
        String::new(),
        "x".repeat(MAX_MEDIA_ATTESTATION_V2_BYTES + 1),
        format!("sha256:{}", "z".repeat(64)),
    ] {
        assert_eq!(
            verify_media_backend_attestation_v2(
                &report,
                &verifier,
                &mut replay,
                10,
                &invalid_parent,
            ),
            Err(MediaAttestationV2Error::InvalidBinding),
        );
        assert_eq!(replay.observed_count(), 0);
    }
    // A well-formed but different expected parent is still a policy rejection,
    // not a size/encoding failure, and does not consume the legitimate nonce.
    assert_eq!(
        verify_media_backend_attestation_v2(
            &report,
            &verifier,
            &mut replay,
            10,
            &stable_sha256("different valid parent"),
        ),
        Err(MediaAttestationV2Error::VerificationRejected),
    );
    assert_eq!(replay.observed_count(), 0);
    // Early wire-size rejection must not consume the legitimate original nonce.
    report.authenticity_attestation = Some(wire);
    assert!(
        verify_media_backend_attestation_v2(&report, &verifier, &mut replay, 10, &parent).is_ok()
    );
}
