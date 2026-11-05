use super::*;
use std::fs;

pub(super) struct Snapshot {
    pub(super) entries: Vec<JournalBundleDescriptor>,
    pub(super) files: usize,
    pub(super) bytes: u64,
    pub(super) marker: bool,
}

fn reparse(metadata: &fs::Metadata) -> bool {
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

// Caller holds SESSION_SCRATCH for metadata and the maximum bounded read.
pub(super) fn scan(
    anchor: &DirectoryAnchor,
    limits: JournalBundleLimits,
    resources: &ResourceReservationView<'_>,
) -> Result<Snapshot, HostError> {
    resources.check().map_err(resource_error)?;
    anchor.validate_current().map_err(io_error)?;
    let max_files = limits
        .max_bundles
        .checked_mul(3)
        .and_then(|n| n.checked_add(2))
        .ok_or_else(|| store_error("bundle_file_count_overflow"))?;
    let max_parts = limits
        .max_bundles
        .checked_mul(3)
        .ok_or_else(|| store_error("bundle_file_count_overflow"))?;
    let mut parts = Vec::new();
    parts
        .try_reserve_exact(max_parts)
        .map_err(|_| store_error("bundle_inventory_allocation_failed"))?;
    let mut files = 0_usize;
    let mut total = 0_u64;
    let mut marker = false;
    for item in fs::read_dir(anchor.path()).map_err(io_error)? {
        resources.check().map_err(resource_error)?;
        let item = item.map_err(io_error)?;
        let name = item.file_name();
        let name = name
            .to_str()
            .ok_or_else(|| store_error("bundle_name_not_utf8"))?;
        let metadata = fs::symlink_metadata(item.path()).map_err(io_error)?;
        if metadata.file_type().is_symlink() || reparse(&metadata) || !metadata.is_file() {
            return Err(store_error("bundle_nonregular_entry"));
        }
        files = files
            .checked_add(1)
            .filter(|n| *n <= max_files)
            .ok_or_else(|| store_error("bundle_file_count_exhausted"))?;
        if name == LOCK {
            if metadata.len() != 0 {
                return Err(store_error("corrupt_lock"));
            }
            continue;
        }
        if name == MARKER {
            let mut file = anchor.open_read_only(Path::new(MARKER)).map_err(io_error)?;
            let bytes = file.read_bounded(MARKER_BYTES.len()).map_err(io_error)?;
            file.validate_current().map_err(io_error)?;
            if bytes.as_slice() != MARKER_BYTES {
                return Err(store_error("bundle_marker_invalid"));
            }
            total = total
                .checked_add(
                    u64::try_from(bytes.len()).map_err(|_| store_error("bundle_size_overflow"))?,
                )
                .filter(|n| *n <= limits.max_total_bytes)
                .ok_or_else(|| store_error("bundle_root_bytes_exhausted"))?;
            marker = true;
            continue;
        }
        let (sequence, part) = parse_name(name)?;
        let mut file = anchor.open_read_only(Path::new(name)).map_err(io_error)?;
        let bytes = file.read_bounded(part.cap()).map_err(io_error)?;
        file.validate_current().map_err(io_error)?;
        let meta = descriptor(&bytes)?;
        total = total
            .checked_add(meta.byte_len)
            .filter(|n| *n <= limits.max_total_bytes)
            .ok_or_else(|| store_error("bundle_root_bytes_exhausted"))?;
        if bytes.is_empty() {
            return Err(store_error("bundle_part_empty"));
        }
        if parts.len() >= max_parts {
            return Err(store_error("bundle_part_count_exhausted"));
        }
        parts.push((sequence, part, meta));
    }
    parts.sort_unstable_by_key(|(sequence, part, _)| (*sequence, part.slot()));
    let mut entries: Vec<JournalBundleDescriptor> = Vec::new();
    entries
        .try_reserve_exact(limits.max_bundles)
        .map_err(|_| store_error("bundle_inventory_allocation_failed"))?;
    for (sequence, part, meta) in parts {
        if entries.last().is_none_or(|last| last.sequence != sequence) {
            if entries.len() == limits.max_bundles {
                return Err(store_error("bundle_count_exhausted"));
            }
            entries.push(JournalBundleDescriptor {
                sequence,
                parts: [None; 3],
            });
        }
        let last = entries
            .last_mut()
            .ok_or_else(|| store_error("bundle_inventory_invalid"))?;
        if last.parts[part.slot()].replace(meta).is_some() {
            return Err(store_error("bundle_duplicate_part"));
        }
    }
    anchor.validate_current().map_err(io_error)?;
    resources.check().map_err(resource_error)?;
    Ok(Snapshot {
        entries,
        files,
        bytes: total,
        marker,
    })
}
pub(super) fn next_complete(entries: &[JournalBundleDescriptor]) -> Result<u64, HostError> {
    let mut next = 1_u64;
    for entry in entries {
        if !entry.complete() {
            return Err(store_error("bundle_incomplete"));
        }
        if entry.sequence != next {
            return Err(store_error("bundle_sequence_gap"));
        }
        next = next
            .checked_add(1)
            .ok_or_else(|| store_error("bundle_sequence_overflow"))?;
    }
    Ok(next)
}
// Reject any preexisting unmarked payload before strict profile decoding.
// The error calls for explicit migration or review; it does not certify that
// the unknown root is a valid legacy journal.
pub(super) fn require_profile_or_empty(anchor: &DirectoryAnchor) -> Result<(), HostError> {
    anchor.validate_current().map_err(io_error)?;
    match fs::symlink_metadata(anchor.path().join(MARKER)) {
        Ok(_) => return Ok(()), // scan() validates the actual marker.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(io_error(error)),
    }
    for entry in fs::read_dir(anchor.path()).map_err(io_error)? {
        let entry = entry.map_err(io_error)?;
        if entry.file_name().as_os_str() != std::ffi::OsStr::new(LOCK) {
            return Err(store_error("bundle_migration_required"));
        }
    }
    anchor.validate_current().map_err(io_error)
}
