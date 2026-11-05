//! A Given same-clock-domain fixture, not clock calibration or media provenance.
use lc631_media::time_boundary::{Instant, IntervalRelation, TemporalExtent, TimeBoundaryError};
use lc631_media::{
    MediaError, MediaObservation, MultimodalEventGraph, ObservationKind, RationalTime, TimeSpan,
};

fn t(ticks: i64, timescale: u32) -> RationalTime {
    RationalTime { ticks, timescale }
}
fn observation(id: &str, start: RationalTime, end: RationalTime) -> MediaObservation {
    let pin = format!("sha256:{}", "a".repeat(64));
    MediaObservation {
        observation_id: id.into(),
        kind: ObservationKind::ShotBoundary,
        span: TimeSpan { start, end },
        track_id: None,
        label: "Given candidate".into(),
        backend_id: "fixture".into(),
        model_revision: "fixture-v1".into(),
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
fn tlcx_media_graph_exposes_instant_and_half_open_interval_without_changing_json() {
    let mut graph = MultimodalEventGraph::default();
    let interval = observation("interval", t(0, 1), t(1000, 1000));
    let original = serde_json::to_value(&interval).unwrap();
    graph.add(interval).unwrap();
    graph
        .add(observation("instant", t(1000, 1000), t(1, 1)))
        .unwrap();
    assert_eq!(
        serde_json::to_value(&graph.observations()[0]).unwrap(),
        original
    );
    let extent = graph.time_extent(0).unwrap().unwrap();
    let instant = graph.time_extent(1).unwrap().unwrap();
    let TemporalExtent::Interval(interval) = extent else {
        panic!("expected interval");
    };
    let TemporalExtent::Instant(point) = instant else {
        panic!("expected explicit instant");
    };
    assert!(interval.contains(&Instant::checked(t(0, 1)).unwrap()));
    assert!(!interval.contains(&point));
    assert_eq!(point, Instant::checked(t(1, 1)).unwrap());
    assert_eq!(graph.time_extent(2).unwrap(), None);
}

#[test]
fn tlcx_media_graph_interval_consumer_distinguishes_meet_from_overlap() {
    let mut graph = MultimodalEventGraph::default();
    for (id, start, end) in [
        ("a", t(0, 1), t(1, 1)),
        ("overlap", t(1, 2), t(3, 2)),
        ("meet", t(1000, 1000), t(2, 1)),
    ] {
        graph.add(observation(id, start, end)).unwrap();
    }
    let TemporalExtent::Interval(a) = graph.time_extent(0).unwrap().unwrap() else {
        panic!("expected interval");
    };
    let TemporalExtent::Interval(overlap) = graph.time_extent(1).unwrap().unwrap() else {
        panic!("expected interval");
    };
    let TemporalExtent::Interval(meet) = graph.time_extent(2).unwrap().unwrap() else {
        panic!("expected interval");
    };
    assert_eq!(a.relation(&overlap), IntervalRelation::Overlaps);
    assert_eq!(a.relation(&meet), IntervalRelation::Meets);
}

#[test]
fn tlcx_media_observation_mutation_is_revalidated_before_classification_and_graph_add() {
    let mut value = observation("raw", t(0, 1), t(1, 1));
    value.span.start.timescale = 0;
    assert_eq!(value.time_extent(), Err(TimeBoundaryError::ZeroTimescale));
    let mut graph = MultimodalEventGraph::default();
    assert!(matches!(graph.add(value), Err(MediaError::ZeroTimescale)));
    assert!(graph.observations().is_empty());
    let mut reversed = observation("reversed", t(0, 1), t(1, 1));
    reversed.span.start = t(3, 1);
    assert_eq!(reversed.time_extent(), Err(TimeBoundaryError::Reversed));
    assert!(matches!(
        graph.add(reversed),
        Err(MediaError::NegativeTimeSpan)
    ));
    assert!(graph.observations().is_empty());
}

#[test]
fn tlcx_media_checked_extent_snapshot_survives_later_public_mutation() {
    let mut value = observation("mutable", t(0, 1), t(1, 1));
    let captured = value.time_extent().unwrap();
    value.span.end.timescale = 0;
    assert_eq!(value.time_extent(), Err(TimeBoundaryError::ZeroTimescale));
    let TemporalExtent::Interval(interval) = captured else {
        panic!("expected interval");
    };
    assert!(interval.contains(&Instant::checked(t(1, 2)).unwrap()));
}
