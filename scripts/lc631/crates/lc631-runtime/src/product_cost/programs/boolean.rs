//! Serialized ordered AST folds; native postorder indices are checked accesses.
use super::super::{
    term::{app, int, sym},
    Term,
};
use super::dsl::*;
pub(super) fn helpers() -> Vec<Term> {
    let mut h = vec![];
    h.push(function(
        "edge-list",
        2,
        get(
            fold(
                children(a(0)),
                seq(vec![a(1), seq(vec![])]),
                pushed(get(a(0), 0), get(a(0), 1), at(get(a(0), 0), a(1))),
            ),
            1,
        ),
    ));
    // The complete mapping is inline charged data, traversed by ordinary Expr.
    let mappings = [
        ("bool-not-v1", "not-v1", 1, "edges"),
        ("time-not-v1", "not-v1", 1, "edges"),
        ("time-strong-next-v1", "strong-next-v1", 1, "edges"),
        ("time-eventually-v1", "eventually-v1", 1, "edges"),
        ("time-always-v1", "always-v1", 1, "edges"),
        ("time-and-v1", "and-v1", 2, "edges"),
        ("time-or-v1", "or-v1", 2, "edges"),
        ("time-until-v1", "until-v1", 2, "edges"),
        ("bool-and-v1", "and-v1", 1, "multi"),
        ("bool-or-v1", "or-v1", 1, "multi"),
        ("bool-variable-v1", "atom-v1", 1, "atom"),
        ("time-atom-v1", "atom-v1", 1, "atom"),
        ("bool-constant-v1", "constant", 1, "constant"),
        ("time-constant-v1", "constant", 1, "constant"),
    ];
    let table = app(
        "seq",
        mappings
            .into_iter()
            .map(|(native, semantic, arity, mode)| {
                app(
                    "seq",
                    vec![sym(native), sym(semantic), int(arity), sym(mode)],
                )
            })
            .collect(),
    );
    h.push(function(
        "node-meta",
        1,
        call(
            "unique",
            vec![fold(
                l(table),
                seq(vec![a(0), n(0), b(false)]),
                iff(eq(get(a(1), 0), get(a(0), 0)), matched(a(0), a(1)), a(0)),
            )],
        ),
    ));
    let node = iff(
        eq(get(a(3), 3), s("constant")),
        req(eq(unary("tag", get(a(0), 0)), s("Bool")), get(a(0), 0)),
        iff(
            eq(get(a(3), 3), s("atom")),
            mk("atom-v1", vec![at(a(2), get(a(0), 0))]),
            make(
                get(a(3), 1),
                call(
                    "edge-list",
                    vec![iff(eq(get(a(3), 3), s("multi")), get(a(0), 0), a(0)), a(1)],
                ),
            ),
        ),
    );
    h.push(function(
        "node-build",
        4,
        req(eq(unary("arity", a(0)), get(a(3), 2)), node),
    ));
    h.push(function(
        "node",
        3,
        call(
            "node-build",
            vec![
                a(0),
                a(1),
                a(2),
                call("node-meta", vec![unary("operator", a(0))]),
            ],
        ),
    ));
    h.push(function("select-root", 2, at(get(a(0), 1), a(1))));
    h.push(function(
        "lower",
        3,
        call(
            "select-root",
            vec![
                fold(
                    children(a(0)),
                    seq(vec![a(2), seq(vec![])]),
                    pushed(
                        get(a(0), 0),
                        get(a(0), 1),
                        call("node", vec![a(1), get(a(0), 1), get(a(0), 0)]),
                    ),
                ),
                a(1),
            ],
        ),
    ));
    h.push(function(
        "boolean",
        2,
        shape(
            a(0),
            "bool-ast-v1",
            2,
            call("lower", vec![get(a(0), 0), get(a(0), 1), children(a(1))]),
        ),
    ));
    h.push(function(
        "theory",
        2,
        make(
            s("theory-v1"),
            get(
                fold(
                    children(a(0)),
                    seq(vec![a(1), seq(vec![])]),
                    pushed(
                        get(a(0), 0),
                        get(a(0), 1),
                        call("boolean", vec![a(1), get(a(0), 0)]),
                    ),
                ),
                1,
            ),
        ),
    ));
    h
}
