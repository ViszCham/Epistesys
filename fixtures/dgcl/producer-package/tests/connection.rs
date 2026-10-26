#[test]
fn production_api_connection() {
    let observed = dgcl_producer_fixture::checked_add(3, 7);
    ::std::assert_eq!(observed, Some(10));
}
