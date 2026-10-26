// The isolated, never-merged CI negative-control branch changes this expected
// value. This tests CI's Rust failure propagation, not product correctness.
const EXPECTED_CI_SENTINEL: u8 = 4;

#[test]
fn ci_runner_propagates_rust_assertion_failures() {
    assert_eq!([1u8, 2].into_iter().sum::<u8>(), EXPECTED_CI_SENTINEL);
}
