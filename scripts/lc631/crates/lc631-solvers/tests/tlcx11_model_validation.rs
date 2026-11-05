use lc631_solvers::{
    protocol::sexpr::{self, SExprLimits},
    smt::{validate_model, ModelValidationError},
    typed_query::{BoolExpr, BoolSymbolId, IntExpr, IntSymbolId, SmtEmitLimits, SmtStatusQuery},
};

fn problem() -> SmtStatusQuery {
    SmtStatusQuery::checked(
        vec![BoolSymbolId::new(0)],
        vec![IntSymbolId::new(0)],
        vec![
            BoolExpr::Variable(BoolSymbolId::new(0)),
            BoolExpr::Equal(
                IntExpr::Variable(IntSymbolId::new(0)),
                IntExpr::Constant(-3),
            ),
        ],
        SmtEmitLimits::default(),
    )
    .unwrap()
}

#[test]
fn tlcx11_incorrect_model_is_rejected() {
    let doc = sexpr::parse(
        b"((define-fun b0 () Bool true)(define-fun i0 () Int 3))",
        SExprLimits::default(),
    )
    .unwrap();
    assert_eq!(
        validate_model(&problem(), &doc).err(),
        Some(ModelValidationError::AssertionFalse { index: 1 })
    );
}

#[test]
fn tlcx11_missing_assignment_is_not_a_default_value() {
    let doc = sexpr::parse(b"((define-fun b0 () Bool true))", SExprLimits::default()).unwrap();
    assert_eq!(
        validate_model(&problem(), &doc).err(),
        Some(ModelValidationError::MissingAssignment)
    );
}

#[test]
fn tlcx11_complete_model_is_reevaluated() {
    let doc = sexpr::parse(
        b"((define-fun i0 () Int (- 3))(define-fun b0 () Bool true))",
        SExprLimits::default(),
    )
    .unwrap();
    let query = problem();
    let model = validate_model(&query, &doc).unwrap();
    assert_eq!(model.integer(IntSymbolId::new(0)), Some(-3));
    assert_eq!(model.boolean(BoolSymbolId::new(0)), Some(true));
    assert_eq!(model.assertions_checked(), 2);
}

#[test]
fn tlcx11_duplicate_undeclared_and_sort_mismatch_rejected() {
    for (raw, expected) in [
        (
            b"((define-fun b0 () Bool true)(define-fun b0 () Bool false))".as_slice(),
            ModelValidationError::DuplicateAssignment,
        ),
        (
            b"((define-fun b7 () Bool true))",
            ModelValidationError::UndeclaredSymbol,
        ),
        (
            b"((define-fun b0 () Int 1))",
            ModelValidationError::SortMismatch,
        ),
        (
            b"((define-fun b0 () Bool (forall ((x Bool)) x)))",
            ModelValidationError::UnsupportedValue,
        ),
    ] {
        let doc = sexpr::parse(raw, SExprLimits::default()).unwrap();
        assert_eq!(validate_model(&problem(), &doc).err(), Some(expected));
    }
}

#[test]
fn tlcx11_arithmetic_overflow_is_not_a_valid_witness() {
    let i = IntSymbolId::new(0);
    let query = SmtStatusQuery::checked(
        vec![],
        vec![i],
        vec![BoolExpr::Equal(
            IntExpr::Add(vec![IntExpr::Variable(i), IntExpr::Constant(1)]),
            IntExpr::Constant(0),
        )],
        SmtEmitLimits::default(),
    )
    .unwrap();
    let doc = sexpr::parse(
        b"((define-fun i0 () Int 170141183460469231731687303715884105727))",
        SExprLimits::default(),
    )
    .unwrap();
    assert_eq!(
        validate_model(&query, &doc).err(),
        Some(ModelValidationError::ArithmeticOverflow)
    );
}

#[test]
fn tlcx11_false_disjunct_does_not_hide_overflow() {
    let i = IntSymbolId::new(0);
    let query = SmtStatusQuery::checked(
        vec![],
        vec![i],
        vec![BoolExpr::Or(vec![
            BoolExpr::Constant(true),
            BoolExpr::Equal(
                IntExpr::Negate(Box::new(IntExpr::Variable(i))),
                IntExpr::Constant(0),
            ),
        ])],
        SmtEmitLimits::default(),
    )
    .unwrap();
    let doc = sexpr::parse(
        b"((define-fun i0 () Int (- 170141183460469231731687303715884105728)))",
        SExprLimits::default(),
    )
    .unwrap();
    assert_eq!(
        validate_model(&query, &doc).err(),
        Some(ModelValidationError::ArithmeticOverflow)
    );
}

#[test]
fn tlcx11_integer_operations_match_hand_arithmetic() {
    let i = IntSymbolId::new(0);
    let query = SmtStatusQuery::checked(
        vec![],
        vec![i],
        vec![
            BoolExpr::LessThan(IntExpr::Variable(i), IntExpr::Constant(0)),
            BoolExpr::LessEqual(IntExpr::Constant(-5), IntExpr::Variable(i)),
            BoolExpr::Equal(
                IntExpr::Subtract(
                    Box::new(IntExpr::Constant(4)),
                    Box::new(IntExpr::Variable(i)),
                ),
                IntExpr::Constant(7),
            ),
            BoolExpr::Equal(
                IntExpr::Negate(Box::new(IntExpr::Variable(i))),
                IntExpr::Constant(3),
            ),
        ],
        SmtEmitLimits::default(),
    )
    .unwrap();
    let doc = sexpr::parse(
        b"(model (define-fun |i0| () Int (- 3)))",
        SExprLimits::default(),
    )
    .unwrap();
    assert_eq!(
        validate_model(&query, &doc).unwrap().assertions_checked(),
        4
    );
}

#[test]
fn tlcx11_all_eight_atom_assignments_match_separate_boolean_oracle() {
    let symbols: Vec<_> = (0..8).map(BoolSymbolId::new).collect();
    let variable = |i| BoolExpr::Variable(BoolSymbolId::new(i));
    let query = SmtStatusQuery::checked(
        symbols,
        vec![],
        vec![BoolExpr::And(vec![
            BoolExpr::Or(vec![variable(0), BoolExpr::Not(Box::new(variable(1)))]),
            BoolExpr::Not(Box::new(BoolExpr::And(vec![variable(2), variable(3)]))),
            BoolExpr::Or(vec![variable(4), variable(5), variable(6), variable(7)]),
        ])],
        SmtEmitLimits::default(),
    )
    .unwrap();
    let mut accepted = 0;
    for mask in 0u16..256 {
        let bits: Vec<_> = (0..8).map(|i| mask & (1 << i) != 0).collect();
        let expected =
            (bits[0] || !bits[1]) && !(bits[2] && bits[3]) && bits[4..8].iter().any(|value| *value);
        let mut payload = String::from("(");
        for (i, bit) in bits.iter().enumerate() {
            payload.push_str(&format!("(define-fun b{i} () Bool {bit})"));
        }
        payload.push(')');
        let doc = sexpr::parse(payload.as_bytes(), SExprLimits::default()).unwrap();
        let valid = validate_model(&query, &doc).is_ok();
        assert_eq!(valid, expected, "mask={mask}");
        accepted += usize::from(valid);
    }
    assert_eq!(accepted, 135);
}

#[test]
fn tlcx11_non_model_roots_and_functions_are_rejected() {
    for raw in [
        b"sat ((define-fun b0 () Bool true))".as_slice(),
        b"(error \"no model\")",
        b"((define-fun b0 ((x Bool)) Bool true))",
    ] {
        let doc = sexpr::parse(raw, SExprLimits::default()).unwrap();
        assert!(validate_model(&problem(), &doc).is_err());
    }
}

#[test]
fn tlcx11_undeclared_symbol_in_query_never_reaches_validator() {
    let result = SmtStatusQuery::checked(
        vec![],
        vec![],
        vec![BoolExpr::Variable(BoolSymbolId::new(7))],
        SmtEmitLimits::default(),
    );
    assert_eq!(
        result.err(),
        Some(lc631_solvers::typed_query::QueryEmitError::UndeclaredBooleanSymbol)
    );
}
