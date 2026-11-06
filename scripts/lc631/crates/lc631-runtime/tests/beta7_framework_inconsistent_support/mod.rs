//! A false mutable premise and hard p have no legal classical assignment.
use super::*;
use super::native_fixture::capture_native;

pub(super) fn assert_inconsistent_native_admission(h: Inputs<'_>, view: &ResourceReservationView<'_>) {
    // Keep a live positive for the same source/goal/lease/limits before changing Γ.
    let valid = capture_native(&h, context_limits(), view).expect("the original Γ is feasible");
    drop(valid);

    let bad = query(&h, vec![BoolExpr::Constant(false), var(p(&h))]);
    let premises = [
        GivenPremise { formula: &bad.assertions()[0], ..h.premises[0] },
        GivenPremise { formula: &bad.assertions()[1], ..h.premises[1] },
    ];
    let one = q(1, 1);
    let facts = [
        n::FactFeature { formula: &bad.assertions()[0], weight: &one },
        n::FactFeature { formula: &bad.assertions()[1], weight: &one },
    ];
    let constructions = [
        n::ConstructionFeature { formula: &bad.assertions()[0], weight: &one },
        n::ConstructionFeature { formula: &bad.assertions()[1], weight: &one },
    ];
    let inconsistent = Inputs {
        theory: &bad,
        premises: &premises,
        facts: &facts,
        constructions: &constructions,
        ..h
    };
    match capture_native(&inconsistent, context_limits(), view) {
        Err(n::Error::ContextInconsistent) => {}
        Err(other) => panic!("expected ContextInconsistent before framework admission; got {other:?}"),
        Ok(_) => panic!("zero feasible rows admitted a native root"),
    }
}
