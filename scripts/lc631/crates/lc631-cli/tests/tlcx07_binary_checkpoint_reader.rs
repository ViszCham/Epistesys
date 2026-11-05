#![allow(clippy::drop_non_drop)] // End borrowed reservation-view scopes before releasing test leases.

use lc631_host::{
    HostError, HostPersistentStoreOwner, OwnedBinaryCheckpointSegment, PersistentStoreKind,
    PersistentStoreLimits,
};
use lc631_process_supervisor::ResourceError;
use lc631_runtime::resource::{
    Cancellation, Deadline, Digest32, ResourceAmount, ResourceBroker, ResourceKind, ResourceLimits,
    ResourceRequest,
};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const TEST_ROOT_PREFIX: &str = "epistesys-tlcx07-binary-reader-";
const SEGMENT_LIMIT: usize = 4 * 1024 * 1024;
const PLANNED_SEGMENT_BYTES: u64 = 4 * 1024 * 1024;

struct OwnedTestRoot {
    path: PathBuf,
    canonical_base: PathBuf,
}

impl OwnedTestRoot {
    fn create() -> Self {
        let canonical_base = std::env::temp_dir()
            .canonicalize()
            .expect("system temp directory should be accessible");
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        for attempt in 0..32_u32 {
            let path = canonical_base.join(format!(
                "{TEST_ROOT_PREFIX}{}-{}",
                std::process::id(),
                nonce.saturating_add(u128::from(attempt))
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => {
                    return Self {
                        path,
                        canonical_base,
                    };
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("test-owned root creation failed: {error}"),
            }
        }
        panic!("could not allocate a unique test-owned root")
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn segment(&self) -> PathBuf {
        self.path.join("segment-00000000000000000001.bin")
    }
}

impl Drop for OwnedTestRoot {
    fn drop(&mut self) {
        let Ok(metadata) = std::fs::symlink_metadata(&self.path) else {
            return;
        };
        if !metadata.is_dir() || is_reparse_point(&metadata) {
            return;
        }
        let Ok(canonical) = self.path.canonicalize() else {
            return;
        };
        let owned_name = canonical
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(TEST_ROOT_PREFIX));
        if canonical.parent() == Some(self.canonical_base.as_path()) && owned_name {
            let _ = std::fs::remove_dir_all(canonical);
        }
    }
}

#[cfg(windows)]
fn is_reparse_point(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &std::fs::Metadata) -> bool {
    false
}

fn source_revision() -> Digest32 {
    Digest32::from_bytes([0x62; 32])
}

fn new_owner_and_run(
    source: Digest32,
    run_id: u64,
) -> (ResourceBroker, lc631_runtime::resource::RunResources) {
    let limits = ResourceLimits::uniform(32 * 1024 * 1024).unwrap();
    let owner = ResourceBroker::new(limits).unwrap();
    let run = owner
        .begin_run(
            run_id,
            source,
            limits,
            Deadline::after(Duration::from_secs(60)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    (owner, run)
}

fn reserve_checkpoint_resources(
    run: &lc631_runtime::resource::RunResources,
) -> lc631_runtime::resource::Reservation {
    run.reserve(&[
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
            ResourceAmount::checked(16 * 1024 * 1024).unwrap(),
        ),
    ])
    .unwrap()
}

/// Independent golden construction: one bounded record inside one EPITLCX1
/// opaque section. The host owner stores/checks bytes; it does not parse them.
fn journal_segment_fixture(payload: &[u8]) -> Vec<u8> {
    const RECORD_HEADER: usize = 128;
    const FRAME_PREFIX: usize = 64 + 32;
    const FRAME_DOMAIN: &[u8] = b"Epistesys/TLCX/FrameDigest/sha256/v1\0";
    const RECORD_DOMAIN: &[u8] = b"Epistesys/TLCX/JournalRecord/sha256/v1\0";

    let mut record = vec![0_u8; RECORD_HEADER];
    record[..8].copy_from_slice(b"EJREC001");
    record[8..10].copy_from_slice(&1_u16.to_le_bytes());
    record[10..12].copy_from_slice(&6_u16.to_le_bytes());
    record[16..24].copy_from_slice(&1_u64.to_le_bytes());
    record[24..28].copy_from_slice(&u32::try_from(payload.len()).unwrap().to_le_bytes());
    record[32..64].copy_from_slice(&[0x53; 32]);
    let mut record_hash = Sha256::new();
    record_hash.update(RECORD_DOMAIN);
    record_hash.update(&record[..96]);
    record_hash.update([0; 32]);
    record_hash.update(payload);
    record[96..128].copy_from_slice(&record_hash.finalize());
    record.extend_from_slice(payload);

    let mut frame = vec![0_u8; FRAME_PREFIX];
    frame[..8].copy_from_slice(b"EPITLCX1");
    frame[8..10].copy_from_slice(&1_u16.to_le_bytes());
    frame[24..28].copy_from_slice(&1_u32.to_le_bytes());
    frame[28..32].copy_from_slice(&64_u32.to_le_bytes());
    frame[64..66].copy_from_slice(&0x0200_u16.to_le_bytes());
    frame[66..68].copy_from_slice(&1_u16.to_le_bytes());
    frame[72..80].copy_from_slice(&(FRAME_PREFIX as u64).to_le_bytes());
    frame[80..88].copy_from_slice(&(record.len() as u64).to_le_bytes());
    frame[88..92].copy_from_slice(&1_u32.to_le_bytes());
    frame.extend_from_slice(&record);
    frame[16..24].copy_from_slice(&(record.len() as u64).to_le_bytes());

    let mut frame_hash = Sha256::new();
    frame_hash.update(FRAME_DOMAIN);
    frame_hash.update(&frame);
    frame[32..64].copy_from_slice(&frame_hash.finalize());
    frame
}

#[test]
fn tlcx07_binary_reader_holds_wrong_profile_and_missing_target() {
    let root = OwnedTestRoot::create();
    let source = source_revision();
    let (broker, run) = new_owner_and_run(source, 1);
    let mut reservation = reserve_checkpoint_resources(&run);

    {
        let view = reservation.lease();
        let target = root.path().join("checkpoint-00000000000000000001.json");
        let legacy = HostPersistentStoreOwner::begin(
            root.path(),
            &target,
            PersistentStoreKind::Checkpoint,
            PLANNED_SEGMENT_BYTES,
            PersistentStoreLimits::default(),
            &view,
        )
        .unwrap();
        assert!(matches!(
            legacy.read_binary_checkpoint_segment(),
            Err(HostError::Io(message)) if message.contains("binary_checkpoint_segment_profile_required")
        ));
    }

    {
        let view = reservation.lease();
        let target = root.segment();
        let binary = HostPersistentStoreOwner::begin_binary_checkpoint_segment(
            root.path(),
            &target,
            PLANNED_SEGMENT_BYTES,
            PersistentStoreLimits::default(),
            &view,
        )
        .unwrap();
        assert!(!binary.target_exists());
        assert!(matches!(
            binary.read_binary_checkpoint_segment(),
            Err(HostError::Io(message)) if message.contains("target_missing")
        ));
    }

    reservation.release_confirmed().unwrap();
    assert_eq!(broker.used(ResourceKind::Disk).unwrap(), 0);
}

#[test]
fn tlcx07_binary_reader_rejects_growth_past_the_fixed_read_cap() {
    let root = OwnedTestRoot::create();
    let source = source_revision();
    let (broker, run) = new_owner_and_run(source, 1);
    let mut reservation = reserve_checkpoint_resources(&run);
    let target = root.segment();
    let segment = journal_segment_fixture(b"read-cap");

    {
        let view = reservation.lease();
        let mut owner = HostPersistentStoreOwner::begin_binary_checkpoint_segment(
            root.path(),
            &target,
            PLANNED_SEGMENT_BYTES,
            PersistentStoreLimits::default(),
            &view,
        )
        .unwrap();
        owner.write_binary_checkpoint_segment(&segment).unwrap();
        let mut append = std::fs::OpenOptions::new()
            .append(true)
            .open(&target)
            .unwrap();
        append.write_all(&vec![0xa5; SEGMENT_LIMIT + 1]).unwrap();
        append.sync_all().unwrap();

        let output_before = broker.used(ResourceKind::Output).unwrap();
        let ram_before = broker.used(ResourceKind::Ram).unwrap();
        assert!(matches!(
            owner.read_binary_checkpoint_segment(),
            Err(HostError::Io(_))
        ));
        assert_eq!(broker.used(ResourceKind::Output).unwrap(), output_before);
        assert_eq!(broker.used(ResourceKind::Ram).unwrap(), ram_before);
    }

    reservation.release_confirmed().unwrap();
}

#[test]
fn tlcx07_owned_segment_keeps_ram_and_output_claim_until_buffer_drop() {
    let root = OwnedTestRoot::create();
    let source = source_revision();
    let (_broker, run) = new_owner_and_run(source, 1);
    let mut reservation = reserve_checkpoint_resources(&run);
    let target = root.segment();
    let segment = journal_segment_fixture(b"claim-lifetime");

    {
        let view = reservation.lease();
        let mut owner = HostPersistentStoreOwner::begin_binary_checkpoint_segment(
            root.path(),
            &target,
            PLANNED_SEGMENT_BYTES,
            PersistentStoreLimits::default(),
            &view,
        )
        .unwrap();
        owner.write_binary_checkpoint_segment(&segment).unwrap();
        let owned: OwnedBinaryCheckpointSegment<'_> =
            owner.read_binary_checkpoint_segment().unwrap();
        assert_eq!(owned.as_bytes(), segment);
        assert_eq!(owned.len(), segment.len());
        drop(owner);

        let pressure = view.claim_resources(&[
            ResourceRequest::new(
                ResourceKind::Ram,
                ResourceAmount::checked(16 * 1024 * 1024).unwrap(),
            ),
            ResourceRequest::new(
                ResourceKind::Output,
                ResourceAmount::checked(16 * 1024 * 1024).unwrap(),
            ),
        ]);
        assert!(matches!(
            pressure,
            Err(ResourceError::Exhausted(
                ResourceKind::Ram | ResourceKind::Output
            ))
        ));
        drop(owned);
        let available_after_drop = view
            .claim_resources(&[
                ResourceRequest::new(
                    ResourceKind::Ram,
                    ResourceAmount::checked(16 * 1024 * 1024).unwrap(),
                ),
                ResourceRequest::new(
                    ResourceKind::Output,
                    ResourceAmount::checked(16 * 1024 * 1024).unwrap(),
                ),
            ])
            .unwrap();
        drop(available_after_drop);
    }

    reservation.release_confirmed().unwrap();
}
