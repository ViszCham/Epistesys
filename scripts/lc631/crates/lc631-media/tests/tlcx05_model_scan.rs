use lc631_media::{model_cache_manifest_sha256_bounded, MediaError, ModelScanLimits};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn fixture() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "epistesys-tlcx05-model-scan-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir(&root).unwrap();
    root
}

fn limits() -> ModelScanLimits {
    ModelScanLimits {
        max_entries: 8,
        max_depth: 4,
        max_metadata_bytes: 4096,
        max_file_bytes: 64,
        max_total_bytes: 128,
    }
}

#[test]
fn tlcx05_flat_model_directory_is_bounded_before_each_entry_push() {
    let root = fixture();
    std::fs::write(root.join("a"), b"a").unwrap();
    std::fs::write(root.join("b"), b"b").unwrap();
    let mut budget = limits();
    budget.max_entries = 1;
    assert!(matches!(
        model_cache_manifest_sha256_bounded(&root, budget, None),
        Err(MediaError::MediaBudgetExceeded)
    ));
}

#[test]
fn tlcx05_model_hash_rejects_file_and_aggregate_byte_budget_before_success() {
    let root = fixture();
    std::fs::write(root.join("a"), b"12345").unwrap();
    std::fs::write(root.join("b"), b"12345").unwrap();
    let mut budget = limits();
    budget.max_file_bytes = 4;
    assert!(matches!(
        model_cache_manifest_sha256_bounded(&root, budget, None),
        Err(MediaError::MediaBudgetExceeded)
    ));
    budget = limits();
    budget.max_total_bytes = 8;
    assert!(matches!(
        model_cache_manifest_sha256_bounded(&root, budget, None),
        Err(MediaError::MediaBudgetExceeded)
    ));
}

#[test]
fn tlcx05_bounded_model_scan_preserves_the_legacy_manifest_bytes_and_hash() {
    let root = fixture();
    std::fs::write(root.join("a"), b"a").unwrap();
    let basis = concat!(
        "lc631-directory-manifest.v3\0",
        "[[\"a\",1,\"sha256:ca978112ca1bbdcafac231b39a23dc4da786eff8147c4e72b9807785afee48bb\"]]"
    );
    assert_eq!(
        model_cache_manifest_sha256_bounded(&root, limits(), None).unwrap(),
        lc631_core::stable_sha256(basis)
    );
}
