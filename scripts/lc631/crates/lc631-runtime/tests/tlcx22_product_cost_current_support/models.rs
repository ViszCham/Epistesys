//! Actual temporal/spatial inputs, distinct from product-cost programs or reports.
use super::wire::{app, encode, integer, symbol};
use lc631_core::context::{ContextRevision, ContextScope, SourceRevision};
use lc631_runtime::{spatial_preservation::*, temporal_preservation::*};
use lc631_solvers::{
    causal_spatial::{FrameId, Rectangle, SpatialRelation},
    temporal::*,
};
use lc631_substrate::Digest32;

pub fn pin(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}
pub fn context() -> ContextRevision {
    ContextRevision::original(SourceRevision::of("Product cost actual finite fixtures"))
}
pub fn scope() -> ContextScope {
    ContextScope::declared(pin(1), pin(2), pin(3), pin(4))
}
pub fn identity(renamed: bool, n: u8) -> [u8; 32] {
    [if renamed { n + 100 } else { n }; 32]
}
pub fn clock(renamed: bool, secondary: bool) -> ClockSpec {
    ClockSpec::exact(
        Digest32::from_bytes(identity(renamed, if secondary { 44 } else { 40 })),
        TimeScale::Monotonic,
        Digest32::from_bytes(identity(renamed, 41)),
        Digest32::from_bytes(identity(renamed, 42)),
    )
}
pub fn query() -> TemporalQuery {
    TemporalQuery::checked(
        context(),
        vec![TemporalNode::Atom(0)],
        0,
        TemporalProfile::ClosedFiniteKleeneV1,
    )
    .unwrap()
}
pub fn trace(renamed: bool, secondary: bool, truth: Truth) -> FiniteTrace {
    FiniteTrace::checked(
        (0..2)
            .map(|t| {
                TraceStep::new(
                    Instant::absolute(clock(renamed, secondary), t, TimeUnit::Second).unwrap(),
                    vec![truth],
                )
            })
            .collect(),
        TraceClosure::Closed,
    )
    .unwrap()
}
pub fn temporal<'a>(query: &'a TemporalQuery, trace: &'a FiniteTrace) -> TemporalModelInput<'a> {
    TemporalModelInput {
        query,
        trace,
        scope: scope(),
    }
}
pub fn spatial(renamed: bool, swapped: bool) -> SpatialModelInput {
    let frame = FrameId(identity(renamed, 60));
    let a = Rectangle::new(frame, [0, 0], [2, 2]).unwrap();
    let b = Rectangle::new(frame, [4, 0], [6, 2]).unwrap();
    SpatialModelInput {
        left: Some(if swapped { b } else { a }),
        right: Some(if swapped { a } else { b }),
        frame,
        unit: SpatialUnit::Pixel,
        context: context(),
        scope: scope(),
    }
}
pub fn time_policy(
    a: TemporalModelInput<'_>,
    b: TemporalModelInput<'_>,
) -> TemporalPreservationPolicy {
    TemporalPreservationPolicy::declared(
        a,
        b,
        vec![ProtectedTemporalProbe {
            id: pin(50),
            weight: 1,
            relation: ProtectedTemporalRelation::MappedPredicateTruth,
        }],
        Some(DeclaredTimeTranslation {
            source_clock: a.trace.steps()[0].instant().clock(),
            target_clock: b.trace.steps()[0].instant().clock(),
            offset_nanoseconds: 0,
        }),
    )
    .unwrap()
}
pub fn space_policy(a: SpatialModelInput, b: SpatialModelInput) -> SpatialPreservationPolicy {
    SpatialPreservationPolicy::declared(
        a,
        b,
        vec![ProtectedSpatialProbe {
            id: pin(70),
            weight: 1,
            relation: ProtectedSpatialRelation::Kernel(SpatialRelation::StrictlyLeftOf),
        }],
        Some(DeclaredSpatialMap {
            source_frame: a.frame,
            target_frame: b.frame,
            source_unit: a.unit,
            target_unit: b.unit,
            signs: (AxisSign::Positive, AxisSign::Positive),
            translation: (0, 0),
        }),
    )
    .unwrap()
}
pub fn meaning(path: &str, payload: lc631_substrate::semantic_term::Term) -> Vec<u8> {
    encode(&app("identity-meaning-v1", vec![symbol(path), payload]))
}
pub fn clock_meaning(secondary: bool) -> Vec<u8> {
    meaning(
        if secondary {
            "Clock/secondary"
        } else {
            "Clock/primary"
        },
        app(
            "clock-meaning-v1",
            vec![symbol("Monotonic"), integer(0), symbol("logical-clock")],
        ),
    )
}
pub fn frame_meaning() -> Vec<u8> {
    meaning(
        "Frame/primary",
        app(
            "frame-meaning-v1",
            vec![
                symbol("Pixel"),
                symbol("x-right"),
                symbol("y-up"),
                symbol("right-handed"),
                integer(0),
                integer(0),
            ],
        ),
    )
}
