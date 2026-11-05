#![allow(dead_code)]
use lc631_core::measurement_numeric::Rational;
use lc631_core::{
    context::{ContextRevision, ContextScope, SourceRevision},
    CandidateId,
};
use lc631_runtime::joint_selection::SelectionPins;
use lc631_runtime::prediction_measurement::*;
use lc631_runtime::resource::{Reservation, ResourceBroker, ResourceLimits, RunResources};
use lc631_solvers::temporal::*;
use lc631_substrate::{
    resource::{Cancellation, Deadline, ResourceAmount, ResourceKind, ResourceRequest},
    Digest32,
};
use std::time::Duration;
pub fn d(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}
pub fn q(n: i64, d: i64) -> Rational {
    Rational::new(n.into(), d.into())
}
pub fn binding(id: u64, role: SplitRole) -> ForecastBinding {
    let source = SourceRevision::of("prediction finite model");
    ForecastBinding {
        selection: SelectionPins {
            source,
            scope: ContextScope::declared(d(1), d(2), d(3), d(4)),
            baseline: d(5),
            hard_invariants: d(6),
            policy: d(7),
            goal_profile: Some(d(8)),
            evidence_revision: d(9),
        },
        candidate: CandidateId(id),
        candidate_digest: d(20 + id as u8),
        context: ContextRevision::original(source),
        target: d(30),
        transform: d(31),
        definition: definition_revision(),
        probe_set: d(3),
        oracle: d(32),
        dependency_group: d(33),
        baseline_model: d(34),
        candidate_model: d(35),
        split_digest: d(36),
        role,
    }
}
pub fn query(b: ForecastBinding) -> TemporalQuery {
    TemporalQuery::checked(
        b.context,
        vec![TemporalNode::Atom(0)],
        0,
        TemporalProfile::ClosedFiniteKleeneV1,
    )
    .unwrap()
}
pub fn trace(t: Truth) -> FiniteTrace {
    let clock = ClockSpec::exact(d(40), TimeScale::Monotonic, d(41), d(42));
    FiniteTrace::checked(
        vec![TraceStep::new(
            Instant::absolute(clock, 0, TimeUnit::Second).unwrap(),
            vec![t],
        )],
        TraceClosure::Closed,
    )
    .unwrap()
}
pub fn run(
    cancel: Cancellation,
    b: ForecastBinding,
) -> (ResourceBroker, RunResources, Reservation) {
    let cap = ResourceLimits::uniform(256 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(cap).unwrap();
    let run = broker
        .begin_run(
            1,
            b.selection.source.digest(),
            ResourceLimits::uniform(128 * 1024 * 1024).unwrap(),
            Deadline::after(Duration::from_secs(120)).unwrap(),
            cancel,
        )
        .unwrap();
    let res = run
        .reserve(&[
            request(ResourceKind::Ram, 128 * 1024 * 1024),
            request(ResourceKind::Output, 16 * 1024 * 1024),
        ])
        .unwrap();
    (broker, run, res)
}
pub fn request(k: ResourceKind, n: u64) -> ResourceRequest {
    ResourceRequest::new(k, ResourceAmount::checked(n).unwrap())
}
