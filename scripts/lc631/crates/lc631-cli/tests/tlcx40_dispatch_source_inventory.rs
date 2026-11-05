//! Source inventory is partial static coverage, not dispatch execution evidence.
use lc631_core::context::{ContextRevision, SourceRevision, TaskKind};
use lc631_runtime::connection_audit::{audit_current_entry, AuditGap};

#[test]
fn tlcx40_legacy_dispatch_source_inventory_keeps_diagnostics_and_package_routes_distinct() {
    let source = include_str!("../src/lib.rs");
    assert!(source.len() <= 512 * 1024);
    for marker in [
        "\"lc631-doctor\" =>",
        "\"lc631-dgcl-run\" =>",
        "\"lc631-dgcl-package-finalize\"",
        "\"lc631-dgcl-package-repair\"",
        "\"lc631-dgcl-package-resume\"",
        "command == \"lc631-journal-bundle-inspect\"",
        "command == \"lc631-checkpoint-resume-inspect\"",
    ] {
        assert!(
            source.contains(marker),
            "review changed dispatcher source marker: {marker}"
        );
    }
    // Presence checks above are source inventory only, never E01..E20 observations.
    let text = "CLI compile-linked audit scope.";
    let revision = ContextRevision::original(SourceRevision::of(text));
    let report = audit_current_entry(text, revision, TaskKind::Explanation).unwrap();
    assert_eq!(report.gap(), AuditGap::RequiredEdgeExecutionsUnobserved);
}

#[test]
fn tlcx40_resource_entry_signature_is_compiled_without_claiming_cli_dispatch_execution() {
    let _entry: fn(&lc631_runtime::resource::ResourceBroker) = lc631_cli::main_entry_with_resources;
    // Do not call a process-global argv entry from this test or fabricate its output.
}
