//! Conditional-declaration fixtures only; none are actual metric owners.
use lc631_analysis::creativity_measurement::ComponentMetric as M;
use lc631_runtime::{composite_original::*, resource::*};
use lc631_substrate::{
    source::{SourceSpan, SourceStore},
    Digest32,
};
use std::{sync::Arc, time::Duration};
pub const QUESTION: &str = "Finite conditional demonstrator: preserve root duties; compare fixed checked-u32-add-four quality.";
pub const F1: &[M] = &[
    M::DescriptionBits,
    M::ContextDistance,
    M::SemanticConfigurationDistance,
    M::Compatibility,
    M::Knownness,
];
pub const F2: &[M] = &[
    M::DescriptionBits,
    M::FrameworkDistance,
    M::SemanticConfigurationDistance,
    M::Compatibility,
    M::Coverage,
    M::Compression,
    M::Bridge,
    M::Prediction,
    M::Redundancy,
    M::Preserve,
    M::Rebind,
    M::Reframe,
    M::BreakCost,
    M::Explain,
    M::Design,
    M::Transfer,
    M::Solve,
    M::CounterexampleRisk,
    M::BridgeCost,
];
pub const COORDINATES: [Coordinate; 12] = [
    Coordinate::Hypothesis,
    Coordinate::Product,
    Coordinate::BoolPort,
    Coordinate::Graph,
    Coordinate::Units,
    Coordinate::RolesFrames,
    Coordinate::Certificates,
    Coordinate::Forecasts,
    Coordinate::Schedule,
    Coordinate::Transport,
    Coordinate::Coding,
    Coordinate::Conversion,
];
pub fn d(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}
pub fn source() -> SourceStore {
    SourceStore::new(Arc::from(QUESTION), 4096).unwrap()
}
pub fn declarations() -> Vec<CoordinateDeclaration> {
    COORDINATES
        .into_iter()
        .map(|coordinate| CoordinateDeclaration {
            coordinate,
            schema: d(10),
        })
        .collect()
}
pub fn coordinates() -> Vec<GivenCoordinate<'static>> {
    COORDINATES
        .into_iter()
        .map(|coordinate| GivenCoordinate {
            coordinate,
            schema: d(10),
            payload: b"Given conditional declaration; no native-owner correspondence",
        })
        .collect()
}
pub fn applicability() -> Vec<LeafApplicability<'static>> {
    let mut rows: Vec<_> = F2
        .iter()
        .copied()
        .map(|metric| LeafApplicability {
            metric,
            coordinate: coordinate(metric),
            root_goal: d(2),
            regime: d(11),
            claim: ApplicabilityClaim::GivenConditional(
                b"Preregistered package member only; semantic root relation unverified",
            ),
        })
        .collect();
    for metric in [M::ContextDistance, M::Knownness] {
        rows.push(LeafApplicability {
            metric,
            coordinate: coordinate(metric),
            root_goal: d(2),
            regime: d(11),
            claim: ApplicabilityClaim::GivenConditional(
                b"Preregistered conditional scope; not source validated",
            ),
        });
    }
    rows
}
fn coordinate(metric: M) -> Coordinate {
    match metric {
        M::DescriptionBits | M::Compression | M::Knownness => Coordinate::WholeComposite,
        M::FrameworkDistance | M::SemanticConfigurationDistance | M::ContextDistance => {
            Coordinate::Hypothesis
        }
        M::Compatibility | M::Bridge => Coordinate::BoolPort,
        M::Coverage | M::Solve => Coordinate::Coding,
        M::Prediction => Coordinate::Forecasts,
        M::Redundancy => Coordinate::Units,
        M::Preserve => Coordinate::Graph,
        M::Rebind | M::Reframe => Coordinate::RolesFrames,
        M::Explain | M::Transfer => Coordinate::Certificates,
        M::Design => Coordinate::Schedule,
        M::BreakCost => Coordinate::Transport,
        M::CounterexampleRisk => Coordinate::Product,
        M::BridgeCost => Coordinate::Conversion,
        _ => panic!("fixture uses only the strict original leaf union"),
    }
}
pub fn spec<'a>(
    source: Option<&'a SourceStore>,
    decl: &'a [CoordinateDeclaration],
    apps: &'a [LeafApplicability<'a>],
) -> RootSpec<'a> {
    RootSpec {
        source,
        question: source.map(|s| SourceSpan::checked(s, 0, s.text().len()).unwrap()),
        comparison: d(1),
        root_goal: d(2),
        baseline: d(3),
        baseline_profile: CodingBaselineProfile::AlwaysNoneFourV1,
        full_cost_profile: d(4),
        total_tl_policy: d(5),
        coordinates: decl,
        applicability: apps,
    }
}
pub fn run(
    source: &SourceStore,
    cancel: Cancellation,
) -> (ResourceBroker, RunResources, Reservation) {
    let caps = ResourceLimits::uniform(256 * 1024).unwrap();
    let broker = ResourceBroker::new(caps).unwrap();
    let run = broker
        .begin_run(
            2205,
            source.id().digest(),
            caps,
            Deadline::after(Duration::from_secs(120)).unwrap(),
            cancel,
        )
        .unwrap();
    let reservation = run
        .reserve(&[
            ResourceRequest::new(
                ResourceKind::Ram,
                ResourceAmount::checked(128 * 1024).unwrap(),
            ),
            ResourceRequest::new(
                ResourceKind::Output,
                ResourceAmount::checked(16 * 1024).unwrap(),
            ),
        ])
        .unwrap();
    (broker, run, reservation)
}
