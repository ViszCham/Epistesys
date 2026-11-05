//! Actual anchored local capabilities; access scope is not an Authority Grant.
use super::{
    codec::{decode, ScopedCapture},
    types::*,
    FILE, MAX_REPLAY_BYTES,
};
use lc631_host::{
    persistent_owner::{HostPersistentStoreReader, PersistentStoreLimits},
    HostPersistentStoreOwner, OwnedBinaryCheckpointSegment,
};
use lc631_substrate::resource::{ResourceClaim, ResourceReservationView};
use std::path::{Component, Path};

/// Lexical local-root admission only; not permission or a filesystem capability.
pub fn validate_store_root(root: &Path) -> Result<(), ReplayError> {
    if !root.is_absolute()
        || root.file_name().is_none()
        || root.as_os_str().len() > 4096
        || root.components().count() > 64
    {
        return Err(ReplayError::Root);
    }
    for component in root.components() {
        match component {
            Component::RootDir | Component::Normal(_) => {}
            #[cfg(windows)]
            Component::Prefix(prefix)
                if matches!(
                    prefix.kind(),
                    std::path::Prefix::Disk(_) | std::path::Prefix::VerbatimDisk(_)
                ) => {}
            _ => return Err(ReplayError::Root),
        }
    }
    Ok(())
}
fn limits() -> PersistentStoreLimits {
    PersistentStoreLimits {
        max_files: 4,
        max_total_bytes: MAX_REPLAY_BYTES as u64,
        max_depth: 1,
    }
}
/// Caller must authorize this exact local root separately. No result score,
/// Boolean, file hash or this storage receipt grants permission. One immutable
/// artifact per root; no replacement, eviction, replay-state or cleanup API.
pub fn persist_current(
    root: &Path,
    capture: &ScopedCapture<'_, '_, '_>,
    view: &ResourceReservationView<'_>,
) -> Result<(), ReplayError> {
    capture.check(view)?;
    validate_store_root(root)?;
    let mut owner = HostPersistentStoreOwner::begin_binary_checkpoint_segment(
        root,
        &root.join(FILE),
        capture.bytes.len() as u64,
        limits(),
        view,
    )
    .map_err(|_| ReplayError::StorageOpenMayHaveApplied)?;
    #[cfg(test)]
    cancel_at_test_phase(TestPhase::AfterOpen);
    capture.check(view).map_err(ReplayError::after_open)?;
    owner
        .write_binary_checkpoint_segment(&capture.bytes)
        .map_err(|_| ReplayError::StorageMayHaveApplied)?;
    #[cfg(test)]
    after_write_cancel_hook();
    capture.check(view).map_err(ReplayError::after_write)?;
    Ok(())
}

#[cfg(test)]
std::thread_local! {
    static AFTER_WRITE_CANCEL: std::cell::RefCell<Option<(TestPhase, lc631_substrate::resource::Cancellation)>> = const { std::cell::RefCell::new(None) };
}
#[cfg(test)]
#[derive(Clone, Copy, Eq, PartialEq)]
enum TestPhase {
    AfterOpen,
    AfterWrite,
}
#[cfg(test)]
fn after_write_cancel_hook() {
    cancel_at_test_phase(TestPhase::AfterWrite);
}
#[cfg(test)]
fn cancel_at_test_phase(phase: TestPhase) {
    let cancellation = AFTER_WRITE_CANCEL.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot
            .as_ref()
            .is_some_and(|(expected, _)| *expected == phase)
        {
            slot.take().map(|(_, cancellation)| cancellation)
        } else {
            None
        }
    });
    if let Some(cancellation) = cancellation {
        cancellation.cancel();
    }
}
#[cfg(test)]
fn persist_with_after_write(
    root: &Path,
    capture: &ScopedCapture<'_, '_, '_>,
    view: &ResourceReservationView<'_>,
    cancellation: lc631_substrate::resource::Cancellation,
) -> Result<(), ReplayError> {
    persist_with_test_cancel(root, capture, view, cancellation, TestPhase::AfterWrite)
}
#[cfg(test)]
fn persist_with_after_open(
    root: &Path,
    capture: &ScopedCapture<'_, '_, '_>,
    view: &ResourceReservationView<'_>,
    cancellation: lc631_substrate::resource::Cancellation,
) -> Result<(), ReplayError> {
    persist_with_test_cancel(root, capture, view, cancellation, TestPhase::AfterOpen)
}
#[cfg(test)]
fn persist_with_test_cancel(
    root: &Path,
    capture: &ScopedCapture<'_, '_, '_>,
    view: &ResourceReservationView<'_>,
    cancellation: lc631_substrate::resource::Cancellation,
    phase: TestPhase,
) -> Result<(), ReplayError> {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            AFTER_WRITE_CANCEL.with(|slot| {
                slot.borrow_mut().take();
            });
        }
    }
    let previous = AFTER_WRITE_CANCEL.with(|slot| slot.borrow_mut().replace((phase, cancellation)));
    assert!(
        previous.is_none(),
        "only one owned hook may be installed per test thread"
    );
    let _reset = Reset;
    persist_current(root, capture, view)
}
#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;

/// Private ownership of the real root anchor and its same-lease source binding.
/// No constructor accepts a reported successful read or a trusted=true flag.
pub struct CanonicalArtifactReader<'v> {
    host: HostPersistentStoreReader<'v>,
    key: ArtifactKey,
}
impl<'v> CanonicalArtifactReader<'v> {
    pub fn open(
        root: &Path,
        key: ArtifactKey,
        view: &'v ResourceReservationView<'v>,
    ) -> Result<Self, ReplayError> {
        view.check()?;
        if view.source_revision() != key.source {
            return Err(ReplayError::Binding);
        }
        validate_store_root(root)?;
        let host = HostPersistentStoreReader::open_binary_checkpoint_root(root, limits(), view)
            .map_err(|_| ReplayError::StorageRead)?;
        let reader = Self { host, key };
        reader.check(view)?;
        Ok(reader)
    }
    fn check(&self, view: &ResourceReservationView<'_>) -> Result<(), ReplayError> {
        view.check()?;
        if view.source_revision() != self.key.source {
            return Err(ReplayError::Binding);
        }
        let claim = self
            .host
            .claim_inspection_metadata(1)
            .map_err(|_| ReplayError::StorageRead)?;
        if !claim.same_reservation(view) {
            return Err(ReplayError::Binding);
        }
        let inventory = self
            .host
            .inventory()
            .map_err(|_| ReplayError::StorageRead)?;
        if inventory.len() != 1
            || inventory[0].sequence() != 1
            || inventory[0].byte_len() > MAX_REPLAY_BYTES as u64
        {
            return Err(ReplayError::Shape);
        }
        Ok(())
    }
    pub fn read<'r>(
        &'r self,
        view: &ResourceReservationView<'_>,
    ) -> Result<OwnedReplayArtifact<'r, 'v>, ReplayError> {
        self.check(view)?;
        let claim = self
            .host
            .claim_inspection_metadata(4096)
            .map_err(|_| ReplayError::StorageRead)?;
        let raw = self
            .host
            .read_binary_checkpoint_segment_at(1)
            .map_err(|_| ReplayError::StorageRead)?;
        decode(raw.as_bytes(), self.key)?;
        self.check(view)?;
        Ok(OwnedReplayArtifact {
            raw,
            reader: self,
            claim,
        })
    }
}
/// Raw historical bytes retain their native file-reader claim and borrow the
/// actual read capability; access checks do not renew historical model truth.
pub struct OwnedReplayArtifact<'r, 'v> {
    raw: OwnedBinaryCheckpointSegment<'v>,
    reader: &'r CanonicalArtifactReader<'v>,
    claim: ResourceClaim<'v>,
}
impl OwnedReplayArtifact<'_, '_> {
    pub fn inspect_current(
        &self,
        view: &ResourceReservationView<'_>,
    ) -> Result<HistoricalView<'_>, ReplayError> {
        self.claim.check()?;
        if !self.claim.same_reservation(view) {
            return Err(ReplayError::Binding);
        }
        self.reader.check(view)?;
        decode(self.raw.as_bytes(), self.reader.key)
    }
}
