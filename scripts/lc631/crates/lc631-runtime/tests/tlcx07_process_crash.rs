//! Process-exit observations, not power-loss or external-anchor evidence.
use lc631_host::{BundlePart, JournalBundleLimits};
use lc631_runtime::journal::{
    decode_segment, BundleLiveError, JournalBundleLiveSession, JournalChainLimits, JournalCursor,
    JournalRecordKind, JournalSegmentBuilder,
};
use lc631_runtime::resource::{
    Cancellation, Deadline, Reservation, ResourceAmount, ResourceBroker, ResourceKind,
    ResourceLimits, ResourceRequest, RunResources,
};
use lc631_substrate::Digest32;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

const CHILD_TEST: &str = "tlcx07_process_exit_child";
const ROOT_ENV: &str = "EPISTESYS_TLCX07_EXIT_TEST_ROOT";
const STAGE_ENV: &str = "EPISTESYS_TLCX07_EXIT_TEST_STAGE";

struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let base = std::env::temp_dir().canonicalize().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        for attempt in 0..32_u32 {
            let root = base.join(format!(
                "epistesys-tlcx07-exit-{}-{now}-{attempt}",
                std::process::id()
            ));
            match fs::create_dir(&root) {
                Ok(()) => {
                    eprintln!("retained test-owned root: {}", root.display());
                    return Self(root);
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("owned test root: {error}"),
            }
        }
        panic!("unique owned root unavailable")
    }
    fn path(&self) -> &Path {
        &self.0
    }
    fn part(&self, name: &str, sequence: u64) -> PathBuf {
        self.0.join(format!("{name}-{sequence:020}.bin"))
    }
}

fn normal(id: u64) -> (ResourceBroker, RunResources, Reservation) {
    let limits = ResourceLimits::uniform(64 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let run = broker
        .begin_run(
            id,
            Digest32::from_bytes([9; 32]),
            limits,
            Deadline::after(Duration::from_secs(60)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let requests = [ResourceKind::Ram, ResourceKind::Output, ResourceKind::Disk]
        .map(|kind| ResourceRequest::new(kind, ResourceAmount::checked(32 * 1024 * 1024).unwrap()));
    let reservation = run.reserve(&requests).unwrap();
    (broker, run, reservation)
}
fn limits() -> JournalBundleLimits {
    JournalBundleLimits::checked(1000, 16 * 1024 * 1024).unwrap()
}
fn frame() -> Vec<u8> {
    let mut builder = JournalSegmentBuilder::genesis();
    builder
        .append_at(
            1,
            JournalRecordKind::Evidence,
            Digest32::from_bytes([1; 32]),
            b"evidence",
        )
        .unwrap();
    builder.finish().unwrap()
}
fn next_frame(cursor: JournalCursor) -> Vec<u8> {
    let mut builder = JournalSegmentBuilder::after(cursor);
    builder
        .append_at(
            cursor.sequence() + 1,
            JournalRecordKind::Evidence,
            Digest32::from_bytes([2; 32]),
            b"next",
        )
        .unwrap();
    builder.finish().unwrap()
}

// Test-only watchdog. No pipes and no grandchildren are used by this fixture.
struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

fn run_crash_child(root: &Path, stage: &str) -> ExitStatus {
    let child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", CHILD_TEST, "--ignored", "--test-threads=1"])
        .env(ROOT_ENV, root)
        .env(STAGE_ENV, stage)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut guard = ChildGuard(child);
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(status) = guard.0.try_wait().unwrap() {
            return status;
        }
        assert!(Instant::now() < deadline, "owned crash fixture timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
#[ignore = "selected only by the owned-root parent test"]
fn tlcx07_process_exit_child() {
    let root = PathBuf::from(std::env::var_os(ROOT_ENV).expect("parent-owned root"));
    assert!(root.is_absolute() && root.is_dir());
    let stop = match std::env::var(STAGE_ENV).unwrap().as_str() {
        "segment" => BundlePart::Segment,
        "index" => BundlePart::Index,
        "local-head" => BundlePart::LocalHead,
        _ => panic!("unsupported fixture boundary"),
    };
    let (_broker, _run, reservation) = normal(199);
    let view = reservation.lease();
    let mut live = JournalBundleLiveSession::open(
        &root,
        1024 * 1024,
        limits(),
        &view,
        JournalChainLimits::default(),
    )
    .unwrap();
    // Exit bypasses Rust Drop; the OS must release the actual journal lock.
    let result = live.append_observed(JournalCursor::genesis(), &frame(), |part| {
        if part == stop {
            std::process::exit(86);
        }
        Ok(())
    });
    panic!("fixture did not reach the requested boundary: {result:?}");
}

#[test]
fn tlcx07_process_exit_reopen_checks_each_persisted_boundary() {
    for (id, stage) in [(200, "segment"), (201, "index"), (202, "local-head")] {
        let root = Root::new();
        assert_eq!(run_crash_child(root.path(), stage).code(), Some(86));
        let segment = fs::read(root.part("segment", 1)).unwrap();
        assert_eq!(segment, frame());
        assert_eq!(root.part("index", 1).exists(), stage != "segment");
        assert_eq!(root.part("local-head", 1).exists(), stage == "local-head");
        let retained_index = fs::read(root.part("index", 1)).ok();
        let retained_head = fs::read(root.part("local-head", 1)).ok();
        let (_broker, _run, mut reservation) = normal(id);
        {
            let view = reservation.lease();
            let reopened = JournalBundleLiveSession::open(
                root.path(),
                0,
                limits(),
                &view,
                JournalChainLimits::default(),
            );
            if stage == "local-head" {
                let mut live = reopened.unwrap();
                let cursor = decode_segment(&segment).unwrap().cursor();
                assert_eq!(live.cursor().unwrap(), cursor);
                // A validated stored prefix is not a restored Completed flag.
                let appended = live.append(cursor, &next_frame(cursor)).unwrap_err();
                assert!(matches!(
                    appended,
                    BundleLiveError::StorageRejectedBeforeWrite(_)
                ));
            } else {
                assert!(matches!(reopened, Err(BundleLiveError::RebuildRequired)));
            }
        }
        reservation.release_confirmed().unwrap();
        assert_eq!(fs::read(root.part("segment", 1)).unwrap(), segment);
        assert_eq!(fs::read(root.part("index", 1)).ok(), retained_index);
        assert_eq!(fs::read(root.part("local-head", 1)).ok(), retained_head);
        for name in ["segment", "index", "local-head"] {
            assert!(!root.part(name, 2).exists());
        }
    }
}
