//! Independent count algebra for the fixed existing seven-role bank.
//! No production lowering/evaluator supplies these expected counts.
use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Sizes {
    pub source: usize,
    pub native: usize,
    pub largest_native: usize,
}

pub fn placement(children: usize, top_tail: bool) -> BoolExpr {
    assert!(children >= 2);
    BoolExpr::And(
        (0..children)
            .map(|i| {
                if top_tail && i + 1 == children {
                    // Not Bottom is Top in BOTH registered kernels. q AND Top = q.
                    BoolExpr::Not(Box::new(BoolExpr::Constant(false)))
                } else {
                    var(1)
                }
            })
            .collect(),
    )
}

fn tree(e: &BoolExpr) -> (usize, usize) {
    match e {
        BoolExpr::Constant(_) | BoolExpr::Variable(_) => (1, 1),
        BoolExpr::Not(c) => {
            let (source, native) = tree(c);
            (source + 1, native + 1)
        }
        BoolExpr::And(cs) | BoolExpr::Or(cs) => {
            assert!(!cs.is_empty());
            let (source, native) = cs
                .iter()
                .map(tree)
                .fold((0, 0), |(s, n), (child_s, child_n)| {
                    (s + child_s, n + child_n)
                });
            // Ordered binary folding emits k-1 native connectives, not one.
            (source + 1, native + cs.len() - 1)
        }
        BoolExpr::Equal(_, _) | BoolExpr::LessEqual(_, _) | BoolExpr::LessThan(_, _) => {
            panic!("independent oracle admits only the fixed Boolean grammar")
        }
    }
}

pub fn bank(h: &Inputs<'_>, probes: &[f::Probe<'_>]) -> Sizes {
    assert_eq!(h.theory.assertions().len(), 2);
    assert_eq!(h.goal.assertions().len(), 1);
    assert_eq!(probes.len(), 3);
    assert!(std::ptr::eq(probes[0].formula, &h.goal.assertions()[0]));
    // Preserve the actual original hard p and every occurrence, even aliases.
    let roots = [
        &h.theory.assertions()[0],
        &h.theory.assertions()[1],
        h.premises[1].formula,
        &h.goal.assertions()[0],
        probes[0].formula,
        probes[1].formula,
        probes[2].formula,
    ];
    roots.into_iter().map(tree).fold(
        Sizes {
            source: 0,
            native: 0,
            largest_native: 0,
        },
        |sum, (source, native)| Sizes {
            source: sum.source + source,
            native: sum.native + native,
            largest_native: sum.largest_native.max(native),
        },
    )
}
