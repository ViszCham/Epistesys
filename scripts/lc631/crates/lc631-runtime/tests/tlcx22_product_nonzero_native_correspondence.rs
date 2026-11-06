//! Nonzero raw→normalized→meaning must traverse actual current native owners.
//! Given/Unassessed and FullCostOwnerMissing remain; no original scalar score.
#![forbid(unsafe_code)]
#[path = "tlcx22_product_nonzero_native_support/mod.rs"]
mod support;
use support::{exercise, Variant};

#[test]
fn tlcx22_product_nonzero_single_raw17_current_positive_and_cancel() {
    exercise(Variant::Normal, true, false);
}
#[test]
fn tlcx22_product_nonzero_reversed_meaning_order_actual_current_positive() {
    exercise(Variant::Normal, false, false);
}
#[test]
fn tlcx22_product_nonzero_fresh_global_rename_preserves_actual_semantic_payload() {
    let first = exercise(Variant::Normal, false, false);
    let renamed = exercise(Variant::RenamedBoth, false, false);
    assert_eq!(first.baseline, renamed.baseline);
    assert_eq!(first.target, renamed.target);
}
#[test]
fn tlcx22_product_nonzero_equal_truth_risk_different_actual_catalog_cuts() {
    for variant in [
        Variant::SymbolOnly,
        Variant::MeaningOnly,
        Variant::UnusedMeaning,
        Variant::ExtraUnusedAtom,
        Variant::TheoryOnly,
        Variant::ResponseOnly,
    ] {
        exercise(variant, false, false);
    }
}
#[test]
fn tlcx22_product_nonzero_actual_current_inspection_expires_after_capture() {
    exercise(Variant::Normal, false, true);
}
