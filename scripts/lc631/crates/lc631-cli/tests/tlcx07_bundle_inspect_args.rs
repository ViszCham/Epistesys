use lc631_cli::bundle_inspect::{parse_args, render_report, BundleInspectArgsError};
use lc631_host::{HostError, JournalBundleOpenError};
use lc631_runtime::journal::{BundleInspectionError, BundleLiveError};
use lc631_runtime::resource::{ResourceError, ResourceKind};
use std::collections::BTreeSet;
use std::path::Path;

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).into()).collect()
}

#[test]
fn tlcx07_bundle_command_accepts_only_one_explicit_root() {
    let parsed = parse_args(&args(&["--ledger-root", "declared-root"])).unwrap();
    assert_eq!(parsed.ledger_root(), Path::new("declared-root"));
    for (input, expected) in [
        (vec![], BundleInspectArgsError::MissingLedgerRoot),
        (vec!["--ledger-root"], BundleInspectArgsError::MissingValue),
        (
            vec!["--ledger-root", ""],
            BundleInspectArgsError::EmptyValue,
        ),
        (
            vec!["--ledger-root", "root", "--execute"],
            BundleInspectArgsError::UnknownOption,
        ),
        (
            vec!["--ledger-root", "root", "--head-root", "other"],
            BundleInspectArgsError::UnknownOption,
        ),
        (
            vec!["--ledger-root", "root", "--ledger-root", "other"],
            BundleInspectArgsError::DuplicateOption,
        ),
        (
            vec!["--prompt", "source"],
            BundleInspectArgsError::UnknownOption,
        ),
    ] {
        assert_eq!(parse_args(&args(&input)), Err(expected));
    }
    assert_eq!(
        parse_args(&["--ledger-root".into(), "x".repeat(4097)]),
        Err(BundleInspectArgsError::ValueTooLong)
    );
}

#[test]
fn tlcx07_bundle_hold_reports_keep_unknown_counts_and_no_authority() {
    for (error, code) in [
        (BundleInspectionError::EmptyInventory, "empty_inventory"),
        (
            BundleInspectionError::Validation(BundleLiveError::RebuildRequired),
            "bundle_rebuild_required",
        ),
        (
            BundleInspectionError::Validation(BundleLiveError::Resource(ResourceError::Exhausted(
                ResourceKind::Ram,
            ))),
            "resource_hold",
        ),
        (
            BundleInspectionError::Validation(BundleLiveError::Poisoned),
            "poisoned_read",
        ),
    ] {
        let report = serde_json::to_value(render_report(Err(error))).unwrap();
        assert_eq!(report["status"], "hold");
        assert_eq!(report["hold_code"], code);
        for name in [
            "segment_count",
            "record_count",
            "segment_input_bytes",
            "latest_record_sequence",
            "local_head_frame_digest_bytes",
            "head_state",
        ] {
            assert!(report[name].is_null(), "{name} must remain unobserved");
        }
        for name in [
            "authority_created",
            "completion_restored",
            "output_commit_allowed",
            "external_anchor_observed",
        ] {
            assert_eq!(report[name], false);
        }
        assert_eq!(report["rollback_resistance"], "unavailable");
        let schema: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../../schemas/lc631-journal-bundle-inspect.v1.schema.json"
        ))
        .unwrap();
        let required: BTreeSet<_> = schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect();
        let actual: BTreeSet<_> = report
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(actual, required);
    }
}

#[test]
fn tlcx07_bundle_unexpected_effect_diagnostics_preserve_before_and_uncertain_states() {
    for (error, code) in [
        (
            BundleLiveError::HostOpen(JournalBundleOpenError::BeforeEffect(HostError::Io(
                "private diagnostic".into(),
            ))),
            "unexpected_open_before_effect",
        ),
        (
            BundleLiveError::HostOpen(JournalBundleOpenError::MayHaveApplied(HostError::Io(
                "private diagnostic".into(),
            ))),
            "unexpected_open_may_have_applied",
        ),
        (
            BundleLiveError::StorageRejectedBeforeWrite(HostError::Io("private diagnostic".into())),
            "unexpected_write_rejected_before_effect",
        ),
        (
            BundleLiveError::StorageMayHaveApplied(HostError::Io("private diagnostic".into())),
            "unexpected_write_may_have_applied",
        ),
    ] {
        let report =
            serde_json::to_value(render_report(Err(BundleInspectionError::Validation(error))))
                .unwrap();
        assert_eq!(report["status"], "hold");
        assert_eq!(report["hold_code"], code);
        assert_eq!(report["authority_created"], false);
        assert_eq!(report["output_commit_allowed"], false);
        assert!(!report.to_string().contains("private diagnostic"));
    }
}
