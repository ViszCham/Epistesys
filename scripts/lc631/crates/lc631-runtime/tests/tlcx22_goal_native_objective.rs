//! Selected real-native profile. Missing/unavailable binaries are not positive evidence.
#[allow(dead_code)]
#[path = "tlcx40_canonical_support/mod.rs"]
mod native;
use lc631_analysis::creativity_measurement::{
    ComponentMetric, ComponentState, IndependenceStatus, InputBasis, RationalInterval,
    SemanticValidation,
};
use lc631_core::context::ContextScope;
use lc631_runtime::goal_suite_measurement as g;
use lc631_solvers::{
    protocol::sexpr::SExprLimits,
    smt::native_model::{execute_native_model, NativeCheckedModel},
    supervisor_adapter::OwnedPayloadStore,
    typed_query::{BoolExpr, IntExpr, IntSymbolId, SmtEmitLimits, SmtStatusQuery},
};

const SOURCE: &str =
    "Given finite integer x in [0,3]; hard x<=2; maximize x under fixed score x/3.";
const HAND_DOMAIN: [i64; 4] = [0, 1, 2, 3];
const HAND_FEASIBLE: [i64; 3] = [0, 1, 2];
fn q(n: i64, den: i64) -> RationalInterval {
    RationalInterval::exact_ratio(n, den).unwrap()
}
fn scope() -> ContextScope {
    ContextScope::declared(native::d(51), native::d(52), native::d(53), native::d(54))
}
fn root_query() -> SmtStatusQuery {
    let x = IntExpr::Variable(IntSymbolId::new(0));
    SmtStatusQuery::checked(
        vec![],
        vec![IntSymbolId::new(0)],
        vec![
            BoolExpr::LessEqual(IntExpr::Constant(0), x.clone()),
            BoolExpr::LessEqual(x.clone(), IntExpr::Constant(3)),
            BoolExpr::LessEqual(x, IntExpr::Constant(2)),
        ],
        SmtEmitLimits::default(),
    )
    .unwrap()
}
fn assignment_query(value: i64) -> SmtStatusQuery {
    let x = IntExpr::Variable(IntSymbolId::new(0));
    // x=3 is intentionally a checked native model of its own declared query,
    // but NOT a feasible model of the immutable root. Root checker must reject it.
    SmtStatusQuery::checked(
        vec![],
        vec![IntSymbolId::new(0)],
        vec![
            BoolExpr::LessEqual(IntExpr::Constant(0), x.clone()),
            BoolExpr::LessEqual(x.clone(), IntExpr::Constant(3)),
            BoolExpr::Equal(x, IntExpr::Constant(value)),
        ],
        SmtEmitLimits::default(),
    )
    .unwrap()
}
fn values(state: &ComponentState) -> &RationalInterval {
    match state {
        ComponentState::Measured(value) => value,
        other => panic!("{other:?}"),
    }
}
fn fixture(
    body: impl for<'a, 'q, 'v, 'lease> FnOnce(
        &'q native::Fixture,
        &'q SmtStatusQuery,
        &'q lc631_solvers::registry::SolverRegistry,
        &'a [NativeCheckedModel<'q, 'v>],
        &'v lc631_substrate::resource::ResourceReservationView<'lease>,
    ),
) {
    let f = native::Fixture::with_scope(SOURCE, scope());
    let view = f.reservation.lease();
    let registry = native::native_registry();
    let cwd = std::env::current_dir().unwrap();
    let root = root_query();
    let queries = [
        assignment_query(1),
        assignment_query(2),
        assignment_query(3),
    ];
    let mut store = OwnedPayloadStore::with_resources(3, &view).unwrap();
    let models: Vec<_> = queries
        .iter()
        .map(|query| {
            execute_native_model(
                &registry,
                query,
                f.context.state().revision(),
                &native::execution(),
                &view,
                &cwd,
                &mut store,
                SExprLimits::checked(65536, 4096, 64).unwrap(),
            )
            .unwrap_or_else(|_| {
                panic!("selected pinned native model must actually execute and recheck")
            })
        })
        .collect();
    assert_eq!(store.len(), 3);
    for (owner, expected) in models.iter().zip([1, 2, 3]) {
        assert_eq!(owner.model().integer(IntSymbolId::new(0)), Some(expected));
        assert_eq!(owner.model().assertions_checked(), 3);
        assert!(owner.observation().payload().is_some());
    }
    body(&f, &root, &registry, &models, &view);
}
fn input<'a>(
    f: &'a native::Fixture,
    root: &'a SmtStatusQuery,
    registry: &'a lc631_solvers::registry::SolverRegistry,
) -> g::ObjectiveRootInput<'a> {
    g::ObjectiveRootInput {
        source: &f.source,
        context: f.context.state().revision(),
        scope: scope(),
        root,
        symbol: IntSymbolId::new(0),
        registry,
        profile: g::ObjectiveProfile::FiniteZeroToThreeHardAtMostTwoV1,
        limits: g::Limits {
            max_queries: 4,
            max_pairs: 1,
            max_current_checks: 8,
            max_added_ram_bytes: 64 * 1024,
            max_summary_bytes: 64 * 1024,
        },
    }
}

#[test]
#[ignore = "main-only selected real pinned Z3 execution; absent backend fails when selected"]
fn tlcx22_solve_actual_native_models_have_absolute_one_third_two_thirds_and_delta_one_third() {
    fixture(|f, query, registry, models, view| {
        // Independent literal enumeration; not a production score/model oracle call.
        assert_eq!(
            HAND_DOMAIN
                .into_iter()
                .filter(|x| *x <= 2)
                .collect::<Vec<_>>(),
            HAND_FEASIBLE
        );
        assert_eq!(HAND_FEASIBLE.into_iter().max(), Some(2));
        let root = g::ObjectiveRoot::given(input(f, query, registry), view).unwrap();
        let pair = g::NativePair {
            baseline: &models[0],
            candidate: &models[1],
        };
        let measured = g::measure_native_pair(&root, pair, view).unwrap();
        measured
            .with_current(&root, pair, view, |v| {
                assert_eq!(v.baseline_goal(), &q(1, 3));
                assert_eq!(v.candidate_goal(), &q(2, 3));
                assert_eq!(v.solve().metric, ComponentMetric::Solve);
                assert_eq!(values(&v.solve().state), &q(1, 3));
                assert_eq!(v.solve().basis, InputBasis::Given);
                assert_eq!(v.solve().validation, SemanticValidation::Unassessed);
                assert_eq!(v.solve().independence, IndependenceStatus::SharedDependency);
                assert_eq!(v.solve().dependency_group, v.goal_dependency());
                assert_eq!(v.optimality(), g::GoalOptimality::BestAmongEvaluated);
                assert!(matches!(
                    v.require_overlap(g::DeclaredOverlap::Absent),
                    Err(g::Error::UndeclaredOverlap)
                ));
                v.require_overlap(g::DeclaredOverlap::SameArtifactPrimitive)
                    .unwrap();
            })
            .unwrap();
    });
}

#[test]
#[ignore = "main-only selected real native hard-negative; no injected model/report"]
fn tlcx22_solve_checked_native_x_three_fails_fixed_root_before_any_positive_scalar() {
    fixture(|f, query, registry, models, view| {
        assert_eq!(models[2].model().integer(IntSymbolId::new(0)), Some(3));
        let root = g::ObjectiveRoot::given(input(f, query, registry), view).unwrap();
        assert!(matches!(
            g::measure_native_pair(
                &root,
                g::NativePair {
                    baseline: &models[0],
                    candidate: &models[2]
                },
                view
            ),
            Err(g::Error::HardInvariantFailed)
        ));
    });
}

#[test]
#[ignore = "main-only selected current-owner and cancellation cuts"]
fn tlcx22_solve_native_current_root_owner_swap_and_cancel_withhold_cached_delta() {
    fixture(|f, query, registry, models, view| {
        let root = g::ObjectiveRoot::given(input(f, query, registry), view).unwrap();
        let other = g::ObjectiveRoot::given(input(f, query, registry), view).unwrap();
        let pair = g::NativePair {
            baseline: &models[0],
            candidate: &models[1],
        };
        let measured = g::measure_native_pair(&root, pair, view).unwrap();
        assert!(matches!(
            measured.with_current(&other, pair, view, |_| ()),
            Err(g::Error::BindingMismatch)
        ));
        assert!(matches!(
            measured.with_current(
                &root,
                g::NativePair {
                    baseline: &models[1],
                    candidate: &models[0]
                },
                view,
                |_| ()
            ),
            Err(g::Error::RoleOrOriginMismatch)
        ));
        f.cancellation.cancel();
        assert!(matches!(
            measured.with_current(&root, pair, view, |_| ()),
            Err(g::Error::Resource(
                lc631_substrate::resource::ResourceError::Cancelled
            ))
        ));
    });
}
