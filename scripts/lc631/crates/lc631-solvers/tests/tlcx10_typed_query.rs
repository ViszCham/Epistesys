use lc631_solvers::protocol::QueryResultKind;
use lc631_solvers::registry::{BackendId, LogicProfile};
use lc631_solvers::runner::QuerySourceIdentity;
use lc631_solvers::typed_query::{
    BoolExpr, BoolSymbolId, IntExpr, IntSymbolId, QueryEmitError, SmtEmitLimits, SmtStatusQuery,
};

fn source() -> QuerySourceIdentity {
    QuerySourceIdentity::from_bytes([0x6d; 32])
}

fn limits() -> SmtEmitLimits {
    SmtEmitLimits::default()
}

fn valid_query() -> SmtStatusQuery {
    SmtStatusQuery::checked(
        vec![BoolSymbolId::new(7)],
        vec![IntSymbolId::new(3)],
        vec![BoolExpr::And(vec![
            BoolExpr::Variable(BoolSymbolId::new(7)),
            BoolExpr::LessEqual(
                IntExpr::Add(vec![
                    IntExpr::Variable(IntSymbolId::new(3)),
                    IntExpr::Constant(2),
                ]),
                IntExpr::Constant(10),
            ),
        ])],
        limits(),
    )
    .unwrap()
}

#[test]
fn tlcx10_emitter_builds_a_deterministic_status_query_and_borrows_it_safely() {
    let prepared = valid_query().emit(source()).unwrap();
    let expected = concat!(
        "(set-logic QF_LIA)\n",
        "(declare-const b7 Bool)\n",
        "(declare-const i3 Int)\n",
        "(assert (and b7 (<= (+ i3 2) 10)))\n",
        "(check-sat)\n"
    );
    assert_eq!(prepared.payload(), expected.as_bytes());

    let solver_query = prepared.solver_query().unwrap();
    assert_eq!(solver_query.backend(), BackendId::Z3);
    assert_eq!(solver_query.profile(), LogicProfile::SmtBooleanQfLia);
    assert_eq!(solver_query.result_kind(), QueryResultKind::Satisfiability);
    assert_eq!(solver_query.source(), source());
    assert_eq!(solver_query.payload(), expected.as_bytes());
}

#[test]
fn tlcx10_emitter_sorts_declarations_and_is_repeatable() {
    let query = SmtStatusQuery::checked(
        vec![BoolSymbolId::new(9), BoolSymbolId::new(2)],
        vec![IntSymbolId::new(8), IntSymbolId::new(1)],
        vec![BoolExpr::And(vec![
            BoolExpr::Variable(BoolSymbolId::new(9)),
            BoolExpr::LessThan(
                IntExpr::Variable(IntSymbolId::new(8)),
                IntExpr::Variable(IntSymbolId::new(1)),
            ),
        ])],
        limits(),
    )
    .unwrap();

    let first = query.emit(source()).unwrap();
    let second = query.emit(source()).unwrap();
    assert_eq!(first.payload(), second.payload());
    let text = std::str::from_utf8(first.payload()).unwrap();
    assert!(text.find("b2").unwrap() < text.find("b9").unwrap());
    assert!(text.find("i1").unwrap() < text.find("i8").unwrap());
}

#[test]
fn tlcx10_emitter_uses_smtlib_negative_integer_terms_without_overflow() {
    let query = SmtStatusQuery::checked(
        vec![],
        vec![],
        vec![BoolExpr::And(vec![
            BoolExpr::Equal(IntExpr::Constant(-7), IntExpr::Constant(i64::MIN)),
            BoolExpr::LessEqual(IntExpr::Constant(-1), IntExpr::Constant(0)),
        ])],
        limits(),
    )
    .unwrap();

    let text = std::str::from_utf8(query.emit(source()).unwrap().payload())
        .unwrap()
        .to_owned();
    assert!(text.contains("(- 7)"));
    assert!(text.contains("(- 9223372036854775808)"));
    assert!(!text.contains("-9223372036854775808"));
}

#[test]
fn tlcx10_emitter_rejects_duplicate_or_undeclared_symbol_ids() {
    assert!(matches!(
        SmtStatusQuery::checked(
            vec![BoolSymbolId::new(1), BoolSymbolId::new(1)],
            vec![],
            vec![],
            limits(),
        ),
        Err(QueryEmitError::DuplicateBooleanSymbol)
    ));
    assert!(matches!(
        SmtStatusQuery::checked(
            vec![],
            vec![IntSymbolId::new(2), IntSymbolId::new(2)],
            vec![],
            limits(),
        ),
        Err(QueryEmitError::DuplicateIntegerSymbol)
    ));
    assert!(matches!(
        SmtStatusQuery::checked(
            vec![],
            vec![],
            vec![BoolExpr::Variable(BoolSymbolId::new(5))],
            limits(),
        ),
        Err(QueryEmitError::UndeclaredBooleanSymbol)
    ));
    assert!(matches!(
        SmtStatusQuery::checked(
            vec![],
            vec![],
            vec![BoolExpr::LessEqual(
                IntExpr::Variable(IntSymbolId::new(6)),
                IntExpr::Constant(0),
            )],
            limits(),
        ),
        Err(QueryEmitError::UndeclaredIntegerSymbol)
    ));
}

#[test]
fn tlcx10_emitter_rejects_unbounded_ast_shapes_before_serialization() {
    assert!(matches!(
        SmtStatusQuery::checked(vec![], vec![], vec![BoolExpr::And(Vec::new())], limits(),),
        Err(QueryEmitError::EmptyConjunction)
    ));
    let shallow = SmtEmitLimits::checked(1024, 8, 8, 8, 8, 2).unwrap();
    assert!(matches!(
        SmtStatusQuery::checked(
            vec![BoolSymbolId::new(1)],
            vec![],
            vec![BoolExpr::Not(Box::new(BoolExpr::Not(Box::new(
                BoolExpr::Variable(BoolSymbolId::new(1))
            ))))],
            shallow,
        ),
        Err(QueryEmitError::DepthLimitExceeded)
    ));
    let node_limited = SmtEmitLimits::checked(1024, 8, 8, 8, 1, 8).unwrap();
    assert!(matches!(
        SmtStatusQuery::checked(
            vec![BoolSymbolId::new(1)],
            vec![],
            vec![BoolExpr::Not(Box::new(BoolExpr::Variable(
                BoolSymbolId::new(1)
            )))],
            node_limited,
        ),
        Err(QueryEmitError::NodeLimitExceeded)
    ));
}

#[test]
fn tlcx10_emitter_enforces_output_and_symbol_caps() {
    let small_output = SmtEmitLimits::checked(8, 8, 8, 8, 64, 8).unwrap();
    let query = SmtStatusQuery::checked(vec![], vec![], vec![], small_output).unwrap();
    assert!(matches!(
        query.emit(source()),
        Err(QueryEmitError::OutputTooLarge)
    ));

    let small_symbols = SmtEmitLimits::checked(1024, 1, 1, 8, 64, 8).unwrap();
    assert!(matches!(
        SmtStatusQuery::checked(
            vec![BoolSymbolId::new(1), BoolSymbolId::new(2)],
            vec![],
            vec![],
            small_symbols,
        ),
        Err(QueryEmitError::TooManyBooleanSymbols)
    ));
}

#[test]
fn tlcx10_emitter_caps_its_registered_limits() {
    assert!(matches!(
        SmtEmitLimits::checked(0, 1, 1, 1, 1, 1),
        Err(QueryEmitError::InvalidLimits)
    ));
    assert!(matches!(
        SmtEmitLimits::checked(usize::MAX, 1, 1, 1, 1, 1),
        Err(QueryEmitError::LimitsTooLarge)
    ));
}
