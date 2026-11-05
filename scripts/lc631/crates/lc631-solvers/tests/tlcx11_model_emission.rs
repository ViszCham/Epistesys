use lc631_solvers::{
    protocol::QueryResultKind,
    runner::QuerySourceIdentity,
    typed_query::{BoolExpr, IntExpr, IntSymbolId, QueryEmitError, SmtEmitLimits, SmtStatusQuery},
};

#[test]
fn tlcx11_model_emission_preserves_status_bytes_and_uses_payload_kind() {
    let query = SmtStatusQuery::checked(
        vec![],
        vec![IntSymbolId::new(7)],
        vec![BoolExpr::Equal(
            IntExpr::Variable(IntSymbolId::new(7)),
            IntExpr::Constant(-3),
        )],
        SmtEmitLimits::default(),
    )
    .unwrap();
    let source = QuerySourceIdentity::from_bytes([3; 32]);
    let legacy = query.emit(source).unwrap();
    assert_eq!(
        legacy.payload(),
        b"(set-logic QF_LIA)\n(declare-const i7 Int)\n(assert (= i7 (- 3)))\n(check-sat)\n"
    );
    let model = query.emit_model(source).unwrap();
    assert_eq!(model.payload(), b"(set-logic QF_LIA)\n(set-option :produce-models true)\n(declare-const i7 Int)\n(assert (= i7 (- 3)))\n(check-sat)\n(get-model)\n");
    assert_eq!(
        model.solver_query().unwrap().result_kind(),
        QueryResultKind::ModelPayload
    );
}

#[test]
fn tlcx11_model_suffix_is_within_original_output_budget() {
    let query = SmtStatusQuery::checked(
        vec![],
        vec![],
        vec![],
        SmtEmitLimits::checked(33, 0, 0, 0, 1, 1).unwrap(),
    )
    .unwrap();
    assert!(query.emit(QuerySourceIdentity::from_bytes([1; 32])).is_ok());
    assert_eq!(
        query
            .emit_model(QuerySourceIdentity::from_bytes([1; 32]))
            .err(),
        Some(QueryEmitError::OutputTooLarge)
    );
}
