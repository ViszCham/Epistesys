#![forbid(unsafe_code)]

// Synthetic report and in-memory signing only: no media, backend, or model is run.
use lc631_core::stable_sha256;
use lc631_media::{
    attestation_v2::{
        media_backend_payload_digest_v2, media_backend_scope_v2,
        verify_media_backend_attestation_v2,
    },
    media_backend_payload_digest, MediaBackendExecutionReceipt, MediaExecutionPlan,
    MediaObservation, MediaProcessReceipt, MediaResourceBudget, MultimodalEventGraph,
    ObservationKind, RationalTime, TimeSpan, MEDIA_BACKEND_SCHEMA,
};
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptIssuer, ReceiptScope, ReceiptVerifier, ReplayGuard, SubjectRevision,
};
use lc631_tl::{CanonicalTranslationEnvelope, ProjectionDefectGraphV3, SourceLedgerId};
use std::collections::BTreeMap;

fn observation() -> MediaObservation {
    MediaObservation {
        observation_id: "obs-1".into(),
        kind: ObservationKind::TranscriptCandidate,
        span: TimeSpan::checked(
            RationalTime::checked(1, 1000).unwrap(),
            RationalTime::checked(2, 1000).unwrap(),
        )
        .unwrap(),
        track_id: Some(1),
        label: "original transcript".into(),
        backend_id: "synthetic-backend".into(),
        model_revision: stable_sha256("model"),
        evidence_digest: stable_sha256("evidence"),
        source_fragment_digest: stable_sha256("source fragment"),
        provenance_digest: stable_sha256("synthetic provenance"),
        configuration_digest: stable_sha256("config"),
        candidate_only: true,
        identity_committed: false,
        internal_state_committed: false,
    }
}

fn graph(observation: MediaObservation) -> MultimodalEventGraph {
    let mut graph = MultimodalEventGraph::default();
    graph.add(observation).unwrap();
    graph
}

fn report() -> MediaBackendExecutionReceipt {
    let source = stable_sha256("synthetic source, not media");
    let model = stable_sha256("model");
    let projection = ProjectionDefectGraphV3::new(SourceLedgerId(1), &source, "synthetic").unwrap();
    MediaBackendExecutionReceipt {
        schema_version: MEDIA_BACKEND_SCHEMA,
        source_sha256: source.clone(),
        backend_id: "synthetic-backend".into(),
        backend_version: "v1".into(),
        model_revision: model.clone(),
        model_cache_manifest_sha256: model.clone(),
        model_card_license_hint: "fixture-only".into(),
        backend_receipt_sha256: stable_sha256("backend stdout"),
        execution_plan: MediaExecutionPlan {
            source_snapshot_sha256: source,
            source_bytes: 10,
            python_sha256: stable_sha256("python"),
            backend_script_sha256: stable_sha256("script"),
            ffmpeg_sha256: stable_sha256("ffmpeg"),
            model_cache_manifest_sha256: model,
            configuration_digest: stable_sha256("config"),
            resource_budget: MediaResourceBudget::default(),
            network_policy: "fixture".into(),
            network_isolation_os_enforced: false,
        },
        process: MediaProcessReceipt {
            exit_code: Some(0),
            stdout_digest: stable_sha256("backend stdout"),
            stderr_digest: stable_sha256("backend stderr"),
            stdout_truncated: false,
            stderr_truncated: false,
            process_tree_kill_attempted: false,
            process_tree_kill_verified: false,
            memory_limit_bytes: None,
            memory_limit_kind: "fixture".into(),
        },
        executed_family_states: BTreeMap::new(),
        graph: graph(observation()),
        diagnostics: Vec::new(),
        source_authority_preserved: false,
        identity_inference_committed: false,
        internal_state_committed: false,
        family_validation_count: 0,
        authenticity_receipt_digest: None,
        authenticity_attestation: None,
        translation_envelope: CanonicalTranslationEnvelope::from_graph(&projection, vec![]),
        claim_boundary: "synthetic only; no backend or authority",
    }
}

// This reproduces the issuer and verifier policy used by the production
// attestation code. The issuer is a local fixture, not evidence of execution.
fn signed_original_rejects_changed(
    original: &MediaBackendExecutionReceipt,
    changed: &MediaBackendExecutionReceipt,
) {
    assert_ne!(
        serde_json::to_value(original).unwrap(),
        serde_json::to_value(changed).unwrap()
    );
    let issuer = ReceiptIssuer::from_key_bytes("synthetic", &[27; 32]).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("synthetic", &[27; 32]).unwrap();
    let subject = SubjectRevision::checked(&original.source_sha256).unwrap();
    let scope = ReceiptScope::checked(media_backend_scope_v2(&original.backend_id)).unwrap();
    let parent = stable_sha256("synthetic parent");
    let wire = issuer
        .issue(
            ReceiptClass::MediaBackend,
            subject.clone(),
            scope.clone(),
            media_backend_payload_digest_v2(original).unwrap(),
            10,
            None,
            Some(parent.clone()),
        )
        .unwrap();
    let mut original = original.clone();
    let mut changed = changed.clone();
    original.authenticity_receipt_digest = Some(wire.receipt_digest());
    changed.authenticity_receipt_digest = Some(wire.receipt_digest());
    original.authenticity_attestation = Some(wire.clone());
    changed.authenticity_attestation = Some(wire);
    assert!(verify_media_backend_attestation_v2(
        &original,
        &verifier,
        &mut ReplayGuard::default(),
        10,
        &parent
    )
    .is_ok());
    assert!(
        verify_media_backend_attestation_v2(
            &changed,
            &verifier,
            &mut ReplayGuard::default(),
            10,
            &parent
        )
        .is_err(),
        "changed report inherited original attestation"
    );
}

#[test]
fn legacy_v1_collision_remains_historical_not_v2_evidence() {
    let original = report();
    let mut changed = original.clone();
    let mut obs = observation();
    obs.label = "different transcript".into();
    changed.graph = graph(obs);
    assert_eq!(
        media_backend_payload_digest(&original),
        media_backend_payload_digest(&changed)
    );
}

#[test]
fn v2_digest_uses_receipt_kernel_sha256_wire_shape() {
    let digest = media_backend_payload_digest_v2(&report()).unwrap();
    assert_eq!(digest.len(), 71);
    assert!(digest.starts_with("sha256:"));
    assert!(digest[7..]
        .bytes()
        .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()));
}

#[test]
fn changed_label_cannot_inherit_original_attestation() {
    let original = report();
    let mut changed = original.clone();
    let mut obs = observation();
    obs.label = "different transcript".into();
    changed.graph = graph(obs);
    signed_original_rejects_changed(&original, &changed);
}

#[test]
fn changed_time_cannot_inherit_original_attestation() {
    let original = report();
    let mut changed = original.clone();
    let mut obs = observation();
    obs.span.end = RationalTime::checked(3, 1000).unwrap();
    changed.graph = graph(obs);
    signed_original_rejects_changed(&original, &changed);
}

#[test]
fn changed_track_cannot_inherit_original_attestation() {
    let original = report();
    let mut changed = original.clone();
    let mut obs = observation();
    obs.track_id = Some(2);
    changed.graph = graph(obs);
    signed_original_rejects_changed(&original, &changed);
}

#[test]
fn changed_source_byte_count_cannot_inherit_original_attestation() {
    let original = report();
    let mut changed = original.clone();
    changed.execution_plan.source_bytes += 1;
    signed_original_rejects_changed(&original, &changed);
}

#[test]
fn changed_backend_stdout_digest_cannot_inherit_original_attestation() {
    let original = report();
    let mut changed = original.clone();
    changed.backend_receipt_sha256 = stable_sha256("different backend stdout");
    changed.process.stdout_digest = changed.backend_receipt_sha256.clone();
    signed_original_rejects_changed(&original, &changed);
}

#[test]
fn changed_kind_cannot_inherit_original_attestation() {
    let original = report();
    let mut changed = original.clone();
    let mut obs = observation();
    obs.kind = ObservationKind::OcrTextCandidate;
    changed.graph = graph(obs);
    signed_original_rejects_changed(&original, &changed);
}

#[test]
fn changed_translation_envelope_cannot_inherit_original_attestation() {
    let original = report();
    let mut changed = original.clone();
    changed.translation_envelope.scalar_aggregate_used = true;
    signed_original_rejects_changed(&original, &changed);
}

#[test]
fn v2_rejects_legacy_scope_and_wrong_parent_without_fallback() {
    let mut original = report();
    let issuer = ReceiptIssuer::from_key_bytes("synthetic", &[27; 32]).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("synthetic", &[27; 32]).unwrap();
    let subject = SubjectRevision::checked(&original.source_sha256).unwrap();
    let parent = stable_sha256("synthetic parent");
    let wire = issuer
        .issue(
            ReceiptClass::MediaBackend,
            subject,
            ReceiptScope::checked(format!("media/backend/{}", original.backend_id)).unwrap(),
            media_backend_payload_digest(&original),
            10,
            None,
            Some(parent.clone()),
        )
        .unwrap();
    original.authenticity_receipt_digest = Some(wire.receipt_digest());
    original.authenticity_attestation = Some(wire);
    assert!(verify_media_backend_attestation_v2(
        &original,
        &verifier,
        &mut ReplayGuard::default(),
        10,
        &parent
    )
    .is_err());
    let subject = SubjectRevision::checked(&original.source_sha256).unwrap();
    let v2_wire = issuer
        .issue(
            ReceiptClass::MediaBackend,
            subject,
            ReceiptScope::checked(media_backend_scope_v2(&original.backend_id)).unwrap(),
            media_backend_payload_digest_v2(&original).unwrap(),
            10,
            None,
            Some(parent.clone()),
        )
        .unwrap();
    original.authenticity_receipt_digest = Some(v2_wire.receipt_digest());
    original.authenticity_attestation = Some(v2_wire);
    assert!(verify_media_backend_attestation_v2(
        &original,
        &verifier,
        &mut ReplayGuard::default(),
        10,
        &parent
    )
    .is_ok());
    assert!(verify_media_backend_attestation_v2(
        &original,
        &verifier,
        &mut ReplayGuard::default(),
        10,
        &stable_sha256("other parent")
    )
    .is_err());
}

#[test]
fn oversized_report_fails_closed_before_verification() {
    let mut original = report();
    original.diagnostics.push("x".repeat(16 * 1024 * 1024 + 1));
    assert!(media_backend_payload_digest_v2(&original).is_err());
}

#[test]
fn changed_attestation_digest_is_rejected_without_consuming_replay() {
    let mut original = report();
    let issuer = ReceiptIssuer::from_key_bytes("synthetic", &[27; 32]).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("synthetic", &[27; 32]).unwrap();
    let parent = stable_sha256("synthetic parent");
    let wire = issuer
        .issue(
            ReceiptClass::MediaBackend,
            SubjectRevision::checked(&original.source_sha256).unwrap(),
            ReceiptScope::checked(media_backend_scope_v2(&original.backend_id)).unwrap(),
            media_backend_payload_digest_v2(&original).unwrap(),
            10,
            None,
            Some(parent.clone()),
        )
        .unwrap();
    let actual_digest = wire.receipt_digest();
    original.authenticity_attestation = Some(wire);
    original.authenticity_receipt_digest = Some(stable_sha256("wrong receipt"));
    let mut replay = ReplayGuard::default();
    assert!(
        verify_media_backend_attestation_v2(&original, &verifier, &mut replay, 10, &parent)
            .is_err()
    );
    original.authenticity_receipt_digest = Some(actual_digest);
    assert!(
        verify_media_backend_attestation_v2(&original, &verifier, &mut replay, 10, &parent).is_ok()
    );
}
