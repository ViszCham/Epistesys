#![allow(dead_code)] // Shared fixture; each suite uses a different subset.
use lc631_core::measurement_numeric::{Rational, RationalInterval as Interval};
use lc631_core::{
    context::{ContextRevision, ContextScope, SourceRevision},
    CandidateId,
};
use lc631_runtime::joint_selection::*;
use lc631_runtime::memory_measurement::*;
use lc631_runtime::resource::{Reservation, ResourceBroker, ResourceLimits, RunResources};
use lc631_substrate::{
    resource::{Cancellation, Deadline, ResourceAmount, ResourceKind, ResourceRequest},
    Digest32,
};
use std::time::Duration;
pub fn d(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}
pub fn q(n: i64, den: i64) -> Rational {
    Rational::new(n.into(), den.into())
}
pub fn bounds(a: i64, b: i64, c: i64, e: i64) -> Interval {
    Interval::from_ratio(a, b, c, e).unwrap()
}
pub fn unit() -> CostUnit {
    CostUnit(d(7))
}
pub fn binding(id: u64) -> Binding {
    let source = SourceRevision::of("fixed memory question");
    Binding {
        selection: SelectionPins {
            source,
            scope: ContextScope::declared(d(1), d(2), d(3), d(4)),
            baseline: d(5),
            hard_invariants: d(6),
            policy: d(8),
            goal_profile: Some(d(9)),
            evidence_revision: d(10),
        },
        candidate: CandidateId(id),
        candidate_digest: d(20 + id as u8),
        context: ContextRevision::original(source),
        target: d(30 + id as u8),
        transform: d(40),
        definition: definition_revision(),
        probe_set: d(3),
        oracle: d(42),
        dependency_group: d(43),
    }
}
pub struct Fixture {
    pub states: Vec<State<'static>>,
    pub costs: Vec<Interval>,
    pub allowed: Vec<(u16, u16, usize)>,
}
impl Fixture {
    pub fn three(costs: Vec<Interval>, allowed: Vec<(u16, u16, usize)>) -> Self {
        Self {
            states: vec![
                State {
                    id: StateId(1),
                    canonical_payload: b"A",
                },
                State {
                    id: StateId(2),
                    canonical_payload: b"B",
                },
                State {
                    id: StateId(3),
                    canonical_payload: b"C",
                },
            ],
            costs,
            allowed,
        }
    }
    pub fn pairs(&self) -> Vec<Pair<'_>> {
        let mut pairs = Vec::new();
        for a in &self.states {
            for b in &self.states {
                if a.id == b.id {
                    continue;
                }
                let disposition = match self
                    .allowed
                    .iter()
                    .find(|(x, y, _)| StateId(*x) == a.id && StateId(*y) == b.id)
                {
                    Some((_, _, i)) => PairDisposition::Allowed {
                        unit: unit(),
                        lower: self.costs[*i].lower(),
                        upper: self.costs[*i].upper(),
                        canonical_payload: b"declared-nonsecurity-conversion",
                        evidence: d(50),
                        evidence_payload: b"Given conversion provenance v1",
                    },
                    None => PairDisposition::Excluded { basis: d(51) },
                };
                pairs.push(Pair {
                    from: a.id,
                    to: b.id,
                    disposition,
                });
            }
        }
        pairs
    }
}
pub fn budget() -> WorkBudget {
    WorkBudget {
        max_steps: 1_000_000,
    }
}
pub fn run(cancel: Cancellation, source: Digest32) -> (ResourceBroker, RunResources, Reservation) {
    let broker = ResourceBroker::new(ResourceLimits::uniform(256 * 1024 * 1024).unwrap()).unwrap();
    let run = broker
        .begin_run(
            1,
            source,
            ResourceLimits::uniform(128 * 1024 * 1024).unwrap(),
            Deadline::after(Duration::from_secs(120)).unwrap(),
            cancel,
        )
        .unwrap();
    let reservation = run
        .reserve(&[
            request(ResourceKind::Ram, 128 * 1024 * 1024),
            request(ResourceKind::Output, 8 * 1024 * 1024),
        ])
        .unwrap();
    (broker, run, reservation)
}
pub fn request(kind: ResourceKind, n: u64) -> ResourceRequest {
    ResourceRequest::new(kind, ResourceAmount::checked(n).unwrap())
}
