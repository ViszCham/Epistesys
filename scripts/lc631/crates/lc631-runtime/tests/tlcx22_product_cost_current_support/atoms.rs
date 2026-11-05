//! Two independently specified cases; raw symbol order differs from meaning order.
use super::wire::{app, encode, symbol};
use lc631_runtime::adversarial_measurement::SemanticAtom;
use lc631_solvers::typed_query::{BoolExpr, BoolSymbolId, SmtEmitLimits, SmtStatusQuery};
use lc631_substrate::semantic_term::Term;

pub struct CaseInput {
    pub key: &'static [u8],
    pub atoms: Vec<SemanticAtom<'static>>,
    pub theory: SmtStatusQuery,
    pub reference: BoolExpr,
    pub baseline: BoolExpr,
    pub target: BoolExpr,
    pub spec: Vec<u8>,
    pub contract: Vec<u8>,
    pub semantic_spec: Term,
    pub active_meaning: String,
}
pub fn inputs(renamed: bool, single: bool, changed_unused: bool) -> [CaseInput; 2] {
    let first = BoolSymbolId::new(if renamed { 71 } else { 91 });
    let unused = BoolSymbolId::new(if renamed { 12 } else { 17 });
    let other = BoolSymbolId::new(if renamed { 38 } else { 52 });
    let other_unused = BoolSymbolId::new(if renamed { 83 } else { 3 });
    let unused_meaning: &'static [u8] = if changed_unused { b"zeta" } else { b"beta" };
    let first_atoms = if single {
        vec![SemanticAtom {
            symbol: first,
            meaning: b"alpha",
        }]
    } else {
        vec![
            SemanticAtom {
                symbol: unused,
                meaning: unused_meaning,
            },
            SemanticAtom {
                symbol: first,
                meaning: b"alpha",
            },
        ]
    };
    let other_atoms = if single {
        vec![SemanticAtom {
            symbol: other,
            meaning: b"gamma",
        }]
    } else {
        vec![
            SemanticAtom {
                symbol: other,
                meaning: b"gamma",
            },
            SemanticAtom {
                symbol: other_unused,
                meaning: b"delta",
            },
        ]
    };
    let first_meanings = if single {
        vec!["alpha"]
    } else if changed_unused {
        vec!["alpha", "zeta"]
    } else {
        vec!["alpha", "beta"]
    };
    let other_meanings = if single {
        vec!["gamma"]
    } else {
        vec!["delta", "gamma"]
    };
    [
        case(
            b"zeta-native-key",
            first_atoms,
            first,
            "alpha",
            &first_meanings,
            false,
        ),
        case(
            b"alpha-native-key",
            other_atoms,
            other,
            "gamma",
            &other_meanings,
            true,
        ),
    ]
}
fn case(
    key: &'static [u8],
    atoms: Vec<SemanticAtom<'static>>,
    active: BoolSymbolId,
    active_meaning: &str,
    sorted_meanings: &[&str],
    has_true_assertion: bool,
) -> CaseInput {
    let reference = BoolExpr::Variable(active);
    let baseline = BoolExpr::Not(Box::new(reference.clone()));
    let target = reference.clone();
    let assertions = if has_true_assertion {
        vec![BoolExpr::Constant(true)]
    } else {
        vec![]
    };
    let theory = SmtStatusQuery::checked(
        atoms.iter().map(|a| a.symbol).collect(),
        vec![],
        assertions,
        SmtEmitLimits::default(),
    )
    .unwrap();
    let semantic_spec = app(
        "stage-b-case-spec-v1",
        vec![
            symbol("BooleanReferenceAgreement"),
            app(
                "atoms-v1",
                sorted_meanings.iter().map(|s| symbol(s)).collect(),
            ),
            app(
                "theory-v1",
                if has_true_assertion {
                    vec![Term::Bool(true)]
                } else {
                    vec![]
                },
            ),
            symbol("SoftFiniteValidation"),
            symbol("AllAdmissibleAssignments"),
        ],
    );
    let spec = encode(&semantic_spec);
    let contract = encode(&app("atom-v1", vec![symbol(active_meaning)]));
    CaseInput {
        key,
        atoms,
        theory,
        reference,
        baseline,
        target,
        spec,
        contract,
        semantic_spec,
        active_meaning: active_meaning.to_owned(),
    }
}
pub fn hand_oracle() {
    let rows = [
        (false, false, true),
        (false, true, true),
        (true, false, false),
        (true, true, false),
    ];
    // first field is alpha/gamma; second unused atom; last is Not(active).
    let mut failures = 0;
    for (active, _unused, baseline) in rows {
        failures += usize::from(active != baseline);
    }
    assert_eq!(failures, 4);
}
