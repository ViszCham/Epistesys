#![forbid(unsafe_code)]

use lc631_media::attestation_v3::{
    media_backend_payload_digest_v3, media_backend_scope_v3, verify_media_backend_attestation_v3,
    verify_media_backend_with_current_authority_v3,
};
use lc631_media::{
    authorize_media_source, authorize_media_source_attested, media_source_payload_digest,
    media_source_scope, MediaBackendExecutionReceipt, MediaExecutionPlan, MediaProcessReceipt,
    MediaResourceBudget, MediaSourceAuthority, MediaSourceKind, MediaSourceRequest,
    MultimodalEventGraph, MEDIA_BACKEND_SCHEMA,
};
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptIssuer, ReceiptScope, ReceiptVerifier, ReplayGuard, SubjectRevision,
};
use lc631_tl::{CanonicalTranslationEnvelope, ProjectionDefectGraphV3, SourceLedgerId};
use std::collections::BTreeMap;

fn keys() -> (ReceiptIssuer, ReceiptVerifier) {
    (
        ReceiptIssuer::from_key_bytes("current-v3-fixture", &[74; 32]).unwrap(),
        ReceiptVerifier::from_key_bytes("current-v3-fixture", &[74; 32]).unwrap(),
    )
}

// Read a real existing local fixture; no temporary file or cleanup is needed.
fn authority(expires: u64) -> MediaSourceAuthority {
    authority_for(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("Cargo.toml")
            .to_str()
            .unwrap(),
        expires,
    )
}

fn authority_for(path: &str, expires: u64) -> MediaSourceAuthority {
    let request = MediaSourceRequest {
        kind: MediaSourceKind::LocalUserProvided,
        locator: path.into(),
        user_asserts_rights: true,
        media_bytes_requested: true,
        network_requested: false,
    };
    let pending = authorize_media_source(&request);
    let (issuer, verifier) = keys();
    let wire = issuer
        .issue(
            ReceiptClass::MediaSource,
            SubjectRevision::checked(pending.source_snapshot_sha256.as_ref().unwrap()).unwrap(),
            ReceiptScope::checked(media_source_scope(request.kind)).unwrap(),
            media_source_payload_digest(&request, &pending).unwrap(),
            10,
            Some(expires),
            None,
        )
        .unwrap();
    authorize_media_source_attested(&request, wire, &verifier, &mut ReplayGuard::default(), 10)
        .unwrap()
}

fn report(authority: &MediaSourceAuthority) -> MediaBackendExecutionReceipt {
    let digest = authority.source_snapshot_sha256.clone().unwrap();
    let projection = ProjectionDefectGraphV3::new(SourceLedgerId(1), "rev", "source").unwrap();
    MediaBackendExecutionReceipt {
        schema_version: MEDIA_BACKEND_SCHEMA,
        source_sha256: digest.clone(),
        backend_id: "fixture".into(),
        backend_version: "1".into(),
        model_revision: "fixture".into(),
        model_cache_manifest_sha256: digest.clone(),
        model_card_license_hint: "fixture only".into(),
        backend_receipt_sha256: digest.clone(),
        execution_plan: MediaExecutionPlan {
            source_snapshot_sha256: digest.clone(),
            source_bytes: authority.source_bytes.unwrap(),
            python_sha256: digest.clone(),
            backend_script_sha256: digest.clone(),
            ffmpeg_sha256: digest.clone(),
            model_cache_manifest_sha256: digest.clone(),
            configuration_digest: digest.clone(),
            resource_budget: MediaResourceBudget::default(),
            network_policy: "offline".into(),
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
            memory_limit_kind: "unobserved".into(),
        },
        executed_family_states: BTreeMap::new(),
        graph: MultimodalEventGraph::default(),
        diagnostics: vec!["synthetic receipt, no model executed".into()],
        source_authority_preserved: true,
        identity_inference_committed: false,
        internal_state_committed: false,
        family_validation_count: 0,
        authenticity_receipt_digest: None,
        authenticity_attestation: None,
        translation_envelope: CanonicalTranslationEnvelope::from_graph(&projection, vec![]),
        claim_boundary: "synthetic fixture only",
    }
}

fn sign(value: &mut MediaBackendExecutionReceipt, authority: &MediaSourceAuthority) {
    let (issuer, _) = keys();
    let wire = issuer
        .issue(
            ReceiptClass::MediaBackend,
            SubjectRevision::checked(&value.source_sha256).unwrap(),
            ReceiptScope::checked(media_backend_scope_v3(&value.backend_id)).unwrap(),
            media_backend_payload_digest_v3(value).unwrap(),
            10,
            None,
            authority.attestation_receipt_digest.clone(),
        )
        .unwrap();
    value.authenticity_receipt_digest = Some(wire.receipt_digest());
    value.authenticity_attestation = Some(wire);
}

#[test]
fn tlcx30_v3_expired_parent_cannot_be_revived_by_a_timeless_backend_receipt() {
    let authority = authority(20);
    let mut value = report(&authority);
    sign(&mut value, &authority);
    let (_, verifier) = keys();
    // Integrity-only compatibility is explicitly not a parent-currentness gate.
    assert!(verify_media_backend_attestation_v3(
        &value,
        &verifier,
        &mut ReplayGuard::default(),
        20,
        authority.attestation_receipt_digest.as_ref().unwrap(),
    )
    .is_ok());
    let mut replay = ReplayGuard::default();
    assert!(verify_media_backend_with_current_authority_v3(
        &value,
        &authority,
        &verifier,
        &mut replay,
        20,
    )
    .is_err());
    assert_eq!(replay.observed_count(), 0);
}

#[test]
fn tlcx30_v3_current_authority_accepts_exact_snapshot_once() {
    let authority = authority(20);
    let mut value = report(&authority);
    sign(&mut value, &authority);
    let (_, verifier) = keys();
    let mut replay = ReplayGuard::default();
    assert!(verify_media_backend_with_current_authority_v3(
        &value,
        &authority,
        &verifier,
        &mut replay,
        10,
    )
    .is_ok());
    assert_eq!(replay.observed_count(), 1);
    assert!(verify_media_backend_with_current_authority_v3(
        &value,
        &authority,
        &verifier,
        &mut replay,
        10,
    )
    .is_err());
    assert_eq!(replay.observed_count(), 1);
}

#[test]
fn tlcx30_v3_valid_signature_does_not_override_source_or_plan_binding() {
    let authority = authority(20);
    let (_, verifier) = keys();
    for mutate in [
        (|r: &mut MediaBackendExecutionReceipt| {
            r.source_sha256 = lc631_core::stable_sha256("foreign")
        }) as fn(&mut MediaBackendExecutionReceipt),
        |r| r.execution_plan.source_snapshot_sha256 = lc631_core::stable_sha256("foreign"),
        |r| r.execution_plan.source_bytes += 1,
    ] {
        let mut value = report(&authority);
        mutate(&mut value);
        sign(&mut value, &authority);
        let mut replay = ReplayGuard::default();
        assert!(verify_media_backend_with_current_authority_v3(
            &value,
            &authority,
            &verifier,
            &mut replay,
            10,
        )
        .is_err());
        assert_eq!(replay.observed_count(), 0);
    }
}

#[test]
fn tlcx30_v3_public_authority_fields_cannot_replace_the_private_source_binding() {
    let mut authority = authority(20);
    let mut value = report(&authority);
    sign(&mut value, &authority);
    authority.source_bytes = Some(authority.source_bytes.unwrap() + 1);
    let (_, verifier) = keys();
    let mut replay = ReplayGuard::default();
    assert!(verify_media_backend_with_current_authority_v3(
        &value,
        &authority,
        &verifier,
        &mut replay,
        10,
    )
    .is_err());
    assert_eq!(replay.observed_count(), 0);
}

#[test]
fn tlcx30_v3_source_changed_after_authorization_fails_before_replay() {
    use std::io::Write;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "epistesys-v3-owned-snapshot-{}-{nonce}.bin",
        std::process::id(),
    ));
    let mut fixture = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)
        .unwrap();
    fixture.write_all(b"before").unwrap();
    drop(fixture);
    let authority = authority_for(path.to_str().unwrap(), 20);
    let mut value = report(&authority);
    sign(&mut value, &authority);
    // Only this uniquely created six-byte fixture is changed. Same length
    // catches a metadata-only freshness check. Retain it; no cleanup authority.
    std::fs::write(&path, b"after!").unwrap();
    let (_, verifier) = keys();
    let mut replay = ReplayGuard::default();
    assert!(verify_media_backend_with_current_authority_v3(
        &value,
        &authority,
        &verifier,
        &mut replay,
        10,
    )
    .is_err());
    assert_eq!(replay.observed_count(), 0);
    eprintln!(
        "retained six-byte test-owned source fixture: {}",
        path.display()
    );
}
