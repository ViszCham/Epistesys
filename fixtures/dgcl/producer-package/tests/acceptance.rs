#[test]
fn required_checked_arithmetic_contract() {
    assert_eq!(dgcl_producer_fixture::checked_add(3, 7), Some(10));
    assert_eq!(dgcl_producer_fixture::checked_add(u32::MAX, 0), Some(u32::MAX));
    assert_eq!(dgcl_producer_fixture::checked_add(u32::MAX, 1), None);
}
