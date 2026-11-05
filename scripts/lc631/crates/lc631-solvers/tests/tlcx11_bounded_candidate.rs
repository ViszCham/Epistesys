use lc631_solvers::{
    smt::{
        native_probe::bounded::{BoundsError, FiniteObjectiveBounds},
        ObjectiveDirection,
    },
    typed_query::{BoolExpr, IntExpr, IntSymbolId, SmtEmitLimits, SmtStatusQuery},
};
fn var() -> IntExpr {
    IntExpr::Variable(IntSymbolId::new(0))
}
fn make_query(assertions: Vec<BoolExpr>) -> SmtStatusQuery {
    SmtStatusQuery::checked(
        vec![],
        vec![IntSymbolId::new(0)],
        assertions,
        SmtEmitLimits::default(),
    )
    .unwrap()
}

#[test]
fn tlcx11_disjunction_does_not_establish_finite_bounds() {
    let query = make_query(vec![BoolExpr::Or(vec![
        BoolExpr::LessEqual(IntExpr::Constant(0), var()),
        BoolExpr::LessEqual(var(), IntExpr::Constant(3)),
    ])]);
    assert_eq!(
        FiniteObjectiveBounds::from_query(&query, IntSymbolId::new(0)).err(),
        Some(BoundsError::MissingLowerBound)
    );
}
#[test]
fn tlcx11_negative_variable_is_not_silently_reoriented() {
    let query = make_query(vec![
        BoolExpr::LessEqual(IntExpr::Constant(0), IntExpr::Negate(Box::new(var()))),
        BoolExpr::LessEqual(var(), IntExpr::Constant(3)),
    ]);
    assert_eq!(
        FiniteObjectiveBounds::from_query(&query, IntSymbolId::new(0)).err(),
        Some(BoundsError::MissingLowerBound)
    );
}
#[test]
fn tlcx11_conjunctive_bounds_and_directional_gap_match_hand_arithmetic() {
    let query = make_query(vec![BoolExpr::And(vec![
        BoolExpr::LessEqual(IntExpr::Constant(-3), var()),
        BoolExpr::LessThan(var(), IntExpr::Constant(5)),
    ])]);
    let bounds = FiniteObjectiveBounds::from_query(&query, IntSymbolId::new(0)).unwrap();
    assert_eq!((bounds.lower(), bounds.upper()), (-3, 4));
    assert_eq!(
        bounds.gap_upper_bound(1, ObjectiveDirection::Minimize),
        Ok(4)
    );
    assert_eq!(
        bounds.gap_upper_bound(1, ObjectiveDirection::Maximize),
        Ok(3)
    );
    assert_eq!(
        bounds.gap_upper_bound(5, ObjectiveDirection::Minimize),
        Err(BoundsError::ValueOutsideBounds)
    );
}
#[test]
fn tlcx11_inconsistent_bounds_and_strict_integer_overflow_rejected() {
    let query = make_query(vec![
        BoolExpr::LessEqual(IntExpr::Constant(3), var()),
        BoolExpr::LessEqual(var(), IntExpr::Constant(2)),
    ]);
    assert_eq!(
        FiniteObjectiveBounds::from_query(&query, IntSymbolId::new(0)).err(),
        Some(BoundsError::InconsistentBounds)
    );
    let query = make_query(vec![BoolExpr::LessThan(IntExpr::Constant(i64::MAX), var())]);
    assert_eq!(
        FiniteObjectiveBounds::from_query(&query, IntSymbolId::new(0)).err(),
        Some(BoundsError::IntegerBoundaryOverflow)
    );
}
#[test]
fn tlcx11_full_i64_gap_does_not_overflow_signed_integer() {
    let query = make_query(vec![
        BoolExpr::LessEqual(IntExpr::Constant(i64::MIN), var()),
        BoolExpr::LessEqual(var(), IntExpr::Constant(i64::MAX)),
    ]);
    let bounds = FiniteObjectiveBounds::from_query(&query, IntSymbolId::new(0)).unwrap();
    assert_eq!(
        bounds.gap_upper_bound(i64::MAX, ObjectiveDirection::Minimize),
        Ok(u128::from(u64::MAX))
    );
}
