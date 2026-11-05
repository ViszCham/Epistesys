//! Strict bounded DTO. Scope/source digests are derived from exact raw config,
//! never supplied by the caller. JSON declarations are not authenticated truth.
use lc631_core::context::{ContextRevision, ContextScope, SourceRevision};
use lc631_runtime::checked_tl_reduction::input::*;
use lc631_runtime::resource::Digest32;
use lc631_runtime::{spatial_preservation::*, temporal_preservation::*};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Config {
    pub schema_version: String,
    pub clock_label: String,
    pub frame_label: String,
    pub unit: Unit,
    pub declare_identity_map: bool,
    pub spatial_probe: SpatialProbe,
    pub root: Tuple,
    pub target: Tuple,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Tuple {
    pub temporal: Trace,
    pub spatial: Objects,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Trace {
    pub times_ns: [i64; 4],
    pub values: [DeclaredTruth; 4],
}
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum DeclaredTruth {
    False,
    Unknown,
    True,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Objects {
    pub left: Option<Rect>,
    pub right: Option<Rect>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Rect {
    pub min: [i64; 2],
    pub max: [i64; 2],
}
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Unit {
    Pixel,
    Millimeter,
}
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum SpatialProbe {
    StrictLeft,
    ContinuousDistance,
}

pub(super) fn digest(text: &str) -> Digest32 {
    SourceRevision::of(text).digest()
}
pub(super) fn scope(raw: &str) -> ContextScope {
    // Exact config includes declarations AND targets. This is a one-tuple check,
    // not a claim that differing raw configs share a comparison world.
    let source = SourceRevision::of(raw);
    ContextScope::declared(
        source.digest(),
        digest("given-tuple/root-goal-unverified.v1"),
        digest("given-tuple/fixed-probes.v1"),
        digest(lc631_runtime::checked_tl_reduction::PROFILE),
    )
}
impl Config {
    pub fn check(&self) -> Result<(), String> {
        if self.schema_version != "epistesys-given-tl-tuple.v1" {
            return Err("tl_config:Schema".into());
        }
        for label in [&self.clock_label, &self.frame_label] {
            if label.is_empty() || label.len() > 128 || label.chars().any(char::is_control) {
                return Err("tl_config:Label".into());
            }
        }
        Ok(())
    }
    pub fn clock(&self) -> ClockSpec {
        ClockSpec::exact(
            digest(&self.clock_label),
            TimeScale::Monotonic,
            digest("given-tuple/declared-origin.v1"),
            digest("given-tuple/declared-reference.v1"),
        )
    }
    pub fn time_policy(
        &self,
        a: TemporalModelInput<'_>,
        b: TemporalModelInput<'_>,
    ) -> Result<TemporalPreservationPolicy, String> {
        TemporalPreservationPolicy::declared(
            a,
            b,
            vec![
                ProtectedTemporalProbe {
                    id: digest("given-tuple/atom0-truth.v1"),
                    weight: 1,
                    relation: ProtectedTemporalRelation::MappedPredicateTruth,
                },
                ProtectedTemporalProbe {
                    id: digest("given-tuple/interval-duration.v1"),
                    weight: 3,
                    relation: ProtectedTemporalRelation::Intervals {
                        relation: TemporalProbe::EqualDuration,
                        source: [0, 1, 2, 3],
                        target: [0, 1, 2, 3],
                    },
                },
            ],
            self.declare_identity_map
                .then_some(DeclaredTimeTranslation {
                    source_clock: self.clock(),
                    target_clock: self.clock(),
                    offset_nanoseconds: 0,
                }),
        )
        .map_err(|e| format!("tl_time_policy:{e:?}"))
    }
    pub fn space_policy(
        &self,
        a: SpatialModelInput,
        b: SpatialModelInput,
    ) -> Result<SpatialPreservationPolicy, String> {
        SpatialPreservationPolicy::declared(
            a,
            b,
            vec![ProtectedSpatialProbe {
                id: digest("given-tuple/spatial-probe.v1"),
                weight: 1,
                relation: match self.spatial_probe {
                    SpatialProbe::StrictLeft => {
                        ProtectedSpatialRelation::Kernel(SpatialRelation::StrictlyLeftOf)
                    }
                    SpatialProbe::ContinuousDistance => {
                        ProtectedSpatialRelation::ContinuousDistance
                    }
                },
            }],
            self.declare_identity_map.then_some(DeclaredSpatialMap {
                source_frame: a.frame,
                target_frame: b.frame,
                source_unit: a.unit,
                target_unit: b.unit,
                signs: (AxisSign::Positive, AxisSign::Positive),
                translation: (0, 0),
            }),
        )
        .map_err(|e| format!("tl_space_policy:{e:?}"))
    }
}
impl Trace {
    pub fn build(&self, clock: ClockSpec) -> Result<FiniteTrace, String> {
        let mut steps = Vec::new();
        steps
            .try_reserve_exact(4)
            .map_err(|_| "tl_trace:Allocation")?;
        for (time, value) in self.times_ns.iter().zip(&self.values) {
            let instant = Instant::absolute(clock, *time, TimeUnit::Nanosecond)
                .map_err(|e| format!("tl_trace:{e:?}"))?;
            let truth = match value {
                DeclaredTruth::False => Truth::False,
                DeclaredTruth::Unknown => Truth::Unknown,
                DeclaredTruth::True => Truth::True,
            };
            steps.push(TraceStep::new(instant, vec![truth]));
        }
        FiniteTrace::checked(steps, TraceClosure::Closed).map_err(|e| format!("tl_trace:{e:?}"))
    }
}
impl Objects {
    pub fn build(
        &self,
        config: &Config,
        context: ContextRevision,
        scope: ContextScope,
    ) -> Result<SpatialModelInput, String> {
        let frame = FrameId(*digest(&config.frame_label).as_bytes());
        let rect = |input: &Option<Rect>| -> Result<Option<Rectangle>, String> {
            input
                .as_ref()
                .map(|v| {
                    Rectangle::new(frame, v.min, v.max).map_err(|e| format!("tl_rectangle:{e:?}"))
                })
                .transpose()
        };
        Ok(SpatialModelInput {
            left: rect(&self.left)?,
            right: rect(&self.right)?,
            frame,
            unit: match config.unit {
                Unit::Pixel => SpatialUnit::Pixel,
                Unit::Millimeter => SpatialUnit::Millimeter,
            },
            context,
            scope,
        })
    }
}
