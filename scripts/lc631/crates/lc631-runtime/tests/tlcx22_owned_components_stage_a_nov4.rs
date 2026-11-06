//! 日本語: 六つの実ownerを読む限定preflight。TL対応欠測はNoScalarのまま。
//! English: Six actual current owners; absent TL correspondence stays NoScalar.
//! The 256MiB reservation is an EXPLICIT TEST profile for simultaneous retained
//! owners/scratch, not an increase or a successful gate for the normal128MiB run.
#[path = "tlcx22_adversarial_support/mod.rs"]
mod adversarial_support;
#[path = "tlcx22_design_support/mod.rs"]
mod design_support;
#[path = "tlcx22_memory_support/mod.rs"]
mod memory_support;
#[path = "tlcx22_prediction_support/mod.rs"]
mod prediction_support;
mod tlcx22_owned_components_stage_a_nov4_support;

#[test]
fn tlcx22_owned_components_stage_a_nov4_actual_six_current_cut_and_foreign_checks() {
    tlcx22_owned_components_stage_a_nov4_support::exercise_six_actual_owners();
}
