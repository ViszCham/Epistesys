//! Paid dictionary joins and matching-only native admission binding traversal.
use super::super::{
    term::{app, int},
    Term,
};
use super::dsl::*;
pub(super) fn helpers(count: usize) -> Vec<Term> {
    let mut h = vec![function("drows", 0, seq((0..count).map(dict).collect()))];
    for i in 0..8 {
        h.push(function(&format!("field{i}"), 1, at(a(0), n(i))));
    }
    h.push(function(
        "has-shape",
        3,
        req(
            eq(unary("operator", a(0)), a(1)),
            eq(unary("arity", a(0)), a(2)),
        ),
    ));
    h.push(function(
        "unique",
        1,
        req(eq(get(a(0), 1), n(1)), get(a(0), 2)),
    ));
    // Fold accumulator [context, match-count, full-result]. No absent->false.
    let predicate = iff(
        eq(get(a(1), 0), get(get(a(0), 0), 0)),
        eq(get(a(1), 1), get(get(a(0), 0), 1)),
        b(false),
    );
    h.push(function(
        "lookup",
        2,
        call(
            "unique",
            vec![fold(
                call("drows", vec![]),
                seq(vec![seq(vec![a(0), a(1)]), n(0), b(false)]),
                iff(predicate, matched(a(0), a(1)), a(0)),
            )],
        ),
    ));
    h.push(function(
        "count-equal",
        2,
        get(
            fold(
                children(a(0)),
                seq(vec![a(1), n(0)]),
                seq(vec![
                    get(a(0), 0),
                    bin("add", get(a(0), 1), iff(eq(a(1), get(a(0), 0)), n(1), n(0))),
                ]),
            ),
            1,
        ),
    ));
    h.push(function(
        "check-declaration",
        2,
        req(eq(call("count-equal", vec![a(0), a(1)]), n(1)), b(true)),
    ));
    h.push(function(
        "check-declarations",
        1,
        req(
            eq(unary("arity", a(0)), n(count as i128)),
            get(
                fold(
                    call("drows", vec![]),
                    seq(vec![a(0), b(true)]),
                    seq(vec![
                        get(a(0), 0),
                        call("check-declaration", vec![get(a(0), 0), a(1)]),
                    ]),
                ),
                1,
            ),
        ),
    ));
    // Filter semantic kind before comparing opaque tokens. The other kind branch
    // never evaluates equal(token,token), including Case versus Clock.
    let binding_pred = iff(
        eq(get(a(1), 1), get(get(a(0), 0), 1)),
        eq(get(a(1), 0), get(get(a(0), 0), 0)),
        b(false),
    );
    h.push(function(
        "binding",
        3,
        call(
            "binding-check",
            vec![call(
                "unique",
                vec![fold(
                    children(a(2)),
                    seq(vec![seq(vec![a(0), a(1)]), n(0), b(false)]),
                    iff(
                        binding_pred,
                        matched(a(0), row(get(a(1), 1), get(a(1), 2), get(a(1), 3))),
                        a(0),
                    ),
                )],
            )],
        ),
    ));
    h.push(function(
        "binding-check",
        1,
        req(
            eq(a(0), call("lookup", vec![get(a(0), 0), get(a(0), 1)])),
            a(0),
        ),
    ));
    let roles = app(
        "seq",
        [
            ("Case/0/Spec", 0),
            ("Case/1/Spec", 1),
            ("Clock/primary", 0),
            ("Clock/secondary", 1),
            ("Origin/primary", 0),
            ("Reference/primary", 0),
            ("Frame/primary", 0),
            ("Frame/secondary", 1),
        ]
        .into_iter()
        .map(|(path, index)| app("seq", vec![super::super::term::sym(path), int(index)]))
        .collect(),
    );
    h.push(function(
        "role-index",
        1,
        call(
            "unique",
            vec![fold(
                l(roles),
                seq(vec![a(0), n(0), b(false)]),
                iff(
                    eq(get(a(1), 0), get(a(0), 0)),
                    matched(a(0), get(a(1), 1)),
                    a(0),
                ),
            )],
        ),
    ));
    h.push(function(
        "identity-row",
        1,
        seq(vec![call("role-index", vec![get(a(0), 1)]), get(a(0), 2)]),
    ));
    h.push(function(
        "identity",
        3,
        call(
            "identity-row",
            vec![call("binding", vec![a(0), a(1), a(2)])],
        ),
    ));
    h
}
pub(super) fn runtime() -> Term {
    let indices = app("seq", (0..32).map(int).collect());
    let accumulator = seq(vec![
        get(a(0), 0),
        iff(
            bin("less", a(1), unary("arity", get(a(0), 0))),
            cat(get(a(0), 1), seq(vec![at(get(a(0), 0), a(1))])),
            get(a(0), 1),
        ),
    ]);
    let children_body = req(
        bin("less", unary("arity", a(0)), n(33)),
        get(
            fold(l(indices), seq(vec![a(0), seq(vec![])]), accumulator),
            1,
        ),
    );
    app(
        "runtime-v1",
        vec![
            operation("children32", 1, children_body),
            operation("copy", 1, a(0)),
            operation("native", 2, call("product", vec![a(0), a(1)])),
        ],
    )
}
