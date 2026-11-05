#![forbid(unsafe_code)]

use lc631_core::stable_sha256;
use lc631_media::attestation_v3::{
    media_backend_payload_digest_v3, media_backend_scope_v3, verify_media_backend_attestation_v3,
    write_media_report_v3, CanonicalWriter, MediaAttestationV3Error,
};
use lc631_media::{
    BackendFamily, BackendState, MediaBackendExecutionReceipt, MediaExecutionPlan,
    MediaObservation, MediaProcessReceipt, MediaResourceBudget, MultimodalEventGraph,
    ObservationKind, RationalTime, TimeSpan, MEDIA_BACKEND_SCHEMA,
};
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptIssuer, ReceiptScope, ReceiptVerifier, ReplayGuard, SubjectRevision,
    UntrustedReceipt,
};
use lc631_tl::{CanonicalTranslationEnvelope, ProjectionDefectGraphV3, SourceLedgerId};
use std::collections::BTreeMap;

fn report() -> MediaBackendExecutionReceipt {
    let d = stable_sha256("fixture");
    let mut graph = MultimodalEventGraph::default();
    graph
        .add(MediaObservation {
            observation_id: "1".into(),
            kind: ObservationKind::OcrTextCandidate,
            span: TimeSpan::checked(
                RationalTime::checked(0, 1).unwrap(),
                RationalTime::checked(1, 1).unwrap(),
            )
            .unwrap(),
            track_id: Some(1),
            label: "label".into(),
            backend_id: "backend".into(),
            model_revision: "model".into(),
            evidence_digest: d.clone(),
            source_fragment_digest: d.clone(),
            provenance_digest: d.clone(),
            configuration_digest: d.clone(),
            candidate_only: true,
            identity_committed: false,
            internal_state_committed: false,
        })
        .unwrap();
    let projection = ProjectionDefectGraphV3::new(SourceLedgerId(1), "rev", "source").unwrap();
    MediaBackendExecutionReceipt {
        schema_version: MEDIA_BACKEND_SCHEMA,
        source_sha256: d.clone(),
        backend_id: "backend".into(),
        backend_version: "version".into(),
        model_revision: "model".into(),
        model_cache_manifest_sha256: d.clone(),
        model_card_license_hint: "hint".into(),
        backend_receipt_sha256: d.clone(),
        execution_plan: MediaExecutionPlan {
            source_snapshot_sha256: d.clone(),
            source_bytes: 2,
            python_sha256: d.clone(),
            backend_script_sha256: d.clone(),
            ffmpeg_sha256: d.clone(),
            model_cache_manifest_sha256: d.clone(),
            configuration_digest: d.clone(),
            resource_budget: MediaResourceBudget::default(),
            network_policy: "offline".into(),
            network_isolation_os_enforced: false,
        },
        process: MediaProcessReceipt {
            exit_code: Some(0),
            stdout_digest: d.clone(),
            stderr_digest: d.clone(),
            stdout_truncated: false,
            stderr_truncated: false,
            process_tree_kill_attempted: false,
            process_tree_kill_verified: false,
            memory_limit_bytes: None,
            memory_limit_kind: "none".into(),
        },
        executed_family_states: BTreeMap::from([(
            BackendFamily::Ocr,
            BackendState::ExecutedCandidate,
        )]),
        graph,
        diagnostics: vec!["diagnostic".into()],
        source_authority_preserved: false,
        identity_inference_committed: false,
        internal_state_committed: false,
        family_validation_count: 0,
        authenticity_receipt_digest: None,
        authenticity_attestation: None,
        translation_envelope: CanonicalTranslationEnvelope::from_graph(&projection, vec![]),
        claim_boundary: "fixture only",
    }
}
fn encode(value: &MediaBackendExecutionReceipt) -> Vec<u8> {
    let mut bytes = Vec::new();
    write_media_report_v3(&mut CanonicalWriter::new(&mut bytes, 16384), value).unwrap();
    bytes
}
#[test]
fn every_top_level_report_field_and_embedded_component_is_bound() {
    let original = report();
    let baseline = encode(&original);
    let mut changes = Vec::new();
    let mut v = original.clone();
    v.source_sha256.push('x');
    changes.push(v);
    let mut v = original.clone();
    v.backend_id.push('x');
    changes.push(v);
    let mut v = original.clone();
    v.backend_version.push('x');
    changes.push(v);
    let mut v = original.clone();
    v.model_revision.push('x');
    changes.push(v);
    let mut v = original.clone();
    v.model_cache_manifest_sha256.push('x');
    changes.push(v);
    let mut v = original.clone();
    v.model_card_license_hint.push('x');
    changes.push(v);
    let mut v = original.clone();
    v.backend_receipt_sha256.push('x');
    changes.push(v);
    let mut v = original.clone();
    v.execution_plan.source_bytes += 1;
    changes.push(v);
    let mut v = original.clone();
    v.process.exit_code = Some(1);
    changes.push(v);
    let mut v = original.clone();
    v.executed_family_states
        .insert(BackendFamily::Ocr, BackendState::FailedWithDiagnostics);
    changes.push(v);
    let mut v = original.clone();
    let mut obs = v.graph.observations()[0].clone();
    obs.track_id = Some(2);
    let mut g = MultimodalEventGraph::default();
    g.add(obs).unwrap();
    v.graph = g;
    changes.push(v);
    let mut v = original.clone();
    v.diagnostics.push("changed".into());
    changes.push(v);
    let mut v = original.clone();
    v.source_authority_preserved = true;
    changes.push(v);
    let mut v = original.clone();
    v.identity_inference_committed = true;
    changes.push(v);
    let mut v = original.clone();
    v.internal_state_committed = true;
    changes.push(v);
    let mut v = original.clone();
    v.family_validation_count = 1;
    changes.push(v);
    let mut v = original.clone();
    v.translation_envelope.source_digest.push('x');
    changes.push(v);
    let mut v = original.clone();
    v.claim_boundary = "different";
    changes.push(v);
    for (i, v) in changes.iter().enumerate() {
        assert_ne!(encode(v), baseline, "report field {i} omitted");
    }
}
#[test]
fn excluded_self_digest_does_not_create_a_circular_preimage() {
    let original = report();
    let mut changed = original.clone();
    changed.authenticity_receipt_digest = Some(stable_sha256("receipt"));
    assert_eq!(encode(&original), encode(&changed));
}
#[test]
fn report_capacity_failure_is_explicit() {
    let mut bytes = Vec::new();
    assert!(write_media_report_v3(&mut CanonicalWriter::new(&mut bytes, 1), &report()).is_err());
}

#[test]
fn unknown_report_and_envelope_versions_are_rejected_before_writing() {
    let mut value = report();
    value.schema_version = "unknown-version";
    let mut bytes = Vec::new();
    assert!(write_media_report_v3(&mut CanonicalWriter::new(&mut bytes, 16384), &value).is_err());
    assert!(bytes.is_empty());
    value = report();
    value.translation_envelope.schema_version = "unknown-version";
    assert!(write_media_report_v3(&mut CanonicalWriter::new(&mut bytes, 16384), &value).is_err());
    assert!(bytes.is_empty());
}

fn sign_report(value: &mut MediaBackendExecutionReceipt, scope: &str, payload: String) -> String {
    let parent = stable_sha256("parent");
    let issuer = ReceiptIssuer::from_key_bytes("v3-fixture", &[33; 32]).unwrap();
    let wire = issuer
        .issue(
            ReceiptClass::MediaBackend,
            SubjectRevision::checked(&value.source_sha256).unwrap(),
            ReceiptScope::checked(scope).unwrap(),
            payload,
            10,
            Some(20),
            Some(parent.clone()),
        )
        .unwrap();
    value.authenticity_receipt_digest = Some(wire.receipt_digest());
    value.authenticity_attestation = Some(wire);
    parent
}
fn verify(value: &MediaBackendExecutionReceipt, parent: &str, now: u64) -> bool {
    let verifier = ReceiptVerifier::from_key_bytes("v3-fixture", &[33; 32]).unwrap();
    verify_media_backend_attestation_v3(value, &verifier, &mut ReplayGuard::default(), now, parent)
        .is_ok()
}
#[test]
fn v3_fresh_report_verifies_and_mutation_fails() {
    let mut original = report();
    let digest = media_backend_payload_digest_v3(&original).unwrap();
    let scope = media_backend_scope_v3(&original.backend_id);
    let parent = sign_report(&mut original, &scope, digest);
    assert!(verify(&original, &parent, 10));
    let mut changed = original.clone();
    changed.process.exit_code = Some(1);
    assert!(!verify(&changed, &parent, 10));
    assert!(!verify(&original, &stable_sha256("other parent"), 10));
    assert!(!verify(&original, &parent, 20));
    changed = original.clone();
    changed.authenticity_receipt_digest = Some(stable_sha256("other receipt"));
    assert!(!verify(&changed, &parent, 10));
}

#[test]
fn v1_and_v2_scope_and_payload_never_fall_back_to_v3() {
    for version in 1..=2 {
        let mut value = report();
        let digest = if version == 1 {
            lc631_media::media_backend_payload_digest(&value)
        } else {
            lc631_media::attestation_v2::media_backend_payload_digest_v2(&value).unwrap()
        };
        let scope = if version == 1 {
            format!("media/backend/{}", value.backend_id)
        } else {
            lc631_media::attestation_v2::media_backend_scope_v2(&value.backend_id)
        };
        let parent = sign_report(&mut value, &scope, digest);
        assert!(!verify(&value, &parent, 10));
    }
}

#[test]
fn oversized_untrusted_wire_and_parent_are_rejected_before_replay() {
    let mut value = report();
    let digest = media_backend_payload_digest_v3(&value).unwrap();
    let scope = media_backend_scope_v3(&value.backend_id);
    let parent = sign_report(&mut value, &scope, digest);
    let original = value.authenticity_attestation.clone().unwrap();
    let mut json = serde_json::to_value(&original).unwrap();
    json["claims"]["schema_version"] = serde_json::Value::String("x".repeat(4096));
    // The bounded receipt decoder rejects this at ingress. Do not bypass that
    // earlier boundary merely to construct an oversized report fixture.
    assert!(serde_json::from_value::<UntrustedReceipt>(json).is_err());
    let verifier = ReceiptVerifier::from_key_bytes("v3-fixture", &[33; 32]).unwrap();
    let mut replay = ReplayGuard::default();
    value.source_sha256 = "x".repeat(257);
    assert_eq!(
        verify_media_backend_attestation_v3(&value, &verifier, &mut replay, 10, &parent),
        Err(MediaAttestationV3Error::InvalidBinding)
    );
    assert_eq!(replay.observed_count(), 0);
    value.source_sha256 = report().source_sha256;
    value.authenticity_attestation = Some(original);
    assert_eq!(
        verify_media_backend_attestation_v3(&value, &verifier, &mut replay, 10, &"x".repeat(4096)),
        Err(MediaAttestationV3Error::InvalidBinding)
    );
    assert_eq!(replay.observed_count(), 0);
    assert!(
        verify_media_backend_attestation_v3(&value, &verifier, &mut replay, 10, &parent).is_ok()
    );
}

#[test]
fn whole_minimal_report_matches_independent_canonical_byte_vector() {
    let mut value = report();
    value.source_sha256.clear();
    value.backend_id.clear();
    value.backend_version.clear();
    value.model_revision.clear();
    value.model_cache_manifest_sha256.clear();
    value.model_card_license_hint.clear();
    value.backend_receipt_sha256.clear();
    value.execution_plan.source_snapshot_sha256.clear();
    value.execution_plan.source_bytes = 0;
    value.execution_plan.python_sha256.clear();
    value.execution_plan.backend_script_sha256.clear();
    value.execution_plan.ffmpeg_sha256.clear();
    value.execution_plan.model_cache_manifest_sha256.clear();
    value.execution_plan.configuration_digest.clear();
    value.execution_plan.resource_budget = MediaResourceBudget {
        max_stdout_bytes: 0,
        max_stderr_bytes: 0,
        max_temporary_bytes: 0,
        max_decoded_frames: 0,
        max_pcm_samples: 0,
        max_observations: 0,
    };
    value.execution_plan.network_policy.clear();
    value.process.exit_code = None;
    value.process.stdout_digest.clear();
    value.process.stderr_digest.clear();
    value.process.memory_limit_kind.clear();
    value.executed_family_states.clear();
    value.graph = MultimodalEventGraph::default();
    value.diagnostics.clear();
    value.translation_envelope.source_ledger_id = SourceLedgerId(0);
    value.translation_envelope.source_revision.clear();
    value.translation_envelope.source_digest.clear();
    value.translation_envelope.gate = lc631_tl::ProjectionGateDecision::Clarify;
    value.claim_boundary = "";
    fn text(bytes: &mut Vec<u8>, value: &str) {
        bytes.push(1);
        bytes.extend_from_slice(&(value.len() as u64).to_le_bytes());
        bytes.extend_from_slice(value.as_bytes());
    }
    fn zero(bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(&[0x10, 0, 0, 0, 0, 0, 0, 0, 0]);
    }
    fn no(bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(&[0x13, 0]);
    }
    let mut expected = Vec::new();
    text(&mut expected, "lc631/media-backend-report/binary-v3");
    text(&mut expected, "lc631-media-backend.v1");
    for _ in 0..7 {
        text(&mut expected, "");
    }
    text(&mut expected, "");
    zero(&mut expected);
    for _ in 0..5 {
        text(&mut expected, "");
    }
    for _ in 0..6 {
        zero(&mut expected);
    }
    text(&mut expected, "");
    no(&mut expected);
    expected.push(0x17);
    text(&mut expected, "");
    text(&mut expected, "");
    for _ in 0..4 {
        no(&mut expected);
    }
    expected.push(0x15);
    text(&mut expected, "");
    for _ in 0..3 {
        zero(&mut expected);
    }
    for _ in 0..3 {
        no(&mut expected);
    }
    zero(&mut expected);
    text(&mut expected, "lc631-translation-envelope.v3");
    zero(&mut expected);
    text(&mut expected, "");
    text(&mut expected, "");
    for _ in 0..3 {
        zero(&mut expected);
    }
    expected.extend_from_slice(&[0x14, 3, 0, 0, 0]);
    no(&mut expected);
    text(&mut expected, "");
    assert_eq!(encode(&value), expected);
}
