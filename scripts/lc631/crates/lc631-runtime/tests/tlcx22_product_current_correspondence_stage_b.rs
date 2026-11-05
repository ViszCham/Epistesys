//! 日本語: 実native owner対応の負例契約。full-cost Kも原式も未完成。
//! English: Actual-native correspondence contracts, never full-cost K/formula closure.
mod tlcx22_product_current_correspondence_stage_b_support;
use tlcx22_product_current_correspondence_stage_b_support::{exercise, Mode};

#[test]
fn tlcx22_product_current_stage_b_same_owners_full_payload_mutants_and_second_target() {
    exercise(Mode::Normal, false);
}

#[test]
fn tlcx22_product_current_stage_b_fresh_rename_and_reported_split_rejected_by_native_root() {
    let original = exercise(Mode::Normal, false);
    let renamed = exercise(Mode::Renamed, false);
    assert_eq!(original.source, renamed.source);
    assert_eq!(original.target, renamed.target);
    assert_ne!(original.envelope, renamed.envelope);
    // Normal and globally renamed scenarios inspect fresh matching owners.
    // The split scenario is REPORTED codec-only: role0/role1 remains visible,
    // but the unchanged native TL root rejects the different clock mapping.
    // No current inspection token is produced for that unsupported scenario.
    let split = exercise(Mode::SplitTargetClock, false);
    assert_ne!(original.target, split.target);
}

#[test]
fn tlcx22_product_current_stage_b_actual_post_capture_deadline_expiry() {
    // Explicit bounded2s TEST deadline: construction must first succeed live.
    // This is an actual postcapture expiry cut, not an already-expired run.
    exercise(Mode::Normal, true);
}
