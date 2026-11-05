//! Existing marked roots only. No file, marker or writer-lock creation.
use super::*;

/// A bounded raw-byte snapshot, not an authenticated or semantic bundle.
pub struct HostJournalBundleReader<'a> {
    anchor: DirectoryAnchor,
    resources: &'a ResourceReservationView<'a>,
    limits: JournalBundleLimits,
    snapshot: Snapshot,
    poisoned: bool,
    // Snapshot allocation drops before its retained reservation.
    _claim: ResourceClaim<'a>,
}

impl<'a> HostJournalBundleReader<'a> {
    pub fn open_existing(
        root: &Path,
        limits: JournalBundleLimits,
        resources: &'a ResourceReservationView<'a>,
    ) -> Result<Self, HostError> {
        let claim = resources
            .claim_resources(&[
                resource(ResourceKind::Ram, SESSION_SCRATCH)?,
                resource(ResourceKind::Output, SESSION_SCRATCH)?,
            ])
            .map_err(resource_error)?;
        let root = std::path::absolute(root).map_err(io_error)?;
        let anchor = DirectoryAnchor::open_existing(&root).map_err(io_error)?;
        let snapshot = scan(&anchor, limits, resources)?;
        if !snapshot.marker {
            return Err(store_error("read_only_bundle_marker_required"));
        }
        next_complete(&snapshot.entries)?;
        anchor.validate_current().map_err(io_error)?;
        resources.check().map_err(resource_error)?;
        Ok(Self {
            anchor,
            resources,
            limits,
            snapshot,
            poisoned: false,
            _claim: claim,
        })
    }

    fn active(&self) -> Result<(), HostError> {
        if self.poisoned {
            return Err(store_error("bundle_reader_poisoned"));
        }
        self.resources.check().map_err(resource_error)?;
        self.anchor.validate_current().map_err(io_error)?;
        self.resources.check().map_err(resource_error)
    }

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

    /// Compare current raw inventory once after consumer validation. This is
    /// non-atomic and does not detect joint rollback or establish freshness.
    pub fn validate_snapshot(&mut self) -> Result<(), HostError> {
        let result = (|| {
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
            Ok(())
        })();
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    /// Captured sequence only; callers must separately validate the snapshot.
    pub fn next_sequence(&mut self) -> Result<u64, HostError> {
        let result = self
            .active()
            .and_then(|_| next_complete(&self.snapshot.entries));
        if result.is_err() {
            self.poisoned = true;
        }
        result
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
}
