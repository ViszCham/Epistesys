use lc631_host::{
    BinaryCheckpointSegmentWriteError, HostError, HostPersistentStoreOwner, PersistentStoreKind,
    PersistentStoreLimits,
};
use lc631_process_supervisor::ResourceReservationView;
use std::path::Path;

#[test]
fn tlcx07_binary_segment_profile_is_additive_to_the_owned_store_api() {
    fn legacy_kind_name(kind: PersistentStoreKind) -> &'static str {
        match kind {
            PersistentStoreKind::ReceiptKey => "receipt-key",
            PersistentStoreKind::ReplayLedger => "replay-ledger",
            PersistentStoreKind::StopObservation => "stop-observation",
            PersistentStoreKind::Checkpoint => "checkpoint-json",
            PersistentStoreKind::CheckpointHead => "checkpoint-head-json",
            PersistentStoreKind::RepairBackup => "repair-backup",
        }
    }

    fn begins_binary_segment<'a>(
        root: &Path,
        target: &Path,
        resources: &'a ResourceReservationView<'a>,
    ) -> Result<HostPersistentStoreOwner<'a>, HostError> {
        HostPersistentStoreOwner::begin_binary_checkpoint_segment(
            root,
            target,
            4 * 1024 * 1024,
            PersistentStoreLimits::default(),
            resources,
        )
    }

    fn writes_binary_segment(
        owner: &mut HostPersistentStoreOwner<'_>,
        bytes: &[u8],
    ) -> Result<String, BinaryCheckpointSegmentWriteError> {
        owner.write_binary_checkpoint_segment(bytes)
    }

    // Compile the public resource-owning entrypoint shape without manufacturing
    // a reservation or performing filesystem operations in this API test.
    let _entrypoint = begins_binary_segment;
    let _writer = writes_binary_segment;
    assert_eq!(
        legacy_kind_name(PersistentStoreKind::Checkpoint),
        "checkpoint-json"
    );
}
