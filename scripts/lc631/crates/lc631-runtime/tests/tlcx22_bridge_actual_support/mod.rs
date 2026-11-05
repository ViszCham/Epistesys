//! 日本語: 固定Given入力とliteral oracle。測定reportや成功値をproducerへ注入しない。
//! English: Fixed Given input and literal oracle; no report/success injection.
use lc631_analysis::creativity_measurement::{ComponentState, RationalInterval};
use lc631_core::context::{ContextRevision, ContextScope, SourceRevision};
use lc631_runtime::resource::{ResourceBroker, ResourceLimits};
use lc631_runtime::{bridge_gain_measurement as b, compatibility_measurement as c};
use lc631_solvers::typed_query::{BoolExpr, BoolSymbolId, SmtEmitLimits, SmtStatusQuery};
use lc631_substrate::{
    resource::{
        Cancellation, Deadline, ResourceAmount, ResourceKind, ResourceRequest,
        ResourceReservationView,
    },
    source::SourceStore,
    Digest32,
};
use std::{sync::Arc, time::Duration};

pub const NORMAL: [u32; 6] = [0, 1, 2, 10, 11, 12];
pub const RENAMED: [u32; 6] = [4, 8, 17, 20, 31, 42];
// Literal independent truth table: P,Q / source P / baseline U=Q / candidate U=P.
pub const HAND: [(bool, bool, bool, bool); 4] = [
    (false, false, false, false),
    (false, true, true, false),
    (true, false, false, true),
    (true, true, true, true),
];
pub fn d(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}
pub fn bounds(state: &ComponentState) -> &RationalInterval {
    match state {
        ComponentState::Measured(value) => value,
        other => panic!("expected measured finite value, got {other:?}"),
    }
}
pub fn unknown() -> RationalInterval {
    RationalInterval::from_ratio(0, 1, 1, 1).unwrap()
}
pub fn ratio(n: i64, den: i64) -> RationalInterval {
    RationalInterval::exact_ratio(n, den).unwrap()
}
fn query(symbols: &[BoolSymbolId], assertion: BoolSymbolId) -> SmtStatusQuery {
    SmtStatusQuery::checked(
        symbols.to_vec(),
        vec![],
        vec![BoolExpr::Variable(assertion)],
        SmtEmitLimits::default(),
    )
    .unwrap()
}
pub struct Fixture {
    pub source: SourceStore,
    pub target: SourceStore,
    pub source_symbols: [BoolSymbolId; 3],
    pub target_symbols: [BoolSymbolId; 3],
    pub source_theory: SmtStatusQuery,
    pub target_theory: SmtStatusQuery,
    pub source_use: SmtStatusQuery,
    pub target_use: SmtStatusQuery,
    pub expected: [c::PortMap; 3],
    pub values: [[c::Assignment; 3]; 4],
}
impl Fixture {
    pub fn new(ids: [u32; 6]) -> Self {
        let [h, p, q, hp, u, v] = ids.map(BoolSymbolId::new);
        Self {
            source: SourceStore::new(Arc::from("Given input-register H=true; P,Q; return P"), 128)
                .unwrap(),
            target: SourceStore::new(
                Arc::from("Given control-port Hprime=true; U,V; consume U"),
                128,
            )
            .unwrap(),
            source_symbols: [h, p, q],
            target_symbols: [hp, u, v],
            source_theory: query(&[h, p, q], h),
            target_theory: query(&[hp, u, v], hp),
            source_use: query(&[h, p, q], p),
            target_use: query(&[hp, u, v], u),
            expected: [
                c::PortMap {
                    source: h,
                    target: hp,
                },
                c::PortMap {
                    source: p,
                    target: u,
                },
                c::PortMap {
                    source: q,
                    target: v,
                },
            ],
            values: [[false, false], [false, true], [true, false], [true, true]].map(|pq| {
                [
                    c::Assignment {
                        symbol: h,
                        value: true,
                    },
                    c::Assignment {
                        symbol: p,
                        value: pq[0],
                    },
                    c::Assignment {
                        symbol: q,
                        value: pq[1],
                    },
                ]
            }),
        }
    }
    pub fn cases(&self) -> [c::ProbeCase<'_>; 4] {
        self.values
            .each_ref()
            .map(|input| c::ProbeCase { input, weight: 1 })
    }
    pub fn forward(&self, swap: bool) -> [c::PortMap; 3] {
        let mut map = self.expected;
        if swap {
            map[1].target = self.target_symbols[2];
            map[2].target = self.target_symbols[1];
        }
        map
    }
    pub fn inverse(forward: &[c::PortMap; 3]) -> [c::PortMap; 3] {
        forward.map(|m| c::PortMap {
            source: m.target,
            target: m.source,
        })
    }
    pub fn input<'i>(&'i self, cases: &'i [c::ProbeCase<'i>]) -> c::PlanInput<'i> {
        c::PlanInput {
            source: &self.source,
            target: &self.target,
            source_context: ContextRevision::original(SourceRevision::of(self.source.text())),
            target_context: ContextRevision::original(SourceRevision::of(self.target.text())),
            scope: ContextScope::declared(d(1), d(2), d(3), d(4)),
            root_goal: &self.source_theory,
            source_theory: &self.source_theory,
            target_theory: &self.target_theory,
            source_use: &self.source_use,
            target_use: &self.target_use,
            source_hard: self.source_symbols[0],
            target_hard: self.target_symbols[0],
            expected_mapping: &self.expected,
            cases,
            // Closed profile IDs, not claims of oracle authenticity/independence.
            pins: c::ContractPins {
                definition: d(0xb1),
                oracle: d(0xb2),
                policy: d(7),
                baseline: d(8),
            },
        }
    }
}
pub fn spec<'i>(forward: &'i [c::PortMap], inverse: &'i [c::PortMap]) -> b::ProgramSpec<'i> {
    b::ProgramSpec { forward, inverse }
}
pub fn root_input<'i>(
    input: c::PlanInput<'i>,
    baseline: b::ProgramSpec<'i>,
    work: u64,
) -> b::BridgeRootInput<'i> {
    b::BridgeRootInput {
        endpoints: b::EndpointPair::InputRegisters3ToControlPorts3V1,
        compatibility: input,
        frozen_baseline: baseline,
        limits: b::Limits { max_work: work },
    }
}
pub fn resource(kind: ResourceKind, amount: u64) -> ResourceRequest {
    ResourceRequest::new(kind, ResourceAmount::checked(amount).unwrap())
}
pub fn fixture<T>(
    ids: [u32; 6],
    output: u64,
    body: impl FnOnce(&Fixture, &[c::ProbeCase<'_>], &ResourceReservationView<'_>, &Cancellation) -> T,
) -> T {
    let fx = Fixture::new(ids);
    let cases = fx.cases();
    let broker = ResourceBroker::new(ResourceLimits::uniform(64 * 1024 * 1024).unwrap()).unwrap();
    let cancel = Cancellation::default();
    let run = broker
        .begin_run(
            1,
            fx.source.id().digest(),
            ResourceLimits::uniform(64 * 1024 * 1024).unwrap(),
            Deadline::after(Duration::from_secs(60)).unwrap(),
            cancel.clone(),
        )
        .unwrap();
    let reservation = run
        .reserve(&[
            resource(ResourceKind::Ram, 16 * 1024 * 1024),
            resource(ResourceKind::Output, output),
        ])
        .unwrap();
    let view = reservation.lease();
    body(&fx, &cases, &view, &cancel)
}
// Orchestration only. Expected rationals are literal arguments in the tests.
pub fn measured(ids: [u32; 6], baseline_swap: bool, candidate_swap: bool) -> RationalInterval {
    fixture(ids, 2 * 1024 * 1024, |fx, cases, view, _| {
        let base = fx.forward(baseline_swap);
        let bi = Fixture::inverse(&base);
        let cand = fx.forward(candidate_swap);
        let ci = Fixture::inverse(&cand);
        let root =
            b::BridgeRoot::given(root_input(fx.input(cases), spec(&base, &bi), 48), view).unwrap();
        let bp = root.baseline_program(view).unwrap();
        let cp = root.candidate_program(spec(&cand, &ci), view).unwrap();
        let bb = root.prepare(&bp, view).unwrap();
        let cb = root.prepare(&cp, view).unwrap();
        let bu = bb
            .execute_uses(view)
            .unwrap_or_else(|_| panic!("baseline actual use failed"));
        let cu = cb
            .execute_uses(view)
            .unwrap_or_else(|_| panic!("candidate actual use failed"));
        let gain = b::measure_pair(&root, &bu, &cu, view).unwrap();
        bounds(&gain.report_current(&root, &cp, view).unwrap().state).clone()
    })
}
