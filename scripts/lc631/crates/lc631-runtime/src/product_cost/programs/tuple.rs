//! Charged temporal and spatial construction from actual native field records.
use super::super::Term;
use super::dsl::*;
pub(super) fn helpers(time_rows: &[usize]) -> Vec<Term> {
    let mut h = vec![function(
        "time-atoms",
        0,
        seq(time_rows.iter().map(|i| get(dict(*i), 2)).collect()),
    )];
    h.push(function(
        "clock",
        2,
        call(
            "clock-build",
            vec![
                a(0),
                call("identity", vec![get(a(0), 0), s("Clock"), a(1)]),
                call("identity", vec![get(a(0), 2), s("Origin"), a(1)]),
                call("identity", vec![get(a(0), 3), s("Reference"), a(1)]),
            ],
        ),
    ));
    let clock = mk(
        "clock-v1",
        vec![
            get(a(1), 0),
            get(a(1), 1),
            mk("origin-v1", vec![get(a(2), 0), get(a(2), 1)]),
            mk("reference-v1", vec![get(a(3), 0), get(a(3), 1)]),
        ],
    );
    h.push(function(
        "clock-build",
        4,
        shape(
            a(0),
            "clock-fields-v1",
            5,
            req(
                eq(get(a(0), 1), get(get(get(a(1), 1), 1), 0)),
                req(
                    eq(get(a(0), 4), n(0)),
                    req(eq(get(get(get(a(1), 1), 1), 1), get(a(0), 4)), clock),
                ),
            ),
        ),
    ));
    h.push(function(
        "steps",
        4,
        get(
            fold(
                children(a(0)),
                seq(vec![seq(vec![a(1), a(2), a(3)]), seq(vec![])]),
                pushed(
                    get(a(0), 0),
                    get(a(0), 1),
                    call("step", vec![a(1), get(a(0), 0)]),
                ),
            ),
            1,
        ),
    ));
    h.push(function(
        "step",
        2,
        shape(
            a(0),
            "step-fields-v1",
            3,
            req(
                eq(get(a(0), 1), get(a(1), 0)),
                req(
                    eq(unary("arity", get(a(0), 2)), get(a(1), 2)),
                    mk("step-v1", vec![get(a(0), 0), get(a(1), 1), get(a(0), 2)]),
                ),
            ),
        ),
    ));
    h.push(function(
        "temporal",
        2,
        call(
            "temporal-build",
            vec![
                a(0),
                call("clock", vec![get(a(0), 5), a(1)]),
                call("time-atoms", vec![]),
            ],
        ),
    ));
    let temporal = mk(
        "temporal-model-v1",
        vec![
            get(a(0), 0),
            a(1),
            make(s("atoms-v1"), a(2)),
            call("lower", vec![get(a(0), 1), get(a(0), 2), a(2)]),
            make(
                s("closed-trace-v1"),
                call(
                    "steps",
                    vec![
                        get(a(0), 6),
                        get(get(a(0), 5), 0),
                        get(a(1), 0),
                        get(a(0), 3),
                    ],
                ),
            ),
        ],
    );
    h.push(function(
        "temporal-build",
        3,
        shape(
            a(0),
            "temporal-fields-v1",
            7,
            req(
                eq(get(a(0), 0), s("ClosedFiniteKleeneV1")),
                req(
                    eq(get(a(0), 4), b(true)),
                    req(eq(get(a(0), 3), unary("arity", a(2))), temporal),
                ),
            ),
        ),
    ));
    h.push(function(
        "spatial",
        2,
        call(
            "spatial-build",
            vec![a(0), call("identity", vec![get(a(0), 0), s("Frame"), a(1)])],
        ),
    ));
    let spatial = mk(
        "spatial-pair-v1",
        vec![
            mk("frame-v1", vec![get(a(1), 0), get(a(1), 1)]),
            get(a(0), 1),
            call("rectangle", vec![get(a(0), 2), get(a(0), 0), get(a(1), 0)]),
            call("rectangle", vec![get(a(0), 3), get(a(0), 0), get(a(1), 0)]),
        ],
    );
    h.push(function(
        "spatial-build",
        2,
        shape(
            a(0),
            "spatial-fields-v1",
            4,
            req(eq(get(a(0), 1), get(get(get(a(1), 1), 1), 0)), spatial),
        ),
    ));
    h.push(function(
        "rectangle",
        3,
        iff(
            eq(unary("operator", a(0)), s("none-v1")),
            shape(a(0), "none-v1", 0, mk("missing-rectangle-v1", vec![])),
            shape(
                a(0),
                "some-v1",
                1,
                call("rectangle-present", vec![get(a(0), 0), a(1), a(2)]),
            ),
        ),
    ));
    h.push(function(
        "rectangle-present",
        3,
        shape(
            a(0),
            "rectangle-fields-v1",
            5,
            req(
                eq(get(a(0), 0), a(1)),
                mk(
                    "rect-v1",
                    vec![a(2), get(a(0), 1), get(a(0), 2), get(a(0), 3), get(a(0), 4)],
                ),
            ),
        ),
    ));
    h
}
