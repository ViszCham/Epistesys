//! Paid case selection by full Given specification and canonical semantic rank.
use super::super::Term;
use super::dsl::*;
pub(super) fn helpers(case_rows: &[(usize, usize)]) -> Vec<Term> {
    let output = case_rows
        .iter()
        .enumerate()
        .map(|(slot, (spec, reference))| {
            call(
                "case",
                vec![a(0), a(1), dict(*spec), dict(*reference), n(slot as i128)],
            )
        })
        .collect();
    let mut h = vec![function(
        "cases",
        2,
        shape(
            a(0),
            "cases-v1",
            case_rows.len() as i128,
            mk("response-cases-v1", output),
        ),
    )];
    h.push(function(
        "case-rank",
        2,
        get(
            fold(
                children(a(0)),
                seq(vec![a(1), n(0)]),
                seq(vec![
                    get(a(0), 0),
                    bin(
                        "add",
                        get(a(0), 1),
                        iff(
                            bin("canonical-less", get(a(1), 7), get(a(0), 0)),
                            n(1),
                            n(0),
                        ),
                    ),
                ]),
            ),
            1,
        ),
    ));
    h.push(function(
        "find-case",
        2,
        call(
            "unique",
            vec![fold(
                children(a(0)),
                seq(vec![a(1), n(0), b(false)]),
                iff(eq(get(a(1), 7), get(a(0), 0)), matched(a(0), a(1)), a(0)),
            )],
        ),
    ));
    h.push(function(
        "atom-rank",
        2,
        get(
            fold(
                children(a(0)),
                seq(vec![a(1), n(0)]),
                seq(vec![
                    get(a(0), 0),
                    bin(
                        "add",
                        get(a(0), 1),
                        iff(bin("symbol-less", get(a(1), 2), get(a(0), 0)), n(1), n(0)),
                    ),
                ]),
            ),
            1,
        ),
    ));
    // Each actual normalized position must equal the UTF-8 meaning rank and the
    // full specification atom at that position; raw-symbol tokens are not ranks.
    h.push(function(
        "atom-check",
        3,
        shape(
            a(1),
            "atom-fields-v1",
            3,
            req(
                eq(get(a(1), 1), call("atom-rank", vec![a(0), get(a(1), 2)])),
                req(eq(at(a(2), get(a(1), 1)), get(a(1), 2)), b(true)),
            ),
        ),
    ));
    h.push(function(
        "atoms-check",
        2,
        req(
            eq(unary("arity", a(0)), unary("arity", a(1))),
            get(
                fold(
                    children(a(0)),
                    seq(vec![seq(vec![a(0), a(1)]), b(true)]),
                    seq(vec![
                        get(a(0), 0),
                        call(
                            "atom-check",
                            vec![get(get(a(0), 0), 0), a(1), get(get(a(0), 0), 1)],
                        ),
                    ]),
                ),
                1,
            ),
        ),
    ));
    h.push(function(
        "case",
        5,
        req(
            eq(call("case-rank", vec![a(0), get(a(2), 2)]), a(4)),
            call(
                "case-build",
                vec![
                    call("find-case", vec![a(0), get(a(2), 2)]),
                    a(1),
                    a(2),
                    a(3),
                    a(4),
                ],
            ),
        ),
    ));
    let out = mk(
        "case-slot-v1",
        vec![
            a(4),
            get(a(2), 2),
            call("boolean", vec![get(a(0), 4), get(get(a(2), 2), 1)]),
        ],
    );
    let checks = [
        eq(
            call("binding", vec![get(a(0), 0), s("CaseSpec"), a(1)]),
            a(2),
        ),
        eq(get(a(2), 0), s("CaseSpec")),
        eq(get(a(3), 0), s("CaseReference")),
        eq(get(get(a(2), 2), 0), s("BooleanReferenceAgreement")),
        eq(get(a(0), 5), get(get(a(2), 2), 3)),
        eq(get(a(0), 6), get(get(a(2), 2), 4)),
        call("atoms-check", vec![get(a(0), 1), get(get(a(2), 2), 1)]),
        eq(
            call("theory", vec![get(a(0), 2), get(get(a(2), 2), 1)]),
            get(get(a(2), 2), 2),
        ),
        eq(
            call("boolean", vec![get(a(0), 3), get(get(a(2), 2), 1)]),
            get(a(3), 2),
        ),
    ];
    let checked = checks.into_iter().rev().fold(out, |out, p| req(p, out));
    h.push(function(
        "case-build",
        5,
        shape(a(0), "case-fields-v1", 8, checked),
    ));
    h
}
