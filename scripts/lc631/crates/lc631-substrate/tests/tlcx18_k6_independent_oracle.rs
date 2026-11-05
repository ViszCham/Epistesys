//! Independent small encoder for the fixed K6 wire specification. This oracle
//! does not call the production writer or use its computed sizes as a golden.
use lc631_substrate::measurement_numeric::BigInt;
use lc631_substrate::semantic_term::{bundle_bits, decode, encode, BinderKind, Term, TermLimits};

fn put_uleb(mut value: usize, bytes: &mut Vec<u8>) {
    loop {
        let low = (value & 0x7f) as u8;
        value >>= 7;
        bytes.push(if value == 0 { low } else { low | 0x80 });
        if value == 0 {
            break;
        }
    }
}

fn reference(term: &Term, bytes: &mut Vec<u8>) {
    match term {
        Term::Bool(value) => bytes.extend([0x01, u8::from(*value)]),
        Term::Integer(value) => {
            let (sign, mut magnitude) = value.to_bytes_le();
            if magnitude.is_empty() {
                magnitude.push(0);
            }
            bytes.extend([0x02, u8::from(sign == num_bigint::Sign::Minus)]);
            put_uleb(magnitude.len(), bytes);
            bytes.extend(magnitude);
        }
        Term::Symbol(name) => {
            bytes.push(0x03);
            put_uleb(name.len(), bytes);
            bytes.extend(name.as_bytes());
        }
        Term::Apply(operator, children) => {
            bytes.push(0x04);
            reference(&Term::Symbol(operator.clone()), bytes);
            put_uleb(children.len(), bytes);
            for child in children {
                reference(child, bytes);
            }
        }
        Term::Binder(kind, sort, body) => {
            bytes.extend([
                0x05,
                match kind {
                    BinderKind::Forall => 0,
                    BinderKind::Exists => 1,
                    BinderKind::Lambda => 2,
                },
            ]);
            reference(&Term::Symbol(sort.clone()), bytes);
            reference(body, bytes);
        }
        Term::BoundVar(index) => {
            bytes.push(0x06);
            put_uleb(*index, bytes);
        }
    }
}

#[test]
fn independent_k6_encoder_matches_nested_terms_and_bundle_cost() {
    let leaves = vec![
        Term::Bool(false),
        Term::Bool(true),
        Term::Integer(BigInt::from(-65_536)),
        Term::Integer(BigInt::from(0)),
        Term::Integer(BigInt::from(128)),
        Term::Symbol("é".into()),
        Term::Symbol("e\u{301}".into()),
        Term::Symbol("x".repeat(128)),
    ];
    for (index, left) in leaves.iter().enumerate() {
        for right in &leaves {
            let term = Term::Binder(
                [BinderKind::Forall, BinderKind::Exists, BinderKind::Lambda][index % 3],
                "sort".into(),
                Box::new(Term::Apply(
                    "pair".into(),
                    vec![left.clone(), right.clone(), Term::BoundVar(0)],
                )),
            );
            let mut expected = Vec::new();
            reference(&term, &mut expected);
            assert_eq!(encode(&term, TermLimits::default()).unwrap(), expected);
            assert_eq!(decode(&expected, TermLimits::default()).unwrap(), term);
            let bundle = Term::Apply(
                "bundle-v1".into(),
                vec![term.clone(), Term::Bool(false), Term::Bool(false)],
            );
            let mut expected_bundle = Vec::new();
            reference(&bundle, &mut expected_bundle);
            assert_eq!(
                bundle_bits(
                    &term,
                    &Term::Bool(false),
                    &Term::Bool(false),
                    TermLimits::default()
                )
                .unwrap(),
                (expected_bundle.len() * 8) as u64
            );
        }
    }
}
