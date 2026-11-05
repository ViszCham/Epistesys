use lc631_solvers::{
    protocol::sexpr::{self, SExprLimits},
    runner::QuerySourceIdentity,
    smt::{validate_model, ObjectiveDirection, OptimizationProbe, OptimizationProbeError},
    typed_query::{BoolExpr, IntExpr, IntSymbolId, SmtEmitLimits, SmtStatusQuery},
};

fn query() -> SmtStatusQuery {
    let i = IntSymbolId::new(0);
    SmtStatusQuery::checked(
        vec![],
        vec![i],
        vec![BoolExpr::LessEqual(
            IntExpr::Constant(0),
            IntExpr::Variable(i),
        )],
        SmtEmitLimits::default(),
    )
    .unwrap()
}

#[test]
fn tlcx11_optimization_minimum_probe_is_strict_and_preserves_base() {
    let query = query();
    let doc = sexpr::parse(b"((define-fun i0 () Int 3))", SExprLimits::default()).unwrap();
    let model = validate_model(&query, &doc).unwrap();
    let probe =
        OptimizationProbe::checked(&model, IntSymbolId::new(0), ObjectiveDirection::Minimize)
            .unwrap();
    assert_eq!(probe.incumbent(), 3);
    let payload = probe
        .better_query()
        .emit(QuerySourceIdentity::from_bytes([1; 32]))
        .unwrap();
    assert_eq!(payload.payload(), b"(set-logic QF_LIA)\n(declare-const i0 Int)\n(assert (<= 0 i0))\n(assert (< i0 3))\n(check-sat)\n");
    assert_eq!(query.assertions().len(), 1);
}

#[test]
fn tlcx11_optimization_maximum_probe_direction_and_no_integer_wrap() {
    let query = query();
    let doc = sexpr::parse(b"((define-fun i0 () Int 3))", SExprLimits::default()).unwrap();
    let model = validate_model(&query, &doc).unwrap();
    let probe =
        OptimizationProbe::checked(&model, IntSymbolId::new(0), ObjectiveDirection::Maximize)
            .unwrap();
    let payload = probe
        .better_query()
        .emit(QuerySourceIdentity::from_bytes([1; 32]))
        .unwrap();
    assert!(std::str::from_utf8(payload.payload())
        .unwrap()
        .contains("(assert (< 3 i0))"));
    let doc = sexpr::parse(
        b"((define-fun i0 () Int 9223372036854775808))",
        SExprLimits::default(),
    )
    .unwrap();
    let model = validate_model(&query, &doc).unwrap();
    assert_eq!(
        OptimizationProbe::checked(&model, IntSymbolId::new(0), ObjectiveDirection::Minimize).err(),
        Some(OptimizationProbeError::IncumbentOutOfRange)
    );
    assert_eq!(
        OptimizationProbe::checked(&model, IntSymbolId::new(7), ObjectiveDirection::Minimize).err(),
        Some(OptimizationProbeError::UndeclaredObjective)
    );
}

#[test]
fn tlcx11_optimization_probe_cannot_expand_original_assertion_budget() {
    let query = SmtStatusQuery::checked(
        vec![],
        vec![IntSymbolId::new(0)],
        vec![BoolExpr::Constant(true)],
        SmtEmitLimits::checked(4096, 0, 1, 1, 16, 8).unwrap(),
    )
    .unwrap();
    let doc = sexpr::parse(b"((define-fun i0 () Int 0))", SExprLimits::default()).unwrap();
    let model = validate_model(&query, &doc).unwrap();
    assert_eq!(
        OptimizationProbe::checked(&model, IntSymbolId::new(0), ObjectiveDirection::Minimize).err(),
        Some(OptimizationProbeError::Query(
            lc631_solvers::typed_query::QueryEmitError::TooManyAssertions
        ))
    );
}

#[test]
fn tlcx11_unbounded_objective_sat_model_only_produces_another_query() {
    let query = SmtStatusQuery::checked(
        vec![],
        vec![IntSymbolId::new(0)],
        vec![],
        SmtEmitLimits::default(),
    )
    .unwrap();
    let doc = sexpr::parse(b"((define-fun i0 () Int 0))", SExprLimits::default()).unwrap();
    let model = validate_model(&query, &doc).unwrap();
    let probe =
        OptimizationProbe::checked(&model, IntSymbolId::new(0), ObjectiveDirection::Minimize)
            .unwrap();
    assert_eq!(
        probe.better_query().assertions(),
        &[BoolExpr::LessThan(
            IntExpr::Variable(IntSymbolId::new(0)),
            IntExpr::Constant(0)
        )]
    );
    // No optimality constructor/result exists on the probe. An unbounded
    // objective requires continuing search or a distinct backend certificate.
}
