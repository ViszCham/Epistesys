//! 実2attemptとprocess終了後の保守的recovery。再開成功を捏造しない。
//! Actual two attempts and conservative post-process recovery, never invented continuation.
#[path = "tlcx32_checkpoint_support/mod.rs"]
mod fixture;
use fixture::*;
use lc631_analysis::{DgclRepairStatus, RepairEffectState};
use lc631_runtime::contextual_resume::budget::BudgetPhase;
use std::{
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

#[test]
fn tlcx32_verified_two_actual_live_attempts_persist_v2_and_reopen_without_permission() {
    let root = root();
    let (s, actual, calls, r) = exercise(&root, 2, false);
    assert_eq!(actual, SECOND);
    assert_eq!(calls, 2);
    assert_eq!(r.attempts.len(), 2);
    assert_eq!(s.phase, BudgetPhase::QuotaReadyRequiresFreshAuthority);
    assert_eq!(s.charged_attempts, 2);
    assert_eq!(s.charged_delta_bytes, 2000);
    assert_eq!(r.status, DgclRepairStatus::Hold);
    assert!(r
        .attempts
        .iter()
        .all(|a| a.application_receipt_digest.is_some()
            && a.validation_receipt_digest.is_some()
            && a.reason == "repair_revalidated_but_gaps_remain"
            && a.effect_state == RepairEffectState::ReceiptVerified));
    let backups = std::fs::read_dir(root.join(".dgcl-repair-backups"))
        .unwrap()
        .map(|e| std::fs::read_to_string(e.unwrap().path()).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(backups.len(), 2);
    assert!(backups.contains(&BEFORE.into()));
    assert!(backups.contains(&AFTER.into()));
    recovery::assert_recovered(&root, 2);
}
const CHILD: &str = "tlcx32_verified_actual_v2_exit_child";
const ROOT_ENV: &str = "EPISTESYS_TLCX32_ACTUAL_V2_EXIT_ROOT";
struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}
#[test]
#[ignore = "selected explicitly by owned cooperative process parent"]
fn tlcx32_verified_actual_v2_exit_child() {
    let root = PathBuf::from(std::env::var_os(ROOT_ENV).unwrap());
    assert!(root.is_absolute() && root.is_dir());
    let _ = exercise(&root, 1, true);
    panic!("actual v2 exit boundary not reached");
}
#[test]
fn tlcx32_verified_process_exit_recovery_blocks_even_fresh_signed_postimage_and_source() {
    let root = root();
    let child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", CHILD, "--ignored", "--test-threads=1"])
        .env(ROOT_ENV, &root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut guard = ChildGuard(child);
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = guard.0.try_wait().unwrap() {
            assert_eq!(status.code(), Some(86));
            break;
        }
        assert!(Instant::now() < deadline, "owned v2 child timeout");
        std::thread::sleep(Duration::from_millis(10));
    }
    recovery::assert_recovered(&root, 1);
    recovery::assert_fresh_postimage_and_new_source_cannot_reset(&root);
    assert!(!root
        .join("budget-journal/segment-00000000000000000004.bin")
        .exists());
    assert_eq!(
        std::fs::read_dir(root.join(".dgcl-repair-backups"))
            .unwrap()
            .count(),
        1
    );
}
