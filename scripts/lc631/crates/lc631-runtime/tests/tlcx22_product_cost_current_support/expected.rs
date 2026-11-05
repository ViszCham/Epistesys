//! Complete independently authored fixture product/declaration gold, not runtime projection.
use super::{
    atoms::CaseInput,
    wire::{app, encode, integer, symbol},
};
use lc631_substrate::semantic_term::{self, Term, TermLimits};

fn meaning(path: &str, payload: Term) -> Term {
    app("identity-meaning-v1", vec![symbol(path), payload])
}
fn clock_meaning(secondary: bool) -> Term {
    meaning(
        if secondary {
            "Clock/secondary"
        } else {
            "Clock/primary"
        },
        app(
            "clock-meaning-v1",
            vec![symbol("Monotonic"), integer(0), symbol("logical-clock")],
        ),
    )
}
fn origin_meaning() -> Term {
    meaning("Origin/primary", symbol("epoch-zero"))
}
fn reference_meaning() -> Term {
    meaning("Reference/primary", symbol("first-event"))
}
fn frame_meaning() -> Term {
    meaning(
        "Frame/primary",
        app(
            "frame-meaning-v1",
            vec![
                symbol("Pixel"),
                symbol("x-right"),
                symbol("y-up"),
                symbol("right-handed"),
                integer(0),
                integer(0),
            ],
        ),
    )
}
pub fn sorted_cases(cases: &[CaseInput; 2]) -> Vec<&CaseInput> {
    let mut cases: Vec<_> = cases.iter().collect();
    cases.sort_by(|a, b| a.spec.cmp(&b.spec));
    cases
}
pub fn product(cases: &[CaseInput; 2], candidate: bool, secondary_clock: bool) -> Vec<u8> {
    let responses = sorted_cases(cases)
        .iter()
        .enumerate()
        .map(|(slot, case)| {
            let atom = app("atom-v1", vec![symbol(&case.active_meaning)]);
            app(
                "case-slot-v1",
                vec![
                    integer(slot as i64),
                    case.semantic_spec.clone(),
                    if candidate {
                        atom
                    } else {
                        app("not-v1", vec![atom])
                    },
                ],
            )
        })
        .collect();
    let clock_role = i64::from(secondary_clock);
    let clock = app(
        "clock-v1",
        vec![
            integer(clock_role),
            clock_meaning(secondary_clock),
            app("origin-v1", vec![integer(0), origin_meaning()]),
            app("reference-v1", vec![integer(0), reference_meaning()]),
        ],
    );
    let temporal = app(
        "temporal-model-v1",
        vec![
            symbol("ClosedFiniteKleeneV1"),
            clock,
            app("atoms-v1", vec![symbol("observed-p")]),
            app("atom-v1", vec![symbol("observed-p")]),
            app(
                "closed-trace-v1",
                (0..2)
                    .map(|n| {
                        app(
                            "step-v1",
                            vec![
                                integer(n * 1_000_000_000),
                                integer(clock_role),
                                app(
                                    "truths-v1",
                                    vec![symbol(if candidate { "False" } else { "True" })],
                                ),
                            ],
                        )
                    })
                    .collect(),
            ),
        ],
    );
    let a = app(
        "rect-v1",
        vec![integer(0), integer(0), integer(0), integer(2), integer(2)],
    );
    let b = app(
        "rect-v1",
        vec![integer(0), integer(4), integer(0), integer(6), integer(2)],
    );
    let (left, right) = if candidate { (b, a) } else { (a, b) };
    let spatial = app(
        "spatial-pair-v1",
        vec![
            app("frame-v1", vec![integer(0), frame_meaning()]),
            symbol("Pixel"),
            left,
            right,
        ],
    );
    encode(&app(
        "tlcx-finite-product-v1",
        vec![app("response-cases-v1", responses), temporal, spatial],
    ))
}

fn declaration(kind: &str, path: &str, value: Term) -> Term {
    app("declaration-v1", vec![symbol(kind), symbol(path), value])
}
pub fn declarations(cases: &[CaseInput; 2]) -> Term {
    let mut table: Vec<(String, String, Term)> = Vec::new();
    for (slot, case) in sorted_cases(cases).iter().enumerate() {
        let base = format!("Case/{slot}");
        table.push((
            "CaseSpec".into(),
            format!("{base}/Spec"),
            case.semantic_spec.clone(),
        ));
        table.push((
            "CaseReference".into(),
            format!("{base}/Reference"),
            app("atom-v1", vec![symbol(&case.active_meaning)]),
        ));
        let mut atoms: Vec<_> = case.atoms.iter().collect();
        atoms.sort_by(|a, b| a.meaning.cmp(b.meaning));
        for (index, atom) in atoms.iter().enumerate() {
            table.push((
                "BooleanAtom".into(),
                format!("{base}/Atom/{index}"),
                symbol(std::str::from_utf8(atom.meaning).unwrap()),
            ));
        }
    }
    for (kind, path, value) in [
        ("Clock", "Clock/primary", clock_meaning(false)),
        ("Clock", "Clock/secondary", clock_meaning(true)),
        ("Origin", "Origin/primary", origin_meaning()),
        ("Reference", "Reference/primary", reference_meaning()),
        ("Frame", "Frame/primary", frame_meaning()),
        ("TemporalAtom", "Temporal/Atom/0", symbol("observed-p")),
    ] {
        table.push((kind.into(), path.into(), value));
    }
    table.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
    app(
        "declarations-v1",
        table
            .into_iter()
            .map(|(kind, path, value)| declaration(&kind, &path, value))
            .collect(),
    )
}
pub fn dictionary(declarations: Term) -> Vec<u8> {
    encode(&app(
        "dictionary-v1",
        vec![
            declarations,
            app("constants-v1", vec![]),
            app("helper-data-v1", vec![]),
            app("residuals-v1", vec![]),
        ],
    ))
}
pub fn decode(bytes: &[u8]) -> Term {
    semantic_term::decode(
        bytes,
        TermLimits {
            max_bytes: 262_144,
            max_nodes: 16_384,
            max_depth: 64,
        },
    )
    .unwrap()
}
pub fn independent_count(bundle: &[u8]) -> u64 {
    // Decode is the existing stable K6 parser; encoding/count is independent.
    let term = decode(bundle);
    let encoded = encode(&term);
    assert_eq!(encoded, bundle);
    u64::try_from(encoded.len())
        .unwrap()
        .checked_mul(8)
        .unwrap()
}
