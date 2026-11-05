#![forbid(unsafe_code)]

use lc631_core::stable_sha256;
use lc631_media::attestation_v3::{write_observation_v3, CanonicalWriter};
use lc631_media::{MediaObservation, ObservationKind, RationalTime, TimeSpan};

fn sample() -> MediaObservation {
    MediaObservation {
        observation_id: "observation".into(),
        kind: ObservationKind::TranscriptCandidate,
        span: TimeSpan::checked(
            RationalTime::checked(1, 1000).unwrap(),
            RationalTime::checked(2, 1000).unwrap(),
        )
        .unwrap(),
        track_id: Some(1),
        label: "label".into(),
        backend_id: "backend".into(),
        model_revision: "model".into(),
        evidence_digest: stable_sha256("evidence"),
        source_fragment_digest: stable_sha256("source"),
        provenance_digest: stable_sha256("provenance"),
        configuration_digest: stable_sha256("config"),
        candidate_only: true,
        identity_committed: false,
        internal_state_committed: false,
    }
}

fn encode(value: &MediaObservation) -> Vec<u8> {
    let mut bytes = Vec::new();
    write_observation_v3(&mut CanonicalWriter::new(&mut bytes, 4096), value).unwrap();
    bytes
}

#[test]
fn each_observation_field_changes_its_exact_binary_encoding() {
    let original = sample();
    let baseline = encode(&original);
    let mut changes = Vec::new();
    let mut value = original.clone();
    value.observation_id.push('x');
    changes.push(value);
    let mut value = original.clone();
    value.kind = ObservationKind::OcrTextCandidate;
    changes.push(value);
    let mut value = original.clone();
    value.span.start.ticks += 1;
    changes.push(value);
    let mut value = original.clone();
    value.span.start.timescale += 1;
    changes.push(value);
    let mut value = original.clone();
    value.span.end.ticks += 1;
    changes.push(value);
    let mut value = original.clone();
    value.span.end.timescale += 1;
    changes.push(value);
    let mut value = original.clone();
    value.track_id = None;
    changes.push(value);
    let mut value = original.clone();
    value.label.push('x');
    changes.push(value);
    let mut value = original.clone();
    value.backend_id.push('x');
    changes.push(value);
    let mut value = original.clone();
    value.model_revision.push('x');
    changes.push(value);
    let mut value = original.clone();
    value.evidence_digest.push('x');
    changes.push(value);
    let mut value = original.clone();
    value.source_fragment_digest.push('x');
    changes.push(value);
    let mut value = original.clone();
    value.provenance_digest.push('x');
    changes.push(value);
    let mut value = original.clone();
    value.configuration_digest.push('x');
    changes.push(value);
    let mut value = original.clone();
    value.candidate_only = false;
    changes.push(value);
    let mut value = original.clone();
    value.identity_committed = true;
    changes.push(value);
    let mut value = original.clone();
    value.internal_state_committed = true;
    changes.push(value);
    assert_eq!(changes.len(), 17);
    for (index, value) in changes.iter().enumerate() {
        assert_ne!(
            encode(value),
            baseline,
            "field mutation {index} was omitted"
        );
    }
    assert_eq!(encode(&original), baseline);
}

#[test]
fn observation_encoder_propagates_bounded_writer_failure() {
    let mut bytes = Vec::new();
    assert!(write_observation_v3(&mut CanonicalWriter::new(&mut bytes, 1), &sample()).is_err());
}

#[test]
fn minimal_observation_has_independent_literal_binary_vector() {
    let mut value = sample();
    value.observation_id.clear();
    value.kind = ObservationKind::ShotBoundary;
    value.span = TimeSpan::checked(
        RationalTime::checked(0, 1).unwrap(),
        RationalTime::checked(0, 1).unwrap(),
    )
    .unwrap();
    value.track_id = None;
    value.label.clear();
    value.backend_id.clear();
    value.model_revision.clear();
    value.evidence_digest.clear();
    value.source_fragment_digest.clear();
    value.provenance_digest.clear();
    value.configuration_digest.clear();
    let mut expected = vec![
        1, 0, 0, 0, 0, 0, 0, 0, 0, // empty id
        0x14, 1, 0, 0, 0, // explicit shot-boundary enum code
        0x11, 0, 0, 0, 0, 0, 0, 0, 0, // start ticks
        0x12, 1, 0, 0, 0, // start scale
        0x11, 0, 0, 0, 0, 0, 0, 0, 0, // end ticks
        0x12, 1, 0, 0, 0,    // end scale
        0x15, // absent track
    ];
    for _ in 0..7 {
        expected.extend_from_slice(&[1, 0, 0, 0, 0, 0, 0, 0, 0]);
    }
    expected.extend_from_slice(&[0x13, 1, 0x13, 0, 0x13, 0]);
    assert_eq!(encode(&value), expected);
}
