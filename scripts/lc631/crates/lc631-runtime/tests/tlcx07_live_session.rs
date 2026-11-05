//! Proposed integration tests using real binary frames and owned storage.
//! Each unique test root is printed and retained; no cleanup is performed.
use lc631_host::{HostPersistentStoreOwner, PersistentStoreLimits};
use lc631_receipt_kernel::digest_bytes;
use lc631_runtime::journal::{
    decode_segment, JournalChainLimits, JournalCursor, JournalError, JournalLiveError,
    JournalLiveSession, JournalRecordKind, JournalSegmentBuilder,
};
use lc631_runtime::resource::{
    Cancellation, Deadline, Reservation, ResourceAmount, ResourceBroker, ResourceKind,
    ResourceLimits, ResourceRequest, RunResources,
};
use lc631_substrate::Digest32;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

struct TestRoot(PathBuf);
impl TestRoot {
    fn create() -> Self {
        let base = std::env::temp_dir().canonicalize().unwrap();
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        for n in 0..32_u32 {
            let path = base.join(format!(
                "epistesys-tlcx07-runtime-live-{}-{}",
                std::process::id(),
                nonce + u128::from(n)
            ));
            match fs::create_dir(&path) {
                Ok(()) => {
                    eprintln!("retained test-owned root: {}", path.display());
                    return Self(path);
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("unique test root: {e}"),
            }
        }
        panic!("unable to allocate test-owned root")
    }
    fn path(&self) -> &Path {
        &self.0
    }
    fn segment(&self, n: u64) -> PathBuf {
        self.0.join(format!("segment-{n:020}.bin"))
    }
}

fn resources(run_id: u64) -> (ResourceBroker, RunResources, Reservation) {
    let limits = ResourceLimits::uniform(64 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let run = broker
        .begin_run(
            run_id,
            Digest32::from_bytes([0x57; 32]),
            limits,
            Deadline::after(Duration::from_secs(60)).unwrap(),
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

// Write pre-existing bytes through the actual host owner; no runtime decoder
// or cache manufactures the starting state.
fn seed(root: &TestRoot, reservation: &mut Reservation, file_sequence: u64, bytes: &[u8]) {
    let view = reservation.lease();
    let mut owner = HostPersistentStoreOwner::begin_binary_checkpoint_segment(
        root.path(),
        &root.segment(file_sequence),
        bytes.len() as u64,
        PersistentStoreLimits::default(),
        &view,
    )
    .unwrap();
    assert_eq!(
        owner.write_binary_checkpoint_segment(bytes).unwrap(),
        digest_bytes(bytes)
    );
}
fn frame_after(cursor: JournalCursor, id: u8, payload: &[u8]) -> Vec<u8> {
    let mut builder = JournalSegmentBuilder::after(cursor);
    builder
        .append_at(
            cursor.sequence() + 1,
            JournalRecordKind::Evidence,
            Digest32::from_bytes([id; 32]),
            payload,
        )
        .unwrap();
    builder.finish().unwrap()
}
fn after(bytes: &[u8]) -> JournalCursor {
    decode_segment(bytes).unwrap().cursor()
}
fn open_limits() -> JournalChainLimits {
    JournalChainLimits::default()
}
fn two_records_after(cursor: JournalCursor) -> Vec<u8> {
    let mut builder = JournalSegmentBuilder::after(cursor);
    builder
        .append_at(
            cursor.sequence() + 1,
            JournalRecordKind::Evidence,
            Digest32::from_bytes([3; 32]),
            b"third-a",
        )
        .unwrap();
    builder
        .append_at(
            cursor.sequence() + 2,
            JournalRecordKind::Evidence,
            Digest32::from_bytes([4; 32]),
            b"third-b",
        )
        .unwrap();
    builder.finish().unwrap()
}

#[test]
fn tlcx07_runtime_live_replays_existing_then_appends_two_real_frames() {
    let root = TestRoot::create();
    let (_broker, _run, mut reservation) = resources(1);
    let first = frame_after(JournalCursor::genesis(), 1, b"first");
    let second = frame_after(after(&first), 2, b"second");
    let third = two_records_after(after(&second));
    seed(&root, &mut reservation, 1, &first);
    {
        let view = reservation.lease();
        let mut live = JournalLiveSession::open(
            root.path(),
            1024 * 1024,
            PersistentStoreLimits::default(),
            &view,
            open_limits(),
        )
        .unwrap();
        assert_eq!(live.cursor().unwrap(), after(&first));
        let receipt2 = live.append(after(&first), &second).unwrap();
        assert_eq!(
            receipt2.head(),
            decode_segment(&second).unwrap().local_head()
        );
        assert_eq!(receipt2.record_count(), 1);
        assert_eq!(receipt2.storage_digest(), digest_bytes(&second));
        assert_eq!(live.cursor().unwrap(), after(&second));
        let receipt3 = live.append(after(&second), &third).unwrap();
        assert_eq!(
            receipt3.head(),
            decode_segment(&third).unwrap().local_head()
        );
        assert_eq!(receipt3.record_count(), 2);
        assert_eq!(live.cursor().unwrap().sequence(), 4);
        assert_eq!(receipt3.storage_digest(), digest_bytes(&third));
        assert_eq!(live.cursor().unwrap(), after(&third));
        assert_eq!(fs::read(root.segment(1)).unwrap(), first);
        assert_eq!(fs::read(root.segment(2)).unwrap(), second);
        assert_eq!(fs::read(root.segment(3)).unwrap(), third);
    }
    reservation.release_confirmed().unwrap();
    // A fresh capability must reconstruct, not receive, the earlier cursor.
    {
        let (_b, _r, mut reopened_reservation) = resources(11);
        {
            let view = reopened_reservation.lease();
            let mut live = JournalLiveSession::open(
                root.path(),
                1024 * 1024,
                PersistentStoreLimits::default(),
                &view,
                open_limits(),
            )
            .unwrap();
            assert_eq!(live.cursor().unwrap(), after(&third));
        }
        reopened_reservation.release_confirmed().unwrap();
    }
}

#[test]
fn tlcx07_runtime_live_rejects_torn_existing_history_during_bootstrap() {
    let root = TestRoot::create();
    let (_broker, _run, mut reservation) = resources(2);
    let first = frame_after(JournalCursor::genesis(), 1, b"first");
    let complete = frame_after(after(&first), 2, b"second");
    let torn = &complete[..complete.len() - 1];
    seed(&root, &mut reservation, 1, &first);
    seed(&root, &mut reservation, 2, torn);
    {
        let view = reservation.lease();
        let result = JournalLiveSession::open(
            root.path(),
            1024 * 1024,
            PersistentStoreLimits::default(),
            &view,
            open_limits(),
        );
        assert!(matches!(result, Err(JournalLiveError::InvalidSegment(_))));
    }
    assert_eq!(fs::read(root.segment(1)).unwrap(), first);
    assert_eq!(fs::read(root.segment(2)).unwrap(), torn);
    assert!(!root.segment(3).exists());
}

#[test]
fn tlcx07_runtime_live_rejects_fork_before_write_and_discards_cursor() {
    let root = TestRoot::create();
    let (_broker, _run, mut reservation) = resources(3);
    let first = frame_after(JournalCursor::genesis(), 1, b"first");
    let fork = frame_after(JournalCursor::genesis(), 7, b"fork");
    let valid_next = frame_after(after(&first), 2, b"valid");
    seed(&root, &mut reservation, 1, &first);
    {
        let view = reservation.lease();
        let mut live = JournalLiveSession::open(
            root.path(),
            1024 * 1024,
            PersistentStoreLimits::default(),
            &view,
            open_limits(),
        )
        .unwrap();
        assert!(matches!(
            live.append(after(&first), &fork),
            Err(JournalLiveError::InvalidSegment(
                JournalError::StalePredecessor
            ))
        ));
        assert!(!root.segment(2).exists());
        assert!(matches!(live.cursor(), Err(JournalLiveError::Poisoned)));
        assert!(matches!(
            live.append(after(&first), &valid_next),
            Err(JournalLiveError::Poisoned)
        ));
    }
    assert_eq!(fs::read(root.segment(1)).unwrap(), first);
    assert!(!root.segment(2).exists());
}

#[test]
fn tlcx07_runtime_live_discards_state_on_native_raw_mutation() {
    let root = TestRoot::create();
    let (_broker, _run, mut reservation) = resources(4);
    let first = frame_after(JournalCursor::genesis(), 1, b"first");
    let second = frame_after(after(&first), 2, b"second");
    seed(&root, &mut reservation, 1, &first);
    {
        let view = reservation.lease();
        let mut live = JournalLiveSession::open(
            root.path(),
            1024 * 1024,
            PersistentStoreLimits::default(),
            &view,
            open_limits(),
        )
        .unwrap();
        let mut changed = first.clone();
        changed[0] ^= 1;
        // Deliberate noncooperating change in the test-owned root, after replay.
        fs::write(root.segment(1), &changed).unwrap();
        assert!(matches!(
            live.append(after(&first), &second),
            Err(JournalLiveError::StorageRejectedBeforeWrite(_))
        ));
        assert!(matches!(live.cursor(), Err(JournalLiveError::Poisoned)));
        assert!(matches!(
            live.append(after(&first), &second),
            Err(JournalLiveError::Poisoned)
        ));
        assert_eq!(fs::read(root.segment(1)).unwrap(), changed);
        assert!(!root.segment(2).exists());
    }
}

#[test]
fn tlcx07_runtime_live_applies_chain_count_and_byte_bounds_to_append() {
    // Exercise small exact profile limits without creating 1,000 files or 16 MiB.
    assert!(JournalChainLimits::checked(1000, 16 * 1024 * 1024).is_ok());
    assert!(JournalChainLimits::checked(1001, 16 * 1024 * 1024).is_err());
    assert!(JournalChainLimits::checked(1000, 16 * 1024 * 1024 + 1).is_err());
    for (run_id, count_limited) in [(5, true), (6, false)] {
        let root = TestRoot::create();
        let (_broker, _run, mut reservation) = resources(run_id);
        let first = frame_after(JournalCursor::genesis(), 1, b"first");
        let second = frame_after(after(&first), 2, b"second");
        seed(&root, &mut reservation, 1, &first);
        let limits = if count_limited {
            JournalChainLimits::checked(1, first.len() + second.len()).unwrap()
        } else {
            JournalChainLimits::checked(2, first.len() + second.len() - 1).unwrap()
        };
        {
            let view = reservation.lease();
            let mut live = JournalLiveSession::open(
                root.path(),
                1024 * 1024,
                PersistentStoreLimits::default(),
                &view,
                limits,
            )
            .unwrap();
            assert!(matches!(
                live.append(after(&first), &second),
                Err(JournalLiveError::ChainLimit)
            ));
            assert!(matches!(live.cursor(), Err(JournalLiveError::Poisoned)));
        }
        assert_eq!(fs::read(root.segment(1)).unwrap(), first);
        assert!(!root.segment(2).exists());
    }
}
