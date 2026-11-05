//! Packet for a PRIVATE admission unit, deliberately not registered yet.
//! Central must first approve/apply the behavior-preserving extraction described
//! in the proposal. Missing helper/module compilation is not behavioral RED.
use super::*;

#[test]
fn next_encode_with_one_total_byte_left_cannot_reserve_per_case_4096() {
    assert_eq!(next_input_capacity(4096, 1, 0), Ok(1));
}
#[test]
fn next_encode_stops_before_allocation_when_total_is_empty_or_overused() {
    assert_eq!(next_input_capacity(4096, 1, 1), Err(Error::InputBudget));
    assert_eq!(next_input_capacity(4096, 1, 2), Err(Error::InputBudget));
}
#[test]
fn next_encode_uses_the_smaller_fixed_remaining_limit() {
    for (per_case, total, used, expected) in [
        (4096, 32768, 0, 4096),
        (4096, 5000, 4500, 500),
        (1, 4096, 0, 1),
    ] {
        assert_eq!(next_input_capacity(per_case, total, used), Ok(expected));
    }
}
