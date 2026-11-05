//! Bounded source-route regression, not runtime or whole-program reachability proof.
#[test]
fn tlcx40_private_cli_has_only_resource_bound_package_entrypoints() {
    let package = include_str!("../src/dgcl_package.rs");
    let repair = include_str!("../src/dgcl_repair.rs");
    let journal = include_str!("../src/dgcl_journal.rs");
    assert!(!package.contains("pub fn finalize_package("));
    assert!(!repair.contains("pub(super) fn repair_package("));
    assert!(!journal.contains("pub fn resume_package("));
    assert!(package.contains("fn finalize_package_with_resources("));
    assert!(repair.contains("fn repair_package_with_resources("));
    assert!(journal.contains("fn resume_package_with_resources("));
}
