//! Small independently authored executable controls; no source/owner lookup.
use super::{
    expected,
    wire::{app, encode, integer, symbol},
};
use lc631_runtime::product_cost::InlinePackage;
use lc631_substrate::semantic_term::Term;

pub fn identity_adapter() -> Vec<u8> {
    encode(&app(
        "program-v1",
        vec![
            symbol("adapter"),
            integer(2),
            app(
                "op",
                vec![
                    symbol("test-adapter-identity"),
                    app("arg", vec![integer(0)]),
                ],
            ),
        ],
    ))
}
pub fn wrong_inverse() -> Vec<u8> {
    encode(&app(
        "program-v1",
        vec![
            symbol("inverse"),
            integer(2),
            app("op", vec![symbol("test-return-false")]),
        ],
    ))
}
pub fn extended_runtime(original: &[u8]) -> Vec<u8> {
    let mut runtime = expected::decode(original);
    let operations = match &mut runtime {
        Term::Apply(name, v) if name == "runtime-v1" => v,
        _ => panic!("complete supplied runtime table"),
    };
    operations.push(app(
        "operation-v1",
        vec![
            symbol("test-adapter-identity"),
            integer(1),
            app("arg", vec![integer(0)]),
        ],
    ));
    operations.push(app(
        "operation-v1",
        vec![
            symbol("test-return-false"),
            integer(0),
            app("literal", vec![Term::Bool(false)]),
        ],
    ));
    operations.sort_by(|a, b| op_name(a).cmp(op_name(b)));
    encode(&runtime)
}
fn op_name(term: &Term) -> &str {
    match term {
        Term::Apply(name, v) if name == "operation-v1" => match &v[0] {
            Term::Symbol(name) => name,
            _ => panic!("operation name"),
        },
        _ => panic!("operation"),
    }
}
pub fn with_runtime<'a>(package: InlinePackage<'a>, runtime: &'a [u8]) -> InlinePackage<'a> {
    InlinePackage { runtime, ..package }
}
