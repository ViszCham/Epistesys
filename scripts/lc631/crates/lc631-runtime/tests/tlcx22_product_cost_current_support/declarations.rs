//! Full paid semantic inventory cuts, including unused declarations and fixed slots.
use super::{
    atoms, expected,
    native::{self, Options},
    wire,
};
use lc631_runtime::{
    product_correspondence::inspect_native_current,
    product_cost::{
        capture_baseline_current, declare_cost_profile, stage_b_copy_pack_package_v1, CostError,
        CostGap, InlinePackage, ProfileSpec,
    },
};
use lc631_substrate::semantic_term::Term;

fn entries(term: &mut Term) -> &mut Vec<Term> {
    match term {
        Term::Apply(name, entries) if name == "declarations-v1" => entries,
        _ => panic!("independent declaration table"),
    }
}
fn row(term: &mut Term) -> &mut Vec<Term> {
    match term {
        Term::Apply(name, fields) if name == "declaration-v1" => fields,
        _ => panic!("independent declaration row"),
    }
}
fn text(term: &Term) -> &str {
    match term {
        Term::Symbol(s) => s,
        _ => panic!("declaration text"),
    }
}
fn rejects(result: Result<impl Sized, CostError>) {
    assert!(matches!(
        result,
        Err(CostError::CorrespondenceMismatch
            | CostError::NeedsEvidence(CostGap::NativeCanonicalizationCostClosurePending))
    ));
}
pub fn dictionary_cuts() {
    native::with_fixture(Options::default(), |f| {
        let original = expected::declarations(f.cases);
        let dictionary = expected::dictionary(original.clone());
        let package = stage_b_copy_pack_package_v1(&dictionary).unwrap();
        let profile = declare_cost_profile(
            ProfileSpec::copy_pack_tree_v1(package.as_inline().runtime),
            f.view,
        )
        .unwrap();
        for cut in 0..5 {
            let mut changed = original.clone();
            let rows = entries(&mut changed);
            match cut {
                0 => {
                    // Same-count, same-byte-length UNUSED beta -> zeta.
                    let at = rows
                        .iter()
                        .position(|r| match r {
                            Term::Apply(_, v) => {
                                text(&v[0]) == "BooleanAtom" && v[2] == wire::symbol("beta")
                            }
                            _ => false,
                        })
                        .unwrap();
                    row(&mut rows[at])[2] = wire::symbol("zeta");
                }
                1 => {
                    // Removing unused atom is not permission to shrink inventory.
                    let at = rows
                        .iter()
                        .position(|r| match r {
                            Term::Apply(_, v) => {
                                text(&v[0]) == "BooleanAtom" && v[2] == wire::symbol("beta")
                            }
                            _ => false,
                        })
                        .unwrap();
                    rows.remove(at);
                }
                2 => {
                    // Valid declaration grammar, but no matching actual forward row.
                    rows.push(wire::app(
                        "declaration-v1",
                        vec![
                            wire::symbol("TemporalAtom"),
                            wire::symbol("Temporal/Atom/1"),
                            wire::symbol("unmapped-time-meaning"),
                        ],
                    ));
                    rows.sort_by(|a, b| match (a, b) {
                        (Term::Apply(_, x), Term::Apply(_, y)) => {
                            (text(&x[0]), text(&x[1])).cmp(&(text(&y[0]), text(&y[1])))
                        }
                        _ => panic!("row"),
                    });
                }
                3 => {
                    // Distinct Clock roles cannot be collapsed to primary meaning.
                    let primary = rows
                        .iter()
                        .find_map(|r| match r {
                            Term::Apply(_, v) if text(&v[1]) == "Clock/primary" => {
                                Some(v[2].clone())
                            }
                            _ => None,
                        })
                        .unwrap();
                    let at = rows
                        .iter()
                        .position(|r| match r {
                            Term::Apply(_, v) => text(&v[1]) == "Clock/secondary",
                            _ => false,
                        })
                        .unwrap();
                    row(&mut rows[at])[2] = primary;
                }
                4 => {
                    // Fixed semantic CaseSpecs cannot move to another slot.
                    let slots: Vec<_> = rows
                        .iter()
                        .enumerate()
                        .filter_map(|(i, r)| match r {
                            Term::Apply(_, v) if text(&v[0]) == "CaseSpec" => Some(i),
                            _ => None,
                        })
                        .collect();
                    assert_eq!(slots.len(), 2);
                    let a = row(&mut rows[slots[0]])[2].clone();
                    let b = row(&mut rows[slots[1]])[2].clone();
                    row(&mut rows[slots[0]])[2] = b;
                    row(&mut rows[slots[1]])[2] = a;
                }
                _ => unreachable!(),
            }
            let altered = expected::dictionary(changed);
            // Reuse valid actual code templates, so producer-side validation
            // cannot replace the intended actual-factory inventory cut.
            let supplied = InlinePackage {
                dictionary: &altered,
                ..package.as_inline()
            };
            rejects(capture_baseline_current(
                &profile, &f.input, supplied, f.view,
            ));
        }
        capture_baseline_current(&profile, &f.input, package.as_inline(), f.view).unwrap();
    });
    // Fully matching NEW actual owners/Plan/Descriptions, SAME truth/loss and
    // same-length changed unused meaning. Old D must fail beyond Stage B.
    let original_cases = atoms::inputs(false, false, false);
    let old_dictionary = expected::dictionary(expected::declarations(&original_cases));
    native::with_fixture(
        Options {
            changed_unused: true,
            ..Options::default()
        },
        |f| {
            inspect_native_current(
                f.input.plan,
                f.input.baseline,
                f.input.target,
                f.input.owners,
                f.view,
            )
            .unwrap()
            .with_current(f.view, |_| ())
            .unwrap();
            let package = stage_b_copy_pack_package_v1(&old_dictionary).unwrap();
            let profile = declare_cost_profile(
                ProfileSpec::copy_pack_tree_v1(package.as_inline().runtime),
                f.view,
            )
            .unwrap();
            rejects(capture_baseline_current(
                &profile,
                &f.input,
                package.as_inline(),
                f.view,
            ));
            let fixed_dictionary = expected::dictionary(expected::declarations(f.cases));
            assert_eq!(old_dictionary.len(), fixed_dictionary.len());
            let restored = stage_b_copy_pack_package_v1(&fixed_dictionary).unwrap();
            let baseline =
                capture_baseline_current(&profile, &f.input, restored.as_inline(), f.view).unwrap();
            assert_eq!(
                baseline.historical_report().semantic_bytes(),
                f.expected_source
            );
        },
    );
}
