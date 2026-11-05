#![allow(dead_code)]
use lc631_core::{
    context::{ContextRevision, ContextScope, SourceRevision},
    measurement_numeric::Rational,
    CandidateId,
};
use lc631_runtime::{
    design_measurement::*,
    joint_selection::SelectionPins,
    resource::{Reservation, ResourceBroker, ResourceLimits, RunResources},
};
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
pub fn unit() -> TickUnit {
    TickUnit(d(40))
}
pub fn binding() -> DesignBinding {
    let source = SourceRevision::of("finite scheduling design");
    DesignBinding {
        selection: SelectionPins {
            source,
            scope: ContextScope::declared(d(1), d(2), d(3), d(4)),
            baseline: d(5),
            hard_invariants: d(6),
            policy: d(7),
            goal_profile: Some(d(8)),
            evidence_revision: d(9),
        },
        context: ContextRevision::original(source),
        suite: d(10),
        oracle: d(11),
        definition: definition_revision(),
        probe_set: d(3),
        dependency_group: d(12),
        role: DesignRole::DesignValidation,
    }
}
pub fn candidate(id: u64) -> CandidateBinding {
    CandidateBinding {
        id: CandidateId(id),
        candidate_digest: d(20 + id as u8),
        context: binding().context,
        target: d(30),
        transform: d(31),
    }
}
pub fn limits() -> DesignLimits {
    DesignLimits {
        max_events_per_run: 32,
        max_replay_ticks_per_run: 256,
    }
}
pub struct Fixture {
    pub jobs: Vec<Job<'static>>,
    pub dependencies: Vec<(JobId, JobId)>,
    pub baseline: Vec<JobId>,
    pub accepted_low: Rational,
    pub accepted_high: Rational,
    pub m: Rational,
    pub upper: Rational,
    pub weight: Rational,
}
impl Fixture {
    pub fn gold() -> Self {
        Self {
            jobs: vec![
                Job {
                    id: JobId(11),
                    canonical_payload: b"a-long",
                    duration_ticks: 3,
                },
                Job {
                    id: JobId(22),
                    canonical_payload: b"b-short",
                    duration_ticks: 2,
                },
                Job {
                    id: JobId(33),
                    canonical_payload: b"c-short",
                    duration_ticks: 2,
                },
            ],
            dependencies: vec![],
            baseline: vec![JobId(22), JobId(33), JobId(11)],
            accepted_low: q(0, 1),
            accepted_high: q(20, 1),
            m: q(3, 1),
            upper: q(6, 1),
            weight: q(1, 1),
        }
    }
    pub fn workload<'a>(&'a self, key: &'a [u8]) -> Workload<'a> {
        Workload {
            case: key,
            jobs: &self.jobs,
            dependencies: &self.dependencies,
            workers: 2,
            unit: unit(),
            baseline_priority: &self.baseline,
            accepted: RawRange {
                unit: unit(),
                lower: &self.accepted_low,
                upper: &self.accepted_high,
            },
            normalization: NormalizationMap {
                unit: unit(),
                lower: &self.m,
                upper: &self.upper,
                revision: d(41),
                direction: Direction::Minimize,
            },
            weight: &self.weight,
        }
    }
}
pub fn improved() -> [JobId; 3] {
    [JobId(11), JobId(22), JobId(33)]
}
pub fn run(cancel: Cancellation) -> (ResourceBroker, RunResources, Reservation) {
    run_binding(binding(), cancel)
}
pub fn run_binding(
    b: DesignBinding,
    cancel: Cancellation,
) -> (ResourceBroker, RunResources, Reservation) {
    run_with_output(b, cancel, 16 * 1024 * 1024)
}
pub fn run_with_output(
    b: DesignBinding,
    cancel: Cancellation,
    output: u64,
) -> (ResourceBroker, RunResources, Reservation) {
    let br = ResourceBroker::new(ResourceLimits::uniform(256 * 1024 * 1024).unwrap()).unwrap();
    let run = br
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
            req(ResourceKind::Ram, 128 * 1024 * 1024),
            req(ResourceKind::Output, output),
        ])
        .unwrap();
    (br, run, res)
}
pub fn req(k: ResourceKind, n: u64) -> ResourceRequest {
    ResourceRequest::new(k, ResourceAmount::checked(n).unwrap())
}
