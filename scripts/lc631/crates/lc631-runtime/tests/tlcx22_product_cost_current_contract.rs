//! TEST-FIRST absent API. All new cost assertions are unexecuted until central RED/body/GREEN.
//! Explicit256MiB TEST fixtures; no normal128MiB, strict formula or full-native closure.
#[path = "tlcx22_product_cost_current_support/mod.rs"]
mod support;

#[test]
fn tlcx22_cf_p01_p03_n09_n22_actual_fresh_k0_and_second_candidate() {
    support::positive(support::Options::default());
    support::positive(support::Options {
        single_atom: true,
        ..support::Options::default()
    });
}
#[test]
fn tlcx22_cf_n10_n24_equal_bytes_foreign_owners_and_baseline() {
    support::owner_cuts();
}
#[test]
fn tlcx22_cf_n11_n13_n14_full_paid_semantic_inventory_cuts() {
    support::dictionary_cuts();
    support::metadata_cuts();
}
#[test]
fn tlcx22_cf_n12_n13_n14_whole_plan_roles_and_global_rename() {
    let original = support::positive(support::Options::default());
    let renamed = support::positive(support::Options {
        renamed: true,
        reverse_responses: true,
        ..support::Options::default()
    });
    assert_eq!(original.source_bundle, renamed.source_bundle);
    assert_eq!(original.target_bundle, renamed.target_bundle);
    assert_eq!((original.k0, original.k), (renamed.k0, renamed.k));
    assert_ne!(original.raw_envelope, renamed.raw_envelope);
    assert_ne!(original.baseline_binding, renamed.baseline_binding);
    assert_ne!(original.candidate_binding, renamed.candidate_binding);
    // The entire actual root/baseline/target uses SECONDARY, with primary also
    // present in the same predeclared role table. No old candidate-only split.
    let secondary = support::positive(support::Options {
        secondary_clock: true,
        ..support::Options::default()
    });
    let renamed_secondary = support::positive(support::Options {
        secondary_clock: true,
        renamed: true,
        ..support::Options::default()
    });
    assert_eq!(secondary.source_bundle, renamed_secondary.source_bundle);
    assert_eq!(secondary.target_bundle, renamed_secondary.target_bundle);
    assert_ne!(secondary.raw_envelope, renamed_secondary.raw_envelope);
    assert_ne!(
        secondary.candidate_binding,
        renamed_secondary.candidate_binding
    );
    assert_ne!(original.source_bundle, secondary.source_bundle);
    assert_ne!(original.target_bundle, secondary.target_bundle);
}
#[test]
fn tlcx22_cf_n16_n24_callback_cancel_cannot_restore_current() {
    support::cancellation();
}
#[test]
fn tlcx22_cf_n16_actual_postcapture_expiry() {
    support::expiry();
}
#[test]
fn tlcx22_cf_n17_foreign_reservation_and_real_ram_admission_cut() {
    support::foreign_and_budget();
}
#[test]
fn tlcx22_cf_n05_n22_n23_n26_actual_adapter_cost_inverse_and_profile() {
    support::profile_and_inverse();
}
