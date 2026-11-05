mod tlcx22_adversarial_support;
use lc631_core::{context::SourceRevision, measurement_numeric::Rational, CandidateId};
use lc631_runtime::adversarial_measurement::*;
use lc631_solvers::typed_query::{BoolExpr, BoolSymbolId, IntExpr};
use lc631_substrate::resource::Cancellation;
use tlcx22_adversarial_support::*;
#[test]
fn tlcx22_adversarial_raw_weights_duplicate_scope_and_mapping_cuts() {
    let gamma = theory(&[], vec![]);
    let yes = BoolExpr::Constant(true);
    let weight = q(1, 1);
    let case = AttackCase {
        key: b"case",
        atoms: &[],
        theory: &gamma,
        reference: &yes,
        weight: &weight,
    };
    let (_br, _run, res) = run(Cancellation::default());
    let view = res.lease();
    for bad in [
        Rational::new_raw(1.into(), 0.into()),
        Rational::new_raw(2.into(), 4.into()),
    ] {
        assert!(matches!(
            register(
                binding(),
                &[AttackCase {
                    weight: &bad,
                    ..case
                }],
                limits(),
                &view
            ),
            Err(Error::Numeric(_))
        ));
    }
    for bad in [q(0, 1), q(-1, 1)] {
        assert!(matches!(
            register(
                binding(),
                &[AttackCase {
                    weight: &bad,
                    ..case
                }],
                limits(),
                &view
            ),
            Err(Error::InvalidWeight)
        ));
    }
    assert!(matches!(
        register(binding(), &[case, case], limits(), &view),
        Err(Error::DuplicateCase)
    ));
    let mut bad = binding();
    bad.probe_set = d(222);
    assert!(matches!(
        register(bad, &[case], limits(), &view),
        Err(Error::BindingMismatch)
    ));
    bad = binding();
    bad.selection.source = SourceRevision::of("other");
    assert!(matches!(
        register(bad, &[case], limits(), &view),
        Err(Error::Resource(_))
    ));
    let atom = SemanticAtom {
        symbol: BoolSymbolId::new(99),
        meaning: b"undeclared",
    };
    assert!(matches!(
        register(
            binding(),
            &[AttackCase {
                atoms: &[atom],
                ..case
            }],
            limits(),
            &view
        ),
        Err(Error::InvalidMap)
    ));
}
#[test]
fn tlcx22_adversarial_zero_trial_unsupported_or_bad_response_cannot_hide() {
    let gamma = theory(&[], vec![]);
    let yes = BoolExpr::Constant(true);
    let weight = q(1, 1);
    let bad = BoolExpr::Equal(IntExpr::Constant(1), IntExpr::Constant(1));
    let case = AttackCase {
        key: b"case",
        atoms: &[],
        theory: &gamma,
        reference: &yes,
        weight: &weight,
    };
    let (_br, _run, res) = run(Cancellation::default());
    let view = res.lease();
    let mut l = limits();
    l.max_trials = 0;
    assert!(matches!(
        register(
            binding(),
            &[AttackCase {
                reference: &bad,
                ..case
            }],
            l,
            &view
        ),
        Err(Error::UnsupportedFragment)
    ));
    let catalog = register(binding(), &[case], l, &view).unwrap();
    assert!(matches!(
        bind_candidate(
            &catalog,
            candidate(1),
            &[Response {
                case: b"case",
                term: &bad
            }],
            &view
        ),
        Err(Error::UnsupportedFragment)
    ));
    assert!(matches!(
        bind_candidate(&catalog, candidate(1), &[], &view),
        Err(Error::BindingMismatch)
    ));
    let mut deep = yes.clone();
    for _ in 0..33 {
        deep = BoolExpr::Not(Box::new(deep));
    }
    assert!(matches!(
        bind_candidate(
            &catalog,
            candidate(1),
            &[Response {
                case: b"case",
                term: &deep
            }],
            &view
        ),
        Err(Error::AstLimit)
    ));
}
#[test]
fn tlcx22_adversarial_candidate_id_relabel_foreign_owner_and_cancel() {
    let gamma = theory(&[], vec![]);
    let yes = BoolExpr::Constant(true);
    let weight = q(1, 1);
    let case = AttackCase {
        key: b"case",
        atoms: &[],
        theory: &gamma,
        reference: &yes,
        weight: &weight,
    };
    let cancel = Cancellation::default();
    let (_br, _run, res) = run(cancel.clone());
    let view = res.lease();
    let catalog = register(binding(), &[case], limits(), &view).unwrap();
    let response = [Response {
        case: b"case",
        term: &yes,
    }];
    let b = candidate(1);
    let mut changed_context = b;
    changed_context.context = b.context.reconstructed([123; 32]);
    assert!(matches!(
        bind_candidate(&catalog, changed_context, &response, &view),
        Err(Error::BindingMismatch)
    ));
    let mut relabel = b;
    relabel.id = CandidateId(9999);
    let a = bind_candidate(&catalog, b, &response, &view).unwrap();
    let c = bind_candidate(&catalog, relabel, &response, &view).unwrap();
    assert_eq!(a.historical_digest(), c.historical_digest());
    let oa = execute(&a, &view).unwrap();
    let oc = execute(&c, &view).unwrap();
    assert_eq!(oa.historical_report(), oc.historical_report());
    assert!(matches!(
        oa.report_current(&c, &view),
        Err(Error::BindingMismatch)
    ));
    let (_br2, _run2, res2) = run(Cancellation::default());
    let view2 = res2.lease();
    assert!(matches!(
        execute(&a, &view2),
        Err(Error::ForeignReservation)
    ));
    cancel.cancel();
    assert!(oa.report_current(&a, &view).is_err());
}

#[test]
fn tlcx22_adversarial_oversized_borrowed_payloads_rejected() {
    let gamma = theory(&[7], vec![]);
    let reference = v(7);
    let weight = q(1, 1);
    let atoms = [SemanticAtom {
        symbol: BoolSymbolId::new(7),
        meaning: b"input",
    }];
    let case = AttackCase {
        key: b"case",
        atoms: &atoms,
        theory: &gamma,
        reference: &reference,
        weight: &weight,
    };
    let oversized = vec![b'x'; 65537];
    let (_br, _run, res) = run(Cancellation::default());
    let view = res.lease();
    assert!(matches!(
        register(
            binding(),
            &[AttackCase {
                key: &oversized,
                ..case
            }],
            limits(),
            &view
        ),
        Err(Error::InvalidCase)
    ));
    let large_atom = [SemanticAtom {
        meaning: &oversized,
        ..atoms[0]
    }];
    assert!(matches!(
        register(
            binding(),
            &[AttackCase {
                atoms: &large_atom,
                ..case
            }],
            limits(),
            &view
        ),
        Err(Error::InvalidCase)
    ));
    let catalog = register(binding(), &[case], limits(), &view).unwrap();
    assert!(matches!(
        bind_candidate(
            &catalog,
            candidate(1),
            &[Response {
                case: &oversized,
                term: &reference
            }],
            &view
        ),
        Err(Error::InvalidCase)
    ));
}
