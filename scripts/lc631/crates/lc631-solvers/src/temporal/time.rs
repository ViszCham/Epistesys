//! 日本語: clockを推測せず、整数単位だけを厳密換算する。
//! English: Exact integer-unit conversion without inferred clock calibration.
use super::TemporalError;
use lc631_substrate::Digest32;
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimeScale {
    Monotonic,
    Utc,
    Tai,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimeUnit {
    Nanosecond,
    Microsecond,
    Millisecond,
    Second,
}

impl TimeUnit {
    fn nanoseconds(self) -> i128 {
        match self {
            Self::Nanosecond => 1,
            Self::Microsecond => 1_000,
            Self::Millisecond => 1_000_000,
            Self::Second => 1_000_000_000,
        }
    }
}

/// Clock identities are declared bindings, not authentication or calibration receipts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClockSpec {
    clock: Digest32,
    scale: TimeScale,
    origin: Digest32,
    reference: Digest32,
}

impl ClockSpec {
    pub const fn exact(
        clock: Digest32,
        scale: TimeScale,
        origin: Digest32,
        reference: Digest32,
    ) -> Self {
        Self {
            clock,
            scale,
            origin,
            reference,
        }
    }

    pub fn checked(
        clock: Digest32,
        scale: TimeScale,
        origin: Digest32,
        reference: Digest32,
        precision_nanoseconds: u64,
    ) -> Result<Self, TemporalError> {
        if precision_nanoseconds != 0 {
            return Err(TemporalError::UnsupportedPrecision);
        }
        Ok(Self::exact(clock, scale, origin, reference))
    }

    pub const fn clock(self) -> Digest32 {
        self.clock
    }
    pub const fn scale(self) -> TimeScale {
        self.scale
    }
    pub const fn origin(self) -> Digest32 {
        self.origin
    }
    pub const fn reference(self) -> Digest32 {
        self.reference
    }
    pub const fn precision_nanoseconds(self) -> u64 {
        0
    }

    pub(super) fn hash_into(self, hash: &mut Sha256) {
        hash.update(self.clock.as_bytes());
        hash.update([match self.scale {
            TimeScale::Monotonic => 0,
            TimeScale::Utc => 1,
            TimeScale::Tai => 2,
        }]);
        hash.update(self.origin.as_bytes());
        hash.update(self.reference.as_bytes());
        hash.update(0u64.to_le_bytes());
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Instant {
    clock: ClockSpec,
    nanoseconds: i128,
}

impl Instant {
    pub fn absolute(clock: ClockSpec, value: i64, unit: TimeUnit) -> Result<Self, TemporalError> {
        let nanoseconds = i128::from(value)
            .checked_mul(unit.nanoseconds())
            .ok_or(TemporalError::ArithmeticOverflow)?;
        Ok(Self { clock, nanoseconds })
    }

    pub fn relative(anchor: Self, offset: i64, unit: TimeUnit) -> Result<Self, TemporalError> {
        let delta = i128::from(offset)
            .checked_mul(unit.nanoseconds())
            .ok_or(TemporalError::ArithmeticOverflow)?;
        let nanoseconds = anchor
            .nanoseconds
            .checked_add(delta)
            .ok_or(TemporalError::ArithmeticOverflow)?;
        Ok(Self {
            clock: anchor.clock,
            nanoseconds,
        })
    }

    pub const fn clock(self) -> ClockSpec {
        self.clock
    }
    pub const fn nanoseconds(self) -> i128 {
        self.nanoseconds
    }

    pub(super) fn hash_into(self, hash: &mut Sha256) {
        self.clock.hash_into(hash);
        hash.update(self.nanoseconds.to_le_bytes());
    }
}

/// Nonempty half-open interval, distinct from an instantaneous event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Interval {
    start: Instant,
    end: Instant,
}

impl Interval {
    pub fn checked(start: Instant, end: Instant) -> Result<Self, TemporalError> {
        if start.clock != end.clock {
            return Err(TemporalError::ClockMismatch);
        }
        if start.nanoseconds >= end.nanoseconds {
            return Err(TemporalError::EmptyOrReversedInterval);
        }
        Ok(Self { start, end })
    }
    pub const fn start(self) -> Instant {
        self.start
    }
    pub const fn end(self) -> Instant {
        self.end
    }
    pub fn duration_nanoseconds(self) -> Result<i128, TemporalError> {
        self.end
            .nanoseconds
            .checked_sub(self.start.nanoseconds)
            .ok_or(TemporalError::ArithmeticOverflow)
    }
    pub fn before(self, other: Self) -> Result<bool, TemporalError> {
        self.same_clock(other)?;
        Ok(self.end.nanoseconds < other.start.nanoseconds)
    }
    pub fn meets(self, other: Self) -> Result<bool, TemporalError> {
        self.same_clock(other)?;
        Ok(self.end.nanoseconds == other.start.nanoseconds)
    }
    pub fn after(self, other: Self) -> Result<bool, TemporalError> {
        other.before(self)
    }
    fn same_clock(self, other: Self) -> Result<(), TemporalError> {
        if self.start.clock != other.start.clock {
            Err(TemporalError::ClockMismatch)
        } else {
            Ok(())
        }
    }
}
