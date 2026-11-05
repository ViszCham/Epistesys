use lc631_analysis::spatiotemporal_loss;

use lc631_substrate::Digest32;
use spatiotemporal_loss::*;

fn digest(value: u8) -> Digest32 {
    Digest32::from_bytes([value; 32])
}
fn binding() -> Binding {
    Binding {
        scope: digest(1),
        probe_set: digest(2),
        policy: digest(3),
        source_revision: digest(4),
        target_revision: digest(5),
        direction: Direction::Forward,
    }
}
fn interval(clock: u8, start: i64, end: i64) -> TimeInterval {
    TimeInterval::checked(
        Instant::declared(digest(clock), start, TimeUnit::Second),
        Instant::declared(digest(clock), end, TimeUnit::Second),
    )
    .unwrap()
}
fn time_map(b: Binding, offset: i64) -> TimeTranslation {
    TimeTranslation::declared(b, digest(10), digest(11), offset, TimeUnit::Second)
}
fn pair() -> (TimeInterval, TimeInterval) {
    (interval(10, 0, 1), interval(10, 2, 3))
}
fn target_pair() -> (TimeInterval, TimeInterval) {
    (interval(11, 10, 11), interval(11, 12, 13))
}

#[test]
fn tlcx19_negative_missing_observation_and_calibration_do_not_become_zero() {
    let b = binding();
    assert_eq!(
        compare_temporal(
            b,
            TemporalProbe::Before,
            None,
            Some(target_pair()),
            Some(time_map(b, 10))
        )
        .unwrap()
        .comparison(),
        RelationComparison::NeedsEvidence(Gap::MissingObservation)
    );
    assert_eq!(
        compare_temporal(
            b,
            TemporalProbe::Before,
            Some(pair()),
            Some(target_pair()),
            None
        )
        .unwrap()
        .comparison(),
        RelationComparison::NeedsEvidence(Gap::MissingCalibration)
    );
    assert_eq!(quantitative_gap(), Gap::UnsupportedQuantity);
}

#[test]
fn tlcx19_negative_reversed_order_is_a_difference() {
    let b = binding();
    let target = target_pair();
    assert_eq!(
        compare_temporal(
            b,
            TemporalProbe::Before,
            Some(pair()),
            Some((target.1, target.0)),
            Some(time_map(b, 10))
        )
        .unwrap()
        .comparison(),
        RelationComparison::DiffersFromDeclaredRelation
    );
}

#[test]
fn tlcx19_negative_foreign_clock_and_binding_are_incomparable() {
    let b = binding();
    let foreign = (interval(12, 10, 11), interval(12, 12, 13));
    assert_eq!(
        compare_temporal(
            b,
            TemporalProbe::Before,
            Some(pair()),
            Some(foreign),
            Some(time_map(b, 10))
        )
        .unwrap()
        .comparison(),
        RelationComparison::Incomparable(IncomparableReason::ClockMismatch)
    );
    for field in 0..6 {
        let mut stale = b;
        match field {
            0 => stale.scope = digest(90),
            1 => stale.probe_set = digest(90),
            2 => stale.policy = digest(90),
            3 => stale.source_revision = digest(90),
            4 => stale.target_revision = digest(90),
            5 => stale.direction = Direction::Reverse,
            _ => unreachable!(),
        }
        assert_eq!(
            compare_temporal(
                b,
                TemporalProbe::Before,
                Some(pair()),
                Some(target_pair()),
                Some(time_map(stale, 10))
            )
            .unwrap()
            .comparison(),
            RelationComparison::Incomparable(IncomparableReason::BindingMismatch)
        );
    }
}

#[test]
fn tlcx19_negative_empty_and_reversed_intervals_are_invalid() {
    let instant = Instant::declared(digest(10), 1, TimeUnit::Second);
    assert_eq!(
        TimeInterval::checked(instant, instant).unwrap_err(),
        InputError::EmptyOrReversedInterval
    );
    assert_eq!(
        TimeInterval::checked(instant, Instant::declared(digest(11), 2, TimeUnit::Second))
            .unwrap_err(),
        InputError::ClockMismatch
    );
    assert!(
        TimeInterval::checked(instant, Instant::declared(digest(10), 0, TimeUnit::Second)).is_err()
    );
}

#[test]
fn tlcx19_positive_translation_preserves_order_duration_and_mapped_start() {
    let b = binding();
    for probe in [
        TemporalProbe::Before,
        TemporalProbe::EqualDuration,
        TemporalProbe::MappedAbsoluteStart,
    ] {
        let result = compare_temporal(
            b,
            probe,
            Some(pair()),
            Some(target_pair()),
            Some(time_map(b, 10)),
        )
        .unwrap();
        assert_eq!(
            result.comparison(),
            RelationComparison::MatchesDeclaredRelation
        );
        assert_eq!(result.binding(), b);
        assert_eq!(result.validation_gap(), Gap::MissingEvidence);
    }
    // A translation never adds its origin offset to elapsed duration.
    assert_eq!(target_pair().0.duration_nanoseconds(), 1_000_000_000);
    assert_eq!(
        DEFINITION_REVISION,
        "OperationalMeasurement/finite-formal-v1.4"
    );
}

#[test]
fn tlcx19_positive_unit_conversion_and_before_meets_boundary() {
    let a = Instant::declared(digest(10), 1, TimeUnit::Second);
    let b = Instant::declared(digest(10), 1000, TimeUnit::Millisecond);
    assert_eq!(a, b);
    assert_eq!(
        Instant::declared(digest(10), 7, TimeUnit::Nanosecond).nanoseconds(),
        7
    );
    let scope = binding();
    let source = (interval(10, 0, 1), interval(10, 1, 2));
    let target = (interval(11, 10, 11), interval(11, 12, 13));
    for probe in [TemporalProbe::Before, TemporalProbe::Meets] {
        assert_eq!(
            compare_temporal(
                scope,
                probe,
                Some(source),
                Some(target),
                Some(time_map(scope, 10))
            )
            .unwrap()
            .comparison(),
            RelationComparison::DiffersFromDeclaredRelation
        );
    }
}

#[test]
fn tlcx19_positive_reverse_is_separately_bound() {
    let forward = binding();
    let reverse = Binding {
        source_revision: forward.target_revision,
        target_revision: forward.source_revision,
        direction: Direction::Reverse,
        ..forward
    };
    let map = TimeTranslation::declared(reverse, digest(11), digest(10), -10, TimeUnit::Second);
    assert_eq!(
        compare_temporal(
            reverse,
            TemporalProbe::MappedAbsoluteStart,
            Some(target_pair()),
            Some(pair()),
            Some(map)
        )
        .unwrap()
        .comparison(),
        RelationComparison::MatchesDeclaredRelation
    );
    assert_eq!(
        compare_temporal(
            forward,
            TemporalProbe::MappedAbsoluteStart,
            Some(pair()),
            Some(target_pair()),
            Some(map)
        )
        .unwrap()
        .comparison(),
        RelationComparison::Incomparable(IncomparableReason::BindingMismatch)
    );
}

fn frame(identity: u8, unit: SpatialUnit) -> Frame2 {
    Frame2::declared(digest(identity), unit)
}
fn points(f: Frame2) -> (Point2, Point2) {
    (Point2::declared(f, 0, 0), Point2::declared(f, 2, 3))
}
fn grid_map(b: Binding, source: Frame2, target: Frame2) -> GridMap2 {
    GridMap2::declared(
        b,
        source,
        target,
        (AxisSign::Positive, AxisSign::Positive),
        (10, 20),
    )
}

#[test]
fn tlcx19_negative_pixel_world_mix_and_missing_calibration_remain_visible() {
    let b = binding();
    let source = frame(20, SpatialUnit::Pixel);
    let target = frame(21, SpatialUnit::Millimeter);
    assert_eq!(
        compare_spatial(
            b,
            SpatialProbe::MappedPlacement,
            Some(points(source)),
            Some(points(target)),
            Some(grid_map(b, source, target))
        )
        .comparison(),
        RelationComparison::Incomparable(IncomparableReason::UnitMismatch)
    );
    assert_eq!(
        compare_spatial(
            b,
            SpatialProbe::Coincidence,
            Some(points(source)),
            Some(points(source)),
            None
        )
        .comparison(),
        RelationComparison::NeedsEvidence(Gap::MissingCalibration)
    );
    assert_eq!(
        compare_spatial(
            b,
            SpatialProbe::Coincidence,
            None,
            Some(points(source)),
            None
        )
        .comparison(),
        RelationComparison::NeedsEvidence(Gap::MissingObservation)
    );
}

#[test]
fn tlcx19_negative_wrong_spatial_frame_or_direction_is_incomparable() {
    let b = binding();
    let a = frame(20, SpatialUnit::Pixel);
    let c = frame(21, SpatialUnit::Pixel);
    assert_eq!(
        compare_spatial(
            b,
            SpatialProbe::XOrder,
            Some(points(c)),
            Some(points(c)),
            Some(grid_map(b, a, c))
        )
        .comparison(),
        RelationComparison::Incomparable(IncomparableReason::FrameMismatch)
    );
    let reverse = Binding {
        direction: Direction::Reverse,
        ..b
    };
    assert_eq!(
        compare_spatial(
            b,
            SpatialProbe::XOrder,
            Some(points(a)),
            Some(points(c)),
            Some(grid_map(reverse, a, c))
        )
        .comparison(),
        RelationComparison::Incomparable(IncomparableReason::BindingMismatch)
    );
}

#[test]
fn tlcx19_positive_declared_reflection_and_translation_are_not_defects() {
    let b = binding();
    let a = frame(20, SpatialUnit::Millimeter);
    let c = frame(21, SpatialUnit::Millimeter);
    let map = GridMap2::declared(b, a, c, (AxisSign::Negative, AxisSign::Positive), (10, 20));
    let target = (Point2::declared(c, 10, 20), Point2::declared(c, 8, 23));
    for probe in [
        SpatialProbe::XOrder,
        SpatialProbe::YOrder,
        SpatialProbe::Coincidence,
        SpatialProbe::MappedPlacement,
    ] {
        let result = compare_spatial(b, probe, Some(points(a)), Some(target), Some(map));
        assert_eq!(
            result.comparison(),
            RelationComparison::MatchesDeclaredRelation
        );
        assert_eq!(result.validation_gap(), Gap::MissingEvidence);
    }
    let reversed = (target.1, target.0);
    assert_eq!(
        compare_spatial(
            b,
            SpatialProbe::XOrder,
            Some(points(a)),
            Some(reversed),
            Some(map)
        )
        .comparison(),
        RelationComparison::DiffersFromDeclaredRelation
    );
}

#[test]
fn tlcx19_boundary_grid_extremes_do_not_negate_in_i64() {
    let b = binding();
    let a = frame(20, SpatialUnit::Pixel);
    let points = (
        Point2::declared(a, i64::MIN, i64::MAX),
        Point2::declared(a, i64::MAX, i64::MIN),
    );
    let map = GridMap2::declared(b, a, a, (AxisSign::Negative, AxisSign::Negative), (0, 0));
    assert_eq!(
        compare_spatial(
            b,
            SpatialProbe::XOrder,
            Some(points),
            Some(points),
            Some(map)
        )
        .comparison(),
        RelationComparison::DiffersFromDeclaredRelation
    );
}

#[test]
fn tlcx19_independent_hand_table_order_mutations() {
    let b = binding();
    // Hand-specified strict-before / meets expectations: no production-generated gold.
    let cases = [
        (0, 1, 2, 3, true, false),
        (0, 1, 1, 2, false, true),
        (1, 3, 2, 4, false, false),
        (4, 5, 0, 1, false, false),
    ];
    let fixed = pair(); // strict-before=true, meets=false
    for (a, z, c, d, before, meets) in cases {
        let target = (interval(11, a, z), interval(11, c, d));
        for (probe, same) in [
            (TemporalProbe::Before, before),
            (TemporalProbe::Meets, !meets),
        ] {
            let expected = if same {
                RelationComparison::MatchesDeclaredRelation
            } else {
                RelationComparison::DiffersFromDeclaredRelation
            };
            assert_eq!(
                compare_temporal(b, probe, Some(fixed), Some(target), Some(time_map(b, 0)))
                    .unwrap()
                    .comparison(),
                expected
            );
        }
    }
}
