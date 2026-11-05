//! Actual no-follow inventory; no manifest is treated as authoritative.
use super::policy::{
    binary_segment_sequence, class_cap, FileClass, PersistentStoreKind, PersistentStoreLimits,
    PersistentStoreProfile, LOCK,
};
use super::{
    io_error, resource, resource_error, store_error, BinaryCheckpointSegmentDescriptor,
    PersistentStoreUsage,
};
use crate::{HostError, HOST_CONTRACT_SCHEMA};
use lc631_core::stable_sha256;
use lc631_process_supervisor::filesystem::DirectoryAnchor;
use lc631_process_supervisor::{ResourceKind, ResourceReservationView};
use lc631_receipt_kernel::digest_bytes;
use std::path::{Path, PathBuf};

pub(super) struct Scan {
    pub(super) usage: PersistentStoreUsage,
    pub(super) target_bytes: Option<u64>,
    pub(super) target_digest: Option<String>,
    pub(super) lock_present: bool,
    pub(super) writer_lock_present: bool,
    pub(super) payload_files: usize,
    pub(super) checkpoint: bool,
    pub(super) binary_segments: Vec<BinaryCheckpointSegmentDescriptor>,
}

pub(super) fn scan_root(
    anchor: &DirectoryAnchor,
    target: &Path,
    profile: PersistentStoreProfile,
    limits: PersistentStoreLimits,
    resources: &ResourceReservationView<'_>,
) -> Result<Scan, HostError> {
    let mut scan = Scan {
        usage: PersistentStoreUsage::default(),
        target_bytes: None,
        target_digest: None,
        lock_present: false,
        writer_lock_present: false,
        payload_files: 0,
        checkpoint: profile.is_checkpoint(),
        binary_segments: Vec::new(),
    };
    scan_directory(
        anchor,
        Path::new(""),
        target,
        profile,
        limits,
        resources,
        &mut scan,
    )?;
    Ok(scan)
}

fn scan_directory(
    anchor: &DirectoryAnchor,
    relative: &Path,
    target: &Path,
    profile: PersistentStoreProfile,
    limits: PersistentStoreLimits,
    resources: &ResourceReservationView<'_>,
    scan: &mut Scan,
) -> Result<(), HostError> {
    resources.check().map_err(resource_error)?;
    anchor.validate_current().map_err(io_error)?;
    if relative.components().count() > limits.max_depth {
        return Err(store_error("root_depth_exhausted"));
    }
    for entry in std::fs::read_dir(anchor.path()).map_err(io_error)? {
        resources.check().map_err(resource_error)?;
        let entry = entry.map_err(io_error)?;
        let local = PathBuf::from(entry.file_name());
        let path = relative.join(&local);
        let metadata = std::fs::symlink_metadata(entry.path()).map_err(io_error)?;
        if metadata.file_type().is_symlink() || reparse(&metadata) {
            return Err(store_error("linked_entry"));
        }
        if metadata.is_dir() {
            if path == target {
                return Err(store_error("target_not_a_file"));
            }
            if profile.kind() == PersistentStoreKind::ReceiptKey
                || profile.is_flat()
                || !(target.starts_with(&path)
                    || path == Path::new("receipt-root")
                    || path == Path::new("observations"))
            {
                return Err(store_error("unknown_directory"));
            }
            let child = anchor.directory(&local, false).map_err(io_error)?;
            scan_directory(&child, &path, target, profile, limits, resources, scan)?;
        } else if metadata.is_file() {
            let class = profile.classify(&path, target)?;
            scan.usage.files = scan
                .usage
                .files
                .checked_add(1)
                .ok_or_else(|| store_error("file_count_overflow"))?;
            if scan.usage.files > limits.max_files {
                return Err(store_error("file_count_exhausted"));
            }
            let (bytes, digest) = if class == FileClass::Lock {
                if metadata.len() != 0 {
                    return Err(store_error("corrupt_lock"));
                }
                if path == Path::new(LOCK) {
                    scan.lock_present = true;
                }
                if path == Path::new(".writer.lock") {
                    scan.writer_lock_present = true;
                }
                (0, None)
            } else {
                scan.payload_files = scan
                    .payload_files
                    .checked_add(1)
                    .ok_or_else(|| store_error("file_count_overflow"))?;
                if profile.is_flat() && scan.payload_files > 1000 {
                    return Err(store_error("payload_count_exhausted"));
                }
                inspect_file(anchor, &local, class, resources)?
            };
            scan.usage.total_bytes = scan
                .usage
                .total_bytes
                .checked_add(bytes)
                .filter(|bytes| *bytes <= limits.max_total_bytes)
                .ok_or_else(|| store_error("root_byte_budget_exhausted"))?;
            if class == FileClass::CheckpointSegment {
                let sequence = binary_segment_sequence(&path)?;
                let digest = digest
                    .as_deref()
                    .ok_or_else(|| store_error("binary_checkpoint_segment_digest_missing"))?;
                scan.binary_segments
                    .try_reserve(1)
                    .map_err(|_| store_error("inventory_allocation_failed"))?;
                scan.binary_segments
                    .push(BinaryCheckpointSegmentDescriptor {
                        sequence,
                        byte_len: bytes,
                        digest: digest.to_owned(),
                    });
            }
            if path == target {
                scan.target_bytes = Some(bytes);
                scan.target_digest = digest;
            }
        } else {
            return Err(store_error("unsupported_entry_type"));
        }
    }
    anchor.validate_current().map_err(io_error)
}

fn inspect_file(
    anchor: &DirectoryAnchor,
    path: &Path,
    class: FileClass,
    resources: &ResourceReservationView<'_>,
) -> Result<(u64, Option<String>), HostError> {
    let limit = class_cap(class);
    // `read_bounded` may observe one sentinel byte beyond the accepted limit.
    // This is logical payload accounting, not a claim about Vec capacity or RSS.
    let reservation_bytes = limit
        .checked_add(1)
        .and_then(|value| u64::try_from(value).ok())
        .ok_or_else(|| store_error("file_read_reservation_overflow"))?;
    let _claim = resources
        .claim_resources(&[
            resource(ResourceKind::Ram, reservation_bytes)?,
            resource(ResourceKind::Output, reservation_bytes)?,
        ])
        .map_err(resource_error)?;
    let mut file = anchor.open_read_only(path).map_err(io_error)?;
    file.validate_current().map_err(io_error)?;
    let bytes = file.read_bounded(limit).map_err(io_error)?;
    if class == FileClass::CheckpointSegment {
        file.validate_current().map_err(io_error)?;
        resources.check().map_err(resource_error)?;
        return binary_checkpoint_segment_digest(&bytes);
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| store_error("invalid_utf8"))?;
    match class {
        FileClass::Key => {
            crate::decode_key(text.trim()).map_err(|_| store_error("corrupt_key"))?;
        }
        FileClass::Ledger => {
            crate::parse_and_verify_ledger(text).map_err(|_| store_error("corrupt_ledger"))?;
        }
        FileClass::Observation => {
            let value: serde_json::Value =
                serde_json::from_str(text).map_err(|_| store_error("corrupt_observation"))?;
            let known_schema = match (value["schema_version"].as_str(), value["host"].as_str()) {
                (Some(HOST_CONTRACT_SCHEMA), _) => true, // historical reported record
                (Some(crate::stop_hook::CODEX_STOP_HOOK_SCHEMA), Some("codex"))
                | (Some(crate::stop_hook::CLAUDE_STOP_HOOK_SCHEMA), Some("claude")) => true,
                _ => false,
            };
            if !known_schema
                || !value["session_digest"].is_string()
                || !value["host_turn_digest"].is_string()
                || !value["output_digest"].is_string()
                || !value["exact_binding"].is_boolean()
            {
                return Err(store_error("corrupt_observation"));
            }
        }
        FileClass::Checkpoint => {
            let value: serde_json::Value =
                serde_json::from_str(text).map_err(|_| store_error("corrupt_checkpoint_record"))?;
            if !value["body"].is_object()
                || !value["digest"].is_string()
                || !matches!(
                    value["body"]["schema_version"].as_str(),
                    Some("epistesys-dgcl-checkpoint.v1" | "epistesys-dgcl-checkpoint.v2")
                )
            {
                return Err(store_error("corrupt_checkpoint_record"));
            }
        }
        FileClass::CheckpointHead => {
            let value: serde_json::Value =
                serde_json::from_str(text).map_err(|_| store_error("corrupt_checkpoint_head"))?;
            if value["schema_version"] != "epistesys-dgcl-checkpoint-head.v1"
                || !value["attestation"].is_object()
                || !value["source_revision"].is_string()
                || !value["plan_digest"].is_string()
                || !value["authority_revision"].is_string()
                || !value["issued_at_epoch"].is_u64()
            {
                return Err(store_error("corrupt_checkpoint_head"));
            }
            // These are bounded format checks, not receipt authentication,
            // trusted-head freshness, or rollback-resistance verification.
        }
        FileClass::CheckpointSegment => {
            return Err(store_error("binary_checkpoint_segment_requires_raw"));
        }
        FileClass::RepairBackup => {}
        FileClass::Lock => return Err(store_error("lock_read_invalid")),
    }
    file.validate_current().map_err(io_error)?;
    resources.check().map_err(resource_error)?;
    Ok((bytes.len() as u64, Some(stable_sha256(text))))
}

fn reparse(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        let _ = metadata;
        false
    }
}

fn binary_checkpoint_segment_digest(bytes: &[u8]) -> Result<(u64, Option<String>), HostError> {
    let byte_count = u64::try_from(bytes.len()).map_err(|_| store_error("file_size_overflow"))?;
    Ok((byte_count, Some(digest_bytes(bytes))))
}

#[cfg(test)]
mod tlcx07_binary_segment_scan_tests {
    use super::*;
    use lc631_process_supervisor::{
        test_fixtures::bounded_test_lease, ResourceAmount, ResourceKind, ResourceRequest,
    };
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn binary_checkpoint_segment_hashes_raw_invalid_utf8_without_json_decode() {
        let bytes = [0xff, 0x00, 0x81, b'{', 0xfe];
        let observed = binary_checkpoint_segment_digest(&bytes).unwrap();
        assert_eq!(observed.0, u64::try_from(bytes.len()).unwrap());
        assert_eq!(observed.1.as_deref(), Some(digest_bytes(&bytes).as_str()));
        assert_eq!(
            observed.1.as_deref(),
            Some(lc631_receipt_kernel::digest_bytes(&bytes).as_str())
        );
    }

    #[test]
    fn scan_claim_covers_the_bounded_reader_sentinel_before_opening_a_file() {
        let parent = std::env::temp_dir().canonicalize().unwrap();
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let name = format!("epistesys-tlcx07-no-file-{}-{unique}", std::process::id());
        assert!(!parent.join(&name).exists());
        let anchor = DirectoryAnchor::open_existing(&parent).unwrap();
        let lease = bounded_test_lease(1).unwrap();
        let view = lease.view();
        let pressure = view
            .claim_resources(&[
                ResourceRequest::new(
                    ResourceKind::Ram,
                    ResourceAmount::checked(12 * 1024 * 1024).unwrap(),
                ),
                ResourceRequest::new(
                    ResourceKind::Output,
                    ResourceAmount::checked(12 * 1024 * 1024).unwrap(),
                ),
            ])
            .unwrap();

        let result = inspect_file(
            &anchor,
            Path::new(&name),
            FileClass::CheckpointSegment,
            &view,
        );
        assert!(matches!(
            result,
            Err(HostError::Io(message)) if message.contains("persistent_resource:Exhausted")
        ));
        assert!(!parent.join(&name).exists());
        drop(pressure);
        lease.close_confirmed().unwrap();
    }
}
