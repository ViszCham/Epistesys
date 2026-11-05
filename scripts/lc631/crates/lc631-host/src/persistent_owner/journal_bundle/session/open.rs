//! Resource admission and marker setup before a live bundle session.
use super::*;
use std::fs;

impl<'a> HostJournalBundleSession<'a> {
    /// Claims all planned Disk plus a conservative marker reservation before
    /// any possible root or lock creation. Later errors retain effect ambiguity.
    pub fn open(
        root: &Path,
        planned_total_bytes: u64,
        limits: JournalBundleLimits,
        resources: &'a ResourceReservationView<'a>,
    ) -> Result<Self, JournalBundleOpenError> {
        let preflight = (|| -> Result<_, HostError> {
            if planned_total_bytes > limits.max_total_bytes {
                return Err(store_error("bundle_reservation_too_large"));
            }
            let disk_bytes = planned_total_bytes
                .checked_add(
                    u64::try_from(MARKER_BYTES.len())
                        .map_err(|_| store_error("bundle_size_overflow"))?,
                )
                .ok_or_else(|| store_error("bundle_size_overflow"))?;
            let claim = resources
                .claim_resources(&[
                    resource(ResourceKind::Disk, disk_bytes)?,
                    resource(ResourceKind::Ram, SESSION_SCRATCH)?,
                    resource(ResourceKind::Output, SESSION_SCRATCH)?,
                ])
                .map_err(resource_error)?;
            let root = std::path::absolute(root).map_err(io_error)?;
            match fs::symlink_metadata(&root) {
                Ok(_) => {
                    let existing = DirectoryAnchor::open_existing(&root).map_err(io_error)?;
                    require_profile_or_empty(&existing)?;
                    let initial = scan(&existing, limits, resources)?;
                    if !initial.marker && !initial.entries.is_empty() {
                        return Err(store_error("bundle_migration_required"));
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(io_error(error)),
            }
            Ok((root, claim))
        })()
        .map_err(JournalBundleOpenError::BeforeEffect)?;
        let (root, claim) = preflight;
        let effect = (|| -> Result<Self, HostError> {
            let anchor = create_owned_root(&root)?;
            let mut lock = anchor
                .open_or_create_file(Path::new(LOCK))
                .map_err(io_error)?;
            if lock.metadata().map_err(io_error)?.len() != 0 {
                return Err(store_error("corrupt_lock"));
            }
            lock.append_and_sync(&[]).map_err(io_error)?;
            require_profile_or_empty(&anchor)?;
            let mut snapshot = scan(&anchor, limits, resources)?;
            lock.validate_current().map_err(io_error)?;
            if !snapshot.marker && !snapshot.entries.is_empty() {
                return Err(store_error("bundle_migration_required"));
            }
            let marker_bytes = if snapshot.marker {
                0
            } else {
                u64::try_from(MARKER_BYTES.len())
                    .map_err(|_| store_error("bundle_size_overflow"))?
            };
            if !snapshot.marker {
                snapshot
                    .bytes
                    .checked_add(marker_bytes)
                    .filter(|n| *n <= limits.max_total_bytes)
                    .ok_or_else(|| store_error("bundle_root_bytes_exhausted"))?;
                let max_files = limits
                    .max_bundles
                    .checked_mul(3)
                    .and_then(|n| n.checked_add(2))
                    .ok_or_else(|| store_error("bundle_file_count_overflow"))?;
                snapshot
                    .files
                    .checked_add(1)
                    .filter(|n| *n <= max_files)
                    .ok_or_else(|| store_error("bundle_file_count_exhausted"))?;
                let marker_handoff = (|| {
                    let mut marker_file = anchor
                        .create_private_file(Path::new(MARKER))
                        .map_err(io_error)?;
                    marker_file
                        .append_and_sync(MARKER_BYTES)
                        .map_err(io_error)?;
                    marker_file.validate_current().map_err(io_error)?;
                    drop(marker_file);
                    let marked = scan(&anchor, limits, resources)?;
                    lock.validate_current().map_err(io_error)?;
                    if !marked.marker
                        || marked.entries != snapshot.entries
                        || snapshot.files.checked_add(1) != Some(marked.files)
                        || snapshot.bytes.checked_add(marker_bytes) != Some(marked.bytes)
                    {
                        return Err(store_error("bundle_marker_handoff_changed"));
                    }
                    Ok(marked)
                })();
                snapshot = match marker_handoff {
                    Ok(marked) => marked,
                    Err(error) => {
                        resources.record_cleanup_unknown();
                        return Err(error);
                    }
                };
            }
            Ok(Self {
                anchor,
                lock,
                resources,
                limits,
                remaining: planned_total_bytes,
                snapshot,
                poisoned: false,
                _claim: claim,
            })
        })();
        match effect {
            Ok(session) => Ok(session),
            Err(error) => {
                resources.record_cleanup_unknown();
                Err(JournalBundleOpenError::MayHaveApplied(error))
            }
        }
    }
}
