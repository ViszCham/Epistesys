//! Finite rational-time value boundaries only, not clock calibration or media truth.
use lc631_media::time_boundary::{HalfOpenInterval, Instant, IntervalRelation, TimeBoundaryError};
use lc631_media::{RationalTime, TimeSpan};

fn raw(ticks: i64, timescale: u32) -> RationalTime {
    RationalTime { ticks, timescale }
}
fn span(start: RationalTime, end: RationalTime) -> TimeSpan {
    TimeSpan { start, end }
}

#[test]
fn tlcx_media_interval_rejects_raw_zero_reversed_and_empty() {
    for bad in [span(raw(0, 0), raw(1, 1)), span(raw(0, 1), raw(1, 0))] {
        assert_eq!(
            HalfOpenInterval::checked(&bad),
            Err(TimeBoundaryError::ZeroTimescale)
        );
    }
    assert_eq!(
        HalfOpenInterval::checked(&span(raw(2, 1), raw(1, 1))),
        Err(TimeBoundaryError::Reversed)
    );
    assert_eq!(
        HalfOpenInterval::checked(&span(raw(1000, 1000), raw(1, 1))),
        Err(TimeBoundaryError::EmptyInterval)
    );
}

#[test]
fn tlcx_media_instant_is_explicit_and_rejects_zero_scale() {
    assert_eq!(
        Instant::checked(raw(1, 0)),
        Err(TimeBoundaryError::ZeroTimescale)
    );
    let instant = Instant::checked(raw(1000, 1000)).unwrap();
    let equal = Instant::checked(raw(1, 1)).unwrap();
    assert_eq!(instant.compare(&equal), std::cmp::Ordering::Equal);
    assert_eq!(instant, equal);
}

#[test]
fn tlcx_media_nonempty_interval_boundary_is_half_open() {
    let interval = HalfOpenInterval::checked(&span(raw(1, 1), raw(2, 1))).unwrap();
    for (time, expected) in [
        (raw(999, 1000), false),
        (raw(1, 1), true),
        (raw(1500, 1000), true),
        (raw(2, 1), false),
    ] {
        assert_eq!(
            interval.contains(&Instant::checked(time).unwrap()),
            expected
        );
    }
}

#[test]
fn tlcx_media_interval_relation_distinguishes_before_meets_overlap_after() {
    let a = HalfOpenInterval::checked(&span(raw(0, 1), raw(1, 1))).unwrap();
    let meet = HalfOpenInterval::checked(&span(raw(1000, 1000), raw(2, 1))).unwrap();
    let later = HalfOpenInterval::checked(&span(raw(2, 1), raw(3, 1))).unwrap();
    let overlap = HalfOpenInterval::checked(&span(raw(1, 2), raw(3, 2))).unwrap();
    assert_eq!(a.relation(&meet), IntervalRelation::Meets);
    assert_eq!(meet.relation(&a), IntervalRelation::MetBy);
    assert_eq!(a.relation(&later), IntervalRelation::Before);
    assert_eq!(later.relation(&a), IntervalRelation::After);
    assert_eq!(a.relation(&overlap), IntervalRelation::Overlaps);
    assert_eq!(overlap.relation(&a), IntervalRelation::Overlaps);
}

#[test]
fn tlcx_media_owned_boundary_detaches_from_public_mutation() {
    let mut original = span(raw(0, 1), raw(1, 1));
    let interval = HalfOpenInterval::checked(&original).unwrap();
    original.end.timescale = 0;
    assert_eq!(
        HalfOpenInterval::checked(&original),
        Err(TimeBoundaryError::ZeroTimescale)
    );
    assert!(interval.contains(&Instant::checked(raw(1, 2)).unwrap()));
}

#[test]
fn tlcx_media_extreme_i64_ticks_and_u32_scales_compare_without_overflow() {
    let interval =
        HalfOpenInterval::checked(&span(raw(i64::MIN, u32::MAX), raw(i64::MAX, u32::MAX))).unwrap();
    assert!(interval.contains(&Instant::checked(raw(0, u32::MAX)).unwrap()));
    assert_eq!(
        Instant::checked(raw(i64::MIN, u32::MAX))
            .unwrap()
            .compare(&Instant::checked(raw(i64::MAX, 1)).unwrap()),
        std::cmp::Ordering::Less
    );
}

#[test]
fn tlcx_media_negative_fraction_order_and_zero_equality_are_transitive() {
    let minus_three_halves = Instant::checked(raw(-3, 2)).unwrap();
    let minus_one = Instant::checked(raw(-1000, 1000)).unwrap();
    let minus_half = Instant::checked(raw(-1, 2)).unwrap();
    let zero_a = Instant::checked(raw(0, 1)).unwrap();
    let zero_b = Instant::checked(raw(0, u32::MAX)).unwrap();
    assert_eq!(
        minus_three_halves.compare(&minus_one),
        std::cmp::Ordering::Less
    );
    assert_eq!(minus_one.compare(&minus_half), std::cmp::Ordering::Less);
    assert_eq!(
        minus_three_halves.compare(&minus_half),
        std::cmp::Ordering::Less
    );
    assert_eq!(minus_half.compare(&zero_a), std::cmp::Ordering::Less);
    assert_eq!(zero_a, zero_b);
    assert_eq!(
        HalfOpenInterval::checked(&span(raw(-1, 1), raw(-3, 2))),
        Err(TimeBoundaryError::Reversed)
    );
}

#[test]
fn tlcx_media_equal_and_contained_intervals_overlap_in_both_directions() {
    let outer = HalfOpenInterval::checked(&span(raw(0, 1), raw(2000, 1000))).unwrap();
    let equal = HalfOpenInterval::checked(&span(raw(0, u32::MAX), raw(2, 1))).unwrap();
    let inner = HalfOpenInterval::checked(&span(raw(1, 2), raw(3, 2))).unwrap();
    assert_eq!(outer, equal);
    assert_eq!(outer.relation(&equal), IntervalRelation::Overlaps);
    assert_eq!(outer.relation(&inner), IntervalRelation::Overlaps);
    assert_eq!(inner.relation(&outer), IntervalRelation::Overlaps);
}

#[test]
fn tlcx_media_extreme_valid_instants_obey_order_matrix() {
    let values = [
        Instant::checked(raw(i64::MIN, 1)).unwrap(),
        Instant::checked(raw(i64::MIN, u32::MAX)).unwrap(),
        Instant::checked(raw(0, u32::MAX)).unwrap(),
        Instant::checked(raw(i64::MAX, u32::MAX)).unwrap(),
        Instant::checked(raw(i64::MAX, 1)).unwrap(),
    ];
    for (i, left) in values.iter().enumerate() {
        for (j, right) in values.iter().enumerate() {
            assert_eq!(left.compare(right), i.cmp(&j));
        }
    }
}

#[test]
fn tlcx_media_interval_owns_original_start_and_end_values() {
    let mut original = span(raw(-2, 1), raw(2, 1));
    let captured = HalfOpenInterval::checked(&original).unwrap();
    original.start.ticks = 100;
    original.end.ticks = -100;
    assert_eq!(
        HalfOpenInterval::checked(&original),
        Err(TimeBoundaryError::Reversed)
    );
    assert!(captured.contains(&Instant::checked(raw(0, 1)).unwrap()));
    assert!(!captured.contains(&Instant::checked(raw(2, 1)).unwrap()));
}
