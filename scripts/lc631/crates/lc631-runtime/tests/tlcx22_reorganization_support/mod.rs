//! Given input fixtures and literal hand gold only; no producer implementation.
#![allow(dead_code)]
pub use lc631_analysis::creativity_measurement::{
    ComponentMetric, ComponentReport, ComponentState, IndependenceStatus, InputBasis,
    RationalInterval, SemanticValidation,
};
pub use lc631_core::{
    context::{ContextRevision, ContextScope, SourceRevision},
    CandidateId,
};
pub use lc631_runtime::{
    formal_boolean::{DirectionResult, Evaluation, FormalValidation},
    joint_selection::{AxisKind, AxisValue, PairRelation, RecommendationState, SelectionPins},
    reorganization_measurement as r,
    resource::*,
};
pub use lc631_solvers::typed_query::{BoolExpr, BoolSymbolId, SmtEmitLimits, SmtStatusQuery};
pub use lc631_substrate::{source::SourceStore, Digest32};
pub use std::{sync::Arc, time::Duration};
pub const RAM: u64 = 2 * 1024 * 1024;
// Literal independent rows: role Z,I,N,O; map Z,I,N,O; input false,true.
pub const COMPOSED: [[[bool; 2]; 4]; 4] = [
    [[false, false], [false, false], [true, true], [true, true]],
    [[false, false], [false, true], [true, false], [true, true]],
    [[false, false], [true, false], [false, true], [true, true]],
    [[false, false], [true, true], [false, false], [true, true]],
];
pub const ADMISSIBLE: [[bool; 4]; 4] = [
    [false, false, false, false],
    [false, true, false, false],
    [false, false, true, false],
    [false, false, false, false],
];
pub fn d(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}
pub fn ratio(n: i64, den: i64) -> RationalInterval {
    RationalInterval::exact_ratio(n, den).unwrap()
}
pub fn limits() -> r::Limits {
    r::Limits {
        max_units: 16,
        max_atoms: 2,
        max_probes: 64,
        max_ast_nodes: 64,
        max_ast_depth: 16,
        max_payload_bytes: 16 * 1024,
        max_weight: 1_000_000,
        max_check_work: 4096,
        max_total_work: 262_144,
        plan_ram: 65_536,
        program_ram: 65_536,
        queries_ram: 65_536,
        owner_ram: 65_536,
        scratch_ram: 262_144,
        selector_ram: 262_144,
    }
}
pub fn resource(kind: ResourceKind, bytes: u64) -> ResourceRequest {
    ResourceRequest::new(kind, ResourceAmount::checked(bytes).unwrap())
}
pub struct Fixture {
    pub source: SourceStore,
    pub context: ContextRevision,
    pub theory: SmtStatusQuery,
    pub parameter: BoolSymbolId,
    pub units: [r::UnitId; 2],
    pub functions: [BoolExpr; 5],
    pub uses: [r::UseExpr; 2],
}
impl Fixture {
    pub fn new(unit_ids: [u16; 2], symbol: u32) -> Self {
        let parameter = BoolSymbolId::new(symbol);
        let v = BoolExpr::Variable(parameter);
        let not = BoolExpr::Not(Box::new(v.clone()));
        let source = SourceStore::new(
            Arc::from("Given two unary Bool roles; root returns x"),
            1024,
        )
        .unwrap();
        let context = ContextRevision::original(SourceRevision::of(source.text()));
        let units = unit_ids.map(r::UnitId);
        Self {
            source,
            context,
            theory: SmtStatusQuery::checked(
                vec![parameter],
                vec![],
                vec![],
                SmtEmitLimits::checked(16 * 1024, 2, 0, 1, 64, 16).unwrap(),
            )
            .unwrap(),
            parameter,
            units,
            functions: [
                BoolExpr::Constant(false),
                v,
                not.clone(),
                BoolExpr::Constant(true),
                BoolExpr::Not(Box::new(not)),
            ],
            uses: units.map(r::UseExpr::MappedUnit),
        }
    }
    pub fn bindings(&self, functions: [usize; 2], renamed: bool) -> [r::BindingInput<'_>; 2] {
        [0, 1].map(|i| r::BindingInput {
            unit: self.units[i],
            role: r::RoleId(if renamed { 20 + i as u16 } else { i as u16 }),
            parameter: self.parameter,
            body: &self.functions[functions[i]],
        })
    }
    pub fn root_uses(&self) -> [r::RootUseInput<'_>; 2] {
        [0, 1].map(|i| r::RootUseInput {
            unit: self.units[i],
            body: &self.uses[i],
        })
    }
    pub fn maps(&self, functions: [usize; 2]) -> [r::MapBackInput<'_>; 2] {
        [0, 1].map(|i| r::MapBackInput {
            unit: self.units[i],
            parameter: self.parameter,
            body: &self.functions[functions[i]],
        })
    }
    pub fn probes(&self, weights: [u32; 2]) -> [r::RebindProbe<'_>; 2] {
        [0, 1].map(|i| r::RebindProbe {
            unit: self.units[i],
            weight: weights[i],
            root: &self.functions[1],
        })
    }
    pub fn pins(&self) -> SelectionPins {
        SelectionPins {
            source: self.context.source(),
            scope: ContextScope::declared(d(1), d(2), d(3), d(4)),
            baseline: d(5),
            hard_invariants: d(6),
            policy: d(7),
            goal_profile: None,
            evidence_revision: d(9),
        }
    }
    pub fn model<'i>(
        &'i self,
        bindings: &'i [r::BindingInput<'i>],
        uses: &'i [r::RootUseInput<'i>],
    ) -> r::RoleModel<'i> {
        r::RoleModel::checked(&self.source, self.context, &self.theory, bindings, uses).unwrap()
    }
}
pub fn fixture<T>(
    ids: [u16; 2],
    symbol: u32,
    ram: u64,
    ttl: Duration,
    body: impl FnOnce(&Fixture, &ResourceReservationView<'_>, &Cancellation) -> T,
) -> T {
    let fx = Fixture::new(ids, symbol);
    let cap = ResourceLimits::uniform(32 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(cap).unwrap();
    let cancel = Cancellation::default();
    let run = broker
        .begin_run(
            22,
            fx.source.id().digest(),
            cap,
            Deadline::after(ttl).unwrap(),
            cancel.clone(),
        )
        .unwrap();
    let res = run
        .reserve(&[
            resource(ResourceKind::Ram, ram),
            resource(ResourceKind::Output, 65_536),
        ])
        .unwrap();
    let view = res.lease();
    body(&fx, &view, &cancel)
}
pub fn normal<T>(
    body: impl FnOnce(&Fixture, &ResourceReservationView<'_>, &Cancellation) -> T,
) -> T {
    fixture([0, 1], 0, RAM, Duration::from_secs(60), body)
}
pub fn assert_point(report: &ComponentReport, n: i64, den: i64) {
    assert_eq!(report.metric, ComponentMetric::Rebind);
    assert_eq!(report.state, ComponentState::Measured(ratio(n, den)));
    assert_eq!(report.basis, InputBasis::Given);
    assert_eq!(report.validation, SemanticValidation::Unassessed);
    assert_eq!(report.independence, IndependenceStatus::SharedDependency);
}
