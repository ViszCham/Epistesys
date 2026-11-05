//! Single-unit Given AST fixtures and observations only; no dependence interpreter.
pub use lc631_analysis::creativity_measurement::{
    ComponentReport, ComponentState, RationalInterval,
};
use lc631_core::{
    context::{ContextRevision, ContextScope, SourceRevision},
    CandidateId,
};
pub use lc631_runtime::{
    formal_boolean::{DirectionResult, Evaluation},
    joint_selection::RecommendationState,
    reorganization_measurement as r,
};
use lc631_runtime::{joint_selection::project_component_reported, resource::*};
use lc631_solvers::typed_query::{BoolExpr, BoolSymbolId, SmtEmitLimits, SmtStatusQuery};
use lc631_substrate::{source::SourceStore, Digest32};
use std::{sync::Arc, time::Duration};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Term {
    Zero,
    One,
    Input,
    NotInput,
    AndZero,
    OrOne,
    Contradiction,
}
#[derive(Clone, Copy)]
pub struct Case {
    pub baseline: Term,
    pub candidate: Term,
    pub map_back: Term,
    pub root: Term,
}
pub const ZERO_CHANGE: Case = Case {
    baseline: Term::Zero,
    candidate: Term::One,
    map_back: Term::Zero,
    root: Term::Zero,
};
// Literal independently calculated map outputs at false,true. These are NOT
// supplied to any producer constructor and do not implement an evaluator.
pub const MAP_HAND: [(Term, [bool; 2]); 7] = [
    (Term::Zero, [false, false]),
    (Term::One, [true, true]),
    (Term::Input, [false, true]),
    (Term::NotInput, [true, false]),
    (Term::AndZero, [false, false]),
    (Term::OrOne, [true, true]),
    (Term::Contradiction, [false, false]),
];
fn ast(term: Term, parameter: BoolSymbolId) -> BoolExpr {
    let input = || BoolExpr::Variable(parameter);
    match term {
        Term::Zero => BoolExpr::Constant(false),
        Term::One => BoolExpr::Constant(true),
        Term::Input => input(),
        Term::NotInput => BoolExpr::Not(Box::new(input())),
        Term::AndZero => BoolExpr::And(vec![input(), BoolExpr::Constant(false)]),
        Term::OrOne => BoolExpr::Or(vec![input(), BoolExpr::Constant(true)]),
        Term::Contradiction => BoolExpr::And(vec![input(), BoolExpr::Not(Box::new(input()))]),
    }
}
fn d(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}
pub fn ratio(n: i64, den: i64) -> RationalInterval {
    RationalInterval::exact_ratio(n, den).unwrap()
}
fn limits(max_check_work: u64) -> r::Limits {
    r::Limits {
        max_units: 16,
        max_atoms: 2,
        max_probes: 64,
        max_ast_nodes: 64,
        max_ast_depth: 16,
        max_payload_bytes: 16 * 1024,
        max_weight: 1_000_000,
        max_check_work,
        max_total_work: 262_144,
        plan_ram: 65_536,
        program_ram: 65_536,
        queries_ram: 65_536,
        owner_ram: 65_536,
        scratch_ram: 262_144,
        selector_ram: 262_144,
    }
}
pub struct Observation {
    pub report: ComponentReport,
    pub legacy_count: usize,
    pub computed: Vec<(r::CheckKind, DirectionResult, DirectionResult, u64)>,
    pub has_gaps: bool,
    pub recommendation: Option<CandidateId>,
    pub recommendation_state: RecommendationState,
}
pub fn execute(case: Case, max_check_work: u64, symbol: u32) -> Result<Observation, r::Error> {
    let source = SourceStore::new(
        Arc::from("Given unary Bool unit; fixed root function"),
        1024,
    )
    .unwrap();
    let context = ContextRevision::original(SourceRevision::of(source.text()));
    let parameter = BoolSymbolId::new(symbol);
    // Empty assertions are deliberate: the root predicate is NOT a theory axiom.
    let theory = SmtStatusQuery::checked(
        vec![parameter],
        vec![],
        vec![],
        SmtEmitLimits::checked(16 * 1024, 2, 0, 1, 64, 16).unwrap(),
    )
    .unwrap();
    let baseline_ast = ast(case.baseline, parameter);
    let candidate_ast = ast(case.candidate, parameter);
    let map_ast = ast(case.map_back, parameter);
    let root_ast = ast(case.root, parameter);
    let unit = r::UnitId(0);
    let baseline_binding = [r::BindingInput {
        unit,
        role: r::RoleId(0),
        parameter,
        body: &baseline_ast,
    }];
    let candidate_binding = [r::BindingInput {
        unit,
        role: r::RoleId(1),
        parameter,
        body: &candidate_ast,
    }];
    let actual_use = r::UseExpr::MappedUnit(unit);
    let uses = [r::RootUseInput {
        unit,
        body: &actual_use,
    }];
    let maps = [r::MapBackInput {
        unit,
        parameter,
        body: &map_ast,
    }];
    let probes = [r::RebindProbe {
        unit,
        weight: 1,
        root: &root_ast,
    }];
    let baseline = r::RoleModel::checked(&source, context, &theory, &baseline_binding, &uses)?;
    let candidate = r::RoleModel::checked(&source, context, &theory, &candidate_binding, &uses)?;
    let pins = lc631_runtime::joint_selection::SelectionPins {
        source: context.source(),
        scope: ContextScope::declared(d(1), d(2), d(3), d(4)),
        baseline: d(5),
        hard_invariants: d(6),
        policy: d(7),
        goal_profile: None,
        evidence_revision: d(9),
    };
    let cap = ResourceLimits::uniform(16 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(cap).unwrap();
    let run = broker
        .begin_run(
            91,
            source.id().digest(),
            cap,
            Deadline::after(Duration::from_secs(60)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let reservation = run
        .reserve(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(4 * 1024 * 1024).unwrap(),
        )])
        .unwrap();
    let view = reservation.lease();
    let plan =
        r::RebindPlan::capture_fixed(&baseline, &probes, pins, limits(max_check_work), &view)?;
    let program =
        r::PreparedRebindProgram::prepare(&plan, &candidate, &maps, CandidateId(2), &view)?;
    let queries = r::RebindQueries::capture(&program, &view)?;
    let owner = r::OwnedRebind::evaluate(&queries, &view)?;
    owner.with_current(&view, |current| {
        let computed = current
            .checks()
            .iter()
            .filter_map(|check| {
                if let Evaluation::Computed(result) = check.evaluation() {
                    assert!(std::ptr::eq(result.request(), check.request()));
                    Some((
                        check.kind(),
                        result.soundness(),
                        result.completeness(),
                        result.ast_visits(),
                    ))
                } else {
                    None
                }
            })
            .collect();
        let axis = project_component_reported(current.report()).unwrap();
        let row = current.bind_axis(&plan, &program, &queries, &axis).unwrap();
        let decision = r::select_current(&[row], &view).unwrap();
        Observation {
            report: current.report().clone(),
            legacy_count: current.checks().len(),
            computed,
            has_gaps: current.has_gaps(),
            recommendation: decision.recommendation(),
            recommendation_state: decision.recommendation_state(),
        }
    })
}
pub fn expect_missing_use(case: Case, symbol: u32) {
    let (_, hand) = MAP_HAND
        .iter()
        .find(|(term, _)| *term == case.map_back)
        .unwrap();
    assert_eq!(
        hand[0], hand[1],
        "negative fixture must be semantically nondependent"
    );
    match execute(case, 4096, symbol) {
        Err(r::Error::MissingActualUse) => {}
        Err(other) => panic!("expected MissingActualUse for nondependent map; got {other:?}"),
        Ok(observed) => panic!(
            "candidate output was discarded but producer returned {:?}; expected MissingActualUse",
            observed.report.state
        ),
    }
}
