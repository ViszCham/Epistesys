//! Independent literal K6 encoder for expected fixture terms and whole-bundle counts.
use lc631_substrate::{measurement_numeric::BigInt, semantic_term::Term};

pub fn symbol(s: &str) -> Term {
    Term::Symbol(s.to_owned())
}
pub fn app(s: &str, children: Vec<Term>) -> Term {
    Term::Apply(s.to_owned(), children)
}
pub fn integer(n: i64) -> Term {
    Term::Integer(n.into())
}
pub fn encode(term: &Term) -> Vec<u8> {
    let mut out = Vec::new();
    append(term, &mut out);
    out
}
fn uleb(mut n: usize, out: &mut Vec<u8>) {
    loop {
        let low = (n & 127) as u8;
        n >>= 7;
        out.push(if n == 0 { low } else { low | 128 });
        if n == 0 {
            break;
        }
    }
}
fn append(term: &Term, out: &mut Vec<u8>) {
    match term {
        Term::Bool(v) => out.extend_from_slice(&[1, u8::from(*v)]),
        Term::Integer(v) => {
            out.push(2);
            out.push(u8::from(v < &BigInt::from(0)));
            let (_, mut bytes) = v.to_bytes_le();
            if bytes.is_empty() {
                bytes.push(0);
            }
            uleb(bytes.len(), out);
            out.extend_from_slice(&bytes);
        }
        Term::Symbol(s) => {
            out.push(3);
            uleb(s.len(), out);
            out.extend_from_slice(s.as_bytes());
        }
        Term::Apply(name, children) => {
            out.push(4);
            append(&symbol(name), out);
            uleb(children.len(), out);
            for child in children {
                append(child, out);
            }
        }
        _ => panic!("fixture intentionally excludes binders and bound variables"),
    }
}
pub fn literal_goldens() {
    assert_eq!(encode(&Term::Bool(false)), [1, 0]);
    assert_eq!(encode(&symbol("a")), [3, 1, b'a']);
    assert_eq!(encode(&integer(0)), [2, 0, 1, 0]);
    assert_eq!(
        encode(&app("f", vec![Term::Bool(false)])),
        [4, 3, 1, b'f', 1, 1, 0]
    );
    assert_eq!(
        encode(&app("bundle-v1", vec![Term::Bool(false); 3])).len(),
        19
    );
}
