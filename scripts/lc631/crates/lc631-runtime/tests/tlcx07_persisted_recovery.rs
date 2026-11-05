use lc631_host::{HostPersistentStoreOwner, PersistentStoreLimits};
use lc631_runtime::journal::{
    decode_segment, inspect_persisted_segment, persist_segment, JournalCursor, JournalError,
    JournalRecordKind, JournalSegmentBuilder, PersistedSegmentHold, PersistedSegmentInspection,
};
use lc631_runtime::resource::{
    Cancellation, Deadline, ResourceAmount, ResourceBroker, ResourceKind, ResourceLimits,
    ResourceRequest,
};
use lc631_substrate::Digest32;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

struct TestScratchRoot(PathBuf);

impl TestScratchRoot {
    fn create() -> Self {
        for attempt in 0..32_u32 {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
                .saturating_add(u128::from(attempt));
            let path = std::env::temp_dir().join(format!(
                "epistesys-tlcx07-recovery-{}-{nonce}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("test-owned scratch root create failed: {error}"),
            }
        }
        panic!("could not allocate a unique test scratch root")
    }
}

impl Drop for TestScratchRoot {
    fn drop(&mut self) {
        let Ok(entries) = fs::read_dir(&self.0) else {
            return;
        };
        let mut owned_files = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            if name != ".resource-owner-v1.lock"
                && name != ".writer.lock"
                && name != "segment-00000000000000000001.bin"
            {
                return;
            }
            let Ok(metadata) = fs::symlink_metadata(&path) else {
                return;
            };
            if !metadata.is_file() || metadata.file_type().is_symlink() || is_reparse(&metadata) {
                return;
            }
            owned_files.push(path);
        }
        for path in owned_files {
            if fs::remove_file(path).is_err() {
                return;
            }
        }
        let _ = fs::remove_dir(&self.0);
    }
}

#[cfg(windows)]
fn is_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x0400 != 0
}

#[cfg(not(windows))]
fn is_reparse(_metadata: &fs::Metadata) -> bool {
    false
}

fn digest(byte: u8) -> Digest32 {
    Digest32::from_bytes([byte; 32])
}

fn resource_owner(
    source: Digest32,
) -> (
    ResourceBroker,
    lc631_runtime::resource::RunResources,
    lc631_runtime::resource::Reservation,
) {
    let limits = ResourceLimits::uniform(64 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let run = broker
        .begin_run(
            1,
            source,
            limits,
            Deadline::after(Duration::from_secs(30)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let reservation = run
        .reserve(&[
            ResourceRequest::new(
                ResourceKind::Disk,
                ResourceAmount::checked(16 * 1024 * 1024).unwrap(),
            ),
            ResourceRequest::new(
                ResourceKind::Ram,
                ResourceAmount::checked(16 * 1024 * 1024).unwrap(),
            ),
            ResourceRequest::new(
                ResourceKind::Output,
                ResourceAmount::checked(8 * 1024 * 1024).unwrap(),
            ),
        ])
        .unwrap();
    (broker, run, reservation)
}

fn valid_segment() -> Vec<u8> {
    let mut builder = JournalSegmentBuilder::genesis();
    builder
        .append_at(
            1,
            JournalRecordKind::Evidence,
            digest(0x61),
            b"persisted-evidence-ref",
        )
        .unwrap();
    builder.finish().unwrap()
}

#[test]
fn tlcx07_persisted_segment_returns_only_an_unanchored_summary() {
    let scratch = TestScratchRoot::create();
    let root = &scratch.0;
    let target = root.join("segment-00000000000000000001.bin");
    let source = digest(0x55);
    let (_broker, _run, mut reservation) = resource_owner(source);
    let bytes = valid_segment();

    {
        let view = reservation.lease();
        let planned_bytes = u64::try_from(bytes.len()).unwrap();
        let mut owner = HostPersistentStoreOwner::begin_binary_checkpoint_segment(
            root,
            &target,
            planned_bytes,
            PersistentStoreLimits::default(),
            &view,
        )
        .unwrap();
        let persisted = persist_segment(&mut owner, &bytes, JournalCursor::genesis()).unwrap();
        assert!(owner.target_exists());

        let inspection = inspect_persisted_segment(&owner, JournalCursor::genesis());
        match inspection {
            PersistedSegmentInspection::CandidateUnanchored {
                head,
                cursor,
                record_count,
            } => {
                assert_eq!(head, persisted.head());
                assert_eq!(cursor, decode_segment(&bytes).unwrap().cursor());
                assert_eq!(record_count, 1);
                assert!(PersistedSegmentInspection::CandidateUnanchored {
                    head,
                    cursor,
                    record_count,
                }
                .claim_boundary()
                .contains("external rollback resistance"));
            }
            PersistedSegmentInspection::Hold(reason) => {
                panic!("valid owner-persisted segment was held: {reason:?}")
            }
        }
        assert!(owner.target_exists());
    }
    reservation.release_confirmed().unwrap();
}

#[test]
fn tlcx07_malformed_persisted_bytes_hold_and_remain_in_the_owned_root() {
    let scratch = TestScratchRoot::create();
    let root = &scratch.0;
    let target = root.join("segment-00000000000000000001.bin");
    let source = digest(0x56);
    let (_broker, _run, mut reservation) = resource_owner(source);
    let malformed = b"torn-or-invalid-segment-bytes";

    {
        let view = reservation.lease();
        let mut owner = HostPersistentStoreOwner::begin_binary_checkpoint_segment(
            root,
            &target,
            u64::try_from(malformed.len()).unwrap(),
            PersistentStoreLimits::default(),
            &view,
        )
        .unwrap();
        owner.write_binary_checkpoint_segment(malformed).unwrap();

        assert!(matches!(
            inspect_persisted_segment(&owner, JournalCursor::genesis()),
            PersistedSegmentInspection::Hold(PersistedSegmentHold::Decode(_))
        ));
        assert!(owner.target_exists());
        let stored = owner.read_binary_checkpoint_segment().unwrap();
        assert_eq!(stored.as_bytes(), malformed);
    }
    reservation.release_confirmed().unwrap();
}

#[test]
fn tlcx07_stale_predecessor_holds_without_deleting_or_repairing_the_segment() {
    let scratch = TestScratchRoot::create();
    let root = &scratch.0;
    let target = root.join("segment-00000000000000000001.bin");
    let source = digest(0x57);
    let (_broker, _run, mut reservation) = resource_owner(source);
    let bytes = valid_segment();

    {
        let view = reservation.lease();
        let mut owner = HostPersistentStoreOwner::begin_binary_checkpoint_segment(
            root,
            &target,
            u64::try_from(bytes.len()).unwrap(),
            PersistentStoreLimits::default(),
            &view,
        )
        .unwrap();
        owner.write_binary_checkpoint_segment(&bytes).unwrap();
        let wrong_predecessor = decode_segment(&bytes).unwrap().cursor();

        assert!(matches!(
            inspect_persisted_segment(&owner, wrong_predecessor),
            PersistedSegmentInspection::Hold(PersistedSegmentHold::PredecessorMismatch(
                JournalError::StalePredecessor
            ))
        ));
        assert!(owner.target_exists());
    }
    reservation.release_confirmed().unwrap();
}

#[test]
fn tlcx07_missing_persisted_segment_is_a_storage_hold() {
    let scratch = TestScratchRoot::create();
    let root = &scratch.0;
    let target = root.join("segment-00000000000000000001.bin");
    let source = digest(0x58);
    let (_broker, _run, mut reservation) = resource_owner(source);

    {
        let view = reservation.lease();
        let owner = HostPersistentStoreOwner::begin_binary_checkpoint_segment(
            root,
            &target,
            1,
            PersistentStoreLimits::default(),
            &view,
        )
        .unwrap();
        assert!(matches!(
            inspect_persisted_segment(&owner, JournalCursor::genesis()),
            PersistedSegmentInspection::Hold(PersistedSegmentHold::StorageRead(_))
        ));
        assert!(!owner.target_exists());
    }
    reservation.release_confirmed().unwrap();
}
