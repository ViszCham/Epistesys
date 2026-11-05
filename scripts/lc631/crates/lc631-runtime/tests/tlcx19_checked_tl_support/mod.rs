use lc631_core::context::{ContextRevision, ContextScope, SourceRevision};
use lc631_runtime::{resource::*, spatial_preservation::*, temporal_preservation::*};
use lc631_solvers::{
    causal_spatial::{FrameId, Rectangle, SpatialRelation},
    temporal::*,
};
pub use lc631_substrate::resource::Cancellation;
use lc631_substrate::Digest32;
pub fn d(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}
pub fn context() -> ContextRevision {
    ContextRevision::original(SourceRevision::of("checked Given tuple"))
}
pub fn scope() -> ContextScope {
    ContextScope::declared(d(1), d(2), d(3), d(4))
}
pub fn run(cancel: Cancellation) -> (ResourceBroker, RunResources, Reservation) {
    let cap = ResourceLimits::uniform(32 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(cap).unwrap();
    let run = broker
        .begin_run(
            19,
            context().source().digest(),
            cap,
            Deadline::after(std::time::Duration::from_secs(120)).unwrap(),
            cancel,
        )
        .unwrap();
    let res = run
        .reserve(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(24 * 1024 * 1024).unwrap(),
        )])
        .unwrap();
    (broker, run, res)
}
pub fn clock() -> ClockSpec {
    ClockSpec::exact(d(40), TimeScale::Monotonic, d(41), d(42))
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
pub fn trace(truth: Truth) -> FiniteTrace {
    FiniteTrace::checked(
        (0..4)
            .map(|t| {
                TraceStep::new(
                    Instant::absolute(clock(), t, TimeUnit::Second).unwrap(),
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
pub fn time_policy(
    a: TemporalModelInput<'_>,
    b: TemporalModelInput<'_>,
) -> TemporalPreservationPolicy {
    TemporalPreservationPolicy::declared(
        a,
        b,
        vec![ProtectedTemporalProbe {
            id: d(50),
            weight: 1,
            relation: ProtectedTemporalRelation::MappedPredicateTruth,
        }],
        Some(DeclaredTimeTranslation {
            source_clock: clock(),
            target_clock: clock(),
            offset_nanoseconds: 0,
        }),
    )
    .unwrap()
}
pub fn spatial(broken: bool) -> SpatialModelInput {
    let frame = FrameId([60; 32]);
    let a = Rectangle::new(frame, [0, 0], [1, 1]).unwrap();
    let b = Rectangle::new(frame, [3, 0], [4, 1]).unwrap();
    SpatialModelInput {
        left: Some(if broken { b } else { a }),
        right: Some(if broken { a } else { b }),
        frame,
        unit: SpatialUnit::Pixel,
        context: context(),
        scope: scope(),
    }
}
pub fn space_policy(
    a: SpatialModelInput,
    b: SpatialModelInput,
    unsupported: bool,
) -> SpatialPreservationPolicy {
    SpatialPreservationPolicy::declared(
        a,
        b,
        vec![ProtectedSpatialProbe {
            id: d(70),
            weight: 1,
            relation: if unsupported {
                ProtectedSpatialRelation::ContinuousDistance
            } else {
                ProtectedSpatialRelation::Kernel(SpatialRelation::StrictlyLeftOf)
            },
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
pub fn bounds(a: i64, b: i64, c: i64, d: i64) -> lc631_core::measurement_numeric::RationalInterval {
    lc631_core::measurement_numeric::RationalInterval::from_ratio(a, b, c, d).unwrap()
}
