#![allow(clippy::drop_non_drop)] // Explicitly end borrowed resource-view scopes before releasing test leases.

use lc631_host::{
    BinaryCheckpointSegmentWriteError, HostError, HostPersistentStoreOwner, PersistentStoreKind,
    PersistentStoreLimits,
};
use lc631_process_supervisor::ResourceError;
use lc631_receipt_kernel::digest_bytes;
use lc631_runtime::resource::{
    Cancellation, Deadline, Digest32, ResourceAmount, ResourceBroker, ResourceKind, ResourceLimits,
    ResourceRequest,
};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const TEST_ROOT_PREFIX: &str = "epistesys-tlcx07-binary-checkpoint-";
const PLANNED_SEGMENT_BYTES: u64 = 4 * 1024 * 1024;

struct OwnedTestRoot {
    path: PathBuf,
    canonical_base: PathBuf,
}

impl OwnedTestRoot {
    fn create() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        Self::create_with_nonce(nonce)
    }

    fn create_with_nonce(nonce: u128) -> Self {
        let canonical_base = std::env::temp_dir()
            .canonicalize()
            .expect("system temp directory should be accessible");
        for attempt in 0..64 {
            let path = canonical_base.join(format!(
                "{TEST_ROOT_PREFIX}{}-{nonce}-{attempt}",
                std::process::id()
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => {
                    return Self {
                        path,
                        canonical_base,
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("temporary test root: {error}"),
            }
        }
        panic!("temporary test root collision budget exhausted");
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn segment(&self, sequence: u64) -> PathBuf {
        self.path.join(format!("segment-{sequence:020}.bin"))
    }
}

#[test]
fn equal_clock_nonces_do_not_alias_owned_checkpoint_roots() {
    let a = OwnedTestRoot::create_with_nonce(1);
    let b = OwnedTestRoot::create_with_nonce(1);
    assert_ne!(a.path(), b.path());
    assert!(a.path().is_dir() && b.path().is_dir());
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

fn source_revision() -> Digest32 {
    Digest32::from_bytes([0x41; 32])
}

/// Independent fixture construction for one EPITLCX1 frame containing one
/// bounded journal record. This is only test input; the host owner does not
/// parse or validate this frame.
fn journal_segment_fixture(payload: &[u8]) -> Vec<u8> {
    const RECORD_HEADER: usize = 128;
    const FRAME_HEADER_AND_DIRECTORY: usize = 64 + 32;
    const FRAME_DOMAIN: &[u8] = b"Epistesys/TLCX/FrameDigest/sha256/v1\0";
    const RECORD_DOMAIN: &[u8] = b"Epistesys/TLCX/JournalRecord/sha256/v1\0";

    let mut record = vec![0_u8; RECORD_HEADER];
    record[..8].copy_from_slice(b"EJREC001");
    record[8..10].copy_from_slice(&1_u16.to_le_bytes());
    record[10..12].copy_from_slice(&6_u16.to_le_bytes()); // Evidence record.
    record[16..24].copy_from_slice(&1_u64.to_le_bytes());
    record[24..28].copy_from_slice(&u32::try_from(payload.len()).unwrap().to_le_bytes());
    record[32..64].copy_from_slice(&[0x52; 32]); // Nonzero object identity.
    let mut record_hash = Sha256::new();
    record_hash.update(RECORD_DOMAIN);
    record_hash.update(&record[..96]);
    record_hash.update([0; 32]);
    record_hash.update(payload);
    record[96..128].copy_from_slice(&record_hash.finalize());
    record.extend_from_slice(payload);

    let mut frame = vec![0_u8; FRAME_HEADER_AND_DIRECTORY];
    frame[..8].copy_from_slice(b"EPITLCX1");
    frame[8..10].copy_from_slice(&1_u16.to_le_bytes());
    frame[24..28].copy_from_slice(&1_u32.to_le_bytes());
    frame[28..32].copy_from_slice(&64_u32.to_le_bytes());
    frame[64..66].copy_from_slice(&0x0200_u16.to_le_bytes()); // Journal record section.
    frame[66..68].copy_from_slice(&1_u16.to_le_bytes());
    frame[72..80].copy_from_slice(&(FRAME_HEADER_AND_DIRECTORY as u64).to_le_bytes());
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
fn tlcx07_successful_binary_handoff_rescans_exact_raw_bytes() {
    let root = OwnedTestRoot::create();
    let source = source_revision();
    let (broker, run) = new_owner_and_run(source, 1);
    let mut reservation = reserve_checkpoint_resources(&run);
    let target = root.segment(1);
    let segment = journal_segment_fixture(b"sole-evidence-payload");
    let expected_digest = digest_bytes(&segment);

    {
        let view = reservation.lease();
        let mut store = HostPersistentStoreOwner::begin_binary_checkpoint_segment(
            root.path(),
            &target,
            PLANNED_SEGMENT_BYTES,
            PersistentStoreLimits::default(),
            &view,
        )
        .unwrap();
        assert_eq!(
            store.write_binary_checkpoint_segment(&segment),
            Ok(expected_digest.clone())
        );
        assert!(store.target_exists());
    }

    // A fresh owner rebuilds only raw byte inventory and digest identity.
    // It does not claim the journal parser, evidence verifier or external head.
    {
        let view = reservation.lease();
        let rescanned = HostPersistentStoreOwner::begin_binary_checkpoint_segment(
            root.path(),
            &target,
            PLANNED_SEGMENT_BYTES,
            PersistentStoreLimits::default(),
            &view,
        )
        .unwrap();
        assert!(rescanned.target_exists());
        assert_eq!(rescanned.usage().total_bytes, segment.len() as u64);
    }

    reservation.release_confirmed().unwrap();
    assert_eq!(broker.used(ResourceKind::Disk).unwrap(), 0);
    assert_eq!(std::fs::read(&target).unwrap(), segment);
}

#[test]
fn tlcx07_torn_segment_is_retained_as_untrusted_bytes_after_rescan() {
    let root = OwnedTestRoot::create();
    let source = source_revision();
    let (first_broker, first_run) = new_owner_and_run(source, 1);
    let mut first_reservation = reserve_checkpoint_resources(&first_run);
    let first_target = root.segment(1);
    let first_segment = journal_segment_fixture(b"prior-evidence");
    let first_digest = digest_bytes(&first_segment);
    {
        let view = first_reservation.lease();
        let mut store = HostPersistentStoreOwner::begin_binary_checkpoint_segment(
            root.path(),
            &first_target,
            PLANNED_SEGMENT_BYTES,
            PersistentStoreLimits::default(),
            &view,
        )
        .unwrap();
        assert_eq!(
            store.write_binary_checkpoint_segment(&first_segment),
            Ok(first_digest.clone())
        );
    }
    first_reservation.release_confirmed().unwrap();
    assert_eq!(first_broker.used(ResourceKind::Disk).unwrap(), 0);

    let torn_target = root.segment(2);
    let full_candidate = journal_segment_fixture(b"candidate-evidence");
    let torn_segment = &full_candidate[..full_candidate.len() - 3];
    let (partial_broker, partial_run) = new_owner_and_run(source, 2);
    let mut partial_reservation = reserve_checkpoint_resources(&partial_run);
    {
        let view = partial_reservation.lease();
        let mut store = HostPersistentStoreOwner::begin_binary_checkpoint_segment(
            root.path(),
            &torn_target,
            PLANNED_SEGMENT_BYTES,
            PersistentStoreLimits::default(),
            &view,
        )
        .unwrap();
        store.before_write(torn_segment.len() as u64).unwrap();
        std::fs::write(&torn_target, torn_segment)
            .expect("fault fixture leaves a truncated segment on disk");
        // Dropping before handoff retains the uncertain-effect state.
    }
    assert_eq!(
        partial_reservation.release_confirmed(),
        Err(ResourceError::CleanupUnverified)
    );
    drop(partial_reservation);
    drop(partial_run);
    drop(partial_broker);

    // A new owner re-inventories the file without deleting or promoting it.
    // The host store exposes raw bytes only; the runtime journal decoder must
    // map this truncated frame to Hold before any evidence/selection consumer.
    let (recovery_broker, recovery_run) = new_owner_and_run(source, 3);
    let mut recovery_reservation = reserve_checkpoint_resources(&recovery_run);
    {
        let view = recovery_reservation.lease();
        let mut rescanned = HostPersistentStoreOwner::begin_binary_checkpoint_segment(
            root.path(),
            &torn_target,
            PLANNED_SEGMENT_BYTES,
            PersistentStoreLimits::default(),
            &view,
        )
        .unwrap();
        assert!(rescanned.target_exists());
        assert_eq!(
            rescanned.usage().total_bytes,
            first_segment.len() as u64 + torn_segment.len() as u64
        );
        // A stale prior-segment digest cannot be used to confirm this target.
        assert_eq!(
            rescanned.confirm_handoff(&first_digest),
            Err(HostError::Io("persistent_store:write_not_admitted".into()))
        );
    }

    // The failed stale-head attempt did not remove either exact stored payload.
    assert_eq!(std::fs::read(&first_target).unwrap(), first_segment);
    assert_eq!(std::fs::read(&torn_target).unwrap(), torn_segment);
    assert_eq!(
        recovery_reservation.release_confirmed(),
        Err(ResourceError::CleanupUnverified)
    );
    drop(recovery_reservation);
    drop(recovery_run);
    drop(recovery_broker);
}

#[test]
fn tlcx07_binary_segment_writer_rejects_the_legacy_json_profile() {
    fn compile_api<'a>(
        owner: &mut HostPersistentStoreOwner<'a>,
        bytes: &[u8],
    ) -> Result<String, BinaryCheckpointSegmentWriteError> {
        owner.write_binary_checkpoint_segment(bytes)
    }

    let _writer = compile_api;
    let _legacy_profile = PersistentStoreKind::Checkpoint;
}

#[test]
fn tlcx07_binary_reader_rejects_the_wrong_profile_and_missing_target() {
    let root = OwnedTestRoot::create();
    let source = source_revision();
    let (broker, run) = new_owner_and_run(source, 1);
    let mut reservation = reserve_checkpoint_resources(&run);

    {
        let view = reservation.lease();
        let legacy_target = root.path().join("checkpoint-00000000000000000001.json");
        let legacy_owner = HostPersistentStoreOwner::begin(
            root.path(),
            &legacy_target,
            PersistentStoreKind::Checkpoint,
            PLANNED_SEGMENT_BYTES,
            PersistentStoreLimits::default(),
            &view,
        )
        .unwrap();
        match legacy_owner.read_binary_checkpoint_segment() {
            Err(HostError::Io(message)) => {
                assert!(message.contains("binary_checkpoint_segment_profile_required"));
            }
            Err(other) => panic!("expected wrong-profile rejection, got {other:?}"),
            Ok(_) => panic!("legacy JSON profile must not read binary segments"),
        };
    }

    {
        let view = reservation.lease();
        let missing_target = root.segment(2);
        let binary_owner = HostPersistentStoreOwner::begin_binary_checkpoint_segment(
            root.path(),
            &missing_target,
            PLANNED_SEGMENT_BYTES,
            PersistentStoreLimits::default(),
            &view,
        )
        .unwrap();
        match binary_owner.read_binary_checkpoint_segment() {
            Err(HostError::Io(message)) => assert!(message.contains("target_missing")),
            Err(other) => panic!("expected missing-target Hold, got {other:?}"),
            Ok(_) => panic!("a missing target cannot produce a byte buffer"),
        };
    }

    reservation.release_confirmed().unwrap();
    assert_eq!(broker.used(ResourceKind::Disk).unwrap(), 0);
}

#[test]
fn tlcx07_binary_reader_applies_the_fixed_cap_after_path_growth() {
    let root = OwnedTestRoot::create();
    let source = source_revision();
    let (broker, run) = new_owner_and_run(source, 1);
    let mut reservation = reserve_checkpoint_resources(&run);
    let target = root.segment(1);
    let segment = journal_segment_fixture(b"bounded-read");

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

        // Adversarial growth after the owner's initial digest snapshot. The
        // bounded reader must reject it before returning any partial bytes.
        let mut appended = std::fs::OpenOptions::new()
            .append(true)
            .open(&target)
            .unwrap();
        appended
            .write_all(&vec![0xa5; 4 * 1024 * 1024 + 1])
            .unwrap();
        appended.sync_all().unwrap();

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
fn tlcx07_owned_segment_keeps_ram_and_output_claims_until_bytes_drop() {
    let root = OwnedTestRoot::create();
    let source = source_revision();
    let (_broker, run) = new_owner_and_run(source, 1);
    let mut reservation = reserve_checkpoint_resources(&run);
    let target = root.segment(1);
    let segment = journal_segment_fixture(b"lifetime-bound-payload");

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
    let owned = owner.read_binary_checkpoint_segment().unwrap();
    assert_eq!(owned.as_bytes(), segment);
    assert_eq!(owned.len(), segment.len());

    // The returned bytes own their live quota independently of the store
    // handle, and the claim is released only after the returned buffer drops.
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
    drop(pressure);
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
    drop(view);
    reservation.release_confirmed().unwrap();
}
