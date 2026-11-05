use lc631_substrate::measurement_numeric::BigInt;
use lc631_substrate::semantic_term::{
    bundle_bits, decode, encode, encoded_len, BinderKind, Term, TermLimits,
};

#[test]
fn hand_hex_goldens_and_all_tags() {
    let cases = [
        (Term::Bool(false), vec![1, 0]),
        (Term::Symbol("a".into()), vec![3, 1, 97]),
        (Term::Integer(BigInt::from(0)), vec![2, 0, 1, 0]),
        (
            Term::Apply("f".into(), vec![Term::Bool(false)]),
            vec![4, 3, 1, 102, 1, 1, 0],
        ),
        (
            Term::Binder(BinderKind::Lambda, "s".into(), Box::new(Term::BoundVar(0))),
            vec![5, 2, 3, 1, 115, 6, 0],
        ),
        (Term::Integer(BigInt::from(-256)), vec![2, 1, 2, 0, 1]),
    ];
    for (term, gold) in cases {
        assert_eq!(
            encoded_len(&term, TermLimits::default()).unwrap(),
            gold.len()
        );
        assert_eq!(encode(&term, TermLimits::default()).unwrap(), gold);
        assert_eq!(decode(&gold, TermLimits::default()).unwrap(), term);
    }
}

#[test]
fn malformed_never_becomes_an_empty_term() {
    for bad in [
        vec![],
        vec![1, 2],
        vec![1, 0, 0],
        vec![7],
        vec![3, 0x81, 0, 97],
        vec![3, 1, 255],
        vec![2, 2, 1, 1],
        vec![2, 1, 1, 0],
        vec![2, 0, 0],
        vec![2, 0, 2, 1, 0],
        vec![6, 0],
        vec![4, 1, 0, 0],
        vec![5, 3, 3, 0, 1, 0],
        vec![5, 0, 3, 0, 6, 1],
    ] {
        assert!(decode(&bad, TermLimits::default()).is_err(), "{bad:?}");
    }
}

#[test]
fn exact_order_unicode_and_budgets() {
    let a = Term::Apply("f".into(), vec![Term::Symbol("é".into()), Term::Bool(true)]);
    let b = Term::Apply("f".into(), vec![Term::Bool(true), Term::Symbol("é".into())]);
    let bytes = encode(&a, TermLimits::default()).unwrap();
    assert_ne!(bytes, encode(&b, TermLimits::default()).unwrap());
    assert!(decode(
        &bytes,
        TermLimits {
            max_nodes: 2,
            ..TermLimits::default()
        }
    )
    .is_err());
    assert!(decode(
        &bytes,
        TermLimits {
            max_bytes: bytes.len() - 1,
            ..TermLimits::default()
        }
    )
    .is_err());
    assert!(decode(
        &bytes,
        TermLimits {
            max_depth: 1,
            ..TermLimits::default()
        }
    )
    .is_err());
    assert_ne!(
        encode(&Term::Symbol("é".into()), TermLimits::default()).unwrap(),
        encode(&Term::Symbol("e\u{301}".into()), TermLimits::default()).unwrap()
    );
    assert_eq!(
        bundle_bits(
            &Term::Bool(false),
            &Term::Bool(false),
            &Term::Bool(false),
            TermLimits::default()
        )
        .unwrap(),
        152
    );
    // Hand ULEB length 128 = 80 01; independent literal assembly.
    let mut long = vec![3, 128, 1];
    long.extend([b'x'; 128]);
    assert_eq!(
        decode(&long, TermLimits::default()).unwrap(),
        Term::Symbol("x".repeat(128))
    );
    assert_eq!(
        encode(&Term::Symbol("x".repeat(128)), TermLimits::default()).unwrap(),
        long
    );
    let unbound = Term::Binder(BinderKind::Forall, "s".into(), Box::new(Term::BoundVar(1)));
    assert!(encode(&unbound, TermLimits::default()).is_err());
}
