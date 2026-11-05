//! 日本語: TLCX-19の有限関係比較の準備型。数値TL・証拠検証・ledger接続は未実装。
//! English: Preparatory finite-relation comparison, not measured TL or validated evidence.
#![forbid(unsafe_code)]

use lc631_substrate::Digest32;

pub const DEFINITION_REVISION: &str = "OperationalMeasurement/finite-formal-v1.4";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    Forward,
    Reverse,
}

/// Caller-declared bindings; digests do not authenticate observations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Binding {
    pub scope: Digest32,
    pub probe_set: Digest32,
    pub policy: Digest32,
    pub source_revision: Digest32,
    pub target_revision: Digest32,
    pub direction: Direction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gap {
    MissingObservation,
    MissingCalibration,
    MissingEvidence,
    UnsupportedQuantity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IncomparableReason {
    BindingMismatch,
    ClockMismatch,
    FrameMismatch,
    UnitMismatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelationComparison {
    MatchesDeclaredRelation,
    DiffersFromDeclaredRelation,
    NeedsEvidence(Gap),
    Incomparable(IncomparableReason),
}

/// Comparison is arithmetic on declared inputs. Validation is always pending.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelationAssessment {
    comparison: RelationComparison,
    binding: Binding,
}
impl RelationAssessment {
    pub fn comparison(self) -> RelationComparison {
        self.comparison
    }
    pub fn binding(self) -> Binding {
        self.binding
    }
    pub fn validation_gap(self) -> Gap {
        Gap::MissingEvidence
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputError {
    ClockMismatch,
    EmptyOrReversedInterval,
    ArithmeticOverflow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimeUnit {
    Nanosecond,
    Millisecond,
    Second,
}

impl TimeUnit {
    fn multiplier(self) -> i128 {
        match self {
            Self::Nanosecond => 1,
            Self::Millisecond => 1_000_000,
            Self::Second => 1_000_000_000,
        }
    }
}

/// The digest must bind clock/time-scale/origin/reference/precision externally.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Instant {
    clock: Digest32,
    nanoseconds: i128,
}

impl Instant {
    pub fn declared(clock: Digest32, value: i64, unit: TimeUnit) -> Self {
        Self {
            clock,
            nanoseconds: i128::from(value) * unit.multiplier(),
        }
    }
    pub fn nanoseconds(self) -> i128 {
        self.nanoseconds
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimeInterval {
    start: Instant,
    end: Instant,
}

impl TimeInterval {
    pub fn checked(start: Instant, end: Instant) -> Result<Self, InputError> {
        if start.clock != end.clock {
            return Err(InputError::ClockMismatch);
        }
        if start.nanoseconds >= end.nanoseconds {
            return Err(InputError::EmptyOrReversedInterval);
        }
        Ok(Self { start, end })
    }
    pub fn duration_nanoseconds(self) -> i128 {
        // Both endpoints originate from i64 times at most 10^9, so this fits.
        self.end.nanoseconds - self.start.nanoseconds
    }
}

/// Bounded translation-only declared calibration; not a validated clock receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimeTranslation {
    binding: Binding,
    source_clock: Digest32,
    target_clock: Digest32,
    offset_nanoseconds: i128,
}

impl TimeTranslation {
    pub fn declared(
        binding: Binding,
        source_clock: Digest32,
        target_clock: Digest32,
        offset: i64,
        unit: TimeUnit,
    ) -> Self {
        Self {
            binding,
            source_clock,
            target_clock,
            offset_nanoseconds: i128::from(offset) * unit.multiplier(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TemporalProbe {
    Before,
    Meets,
    EqualDuration,
    MappedAbsoluteStart,
}

pub fn compare_temporal(
    binding: Binding,
    probe: TemporalProbe,
    source: Option<(TimeInterval, TimeInterval)>,
    target: Option<(TimeInterval, TimeInterval)>,
    mapping: Option<TimeTranslation>,
) -> Result<RelationAssessment, InputError> {
    let comparison = match (source, target, mapping) {
        (Some(source), Some(target), Some(map)) => {
            if map.binding != binding {
                RelationComparison::Incomparable(IncomparableReason::BindingMismatch)
            } else if source.0.start.clock != map.source_clock
                || source.1.start.clock != map.source_clock
                || target.0.start.clock != map.target_clock
                || target.1.start.clock != map.target_clock
            {
                RelationComparison::Incomparable(IncomparableReason::ClockMismatch)
            } else {
                temporal_relation(probe, source, target, map.offset_nanoseconds)?
            }
        }
        (None, _, _) | (_, None, _) => RelationComparison::NeedsEvidence(Gap::MissingObservation),
        (Some(_), Some(_), None) => RelationComparison::NeedsEvidence(Gap::MissingCalibration),
    };
    Ok(RelationAssessment {
        comparison,
        binding,
    })
}

fn temporal_relation(
    probe: TemporalProbe,
    source: (TimeInterval, TimeInterval),
    target: (TimeInterval, TimeInterval),
    offset: i128,
) -> Result<RelationComparison, InputError> {
    let matches = match probe {
        TemporalProbe::Before => {
            (source.0.end.nanoseconds < source.1.start.nanoseconds)
                == (target.0.end.nanoseconds < target.1.start.nanoseconds)
        }
        TemporalProbe::Meets => {
            (source.0.end.nanoseconds == source.1.start.nanoseconds)
                == (target.0.end.nanoseconds == target.1.start.nanoseconds)
        }
        TemporalProbe::EqualDuration => {
            source.0.duration_nanoseconds() == target.0.duration_nanoseconds()
                && source.1.duration_nanoseconds() == target.1.duration_nanoseconds()
        }
        TemporalProbe::MappedAbsoluteStart => {
            let first = source
                .0
                .start
                .nanoseconds
                .checked_add(offset)
                .ok_or(InputError::ArithmeticOverflow)?;
            let second = source
                .1
                .start
                .nanoseconds
                .checked_add(offset)
                .ok_or(InputError::ArithmeticOverflow)?;
            first == target.0.start.nanoseconds && second == target.1.start.nanoseconds
        }
    };
    Ok(if matches {
        RelationComparison::MatchesDeclaredRelation
    } else {
        RelationComparison::DiffersFromDeclaredRelation
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpatialUnit {
    Pixel,
    Millimeter,
}

/// Exact grid profile only; frame digest binds axes/origin/handedness externally.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Frame2 {
    identity: Digest32,
    unit: SpatialUnit,
}
impl Frame2 {
    pub fn declared(identity: Digest32, unit: SpatialUnit) -> Self {
        Self { identity, unit }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Point2 {
    frame: Frame2,
    x: i64,
    y: i64,
}
impl Point2 {
    pub fn declared(frame: Frame2, x: i64, y: i64) -> Self {
        Self { frame, x, y }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AxisSign {
    Positive,
    Negative,
}
impl AxisSign {
    fn apply(self, value: i64) -> i128 {
        match self {
            Self::Positive => i128::from(value),
            Self::Negative => -i128::from(value),
        }
    }
}

/// Translation/reflection only, in one unit. No pixel/world or 2D/3D inference.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GridMap2 {
    binding: Binding,
    source: Frame2,
    target: Frame2,
    x_sign: AxisSign,
    y_sign: AxisSign,
    dx: i64,
    dy: i64,
}
impl GridMap2 {
    pub fn declared(
        binding: Binding,
        source: Frame2,
        target: Frame2,
        signs: (AxisSign, AxisSign),
        translation: (i64, i64),
    ) -> Self {
        Self {
            binding,
            source,
            target,
            x_sign: signs.0,
            y_sign: signs.1,
            dx: translation.0,
            dy: translation.1,
        }
    }
    fn map(self, point: Point2) -> (i128, i128) {
        (
            self.x_sign.apply(point.x) + i128::from(self.dx),
            self.y_sign.apply(point.y) + i128::from(self.dy),
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpatialProbe {
    XOrder,
    YOrder,
    Coincidence,
    MappedPlacement,
}

pub fn compare_spatial(
    binding: Binding,
    probe: SpatialProbe,
    source: Option<(Point2, Point2)>,
    target: Option<(Point2, Point2)>,
    mapping: Option<GridMap2>,
) -> RelationAssessment {
    let comparison = match (source, target, mapping) {
        (Some(source), Some(target), Some(map)) => {
            if map.binding != binding {
                RelationComparison::Incomparable(IncomparableReason::BindingMismatch)
            } else if map.source.unit != map.target.unit {
                RelationComparison::Incomparable(IncomparableReason::UnitMismatch)
            } else if source.0.frame != map.source
                || source.1.frame != map.source
                || target.0.frame != map.target
                || target.1.frame != map.target
            {
                RelationComparison::Incomparable(IncomparableReason::FrameMismatch)
            } else {
                spatial_relation(probe, source, target, map)
            }
        }
        (None, _, _) | (_, None, _) => RelationComparison::NeedsEvidence(Gap::MissingObservation),
        (Some(_), Some(_), None) => RelationComparison::NeedsEvidence(Gap::MissingCalibration),
    };
    RelationAssessment {
        comparison,
        binding,
    }
}

fn spatial_relation(
    probe: SpatialProbe,
    source: (Point2, Point2),
    target: (Point2, Point2),
    map: GridMap2,
) -> RelationComparison {
    let (a, b) = (map.map(source.0), map.map(source.1));
    let (c, d) = (
        (i128::from(target.0.x), i128::from(target.0.y)),
        (i128::from(target.1.x), i128::from(target.1.y)),
    );
    let matches = match probe {
        SpatialProbe::XOrder => a.0.cmp(&b.0) == c.0.cmp(&d.0),
        SpatialProbe::YOrder => a.1.cmp(&b.1) == c.1.cmp(&d.1),
        SpatialProbe::Coincidence => (a == b) == (c == d),
        SpatialProbe::MappedPlacement => a == c && b == d,
    };
    if matches {
        RelationComparison::MatchesDeclaredRelation
    } else {
        RelationComparison::DiffersFromDeclaredRelation
    }
}

/// Quantitative epsilon/r, interval uncertainty and K1 aggregation are unconnected.
pub fn quantitative_gap() -> Gap {
    Gap::UnsupportedQuantity
}
