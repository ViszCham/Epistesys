#[test]
fn production_api_connection() {
    let observed = audit_api::entry(3, 7);
    ::std::assert_eq!(observed, Some(10));
}
