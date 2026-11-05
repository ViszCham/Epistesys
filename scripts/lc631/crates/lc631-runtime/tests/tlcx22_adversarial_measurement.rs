mod tlcx22_adversarial_support;
use lc631_analysis::creativity_measurement::{
    ComponentState, IndependenceStatus, InputBasis, SemanticValidation,
};
use lc631_core::measurement_numeric::RationalInterval;
use lc631_runtime::adversarial_measurement::*;
use lc631_solvers::{
    protocol::sexpr::{parse, SExprLimits},
    smt::validate_model,
    typed_query::{BoolExpr, BoolSymbolId},
};
use lc631_substrate::resource::Cancellation;
use tlcx22_adversarial_support::*;
#[test]
fn tlcx22_adversarial_weighted_handgold_pass_failure_unknown_and_zero_trials() {
    let theory = theory(&[], vec![]);
    let yes = BoolExpr::Constant(true);
    let no = BoolExpr::Constant(false);
    let w1 = q(1, 1);
    let w3 = q(3, 1);
    for trials in [0, 1, 8, 9, 10] {
        let cases = [
            AttackCase {
                key: b"a-pass",
                atoms: &[],
                theory: &theory,
                reference: &yes,
                weight: &w1,
            },
            AttackCase {
                key: b"b-fail",
                atoms: &[],
                theory: &theory,
                reference: &yes,
                weight: &w3,
            },
        ];
        let (_br, _run, res) = run(Cancellation::default());
        let view = res.lease();
        let mut l = limits();
        l.max_trials = trials.min(8);
        if trials == 10 {
            l.max_ast_visits = 0;
        }
        let catalog = register(binding(), &cases, l, &view).unwrap();
        let frozen = bind_candidate(
            &catalog,
            candidate(1),
            &[
                Response {
                    case: b"a-pass",
                    term: &yes,
                },
                Response {
                    case: b"b-fail",
                    term: if trials == 9 { &yes } else { &no },
                },
            ],
            &view,
        )
        .unwrap();
        let owner = execute(&frozen, &view).unwrap();
        assert_eq!(owner.fixed_weight(), &q(4, 1));
        let expected = match trials {
            0 | 10 => ComponentState::Unknown(
                lc631_analysis::creativity_measurement::MeasurementGap::MissingObservation,
            ),
            1 => ComponentState::Measured(RationalInterval::from_ratio(0, 1, 3, 4).unwrap()),
            9 => ComponentState::Measured(RationalInterval::exact_ratio(0, 1).unwrap()),
            _ => ComponentState::Measured(RationalInterval::exact_ratio(3, 4).unwrap()),
        };
        assert_eq!(owner.historical_report().state, expected);
        assert_eq!(owner.historical_report().basis, InputBasis::Given);
        assert_eq!(
            owner.historical_report().validation,
            SemanticValidation::Unassessed
        );
        assert_eq!(
            owner.historical_report().independence,
            IndependenceStatus::SharedDependency
        );
    }
}
#[test]
fn tlcx22_adversarial_nonzero_row_aliases_whole_query_and_separate_checker() {
    let gamma = theory(&[100, 7], vec![v(100), BoolExpr::Not(Box::new(v(7)))]);
    let expected = BoolExpr::Constant(false);
    let response = BoolExpr::Or(vec![v(100), v(7)]);
    let w = q(1, 1);
    // Semantic-key order puts y first and x second; actual row must be binary10.
    let atoms = [
        SemanticAtom {
            symbol: BoolSymbolId::new(100),
            meaning: b"z-x",
        },
        SemanticAtom {
            symbol: BoolSymbolId::new(7),
            meaning: b"a-y",
        },
    ];
    let cases = [AttackCase {
        key: b"nonzero",
        atoms: &atoms,
        theory: &gamma,
        reference: &expected,
        weight: &w,
    }];
    let (_br, _run, res) = run(Cancellation::default());
    let view = res.lease();
    let catalog = register(binding(), &cases, limits(), &view).unwrap();
    let frozen = bind_candidate(
        &catalog,
        candidate(1),
        &[Response {
            case: b"nonzero",
            term: &response,
        }],
        &view,
    )
    .unwrap();
    let owner = execute(&frozen, &view).unwrap();
    assert_eq!(owner.counterrow(0), Some(2));
    let query = owner.witness_query(0).unwrap();
    let bytes = owner.witness_model(0).unwrap();
    let document = parse(bytes, SExprLimits::checked(4096, 128, 8).unwrap()).unwrap();
    validate_model(query, &document).unwrap();
    let mut wrong_polarity = query.assertions().to_vec();
    let last = wrong_polarity.last_mut().unwrap();
    if let BoolExpr::Not(property) = last {
        *last = (**property).clone();
    } else {
        panic!("expected failure negation");
    }
    let mutant = lc631_solvers::typed_query::SmtStatusQuery::checked(
        query.boolean_symbols().to_vec(),
        vec![],
        wrong_polarity,
        query.limits(),
    )
    .unwrap();
    assert!(validate_model(&mutant, &document).is_err());
    let corrupt = b"(model (define-fun b0 () Bool false) (define-fun b1 () Bool false))";
    let corrupt = parse(corrupt, SExprLimits::checked(4096, 128, 8).unwrap()).unwrap();
    assert!(validate_model(query, &corrupt).is_err());
    // Row1: y=true,x=false satisfies the failure predicate, but violates Γ.
    // This isolates retention of Γ from retention of the negated agreement.
    let outside_domain = b"(model (define-fun b0 () Bool true) (define-fun b1 () Bool false))";
    let outside_domain =
        parse(outside_domain, SExprLimits::checked(4096, 128, 8).unwrap()).unwrap();
    assert!(validate_model(query, &outside_domain).is_err());
    assert_eq!(
        owner.historical_report().state,
        ComponentState::Measured(RationalInterval::exact_ratio(1, 1).unwrap())
    );
}
#[test]
fn tlcx22_adversarial_empty_hard_inconsistent_and_unsupported_never_zero() {
    let (_br, _run, res) = run(Cancellation::default());
    let view = res.lease();
    assert!(matches!(
        register(binding(), &[], limits(), &view),
        Err(Error::EmptySuite)
    ));
    let gamma = theory(&[], vec![BoolExpr::Constant(false)]);
    let expected = BoolExpr::Constant(true);
    let weight = q(1, 1);
    let cases = [AttackCase {
        key: b"impossible",
        atoms: &[],
        theory: &gamma,
        reference: &expected,
        weight: &weight,
    }];
    let mut hard = binding();
    hard.purpose = CasePurpose::HardUniversal;
    assert!(matches!(
        register(hard, &cases, limits(), &view),
        Err(Error::UnsupportedPurpose)
    ));
    assert!(matches!(
        register(binding(), &cases, limits(), &view),
        Err(Error::Prerequisite(_))
    ));
    let mut no_candidate_budget = limits();
    no_candidate_budget.max_ast_visits = 0;
    assert!(matches!(
        register(binding(), &cases, no_candidate_budget, &view),
        Err(Error::Prerequisite(_))
    ));
    let inhabited = theory(&[], vec![]);
    let cases = [AttackCase {
        theory: &inhabited,
        ..cases[0]
    }];
    let mut tiny = limits();
    tiny.registration_ast_visits = 1;
    assert!(matches!(
        register(binding(), &cases, tiny, &view),
        Err(Error::PrerequisitePending)
    ));
}
