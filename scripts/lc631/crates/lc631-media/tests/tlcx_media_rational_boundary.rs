//! 日本語: 公開time fieldからの分母0をtrusted時刻へ昇格させない負例。
//! English: Public time-field mutations must not admit undefined rational times.
//! These are candidate-data boundary tests, not authenticated media evidence.
use lc631_media::{
    MediaError, MediaObservation, MultimodalEventGraph, ObservationKind, RationalTime, TimeSpan,
};

fn fixture() -> MediaObservation {
    let pin = format!("sha256:{}", "a".repeat(64));
    MediaObservation {
        observation_id: "given-time-boundary".into(),
        kind: ObservationKind::ShotBoundary,
        span: TimeSpan::checked(
            RationalTime::checked(0, 1000).unwrap(),
            RationalTime::checked(1000, 1000).unwrap(),
        )
        .unwrap(),
        track_id: None,
        label: "Given candidate, no world truth".into(),
        backend_id: "test-given".into(),
        model_revision: "test-given-v1".into(),
        evidence_digest: pin.clone(),
        source_fragment_digest: pin.clone(),
        provenance_digest: pin.clone(),
        configuration_digest: pin,
        candidate_only: true,
        identity_committed: false,
        internal_state_committed: false,
    }
}

#[test]
fn tlcx_media_timespan_checks_raw_start_timescale_before_comparison() {
    let raw = RationalTime {
        ticks: 0,
        timescale: 0,
    };
    let valid = RationalTime::checked(1, 1).unwrap();
    assert!(matches!(
        TimeSpan::checked(raw, valid),
        Err(MediaError::ZeroTimescale)
    ));
}

#[test]
fn tlcx_media_timespan_checks_raw_end_timescale_before_comparison() {
    let raw = RationalTime {
        ticks: 1,
        timescale: 0,
    };
    let valid = RationalTime::checked(0, 1).unwrap();
    assert!(matches!(
        TimeSpan::checked(valid, raw),
        Err(MediaError::ZeroTimescale)
    ));
}

#[test]
fn tlcx_media_public_observation_validation_rejects_mutated_timescale() {
    let mut observation = fixture();
    observation.span.start.timescale = 0;
    assert!(matches!(
        observation.validate(),
        Err(MediaError::ZeroTimescale)
    ));
}

#[test]
fn tlcx_media_actual_graph_add_rejects_public_zero_timescale_and_retains_nothing() {
    let mut observation = fixture();
    observation.span.end.timescale = 0;
    let mut graph = MultimodalEventGraph::default();
    assert!(matches!(
        graph.add(observation),
        Err(MediaError::ZeroTimescale)
    ));
    assert!(graph.observations().is_empty());
}

#[test]
fn tlcx_media_actual_graph_add_rechecks_raw_reversed_span() {
    let mut observation = fixture();
    // Public fields may bypass the original TimeSpan constructor entirely.
    observation.span.start = RationalTime::checked(2, 1).unwrap();
    observation.span.end = RationalTime::checked(1, 1).unwrap();
    let mut graph = MultimodalEventGraph::default();
    assert!(matches!(
        graph.add(observation),
        Err(MediaError::NegativeTimeSpan)
    ));
    assert!(graph.observations().is_empty());
}

#[test]
fn tlcx_media_nonzero_mixed_scale_and_existing_instant_span_remain_valid() {
    let mut graph = MultimodalEventGraph::default();
    let valid = fixture();
    assert!(valid.validate().is_ok());
    graph.add(valid).unwrap();
    assert_eq!(graph.observations().len(), 1);
    // Different scales represent the same finite instant; no GCD normalization
    // or strict-positive duration is newly imposed by these boundary tests.
    let instant = TimeSpan::checked(
        RationalTime::checked(1000, 1000).unwrap(),
        RationalTime::checked(1, 1).unwrap(),
    )
    .unwrap();
    assert!(instant.start.compare(&instant.end).is_eq());
}
