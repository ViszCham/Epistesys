//! A known empty inventory needs no decode-work reservation.
use lc631_host::JournalBundleLimits;
use lc631_runtime::journal::{inspect_bundle_root, BundleInspectionError, JournalChainLimits};
use lc631_runtime::resource::{
    Cancellation, Deadline, Digest32, ResourceAmount, ResourceBroker, ResourceKind, ResourceLimits,
    ResourceRequest,
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[test]
fn tlcx07_known_empty_snapshot_is_not_hidden_by_unused_parser_budget() {
    let base = std::env::temp_dir().canonicalize().unwrap();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = base.join(format!(
        "epistesys-tlcx07-empty-budget-{}-{now}",
        std::process::id()
    ));
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join(".journal-bundle-v1"), b"EJBDL001").unwrap();
    let limits = ResourceLimits::uniform(32 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let run = broker
        .begin_run(
            1,
            Digest32::from_bytes([9; 32]),
            limits,
            Deadline::after(Duration::from_secs(30)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let amount = ResourceAmount::checked(10 * 1024 * 1024).unwrap();
    let mut reservation = run
        .reserve(&[
            ResourceRequest::new(ResourceKind::Ram, amount),
            ResourceRequest::new(ResourceKind::Output, amount),
        ])
        .unwrap();
    {
        let view = reservation.lease();
        let result = inspect_bundle_root(
            &root,
            JournalBundleLimits::checked(10, 1024 * 1024).unwrap(),
            &view,
            JournalChainLimits::default(),
        );
        assert_eq!(result, Err(BundleInspectionError::EmptyInventory));
    }
    reservation.release_confirmed().unwrap();
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
}
