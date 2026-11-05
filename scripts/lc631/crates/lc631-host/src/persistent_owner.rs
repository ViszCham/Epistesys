//! Per-owned-root persistence accounting, not global disk or OS isolation.
//! Every access reconstructs bounded usage from actual no-follow files. A
//! loose manifest, matching digest, or this owner never grants Authority.
pub mod journal_bundle;
#[cfg(test)]
#[path = "persistent_owner/journal_bundle/legacy_guard_tests.rs"]
mod legacy_guard_tests;
mod live_session;
mod policy;
mod reader_metadata;
mod scan;
use crate::HostError;
use lc631_process_supervisor::filesystem::{AnchoredFile, DirectoryAnchor};
use lc631_process_supervisor::{
    ResourceAmount, ResourceClaim, ResourceKind, ResourceRequest, ResourceReservationView,
};
pub use live_session::HostBinaryCheckpointLiveSession;
use policy::{class_cap, validate_limits, PersistentStoreProfile, LOCK};
pub use policy::{PersistentStoreKind, PersistentStoreLimits};
use scan::{scan_root, Scan};
use std::ops::Deref;
use std::path::{Component, Path, PathBuf};

const SCAN_RAM: u64 = 1024 * 1024;
const MAX_BINARY_CHECKPOINT_SEGMENTS: usize = 1_000;
const INVENTORY_BASE_CLAIM_BYTES: u64 = 4 * 1024;
const INVENTORY_ENTRY_CLAIM_BYTES: u64 = 256;

#[derive(Clone, Copy, Debug, Default)]
pub struct PersistentStoreUsage {
    pub files: usize,
    pub total_bytes: u64,
}

/// Bounded identity for one owner-scanned binary checkpoint segment.
/// The root-relative path is deliberately omitted; the canonical sequence is
/// sufficient to reconstruct only `segment-{sequence:020}.bin`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BinaryCheckpointSegmentDescriptor {
    sequence: u64,
    byte_len: u64,
    digest: String,
}

impl BinaryCheckpointSegmentDescriptor {
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub const fn byte_len(&self) -> u64 {
        self.byte_len
    }

    pub fn digest(&self) -> &str {
        &self.digest
    }
}

/// A bounded snapshot of owner-scanned binary segment descriptors.
///
/// The descriptor vector is declared before its retained claim so the
/// allocation is dropped before its RAM/output accounting is released. The
/// claim is tied to the injected lease and does not outlive this wrapper.
/// Caller-created descriptor clones or collections require separate caller
/// accounting; these logical claims are not an allocator-capacity or RSS bound.
pub struct OwnedBinaryCheckpointInventory<'a> {
    descriptors: Vec<BinaryCheckpointSegmentDescriptor>,
    _ram_claim: ResourceClaim<'a>,
    _output_claim: ResourceClaim<'a>,
}

impl OwnedBinaryCheckpointInventory<'_> {
    pub fn as_slice(&self) -> &[BinaryCheckpointSegmentDescriptor] {
        &self.descriptors
    }

    pub fn iter(&self) -> std::slice::Iter<'_, BinaryCheckpointSegmentDescriptor> {
        self.descriptors.iter()
    }

    pub fn len(&self) -> usize {
        self.descriptors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.descriptors.is_empty()
    }
}

impl Deref for OwnedBinaryCheckpointInventory<'_> {
    type Target = [BinaryCheckpointSegmentDescriptor];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl AsRef<[BinaryCheckpointSegmentDescriptor]> for OwnedBinaryCheckpointInventory<'_> {
    fn as_ref(&self) -> &[BinaryCheckpointSegmentDescriptor] {
        self.as_slice()
    }
}

/// Raw bytes read from one owner-managed binary checkpoint segment.
///
/// The byte vector is declared before the claim so Rust drops the allocation
/// before releasing its RAM/output accounting. Its contents remain unparsed and
/// untrusted until a runtime journal consumer validates the frame.
pub struct OwnedBinaryCheckpointSegment<'a> {
    bytes: Vec<u8>,
    _claim: ResourceClaim<'a>,
}

impl OwnedBinaryCheckpointSegment<'_> {
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

/// Binary-segment persistence failures keep pre-write rejection distinct from
/// failures after owner admission, where bytes may already have been applied.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BinaryCheckpointSegmentWriteError {
    RejectedBeforeWrite(HostError),
    MayHaveApplied(HostError),
}

pub struct HostPersistentStoreOwner<'a> {
    anchor: DirectoryAnchor,
    lock: AnchoredFile,
    target: PathBuf,
    profile: PersistentStoreProfile,
    limits: PersistentStoreLimits,
    planned_bytes: u64,
    scan: Scan,
    resources: &'a ResourceReservationView<'a>,
    _claim: ResourceClaim<'a>,
    writing: bool,
    handed_off: bool,
}

/// Read-only view of an existing binary checkpoint-segment root.
///
/// Opening this reader never creates the root or an owner lock. The inventory
/// is a bounded snapshot collected once at open; each later read is still
/// bound to its descriptor by an anchored no-follow open, exact size, and raw
/// digest verification. The inventory claim remains live as long as its
/// descriptor allocation does.
pub struct HostPersistentStoreReader<'a> {
    anchor: DirectoryAnchor,
    resources: &'a ResourceReservationView<'a>,
    inventory: Vec<BinaryCheckpointSegmentDescriptor>,
    _inventory_claim: ResourceClaim<'a>,
}

// A marked root belongs to the bundle profile, even if no sidecar exists yet.
// Only confirmed absence permits the legacy reader; all other outcomes hold.
fn reject_bundle_marker(anchor: &DirectoryAnchor) -> Result<(), HostError> {
    match anchor.open_read_only(Path::new(".journal-bundle-v1")) {
        Ok(_) => Err(store_error("bundle_marker_not_legacy")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(error)),
    }
}

impl<'a> HostPersistentStoreReader<'a> {
    /// Open an existing binary segment root without creating or modifying any
    /// filesystem entry. Mixed/unknown entries and quota failures reject the
    /// whole snapshot; no partial inventory escapes.
    pub fn open_binary_checkpoint_root(
        root: &Path,
        limits: PersistentStoreLimits,
        resources: &'a ResourceReservationView<'a>,
    ) -> Result<Self, HostError> {
        Self::open_binary_checkpoint_root_after_scan(root, limits, resources, || Ok(()))
    }

    fn open_binary_checkpoint_root_after_scan(
        root: &Path,
        limits: PersistentStoreLimits,
        resources: &'a ResourceReservationView<'a>,
        after_scan: impl FnOnce() -> Result<(), HostError>,
    ) -> Result<Self, HostError> {
        validate_limits(limits)?;
        let profile = PersistentStoreProfile::binary_checkpoint_segment();
        let limits = profile.constrain(limits);
        let inventory_claim = resources
            .claim_resources(&[
                resource(ResourceKind::Ram, SCAN_RAM)?,
                resource(ResourceKind::Output, SCAN_RAM)?,
            ])
            .map_err(resource_error)?;
        resources.check().map_err(resource_error)?;

        let root = std::path::absolute(root).map_err(io_error)?;
        let anchor = DirectoryAnchor::open_existing(&root).map_err(io_error)?;
        reject_bundle_marker(&anchor)?;
        let mut scan = scan_root(&anchor, Path::new(""), profile, limits, resources)?;
        after_scan()?;
        reject_bundle_marker(&anchor)?;
        anchor.validate_current().map_err(io_error)?;
        resources.check().map_err(resource_error)?;

        scan.binary_segments
            .sort_by_key(BinaryCheckpointSegmentDescriptor::sequence);
        if scan
            .binary_segments
            .windows(2)
            .any(|pair| pair[0].sequence() >= pair[1].sequence())
        {
            return Err(store_error("binary_segment_sequence_collision"));
        }

        Ok(Self {
            anchor,
            resources,
            inventory: scan.binary_segments,
            _inventory_claim: inventory_claim,
        })
    }

    /// Return the complete inventory snapshot only while its anchored root and
    /// resource lease remain current. This does not rescan or claim freshness
    /// against later directory mutations.
    pub fn inventory(&self) -> Result<&[BinaryCheckpointSegmentDescriptor], HostError> {
        self.resources.check().map_err(resource_error)?;
        reject_bundle_marker(&self.anchor)?;
        self.anchor.validate_current().map_err(io_error)?;
        self.resources.check().map_err(resource_error)?;
        Ok(&self.inventory)
    }

    /// Read a sequence already present in this reader's one-time inventory.
    /// No path is accepted and no root-wide rescan is performed.
    pub fn read_binary_checkpoint_segment_at(
        &self,
        sequence: u64,
    ) -> Result<OwnedBinaryCheckpointSegment<'a>, HostError> {
        let descriptor = self.inventory().and_then(|inventory| {
            inventory
                .iter()
                .find(|descriptor| descriptor.sequence() == sequence)
                .ok_or_else(|| store_error("binary_checkpoint_sequence_missing"))
        })?;
        self.read_binary_checkpoint_segment_descriptor(descriptor)
    }

    /// Read only an exact descriptor issued by this reader's inventory. The
    /// descriptor is revalidated against current bytes on every read.
    pub fn read_binary_checkpoint_segment_descriptor(
        &self,
        descriptor: &BinaryCheckpointSegmentDescriptor,
    ) -> Result<OwnedBinaryCheckpointSegment<'a>, HostError> {
        self.resources.check().map_err(resource_error)?;
        reject_bundle_marker(&self.anchor)?;
        self.anchor.validate_current().map_err(io_error)?;
        if !self.inventory.iter().any(|known| known == descriptor) {
            return Err(store_error("binary_checkpoint_descriptor_unknown"));
        }
        let canonical_name = format!("segment-{:020}.bin", descriptor.sequence());
        let result = read_binary_checkpoint_target(
            &self.anchor,
            None,
            self.resources,
            Path::new(&canonical_name),
            descriptor.byte_len(),
            descriptor.digest(),
        )?;
        reject_bundle_marker(&self.anchor)?;
        Ok(result)
    }
}

impl<'a> HostPersistentStoreOwner<'a> {
    pub fn begin(
        root: &Path,
        target: &Path,
        kind: PersistentStoreKind,
        planned_bytes: u64,
        limits: PersistentStoreLimits,
        resources: &'a ResourceReservationView<'a>,
    ) -> Result<Self, HostError> {
        Self::begin_with_profile(
            root,
            target,
            PersistentStoreProfile::Registered(kind),
            planned_bytes,
            limits,
            resources,
        )
    }

    /// Begin an immutable owner-managed binary journal segment. The accepted
    /// path profile and byte cap are private; public legacy store kinds remain
    /// unchanged. This method accounts storage only and does not validate the
    /// segment's semantic frame or provide an external rollback anchor.
    pub fn begin_binary_checkpoint_segment(
        root: &Path,
        target: &Path,
        planned_bytes: u64,
        limits: PersistentStoreLimits,
        resources: &'a ResourceReservationView<'a>,
    ) -> Result<Self, HostError> {
        Self::begin_with_profile(
            root,
            target,
            PersistentStoreProfile::binary_checkpoint_segment(),
            planned_bytes,
            limits,
            resources,
        )
    }

    fn begin_with_profile(
        root: &Path,
        target: &Path,
        profile: PersistentStoreProfile,
        planned_bytes: u64,
        limits: PersistentStoreLimits,
        resources: &'a ResourceReservationView<'a>,
    ) -> Result<Self, HostError> {
        validate_limits(limits)?;
        let limits = profile.constrain(limits);
        let root = std::path::absolute(root).map_err(io_error)?;
        let target = std::path::absolute(target).map_err(io_error)?;
        let relative = target
            .strip_prefix(&root)
            .map_err(|_| store_error("target_outside_root"))?;
        if relative.as_os_str().is_empty()
            || !relative
                .components()
                .all(|part| matches!(part, Component::Normal(_)))
            || relative.components().count() > limits.max_depth
        {
            return Err(store_error("target_path_invalid"));
        }
        if profile.classify(relative, relative)? != profile.target_class() || planned_bytes == 0 {
            return Err(store_error("target_kind_invalid"));
        }
        let claim = resources
            .claim_resources(&[
                resource(ResourceKind::Disk, planned_bytes)?,
                resource(ResourceKind::Ram, SCAN_RAM)?,
            ])
            .map_err(resource_error)?;
        // Inspect existing roots read-only before creating even a zero-byte
        // coordination lock. Unsupported roots are not silently appropriated.
        if root.exists() {
            let existing = DirectoryAnchor::open_existing(&root).map_err(io_error)?;
            let initial = scan_root(&existing, relative, profile, limits, resources)?;
            prospective_files(&initial, limits)?;
        } else if limits.max_files < if profile.is_checkpoint() { 3 } else { 2 } {
            return Err(store_error("file_count_exhausted"));
        }
        let anchor = create_owned_root(&root)?;
        let mut lock = anchor
            .open_or_create_file(Path::new(LOCK))
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::WouldBlock
                    || error.raw_os_error() == Some(32)
                {
                    store_error("root_busy")
                } else {
                    io_error(error)
                }
            })?;
        if lock.metadata().map_err(io_error)?.len() != 0 {
            return Err(store_error("corrupt_lock"));
        }
        lock.append_and_sync(&[]).map_err(io_error)?;
        let scan = scan_root(&anchor, relative, profile, limits, resources)?;
        prospective_files(&scan, limits)?;
        Ok(Self {
            anchor,
            lock,
            target: relative.to_path_buf(),
            profile,
            limits,
            planned_bytes,
            scan,
            resources,
            _claim: claim,
            writing: false,
            handed_off: false,
        })
    }

    pub fn usage(&self) -> PersistentStoreUsage {
        self.scan.usage
    }

    /// Return the canonical target's local file sequence while the owner lock
    /// and anchored root remain valid. This is a storage locator, not a
    /// journal cursor, authenticated head, or rollback witness.
    pub fn binary_checkpoint_target_sequence(&self) -> Result<u64, HostError> {
        if self.profile != PersistentStoreProfile::BinaryCheckpointSegment {
            return Err(store_error("binary_checkpoint_segment_profile_required"));
        }
        self.resources.check().map_err(resource_error)?;
        self.anchor.validate_current().map_err(io_error)?;
        self.lock.validate_current().map_err(io_error)?;
        policy::binary_segment_sequence(&self.target)
    }

    /// Rescan this owned root and return a complete bounded inventory of its
    /// canonical binary segment files. Unknown/mixed entries or quota overflow
    /// reject the whole inventory; no partial list is returned. Its bounded
    /// descriptor allocation retains a distinct RAM/output claim even if this
    /// owner is dropped. This is local storage identity only, not a parsed
    /// journal chain or trusted head.
    pub fn binary_checkpoint_segment_inventory(
        &self,
    ) -> Result<OwnedBinaryCheckpointInventory<'a>, HostError> {
        if self.profile != PersistentStoreProfile::BinaryCheckpointSegment {
            return Err(store_error("binary_checkpoint_segment_profile_required"));
        }
        if self.writing && !self.handed_off {
            return Err(store_error("binary_checkpoint_segment_handoff_unverified"));
        }
        self.resources.check().map_err(resource_error)?;
        self.anchor.validate_current().map_err(io_error)?;
        self.lock.validate_current().map_err(io_error)?;
        let inventory_claim_bytes = inventory_claim_bytes(self.limits)?;
        // Reserve output before scanning, then retain a separate RAM claim for
        // the returned allocation. The cap is derived from the profile's
        // maximum descriptor count; it is operational claim accounting, not
        // measured allocator use or RSS.
        let inventory_output_claim = self
            .resources
            .claim_resources(&[resource(ResourceKind::Output, inventory_claim_bytes)?])
            .map_err(resource_error)?;
        let inventory_ram_claim = self
            .resources
            .claim_resources(&[resource(ResourceKind::Ram, inventory_claim_bytes)?])
            .map_err(resource_error)?;
        self.resources.check().map_err(resource_error)?;
        let mut scan = scan_root(
            &self.anchor,
            &self.target,
            self.profile,
            self.limits,
            self.resources,
        )?;
        self.lock.validate_current().map_err(io_error)?;
        self.anchor.validate_current().map_err(io_error)?;
        self.resources.check().map_err(resource_error)?;

        scan.binary_segments
            .sort_by_key(|segment| segment.sequence());
        if scan
            .binary_segments
            .windows(2)
            .any(|pair| pair[0].sequence() >= pair[1].sequence())
        {
            return Err(store_error("binary_segment_sequence_collision"));
        }
        Ok(OwnedBinaryCheckpointInventory {
            descriptors: scan.binary_segments,
            _ram_claim: inventory_ram_claim,
            _output_claim: inventory_output_claim,
        })
    }

    pub fn before_write(&mut self, bytes: u64) -> Result<(), HostError> {
        if self.writing || self.handed_off {
            return Err(store_error("write_already_admitted"));
        }
        self.resources.check().map_err(resource_error)?;
        self.anchor.validate_current().map_err(io_error)?;
        self.lock.validate_current().map_err(io_error)?;
        let kind = self.profile.kind();
        if (bytes == 0 && kind != PersistentStoreKind::RepairBackup) || bytes > self.planned_bytes {
            return Err(store_error("disk_reservation_exhausted"));
        }
        if kind == PersistentStoreKind::ReceiptKey && self.scan.target_bytes.is_some() {
            return Err(store_error("key_rotation_requires_a_separate_contract"));
        }
        if self.profile.is_flat() && self.scan.target_bytes.is_some() {
            return Err(store_error("immutable_record_already_exists"));
        }
        let growth =
            if kind == PersistentStoreKind::StopObservation && self.scan.target_bytes.is_some() {
                0
            } else {
                bytes
            };
        let file_size = if kind == PersistentStoreKind::ReplayLedger {
            self.scan.target_bytes.unwrap_or(0).checked_add(bytes)
        } else {
            Some(bytes)
        }
        .ok_or_else(|| store_error("byte_count_overflow"))?;
        if file_size > class_cap(self.profile.target_class()) as u64
            || self
                .scan
                .usage
                .total_bytes
                .checked_add(growth)
                .is_none_or(|total| total > self.limits.max_total_bytes)
        {
            return Err(store_error("root_byte_budget_exhausted"));
        }
        self.writing = true;
        Ok(())
    }

    /// Write one bounded binary checkpoint segment through the anchored owner.
    /// The caller must validate the EPITLCX1 segment before calling this
    /// storage-only method. It checks the exact persisted raw-byte digest after
    /// a synced handoff; it does not authenticate the frame or establish an
    /// external rollback anchor.
    pub fn write_binary_checkpoint_segment(
        &mut self,
        bytes: &[u8],
    ) -> Result<String, BinaryCheckpointSegmentWriteError> {
        if self.profile != PersistentStoreProfile::BinaryCheckpointSegment {
            return Err(BinaryCheckpointSegmentWriteError::RejectedBeforeWrite(
                store_error("binary_checkpoint_segment_profile_required"),
            ));
        }
        let planned_bytes = u64::try_from(bytes.len()).map_err(|_| {
            BinaryCheckpointSegmentWriteError::RejectedBeforeWrite(store_error(
                "byte_count_overflow",
            ))
        })?;
        self.before_write(planned_bytes)
            .map_err(BinaryCheckpointSegmentWriteError::RejectedBeforeWrite)?;
        let write_result = (|| {
            let mut target = self.create_target()?;
            target.append_and_sync(bytes).map_err(io_error)?;
            target.validate_current().map_err(io_error)?;
            drop(target);
            let digest = lc631_receipt_kernel::digest_bytes(bytes);
            self.confirm_handoff(&digest)?;
            Ok(digest)
        })();
        write_result.map_err(BinaryCheckpointSegmentWriteError::MayHaveApplied)
    }

    /// Read the exact bounded bytes from the owner-managed binary segment.
    /// The returned RAM/output claim is tied to the caller's resource lease and
    /// outlives this storage owner until the byte buffer itself is dropped.
    /// This verifies storage identity only; a journal consumer must still
    /// decode/validate the EPITLCX1 frame and predecessor chain.
    pub fn read_binary_checkpoint_segment(
        &self,
    ) -> Result<OwnedBinaryCheckpointSegment<'a>, HostError> {
        if self.profile != PersistentStoreProfile::BinaryCheckpointSegment {
            return Err(store_error("binary_checkpoint_segment_profile_required"));
        }
        if self.writing && !self.handed_off {
            return Err(store_error("binary_checkpoint_segment_handoff_unverified"));
        }

        let expected_bytes = self
            .scan
            .target_bytes
            .ok_or_else(|| store_error("target_missing"))?;
        let expected_digest = self
            .scan
            .target_digest
            .as_deref()
            .ok_or_else(|| store_error("binary_checkpoint_segment_digest_missing"))?;
        self.read_binary_checkpoint_target(&self.target, expected_bytes, expected_digest)
    }

    /// Read a segment by its canonical fixed sequence, but only if a fresh
    /// anchored inventory contains that sequence. The descriptor returned by
    /// that inventory supplies the exact expected size and digest for the
    /// subsequent no-follow read; no caller-controlled path is accepted.
    pub fn read_binary_checkpoint_segment_at(
        &self,
        sequence: u64,
    ) -> Result<OwnedBinaryCheckpointSegment<'a>, HostError> {
        if self.profile != PersistentStoreProfile::BinaryCheckpointSegment {
            return Err(store_error("binary_checkpoint_segment_profile_required"));
        }
        if self.writing && !self.handed_off {
            return Err(store_error("binary_checkpoint_segment_handoff_unverified"));
        }
        let inventory = self.binary_checkpoint_segment_inventory()?;
        let descriptor = inventory
            .iter()
            .find(|descriptor| descriptor.sequence() == sequence)
            .ok_or_else(|| store_error("binary_checkpoint_sequence_missing"))?;
        self.read_binary_checkpoint_segment_descriptor(descriptor)
    }

    /// Read one segment against an exact owner-issued inventory descriptor.
    /// Unlike the sequence convenience method, this does not rescan other root
    /// entries. It still validates the no-follow anchored handle and checks
    /// the descriptor's exact size and digest after the bounded read.
    pub fn read_binary_checkpoint_segment_descriptor(
        &self,
        descriptor: &BinaryCheckpointSegmentDescriptor,
    ) -> Result<OwnedBinaryCheckpointSegment<'a>, HostError> {
        if self.profile != PersistentStoreProfile::BinaryCheckpointSegment {
            return Err(store_error("binary_checkpoint_segment_profile_required"));
        }
        if self.writing && !self.handed_off {
            return Err(store_error("binary_checkpoint_segment_handoff_unverified"));
        }
        let canonical_name = format!("segment-{:020}.bin", descriptor.sequence());
        if policy::binary_segment_sequence(Path::new(&canonical_name))? != descriptor.sequence() {
            return Err(store_error("binary_checkpoint_sequence_invalid"));
        }
        self.read_binary_checkpoint_target(
            Path::new(&canonical_name),
            descriptor.byte_len(),
            descriptor.digest(),
        )
    }

    fn read_binary_checkpoint_target(
        &self,
        target: &Path,
        expected_bytes: u64,
        expected_digest: &str,
    ) -> Result<OwnedBinaryCheckpointSegment<'a>, HostError> {
        read_binary_checkpoint_target(
            &self.anchor,
            Some(&self.lock),
            self.resources,
            target,
            expected_bytes,
            expected_digest,
        )
    }

    pub fn confirm_handoff(&mut self, expected_target_digest: &str) -> Result<(), HostError> {
        let result = self.confirm_handoff_inner(expected_target_digest);
        if result.is_err() {
            self.resources.record_cleanup_unknown();
        }
        result
    }

    fn confirm_handoff_inner(&mut self, expected_target_digest: &str) -> Result<(), HostError> {
        if !self.writing && self.scan.target_digest.as_deref() != Some(expected_target_digest) {
            return Err(store_error("write_not_admitted"));
        }
        self.resources.check().map_err(resource_error)?;
        // Re-open the actual target under the pinned root. Syncing an empty
        // append changes no payload and does not trust a caller's sync boolean.
        let mut target = self
            .anchor
            .open_read_write(&self.target)
            .map_err(io_error)?;
        target.append_and_sync(&[]).map_err(io_error)?;
        target.validate_current().map_err(io_error)?;
        drop(target);
        let scan = scan_root(
            &self.anchor,
            &self.target,
            self.profile,
            self.limits,
            self.resources,
        )?;
        if scan.target_digest.as_deref() != Some(expected_target_digest) {
            return Err(store_error("durable_target_digest_mismatch"));
        }
        self.lock.validate_current().map_err(io_error)?;
        self.scan = scan;
        self.handed_off = true;
        Ok(())
    }

    pub(crate) fn confirm_existing_handoff(&mut self) -> Result<(), HostError> {
        let expected = self
            .scan
            .target_digest
            .clone()
            .ok_or_else(|| store_error("target_missing"))?;
        self.confirm_handoff(&expected)
    }

    pub fn target_exists(&self) -> bool {
        self.scan.target_bytes.is_some()
    }

    pub(crate) fn create_target(&self) -> Result<AnchoredFile, HostError> {
        self.resources.check().map_err(resource_error)?;
        self.anchor.validate_current().map_err(io_error)?;
        let parent = self
            .anchor
            .directory(self.target.parent().unwrap_or_else(|| Path::new("")), true)
            .map_err(io_error)?;
        let name = self
            .target
            .file_name()
            .ok_or_else(|| store_error("target_missing_name"))?;
        parent
            .create_private_file(Path::new(name))
            .map_err(io_error)
    }

    pub(crate) fn read_key(&self) -> Result<[u8; 32], HostError> {
        if self.profile.kind() != PersistentStoreKind::ReceiptKey {
            return Err(store_error("not_a_key_store"));
        }
        self.with_target_bytes(|bytes| {
            let text = std::str::from_utf8(bytes).map_err(|_| store_error("invalid_utf8"))?;
            crate::decode_key(text.trim())
        })
    }

    pub(crate) fn target_matches(&self, expected: &[u8]) -> Result<bool, HostError> {
        self.with_target_bytes(|bytes| Ok(bytes == expected))
    }

    fn with_target_bytes<T>(
        &self,
        consume: impl FnOnce(&[u8]) -> Result<T, HostError>,
    ) -> Result<T, HostError> {
        let cap = class_cap(self.profile.target_class());
        let _claim = self
            .resources
            .claim_resources(&[
                resource(ResourceKind::Ram, cap as u64)?,
                resource(ResourceKind::Output, cap as u64)?,
            ])
            .map_err(resource_error)?;
        let mut file = self.anchor.open_read_only(&self.target).map_err(io_error)?;
        let bytes = file.read_bounded(cap).map_err(io_error)?;
        file.validate_current().map_err(io_error)?;
        self.resources.check().map_err(resource_error)?;
        consume(&bytes)
    }
}

fn read_binary_checkpoint_target<'a>(
    anchor: &DirectoryAnchor,
    lock: Option<&AnchoredFile>,
    resources: &'a ResourceReservationView<'a>,
    target: &Path,
    expected_bytes: u64,
    expected_digest: &str,
) -> Result<OwnedBinaryCheckpointSegment<'a>, HostError> {
    let profile = PersistentStoreProfile::binary_checkpoint_segment();
    if profile.classify(target, target)? != policy::FileClass::CheckpointSegment {
        return Err(store_error("binary_checkpoint_segment_name_invalid"));
    }
    let limit = class_cap(profile.target_class());
    if expected_bytes > limit as u64 {
        return Err(store_error("binary_checkpoint_segment_read_cap_exceeded"));
    }
    if expected_bytes == 0 {
        return Err(store_error("binary_checkpoint_segment_empty"));
    }
    let expected_limit =
        usize::try_from(expected_bytes).map_err(|_| store_error("read_limit_overflow"))?;
    let charged_limit = expected_bytes
        .checked_add(1)
        .ok_or_else(|| store_error("read_limit_overflow"))?;
    let claim = resources
        .claim_resources(&[
            resource(ResourceKind::Ram, charged_limit)?,
            resource(ResourceKind::Output, charged_limit)?,
        ])
        .map_err(resource_error)?;

    resources.check().map_err(resource_error)?;
    anchor.validate_current().map_err(io_error)?;
    if let Some(lock) = lock {
        lock.validate_current().map_err(io_error)?;
    }
    let mut file = anchor.open_read_only(target).map_err(io_error)?;
    let bytes = file.read_bounded(expected_limit).map_err(io_error)?;
    file.validate_current().map_err(io_error)?;
    drop(file);
    if let Some(lock) = lock {
        lock.validate_current().map_err(io_error)?;
    }
    anchor.validate_current().map_err(io_error)?;
    resources.check().map_err(resource_error)?;

    let observed_bytes =
        u64::try_from(bytes.len()).map_err(|_| store_error("read_size_overflow"))?;
    if observed_bytes != expected_bytes {
        return Err(store_error("binary_checkpoint_segment_size_changed"));
    }
    let observed_digest = lc631_receipt_kernel::digest_bytes(&bytes);
    if observed_digest != expected_digest {
        return Err(store_error("binary_checkpoint_segment_digest_changed"));
    }

    Ok(OwnedBinaryCheckpointSegment {
        bytes,
        _claim: claim,
    })
}

impl Drop for HostPersistentStoreOwner<'_> {
    fn drop(&mut self) {
        if self.writing && !self.handed_off {
            self.resources.record_cleanup_unknown();
        }
        // No deletion, repair, or runtime reservation refund occurs here.
    }
}

fn prospective_files(scan: &Scan, limits: PersistentStoreLimits) -> Result<(), HostError> {
    let additional = usize::from(!scan.lock_present)
        + usize::from(scan.target_bytes.is_none())
        + usize::from(scan.checkpoint && !scan.writer_lock_present);
    if scan
        .usage
        .files
        .checked_add(additional)
        .is_none_or(|files| files > limits.max_files)
    {
        Err(store_error("file_count_exhausted"))
    } else {
        Ok(())
    }
}

fn create_owned_root(root: &Path) -> Result<DirectoryAnchor, HostError> {
    if let Ok(anchor) = DirectoryAnchor::open_existing(root) {
        return Ok(anchor);
    }
    let mut existing = root;
    let mut missing = 0;
    while !existing.exists() {
        missing += 1;
        if missing > 4 {
            return Err(store_error("root_creation_depth_exhausted"));
        }
        existing = existing
            .parent()
            .ok_or_else(|| store_error("root_parent_missing"))?;
    }
    let base = DirectoryAnchor::open_existing(existing).map_err(io_error)?;
    base.directory(
        root.strip_prefix(existing)
            .map_err(|_| store_error("root_scope_invalid"))?,
        true,
    )
    .map_err(io_error)
}

fn inventory_claim_bytes(limits: PersistentStoreLimits) -> Result<u64, HostError> {
    let max_entries = limits.max_files.min(MAX_BINARY_CHECKPOINT_SEGMENTS);
    let max_entries =
        u64::try_from(max_entries).map_err(|_| store_error("inventory_claim_overflow"))?;
    max_entries
        .checked_mul(INVENTORY_ENTRY_CLAIM_BYTES)
        .and_then(|bytes| bytes.checked_add(INVENTORY_BASE_CLAIM_BYTES))
        .ok_or_else(|| store_error("inventory_claim_overflow"))
}

fn resource(kind: ResourceKind, amount: u64) -> Result<ResourceRequest, HostError> {
    Ok(ResourceRequest::new(
        kind,
        ResourceAmount::checked(amount).map_err(resource_error)?,
    ))
}
fn store_error(reason: &str) -> HostError {
    HostError::Io(format!("persistent_store:{reason}"))
}
fn resource_error(error: lc631_process_supervisor::ResourceError) -> HostError {
    HostError::Io(format!("persistent_resource:{error:?}"))
}
fn io_error(error: std::io::Error) -> HostError {
    HostError::Io(format!("persistent_store_io:{error}"))
}
