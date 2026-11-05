//! Shared Given K6 fixtures; no injected measurement, pass/fail, goal or TL values.
use lc631_analysis::creativity_measurement::Rational;
use lc631_core::context::{ContextRevision, SourceRevision};
use lc631_runtime::{facet_ledger::*, reconstruction_measurement::*, resource::*};
use std::time::Duration;

// Independent literal canonical K6 Symbol terms: 6, 4 and 8 bytes.
pub const BASE: &[u8] = &[3, 4, b'a', b'b', b'c', b'd'];
pub const SMALL: &[u8] = &[3, 2, b'a', b'b'];
pub const LARGE: &[u8] = &[3, 6, b'a', b'b', b'c', b'd', b'e', b'f'];
pub fn scope() -> ComparisonScope {
    ComparisonScope {
        id: ScopeId([1; 32]),
        baseline: Revision([2; 32]),
        root_goal: Revision([3; 32]),
        probes: Revision([4; 32]),
        policy: Revision([5; 32]),
    }
}
pub fn probes() -> [ProbeSpec; 1] {
    [ProbeSpec {
        path: vec![],
        kind: FeatureKind::Symbol,
        weight: Rational::from_integer(1.into()),
    }]
}
pub fn fixture(body: impl FnOnce(ContextRevision, &ResourceReservationView<'_>, &Cancellation)) {
    let source = SourceRevision::of("Given finite K6 description-length fixture");
    let cap = ResourceLimits::uniform(96 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(cap).unwrap();
    let cancel = Cancellation::default();
    let run = broker
        .begin_run(
            22,
            source.digest(),
            cap,
            Deadline::after(Duration::from_secs(60)).unwrap(),
            cancel.clone(),
        )
        .unwrap();
    let reservation = run
        .reserve(&[
            ResourceRequest::new(
                ResourceKind::Ram,
                ResourceAmount::checked(80 * 1024 * 1024).unwrap(),
            ),
            ResourceRequest::new(
                ResourceKind::Output,
                ResourceAmount::checked(24 * 1024 * 1024).unwrap(),
            ),
        ])
        .unwrap();
    let view = reservation.lease();
    body(ContextRevision::original(source), &view, &cancel);
}
