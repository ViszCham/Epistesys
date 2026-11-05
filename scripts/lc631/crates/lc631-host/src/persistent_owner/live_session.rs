//! Root-scoped storage session. It validates no journal semantics.
use super::*;

/// Holds the existing owner, root anchor, cooperating-writer lock and one
/// cumulative disk claim for its whole lifetime. This is not a journal cursor,
/// authority, authenticated head, rollback witness or OS-wide exclusion.
pub struct HostBinaryCheckpointLiveSession<'a> {
    owner: HostPersistentStoreOwner<'a>,
    next: u64,
    remaining: u64,
    poisoned: bool,
}

impl<'a> HostBinaryCheckpointLiveSession<'a> {
    /// Bootstrap under the existing owner lock. Even a failed open may create
    /// root/lock artifacts; it is not a NotApplied receipt.
    pub fn open(
        root: &Path,
        planned_total_bytes: u64,
        limits: PersistentStoreLimits,
        resources: &'a ResourceReservationView<'a>,
    ) -> Result<Self, HostError> {
        let target = root.join("segment-00000000000000000001.bin");
        let mut owner = HostPersistentStoreOwner::begin_binary_checkpoint_segment(
            root,
            &target,
            planned_total_bytes,
            limits,
            resources,
        )?;
        owner
            .scan
            .binary_segments
            .sort_by_key(BinaryCheckpointSegmentDescriptor::sequence);
        let segments = &owner.scan.binary_segments;
        for pair in segments.windows(2) {
            if pair[0].sequence().checked_add(1) != Some(pair[1].sequence()) {
                return Err(store_error("binary_checkpoint_sequence_gap"));
            }
        }
        let next = match segments.last() {
            Some(last) => last
                .sequence()
                .checked_add(1)
                .ok_or_else(|| store_error("binary_checkpoint_sequence_overflow"))?,
            None => 1,
        };
        Ok(Self {
            owner,
            next,
            remaining: planned_total_bytes,
            poisoned: false,
        })
    }

    fn active(&self) -> Result<(), HostError> {
        if self.poisoned {
            return Err(store_error("binary_live_session_poisoned"));
        }
        self.owner.resources.check().map_err(resource_error)?;
        self.owner.anchor.validate_current().map_err(io_error)?;
        self.owner.lock.validate_current().map_err(io_error)?;
        self.owner.resources.check().map_err(resource_error)
    }

    /// Fresh raw inventory. The runtime must independently validate every
    /// segment's bytes and chain under this same live session before appending.
    pub fn inventory(&mut self) -> Result<OwnedBinaryCheckpointInventory<'a>, HostError> {
        let result = (|| {
            self.active()?;
            let inventory = self.owner.binary_checkpoint_segment_inventory()?;
            if inventory.as_slice() != self.owner.scan.binary_segments.as_slice() {
                return Err(store_error("binary_live_inventory_changed"));
            }
            Ok(inventory)
        })();
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    /// Only a descriptor present in the currently held initial/updated raw
    /// snapshot is accepted. The owner checks exact bytes on the anchored read.
    pub fn read_descriptor(
        &mut self,
        descriptor: &BinaryCheckpointSegmentDescriptor,
    ) -> Result<OwnedBinaryCheckpointSegment<'a>, HostError> {
        let result = (|| {
            self.active()?;
            if !self
                .owner
                .scan
                .binary_segments
                .iter()
                .any(|known| known == descriptor)
            {
                return Err(store_error("binary_checkpoint_descriptor_unknown"));
            }
            self.owner
                .read_binary_checkpoint_segment_descriptor(descriptor)
        })();
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
        Ok(self.next)
    }

    pub fn remaining_bytes(&self) -> u64 {
        self.remaining
    }

    /// Storage-only append. Runtime must validate the candidate against its
    /// own validated live chain state. Errors poison this session. Before-write
    /// means this invocation did not admit a target write; opening the session
    /// may already have created coordination artifacts. MayHaveApplied preserves
    /// the target without retry, truncation, deletion, or success inference.
    pub fn append(
        &mut self,
        expected_sequence: u64,
        bytes: &[u8],
    ) -> Result<String, BinaryCheckpointSegmentWriteError> {
        let admission = (|| {
            self.active()?;
            if expected_sequence != self.next {
                return Err(store_error("binary_checkpoint_sequence_stale"));
            }
            let next = self
                .next
                .checked_add(1)
                .ok_or_else(|| store_error("binary_checkpoint_sequence_overflow"))?;
            let bytes_len =
                u64::try_from(bytes.len()).map_err(|_| store_error("byte_count_overflow"))?;
            if bytes_len == 0 || bytes_len > class_cap(self.owner.profile.target_class()) as u64 {
                return Err(store_error("binary_checkpoint_segment_size_invalid"));
            }
            let remaining = self
                .remaining
                .checked_sub(bytes_len)
                .ok_or_else(|| store_error("disk_reservation_exhausted"))?;
            let target = PathBuf::from(format!("segment-{expected_sequence:020}.bin"));
            // Raw scan may read/hash the whole root; it never parses history.
            let mut scan = scan_root(
                &self.owner.anchor,
                &target,
                self.owner.profile,
                self.owner.limits,
                self.owner.resources,
            )?;
            self.owner.lock.validate_current().map_err(io_error)?;
            self.owner.anchor.validate_current().map_err(io_error)?;
            self.owner.resources.check().map_err(resource_error)?;
            scan.binary_segments
                .sort_by_key(BinaryCheckpointSegmentDescriptor::sequence);
            if scan.binary_segments != self.owner.scan.binary_segments
                || scan.usage.files != self.owner.scan.usage.files
                || scan.usage.total_bytes != self.owner.scan.usage.total_bytes
            {
                return Err(store_error("binary_live_inventory_changed"));
            }
            if scan.target_bytes.is_some() {
                return Err(store_error("immutable_record_already_exists"));
            }
            prospective_files(&scan, self.owner.limits)?;
            if self.owner.writing && !self.owner.handed_off {
                return Err(store_error("binary_checkpoint_segment_handoff_unverified"));
            }
            // Retain an accounted snapshot across the write so a change
            // observed by confirm_handoff cannot become the next baseline.
            let previous = self.owner.binary_checkpoint_segment_inventory()?;
            if previous.as_slice() != scan.binary_segments.as_slice() {
                return Err(store_error("binary_live_inventory_changed"));
            }
            let prior_files = scan.usage.files;
            let prior_bytes = scan.usage.total_bytes;
            self.owner.target = target;
            self.owner.scan = scan;
            self.owner.writing = false;
            self.owner.handed_off = false;
            Ok((
                next,
                remaining,
                previous,
                prior_files,
                prior_bytes,
                bytes_len,
            ))
        })();
        let (next, remaining, previous, prior_files, prior_bytes, bytes_len) = match admission {
            Ok(values) => values,
            Err(error) => {
                self.poisoned = true;
                return Err(BinaryCheckpointSegmentWriteError::RejectedBeforeWrite(
                    error,
                ));
            }
        };
        match self.owner.write_binary_checkpoint_segment(bytes) {
            Ok(digest) => {
                // confirm_handoff has completed; preserve its scan as the next
                // raw baseline, without claiming journal validation.
                self.owner
                    .scan
                    .binary_segments
                    .sort_by_key(BinaryCheckpointSegmentDescriptor::sequence);
                let after = &self.owner.scan.binary_segments;
                let expected_files = prior_files.checked_add(1);
                let expected_bytes = prior_bytes.checked_add(bytes_len);
                let snapshot_ok = previous.len().checked_add(1) == Some(after.len())
                    && after.iter().take(previous.len()).eq(previous.iter())
                    && after.last().is_some_and(|last| {
                        last.sequence() == expected_sequence
                            && last.byte_len() == bytes_len
                            && last.digest() == digest
                    })
                    && expected_files == Some(self.owner.scan.usage.files)
                    && expected_bytes == Some(self.owner.scan.usage.total_bytes);
                if !snapshot_ok {
                    self.poisoned = true;
                    self.owner.resources.record_cleanup_unknown();
                    return Err(BinaryCheckpointSegmentWriteError::MayHaveApplied(
                        store_error("binary_live_handoff_inventory_changed"),
                    ));
                }
                self.next = next;
                self.remaining = remaining;
                Ok(digest)
            }
            Err(error) => {
                self.poisoned = true;
                Err(error)
            }
        }
    }
}
