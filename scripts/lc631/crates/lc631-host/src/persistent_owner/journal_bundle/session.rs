use super::*;
mod open;

pub struct HostJournalBundleSession<'a> {
    anchor: DirectoryAnchor,
    lock: AnchoredFile,
    resources: &'a ResourceReservationView<'a>,
    limits: JournalBundleLimits,
    remaining: u64,
    snapshot: Snapshot,
    poisoned: bool,
    _claim: ResourceClaim<'a>,
}
fn check_handles(
    anchor: &DirectoryAnchor,
    lock: &AnchoredFile,
    resources: &ResourceReservationView<'_>,
) -> Result<(), HostError> {
    resources.check().map_err(resource_error)?;
    anchor.validate_current().map_err(io_error)?;
    lock.validate_current().map_err(io_error)?;
    resources.check().map_err(resource_error)
}
struct EffectGuard<'s, 'a> {
    poisoned: &'s mut bool,
    resources: &'a ResourceReservationView<'a>,
    armed: bool,
}
impl Drop for EffectGuard<'_, '_> {
    fn drop(&mut self) {
        if self.armed {
            *self.poisoned = true;
            self.resources.record_cleanup_unknown();
        }
    }
}

impl<'a> HostJournalBundleSession<'a> {
    fn active(&self) -> Result<(), HostError> {
        if self.poisoned {
            return Err(store_error("bundle_session_poisoned"));
        }
        self.resources.check().map_err(resource_error)?;
        self.anchor.validate_current().map_err(io_error)?;
        self.lock.validate_current().map_err(io_error)?;
        self.resources.check().map_err(resource_error)
    }
    fn checked_scan(&self) -> Result<Snapshot, HostError> {
        self.active()?;
        let current = scan(&self.anchor, self.limits, self.resources)?;
        self.active()?;
        if !current.marker
            || current.entries != self.snapshot.entries
            || current.files != self.snapshot.files
            || current.bytes != self.snapshot.bytes
        {
            return Err(store_error("bundle_inventory_changed"));
        }
        Ok(current)
    }
    /// Copy the already captured lock-held snapshot with retained accounting.
    pub fn inventory(&mut self) -> Result<OwnedJournalBundleInventory<'a>, HostError> {
        let result = (|| {
            self.active()?;
            let claim = self
                .resources
                .claim_resources(&[
                    resource(ResourceKind::Ram, META_CLAIM)?,
                    resource(ResourceKind::Output, META_CLAIM)?,
                ])
                .map_err(resource_error)?;
            let mut entries = Vec::new();
            entries
                .try_reserve_exact(self.snapshot.entries.len())
                .map_err(|_| store_error("bundle_inventory_allocation_failed"))?;
            entries.extend_from_slice(&self.snapshot.entries);
            self.active()?;
            Ok(OwnedJournalBundleInventory {
                entries,
                _claim: claim,
            })
        })();
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    /// One full raw comparison after runtime replay; no codec semantics.
    pub fn validate_snapshot(&mut self) -> Result<(), HostError> {
        let result = self.checked_scan().map(|_| ());
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    pub fn next_sequence(&mut self) -> Result<u64, HostError> {
        if let Err(error) = self.active() {
            self.poisoned = true;
            return Err(error);
        }
        next_complete(&self.snapshot.entries)
    }
    pub fn remaining_bytes(&self) -> u64 {
        self.remaining
    }
    pub fn read_part(
        &mut self,
        sequence: u64,
        part: BundlePart,
    ) -> Result<OwnedJournalBundlePart<'a>, HostError> {
        let result = (|| {
            self.active()?;
            let bytes = read_snapshot_part(
                &self.anchor,
                &self.snapshot.entries,
                sequence,
                part,
                self.resources,
            )?;
            self.active()?;
            Ok(bytes)
        })();
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    /// Host validates storage only; caller validates chain, index and head bytes.
    /// Every error poisons. After admission, effects may exist and are retained.
    pub fn append_bundle(
        &mut self,
        expected_sequence: u64,
        segment: &[u8],
        index: &[u8],
        local_head: &[u8],
    ) -> Result<(), JournalBundleWriteError> {
        self.append_bundle_observed(expected_sequence, segment, index, local_head, |_| Ok(()))
    }
    /// Observation hook runs after actual sync and close of each part.
    pub fn append_bundle_observed(
        &mut self,
        expected_sequence: u64,
        segment: &[u8],
        index: &[u8],
        local_head: &[u8],
        mut after_part: impl FnMut(BundlePart) -> Result<(), HostError>,
    ) -> Result<(), JournalBundleWriteError> {
        let admission = (|| {
            let current = self.checked_scan()?;
            if next_complete(&current.entries)? != expected_sequence {
                return Err(store_error("bundle_sequence_stale"));
            }
            if current.entries.len() >= self.limits.max_bundles {
                return Err(store_error("bundle_count_exhausted"));
            }
            let mut total = 0_u64;
            let values = [
                (BundlePart::Segment, segment),
                (BundlePart::Index, index),
                (BundlePart::LocalHead, local_head),
            ];
            let mut expected = JournalBundleDescriptor {
                sequence: expected_sequence,
                parts: [None; 3],
            };
            for (part, bytes) in values {
                if bytes.is_empty() || bytes.len() > part.cap() {
                    return Err(store_error("bundle_part_size_invalid"));
                }
                let meta = descriptor(bytes)?;
                total = total
                    .checked_add(meta.byte_len)
                    .ok_or_else(|| store_error("bundle_size_overflow"))?;
                expected.parts[part.slot()] = Some(meta);
            }
            let remaining = self
                .remaining
                .checked_sub(total)
                .ok_or_else(|| store_error("disk_reservation_exhausted"))?;
            current
                .bytes
                .checked_add(total)
                .filter(|n| *n <= self.limits.max_total_bytes)
                .ok_or_else(|| store_error("bundle_root_bytes_exhausted"))?;
            let max_files = self
                .limits
                .max_bundles
                .checked_mul(3)
                .and_then(|n| n.checked_add(2))
                .ok_or_else(|| store_error("bundle_file_count_overflow"))?;
            current
                .files
                .checked_add(3)
                .filter(|n| *n <= max_files)
                .ok_or_else(|| store_error("bundle_file_count_exhausted"))?;
            expected_sequence
                .checked_add(1)
                .ok_or_else(|| store_error("bundle_sequence_overflow"))?;
            Ok((current, remaining, expected, total))
        })();
        let (before, remaining, expected, total) = match admission {
            Ok(value) => value,
            Err(error) => {
                self.poisoned = true;
                return Err(JournalBundleWriteError::RejectedBeforeWrite(error));
            }
        };
        // The guard stays armed through final snapshot; it poisons on error or unwind.
        let mut guard = EffectGuard {
            poisoned: &mut self.poisoned,
            resources: self.resources,
            armed: true,
        };
        let write = (|| {
            for (part, bytes) in [
                (BundlePart::Segment, segment),
                (BundlePart::Index, index),
                (BundlePart::LocalHead, local_head),
            ] {
                check_handles(&self.anchor, &self.lock, self.resources)?;
                let name = part.name(expected_sequence);
                let mut file = self
                    .anchor
                    .create_private_file(Path::new(&name))
                    .map_err(io_error)?;
                file.append_and_sync(bytes).map_err(io_error)?;
                file.validate_current().map_err(io_error)?;
                drop(file);
                after_part(part)?;
            }
            let after = scan(&self.anchor, self.limits, self.resources)?;
            check_handles(&self.anchor, &self.lock, self.resources)?;
            let correct = after.marker
                && before.entries.len().checked_add(1) == Some(after.entries.len())
                && after
                    .entries
                    .iter()
                    .take(before.entries.len())
                    .eq(before.entries.iter())
                && after.entries.last() == Some(&expected)
                && before.files.checked_add(3) == Some(after.files)
                && before.bytes.checked_add(total) == Some(after.bytes);
            if !correct {
                return Err(store_error("bundle_handoff_inventory_changed"));
            }
            Ok(after)
        })();
        match write {
            Ok(after) => {
                guard.armed = false;
                drop(guard);
                self.snapshot = after;
                self.remaining = remaining;
                Ok(())
            }
            Err(error) => {
                drop(guard);
                Err(JournalBundleWriteError::MayHaveApplied(error))
            }
        }
    }
}
