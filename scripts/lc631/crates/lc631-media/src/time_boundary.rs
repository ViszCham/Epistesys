//! Validated finite rational-time values for an explicitly shared clock domain.
//! These types do not establish clock calibration, media provenance or authority.
#![forbid(unsafe_code)]

use crate::{RationalTime, TimeSpan};
use std::cmp::Ordering;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimeBoundaryError {
    ZeroTimescale,
    Reversed,
    EmptyInterval,
}

/// Explicit classification of a legacy span under a caller-Given common clock.
/// This is a numeric interpretation, not evidence of clock identity or provenance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TemporalExtent {
    Instant(Instant),
    Interval(HalfOpenInterval),
}

/// Validate both public endpoints before comparison; equality becomes an
/// explicit instant, never an empty half-open interval.
pub fn classify(span: &TimeSpan) -> Result<TemporalExtent, TimeBoundaryError> {
    let start = Instant::checked(span.start)?;
    let end = Instant::checked(span.end)?;
    match start.compare(&end) {
        Ordering::Less => Ok(TemporalExtent::Interval(HalfOpenInterval { start, end })),
        Ordering::Equal => Ok(TemporalExtent::Instant(start)),
        Ordering::Greater => Err(TimeBoundaryError::Reversed),
    }
}

/// A finite instant copied from a checked positive-timescale input.
/// Clock identity, origin and precision must be checked by the caller.
#[derive(Clone, Copy, Debug)]
pub struct Instant {
    value: RationalTime,
}
impl PartialEq for Instant {
    fn eq(&self, other: &Self) -> bool {
        self.compare(other) == Ordering::Equal
    }
}
impl Eq for Instant {}
impl Instant {
    pub fn checked(value: RationalTime) -> Result<Self, TimeBoundaryError> {
        if value.timescale == 0 {
            return Err(TimeBoundaryError::ZeroTimescale);
        }
        Ok(Self { value })
    }

    /// Exact rational ordering assuming an already shared clock domain.
    /// i64 × u32 fits i128, including both extreme signed endpoints.
    pub fn compare(&self, other: &Self) -> Ordering {
        let left = i128::from(self.value.ticks) * i128::from(other.value.timescale);
        let right = i128::from(other.value.ticks) * i128::from(self.value.timescale);
        left.cmp(&right)
    }
}

/// Strict nonempty [start,end); an equal-time legacy TimeSpan is not an interval.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HalfOpenInterval {
    start: Instant,
    end: Instant,
}
impl HalfOpenInterval {
    pub fn checked(span: &TimeSpan) -> Result<Self, TimeBoundaryError> {
        let start = Instant::checked(span.start)?;
        let end = Instant::checked(span.end)?;
        match start.compare(&end) {
            Ordering::Less => Ok(Self { start, end }),
            Ordering::Equal => Err(TimeBoundaryError::EmptyInterval),
            Ordering::Greater => Err(TimeBoundaryError::Reversed),
        }
    }

    pub fn contains(&self, instant: &Instant) -> bool {
        self.start.compare(instant) != Ordering::Greater
            && instant.compare(&self.end) == Ordering::Less
    }

    pub fn relation(&self, other: &Self) -> IntervalRelation {
        match self.end.compare(&other.start) {
            Ordering::Less => return IntervalRelation::Before,
            Ordering::Equal => return IntervalRelation::Meets,
            Ordering::Greater => {}
        }
        match other.end.compare(&self.start) {
            Ordering::Less => IntervalRelation::After,
            Ordering::Equal => IntervalRelation::MetBy,
            Ordering::Greater => IntervalRelation::Overlaps,
        }
    }
}

/// Overlaps includes containment and equality; adjacent intervals only meet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntervalRelation {
    Before,
    Meets,
    Overlaps,
    MetBy,
    After,
}
