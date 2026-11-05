//! Proposed integration tests for raw host storage. Not journal validation.
use lc631_host::{
    HostBinaryCheckpointLiveSession, HostError, HostPersistentStoreOwner, PersistentStoreLimits,
};
use lc631_process_supervisor::test_fixtures::bounded_test_lease;
use lc631_receipt_kernel::digest_bytes;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const PREFIX: &str = "epistesys-tlcx07-live-test-";

// The fixture intentionally retains its unique test-owned root and files.
struct OwnedRoot {
    path: PathBuf,
}
impl OwnedRoot {
    fn create() -> Self {
        let temp = std::env::temp_dir().canonicalize().unwrap();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        for attempt in 0..32_u32 {
            let path = temp.join(format!(
                "{PREFIX}{}-{}",
                std::process::id(),
                now + u128::from(attempt)
            ));
            match fs::create_dir(&path) {
                Ok(()) => {
                    eprintln!("retained test-owned root: {}", path.display());
                    return Self { path };
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("owned test root creation: {e}"),
            }
        }
        panic!("could not allocate a unique test root")
    }
    fn path(&self) -> &Path {
        &self.path
    }
    fn segment(&self, n: u64) -> PathBuf {
        self.path.join(format!("segment-{n:020}.bin"))
    }
}
#[test]
fn tlcx07_live_session_writes_two_raw_segments_and_excludes_competing_owner() {
    let root = OwnedRoot::create();
    let lease = bounded_test_lease(1).unwrap();
    let competing_lease = bounded_test_lease(2).unwrap();
    let first = b"raw-one";
    let second = b"raw-two";
    {
        let view = lease.view();
        let other_view = competing_lease.view();
        let mut live = HostBinaryCheckpointLiveSession::open(
            root.path(),
            64,
            PersistentStoreLimits::default(),
            &view,
        )
        .unwrap();
        assert_eq!(live.next_sequence().unwrap(), 1);
        assert_eq!(live.append(1, first).unwrap(), digest_bytes(first));
        assert_eq!(live.next_sequence().unwrap(), 2);
        assert_eq!(live.remaining_bytes(), 64 - first.len() as u64);
        assert_eq!(live.append(2, second).unwrap(), digest_bytes(second));
        assert_eq!(live.next_sequence().unwrap(), 3);
        assert_eq!(
            live.remaining_bytes(),
            64 - (first.len() + second.len()) as u64
        );

        let inventory = live.inventory().unwrap();
        assert_eq!(inventory.len(), 2);
        assert_eq!(inventory[0].sequence(), 1);
        assert_eq!(inventory[1].sequence(), 2);
        let one = live.read_descriptor(&inventory[0]).unwrap();
        let two = live.read_descriptor(&inventory[1]).unwrap();
        assert_eq!(one.as_bytes(), first);
        assert_eq!(two.as_bytes(), second);
        assert_eq!(inventory[0].digest(), digest_bytes(first));
        assert_eq!(inventory[1].digest(), digest_bytes(second));
        // Separate lease avoids a resource-claim failure masquerading as lock
        // exclusion; no claim is made against privileged non-cooperators.
        let other = HostPersistentStoreOwner::begin_binary_checkpoint_segment(
            root.path(),
            &root.segment(3),
            1,
            PersistentStoreLimits::default(),
            &other_view,
        );
        assert!(
            matches!(other, Err(HostError::Io(message)) if message.contains("root_busy")),
            "competing owner did not report root_busy"
        );
        assert_eq!(fs::read(root.segment(1)).unwrap(), first);
        assert_eq!(fs::read(root.segment(2)).unwrap(), second);
    }
    {
        let other_view = competing_lease.view();
        let reopened = HostPersistentStoreOwner::begin_binary_checkpoint_segment(
            root.path(),
            &root.segment(3),
            1,
            PersistentStoreLimits::default(),
            &other_view,
        );
        assert!(
            reopened.is_ok(),
            "root did not unlock after the live session dropped"
        );
    }
    lease.close_confirmed().unwrap();
    competing_lease.close_confirmed().unwrap();
    let restart_lease = bounded_test_lease(6).unwrap();
    {
        let view = restart_lease.view();
        let mut fresh = HostBinaryCheckpointLiveSession::open(
            root.path(),
            8,
            PersistentStoreLimits::default(),
            &view,
        )
        .unwrap();
        assert_eq!(fresh.next_sequence().unwrap(), 3);
        let inventory = fresh.inventory().unwrap();
        assert_eq!(inventory.len(), 2);
        assert_eq!(
            fresh.read_descriptor(&inventory[0]).unwrap().as_bytes(),
            first
        );
        assert_eq!(
            fresh.read_descriptor(&inventory[1]).unwrap().as_bytes(),
            second
        );
        // Runtime must separately decode/validate both frames and chain.
    }
    restart_lease.close_confirmed().unwrap();
}

#[test]
fn tlcx07_live_rejects_stale_storage_sequence_and_reopens_from_inventory() {
    let root = OwnedRoot::create();
    let lease = bounded_test_lease(3).unwrap();
    {
        let view = lease.view();
        let mut live = HostBinaryCheckpointLiveSession::open(
            root.path(),
            32,
            PersistentStoreLimits::default(),
            &view,
        )
        .unwrap();
        assert!(live.append(2, b"wrong-sequence").is_err());
        assert!(!root.segment(1).exists());
        assert!(!root.segment(2).exists());
        // Errors consume or poison the live capability; they cannot grant retry.
        assert!(live.append(1, b"retry").is_err());
    }
    // A fresh lease and owner reopens the root after session drop; this is
    // neither a transferred capability nor an OS/process crash.
    let fresh_lease = bounded_test_lease(33).unwrap();
    {
        let view = fresh_lease.view();
        let mut fresh = HostBinaryCheckpointLiveSession::open(
            root.path(),
            32,
            PersistentStoreLimits::default(),
            &view,
        )
        .unwrap();
        assert_eq!(fresh.next_sequence().unwrap(), 1);
        assert_eq!(fresh.append(1, b"fresh").unwrap(), digest_bytes(b"fresh"));
        // This is a stale file sequence, not a decoded journal cursor.
        assert!(fresh.append(1, b"stale").is_err());
        assert!(fresh.next_sequence().is_err());
    }
    // Poison may deliberately retain cleanup-unknown resource state.
    assert_eq!(fs::read(root.segment(1)).unwrap(), b"fresh");
}

#[test]
fn tlcx07_live_enforces_remaining_total_disk_admission_without_overwrite() {
    let root = OwnedRoot::create();
    let lease = bounded_test_lease(4).unwrap();
    {
        let view = lease.view();
        let mut live = HostBinaryCheckpointLiveSession::open(
            root.path(),
            8,
            PersistentStoreLimits::default(),
            &view,
        )
        .unwrap();
        live.append(1, b"abc").unwrap();
        assert_eq!(live.remaining_bytes(), 5);
        assert!(live.append(2, b"123456").is_err());
        assert_eq!(fs::read(root.segment(1)).unwrap(), b"abc");
        assert!(!root.segment(2).exists());
        assert!(
            live.append(2, b"12345").is_err(),
            "failed admission left session reusable"
        );
    }
    assert_eq!(fs::read(root.segment(1)).unwrap(), b"abc");
}

#[test]
fn tlcx07_live_detects_between_appends_root_change_and_never_erases_bytes() {
    let root = OwnedRoot::create();
    let lease = bounded_test_lease(5).unwrap();
    {
        let view = lease.view();
        let mut live = HostBinaryCheckpointLiveSession::open(
            root.path(),
            64,
            PersistentStoreLimits::default(),
            &view,
        )
        .unwrap();
        live.append(1, b"original").unwrap();
        // Test-owned, noncooperating change. A real adversary is not simulated.
        fs::write(root.segment(2), b"foreign").unwrap();
        assert!(live.append(2, b"candidate").is_err());
        assert_eq!(fs::read(root.segment(1)).unwrap(), b"original");
        assert_eq!(fs::read(root.segment(2)).unwrap(), b"foreign");
        assert!(
            live.append(3, b"retry").is_err(),
            "invalidated session was reusable"
        );
    }
    assert_eq!(fs::read(root.segment(1)).unwrap(), b"original");
    assert_eq!(fs::read(root.segment(2)).unwrap(), b"foreign");
}
