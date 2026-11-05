//! 実保存の限定試験。未実行を成功としない。
//! Bounded persistence tests; unrun tests are not passes.
use lc631_runtime::contextual_resume::budget;
use lc631_runtime::journal;
#[path = "tlcx32_budget_support/mod.rs"]
mod support;
use budget::*;
use lc631_host::BundlePart;
use std::{
    fs,
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
use support::*;

struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}
const CHILD: &str = "tlcx32_budget_child_crash_fixture";
const ROOT_ENV: &str = "EPISTESYS_TLCX32_BUDGET_CRASH_ROOT";
const STAGE_ENV: &str = "EPISTESYS_TLCX32_BUDGET_CRASH_STAGE";

#[test]
#[ignore = "only the owned parent selects this cooperative-exit fixture"]
fn tlcx32_budget_child_crash_fixture() {
    let r = PathBuf::from(std::env::var_os(ROOT_ENV).unwrap());
    assert!(r.is_absolute() && r.is_dir());
    let stop = match std::env::var(STAGE_ENV).unwrap().as_str() {
        "segment" => BundlePart::Segment,
        "index" => BundlePart::Index,
        "local-head" => BundlePart::LocalHead,
        _ => panic!("unknown stage"),
    };
    let (_broker, _run, reservation) = resources(3299);
    let lease = reservation.lease();
    let mut session = BudgetSession::initialize_new(
        &r,
        binding(),
        caps(),
        65536,
        limits(),
        journal::JournalChainLimits::default(),
        &lease,
    )
    .unwrap();
    let result = session.reserve_before_apply_observed(proposal(40), |part| {
        if part == stop {
            std::process::exit(86);
        }
        Ok(())
    });
    panic!("cooperative boundary not reached: {}", result.is_ok());
}

#[test]
fn tlcx32_budget_cooperative_exit_retains_pending_or_partial_without_ticket_reissue() {
    for (id, stage) in [(3210, "segment"), (3211, "index"), (3212, "local-head")] {
        let r = root(stage);
        let child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", CHILD, "--ignored", "--test-threads=1"])
            .env(ROOT_ENV, &r)
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
                assert_eq!(status.code(), Some(86));
                break;
            }
            assert!(Instant::now() < deadline, "owned crash child timed out");
            std::thread::sleep(Duration::from_millis(10));
        }
        let segment = fs::read(part(&r, "segment", 2)).unwrap();
        let (_broker, mut run, mut reservation) = resources(id);
        {
            let lease = reservation.lease();
            let expected = actual_head(&r, if stage == "local-head" { 2 } else { 1 });
            let result = BudgetSession::resume_existing(
                &r,
                binding(),
                caps(),
                expected,
                65536,
                limits(),
                journal::JournalChainLimits::default(),
                &lease,
            );
            if stage == "local-head" {
                let mut session = result.unwrap();
                assert_eq!(
                    session.summary().unwrap().phase,
                    BudgetPhase::PendingMayHaveApplied
                );
                assert_eq!(session.summary().unwrap().charged_delta_bytes, 40);
                assert!(matches!(
                    session.reserve_before_apply(proposal(1)),
                    Err(BudgetError::ReconciliationRequired)
                ));
            } else {
                assert!(result.is_err());
            }
        }
        assert_eq!(fs::read(part(&r, "segment", 2)).unwrap(), segment);
        assert!(!part(&r, "segment", 3).exists());
        reservation.release_confirmed().unwrap();
        run.close_confirmed().unwrap();
    }
}
