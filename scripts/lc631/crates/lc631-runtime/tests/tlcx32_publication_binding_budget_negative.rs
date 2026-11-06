//! 予算オブジェクト差し替えでも同一attemptの上限を守る負例。実行は中央待ち。
//! Negative-first same-attempt budget regressions; central execution is pending.
#![forbid(unsafe_code)]

#[path = "tlcx31_coding_support/mod.rs"]
mod coding_support;
#[path = "tlcx32_verified_support/mod.rs"]
mod file_fixture;
#[path = "tlcx32_fresh_support/mod.rs"]
#[allow(
    dead_code,
    reason = "Actual checkpoint setup is reused; old resume is not the entry under test."
)]
mod fresh_support;
#[path = "tlcx32_resume_attempt_support/mod.rs"]
#[allow(
    dead_code,
    reason = "Only the actual configured attempt fixture is reused."
)]
mod support;

use lc631_runtime::contextual_resume::production_contract::*;
use support::*;

#[test]
fn tlcx32_replacement_budget_after_publication_cannot_admit_third_capture() {
    let _serial = SERIAL.lock().unwrap();
    let fixture = PacketFixture::actual();
    let before = protected_state(&fixture.root);
    funded(|run, view| {
        configured(
            &fixture,
            run,
            view,
            |input, again, mut port, mut probe, mut build, _pipeline| {
                let plan = input.plan;
                let identity = (
                    view.owner_id(),
                    view.run_id(),
                    view.source_revision(),
                    view.reservation_id(),
                );
                let mut budget = CodingResumeCallBudget::checked(2, 2).unwrap();
                let mut attempt = resume_coding_attempt(
                    input,
                    &mut port,
                    Some(&mut probe),
                    Some(&mut build),
                    &mut budget,
                    view,
                );
                assert_eq!(attempt.stage(), CodingResumeStage::ReadyForPublication);
                assert_eq!((budget.used_native(), budget.used_captures()), (2, 1));
                assert!(attempt.baseline().unwrap().process().is_some());
                assert!(attempt.current_probe().unwrap().process().is_some());
                assert!(attempt
                    .publish_current(
                        plan,
                        &again,
                        &mut port,
                        &mut probe,
                        &mut build,
                        &mut budget,
                        view
                    )
                    .unwrap());
                assert!(attempt.current(view).unwrap());
                assert_eq!((budget.used_native(), budget.used_captures()), (2, 2));
                assert_eq!(attempt.build_attempts().len(), 2);

                let calls = (probe.calls.len(), build.calls);
                // Keep the same actual attempt, port and pipeline. Only the caller's
                // budget object changes; this must not replenish admitted work.
                let mut replacement = CodingResumeCallBudget::checked(2, 2).unwrap();
                assert_eq!(
                    attempt.publish_current(
                        plan,
                        &again,
                        &mut port,
                        &mut probe,
                        &mut build,
                        &mut replacement,
                        view
                    ),
                    Err(CodingResumeError::BudgetExhausted)
                );
                assert_eq!((probe.calls.len(), build.calls), calls);
                assert_eq!(attempt.build_attempts().len(), 2);
                assert!(attempt.current(view).is_err());
                assert!(attempt.baseline().unwrap().process().is_some());
                assert!(attempt.current_probe().unwrap().process().is_some());
                assert_eq!(
                    identity,
                    (
                        view.owner_id(),
                        view.run_id(),
                        view.source_revision(),
                        view.reservation_id()
                    )
                );
            },
        )
    });
    assert_eq!(before, protected_state(&fixture.root));
}

#[test]
fn tlcx32_replacement_budget_after_late_publication_failure_is_still_exhausted() {
    let _serial = SERIAL.lock().unwrap();
    let fixture = PacketFixture::actual();
    let before = protected_state(&fixture.root);
    funded(|run, view| {
        configured(
            &fixture,
            run,
            view,
            |input, again, mut port, mut probe, mut build, _pipeline| {
                let plan = input.plan;
                let identity = (
                    view.owner_id(),
                    view.run_id(),
                    view.source_revision(),
                    view.reservation_id(),
                );
                let mut budget = CodingResumeCallBudget::checked(2, 2).unwrap();
                let mut attempt = resume_coding_attempt(
                    input,
                    &mut port,
                    Some(&mut probe),
                    Some(&mut build),
                    &mut budget,
                    view,
                );
                assert_eq!(attempt.stage(), CodingResumeStage::ReadyForPublication);
                // Admit the real capture, then reject its post-capture authority check.
                build.reject_after = Some(build.calls + 1);
                assert!(matches!(
                    attempt.publish_current(
                        plan,
                        &again,
                        &mut port,
                        &mut probe,
                        &mut build,
                        &mut budget,
                        view
                    ),
                    Err(CodingResumeError::Build(_))
                ));
                assert_eq!((budget.used_native(), budget.used_captures()), (2, 2));
                assert_eq!(attempt.build_attempts().len(), 2);
                assert!(attempt.build_attempts()[1].capture().is_some());
                assert!(attempt.build_attempts()[1].current().is_err());
                assert!(attempt.baseline().unwrap().observation().is_some());
                assert!(attempt.current_probe().unwrap().observation().is_some());
                assert!(attempt.current(view).is_err());

                let calls = (probe.calls.len(), build.calls);
                let mut replacement = CodingResumeCallBudget::checked(2, 2).unwrap();
                assert_eq!(
                    attempt.publish_current(
                        plan,
                        &again,
                        &mut port,
                        &mut probe,
                        &mut build,
                        &mut replacement,
                        view
                    ),
                    Err(CodingResumeError::BudgetExhausted)
                );
                assert_eq!((probe.calls.len(), build.calls), calls);
                assert_eq!(attempt.build_attempts().len(), 2);
                assert!(attempt.build_attempts()[1].capture().is_some());
                assert!(attempt.baseline().unwrap().process().is_some());
                assert!(attempt.current_probe().unwrap().process().is_some());
                assert!(attempt.current(view).is_err());
                assert_eq!(
                    identity,
                    (
                        view.owner_id(),
                        view.run_id(),
                        view.source_revision(),
                        view.reservation_id()
                    )
                );
            },
        )
    });
    assert_eq!(before, protected_state(&fixture.root));
}
