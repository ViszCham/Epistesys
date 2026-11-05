//! 日本語: CR-ACOMPの固定Given入力。成功値や観測reportは供給しない。
//! English: Fixed Given inputs for CR-ACOMP, never supplied success values/reports.
use lc631_core::context::{ContextRevision, ContextScope, SourceRevision};
use lc631_runtime::compatibility_measurement::{
    Assignment, ContractPins, Limits, PlanInput, PortMap, ProbeCase,
};
use lc631_runtime::resource::{ResourceBroker, ResourceLimits};
use lc631_solvers::typed_query::{BoolExpr, BoolSymbolId, SmtEmitLimits, SmtStatusQuery};
use lc631_substrate::{
    resource::{Cancellation, Deadline, ResourceAmount, ResourceKind, ResourceRequest,
        ResourceReservationView},
    source::SourceStore,
    Digest32,
};
use std::{sync::Arc, time::Duration};

pub const R: BoolSymbolId = BoolSymbolId::new(0);
pub const P: BoolSymbolId = BoolSymbolId::new(1);
pub const Q: BoolSymbolId = BoolSymbolId::new(2);
pub const RP: BoolSymbolId = BoolSymbolId::new(10);
pub const U: BoolSymbolId = BoolSymbolId::new(11);
pub const V: BoolSymbolId = BoolSymbolId::new(12);

pub fn d(n: u8) -> Digest32 { Digest32::from_bytes([n; 32]) }
pub fn limits(max_work: u64) -> Limits {
    Limits { max_work, max_frame_bytes: 1024, max_program_steps: 3 }
}
pub fn map() -> [PortMap; 3] {
    [PortMap { source: R, target: RP }, PortMap { source: P, target: U },
        PortMap { source: Q, target: V }]
}
pub fn inverse() -> [PortMap; 3] {
    [PortMap { source: RP, target: R }, PortMap { source: U, target: P },
        PortMap { source: V, target: Q }]
}
pub fn swapped() -> [PortMap; 3] {
    [PortMap { source: R, target: RP }, PortMap { source: P, target: V },
        PortMap { source: Q, target: U }]
}
pub fn swapped_inverse() -> [PortMap; 3] {
    [PortMap { source: RP, target: R }, PortMap { source: U, target: Q },
        PortMap { source: V, target: P }]
}
pub fn hard_broken() -> [PortMap; 3] {
    [PortMap { source: R, target: U }, PortMap { source: P, target: RP },
        PortMap { source: Q, target: V }]
}
pub fn hard_broken_inverse() -> [PortMap; 3] {
    [PortMap { source: RP, target: P }, PortMap { source: U, target: R },
        PortMap { source: V, target: Q }]
}
fn query(symbols: &[BoolSymbolId], assertion: BoolSymbolId) -> SmtStatusQuery {
    SmtStatusQuery::checked(symbols.to_vec(), vec![],
        vec![BoolExpr::Variable(assertion)], SmtEmitLimits::default()).unwrap()
}

pub struct Fixture {
    pub source: SourceStore,
    pub target: SourceStore,
    pub source_theory: SmtStatusQuery,
    pub target_theory: SmtStatusQuery,
    pub source_use: SmtStatusQuery,
    pub target_use: SmtStatusQuery,
    pub expected: [PortMap; 3],
    pub values: [[Assignment; 3]; 4],
}
impl Fixture {
    pub fn new() -> Self {
        Self {
            source: SourceStore::new(Arc::from("Given Gamma={r}; input p,q"), 128).unwrap(),
            target: SourceStore::new(Arc::from("Given Gamma'={r'}; interface u,v"), 128).unwrap(),
            source_theory: query(&[R, P, Q], R),
            target_theory: query(&[RP, U, V], RP),
            source_use: query(&[R, P, Q], P),
            target_use: query(&[RP, U, V], U),
            expected: map(),
            // Canonical semantic frame order: (p,q)=00,01,10,11.
            values: [[false, false], [false, true], [true, false], [true, true]].map(|pq| [
                Assignment { symbol: R, value: true },
                Assignment { symbol: P, value: pq[0] },
                Assignment { symbol: Q, value: pq[1] },
            ]),
        }
    }
    pub fn cases(&self) -> [ProbeCase<'_>; 4] {
        self.values.each_ref().map(|input| ProbeCase { input, weight: 1 })
    }
    pub fn input<'a>(&'a self, cases: &'a [ProbeCase<'a>]) -> PlanInput<'a> {
        PlanInput {
            source: &self.source, target: &self.target,
            source_context: ContextRevision::original(SourceRevision::of(self.source.text())),
            target_context: ContextRevision::original(SourceRevision::of(self.target.text())),
            scope: ContextScope::declared(d(1), d(2), d(3), d(4)),
            root_goal: &self.source_theory,
            source_theory: &self.source_theory, target_theory: &self.target_theory,
            source_use: &self.source_use, target_use: &self.target_use,
            source_hard: R, target_hard: RP, expected_mapping: &self.expected, cases,
            pins: ContractPins { definition: d(5), oracle: d(6), policy: d(7), baseline: d(8) },
        }
    }
}
pub fn resource(kind: ResourceKind, amount: u64) -> ResourceRequest {
    ResourceRequest::new(kind, ResourceAmount::checked(amount).unwrap())
}

pub fn fixture<T>(output: u64, body: impl FnOnce(
    &Fixture, &[ProbeCase<'_>], &ResourceReservationView<'_>, &Cancellation,
) -> T) -> T {
    let fx = Fixture::new();
    let cases = fx.cases();
    let broker = ResourceBroker::new(ResourceLimits::uniform(64 * 1024 * 1024).unwrap()).unwrap();
    let cancel = Cancellation::default();
    let run = broker.begin_run(1, fx.source.id().digest(),
        ResourceLimits::uniform(64 * 1024 * 1024).unwrap(),
        Deadline::after(Duration::from_secs(60)).unwrap(), cancel.clone()).unwrap();
    let reservation = run.reserve(&[
        resource(ResourceKind::Ram, 32 * 1024 * 1024), resource(ResourceKind::Output, output),
    ]).unwrap();
    let view = reservation.lease();
    body(&fx, &cases, &view, &cancel)
}
