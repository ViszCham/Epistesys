//! 日本語: 公開resume APIのconstruction probeと、同一実ownerの予算差し替え負例。
//! missing exportはconstruction REDのみであり、挙動の成功・失敗とは区別する。
//! English: Public resume API construction probe and budget-replacement negatives on one actual owner.
//! Missing exports establish construction RED only, separately from behavioral outcomes.
//! 日本語: native2/capture2は検査対象attemptの上限であり、fixture準備の別実行を含めない。
//! English: Native2/capture2 bounds the tested attempt, excluding separate fixture preparation executions.
#![forbid(unsafe_code)]

#[path = "tlcx32_verified_support/mod.rs"]
#[allow(dead_code, reason = "Shared actual repair setup has other independent consumers.")]
mod file_fixture;
#[path = "tlcx31_coding_support/mod.rs"]
#[allow(dead_code, reason = "Reuse actual native registration and fixture compilation only.")]
mod coding_support;
#[path = "tlcx32_fresh_support/mod.rs"]
#[allow(dead_code, reason = "Reuse real checkpoint creation, not the older resume entry.")]
mod fresh_support;
#[path = "tlcx32_resume_attempt_support/mod.rs"]
#[allow(dead_code, reason = "Reuse the actual configured opaque-owner fixture.")]
mod support;

use lc631_analysis::declared_build_capture::{DeclaredBuildContentPins, SnapshotCaptureError};
use lc631_runtime::{
    contextual_repair::{attempt::NativeStageEffect, snapshot_gate::SnapshotGateError},
    contextual_resume::production_contract::{
        resume_coding_attempt, CodingResumeCallBudget, CodingResumeError, CodingResumeStage,
        ResumeAttempt,
    },
};
use lc631_substrate::Digest32;
use support::{configured, funded, protected_state, PacketFixture, SERIAL};

// 日本語: getterは既に実行されたopaque ownerの履歴を読むだけで、ownerや成功を生成しない。
// English: Getters inspect existing opaque execution history; they construct neither owners nor success.
fn native_history(attempt: &ResumeAttempt<'_>) -> [Digest32; 2] {
    let baseline = attempt.baseline().expect("positive setup retains baseline owner");
    let current = attempt.current_probe().expect("positive setup retains current owner");
    assert_eq!(baseline.effect(), NativeStageEffect::ExecutedObserved);
    assert_eq!(current.effect(), NativeStageEffect::ExecutedObserved);
    assert_eq!(baseline.process().unwrap().stdout(), b"none\nnone\nnone\nnone\n");
    assert_eq!(current.process().unwrap().stdout(),
        b"some:0\nsome:3\nnone\nsome:4294967295\n");
    let baseline = baseline.observation().expect("actual baseline observation");
    let current = current.observation().expect("actual current observation");
    assert_eq!(baseline.actual_outputs(), &[None, None, None, None]);
    assert_eq!(current.actual_outputs(), &[Some(0), Some(3), None, Some(u32::MAX)]);
    assert_eq!((baseline.correct_probes(), current.correct_probes()), (1, 4));
    [baseline.observation_digest(), current.observation_digest()]
}

fn capture_history(attempt: &ResumeAttempt<'_>) -> Vec<(DeclaredBuildContentPins, Digest32)> {
    attempt.build_attempts().iter().map(|attempt| {
        let capture = attempt.capture().expect("real completed inventory remains owned");
        (capture.content_pins(), capture.snapshot_digest())
    }).collect()
}

#[test]
fn resume_owner_replacement_budget_cannot_publish_a_third_capture() {
    let _serial = SERIAL.lock().unwrap();
    let fixture = PacketFixture::actual();
    let protected = protected_state(&fixture.root);
    funded(|run, view| configured(&fixture, run, view,
        |input, again, mut port, mut probe, mut build, _pipeline| {
            let plan = input.plan;
            let identity = (view.owner_id(), view.run_id(), view.source_revision(), view.reservation_id());
            let before = (probe.calls.len(), build.calls);
            let mut budget = CodingResumeCallBudget::checked(2, 2).unwrap();
            let mut attempt = resume_coding_attempt(input, &mut port,
                Some(&mut probe), Some(&mut build), &mut budget, view);
            assert_eq!(attempt.stage(), CodingResumeStage::ReadyForPublication);
            assert!(attempt.diagnostic().is_none());
            assert_eq!(attempt.current(view), Err(CodingResumeError::NeedsPublication));
            assert_eq!((budget.used_native(), budget.used_captures()), (2, 1));
            assert_eq!((probe.calls.len(), build.calls), (before.0 + 4, before.1 + 2));
            assert_eq!(attempt.build_attempts().len(), 1);
            assert!(attempt.build_attempts()[0].current().is_ok());
            let natives = native_history(&attempt);
            assert_eq!(attempt.gain().unwrap().gain_quarters(), 3);

            // 日本語: 正当な最初のpublicationは同じ実pairとcapture2を使う。
            // English: The first valid publication uses the same actual pair and capture2.
            assert!(attempt.publish_current(plan, &again, &mut port, &mut probe,
                &mut build, &mut budget, view).expect("positive first publication"));
            assert_eq!(attempt.stage(), CodingResumeStage::Published);
            assert!(attempt.current(view).unwrap());
            assert_eq!((budget.used_native(), budget.used_captures()), (2, 2));
            assert_eq!((probe.calls.len(), build.calls), (before.0 + 5, before.1 + 4));
            assert_eq!(attempt.build_attempts().len(), 2);
            assert!(attempt.build_attempts().iter().all(|capture| capture.current().is_ok()));
            let captures = capture_history(&attempt);
            assert!(captures.iter().all(|(pins, _)| pins.build == fixture.pins[10]));
            assert_eq!(native_history(&attempt), natives);

            // 日本語: attempt/port/pipelineは同じで、callerのbudgetだけを新しくする。
            // English: Keep the attempt/port/pipeline; replace only the caller's budget.
            let calls = (probe.calls.len(), build.calls);
            let mut replacement = CodingResumeCallBudget::checked(2, 2).unwrap();
            let result = attempt.publish_current(plan, &again, &mut port, &mut probe,
                &mut build, &mut replacement, view);
            assert_eq!(result, Err(CodingResumeError::BudgetExhausted),
                "same owner must reject before providers/capture3; captures={}, probe_calls={} (before {}), snapshot_calls={} (before {})",
                attempt.build_attempts().len(), probe.calls.len(), calls.0, build.calls, calls.1);
            assert_eq!((probe.calls.len(), build.calls), calls);
            assert_eq!((budget.used_native(), budget.used_captures()), (2, 2));
            assert_eq!(capture_history(&attempt), captures);
            assert_eq!(native_history(&attempt), natives);
            assert_eq!(attempt.stage(), CodingResumeStage::Hold);
            assert!(attempt.current(view).is_err());
            assert_eq!(identity,
                (view.owner_id(), view.run_id(), view.source_revision(), view.reservation_id()));
        }));
    assert_eq!(protected_state(&fixture.root), protected);
}

#[test]
fn resume_owner_late_capture_failure_retry_is_exhausted_without_claiming_capture3() {
    let _serial = SERIAL.lock().unwrap();
    let fixture = PacketFixture::actual();
    let protected = protected_state(&fixture.root);
    funded(|run, view| configured(&fixture, run, view,
        |input, again, mut port, mut probe, mut build, _pipeline| {
            let plan = input.plan;
            let identity = (view.owner_id(), view.run_id(), view.source_revision(), view.reservation_id());
            let before = (probe.calls.len(), build.calls);
            let mut budget = CodingResumeCallBudget::checked(2, 2).unwrap();
            let mut attempt = resume_coding_attempt(input, &mut port,
                Some(&mut probe), Some(&mut build), &mut budget, view);
            assert_eq!(attempt.stage(), CodingResumeStage::ReadyForPublication);
            assert!(attempt.diagnostic().is_none());
            assert_eq!((budget.used_native(), budget.used_captures()), (2, 1));
            assert_eq!((probe.calls.len(), build.calls), (before.0 + 4, before.1 + 2));
            assert_eq!(attempt.build_attempts().len(), 1);
            let natives = native_history(&attempt);

            // 日本語: capture2の実inventoryを得た後のauthority取得だけを拒否する。
            // English: Reject authority only after capture2's real inventory has completed.
            build.reject_after = Some(build.calls + 1);
            assert_eq!(attempt.publish_current(plan, &again, &mut port, &mut probe,
                &mut build, &mut budget, view),
                Err(CodingResumeError::Build(SnapshotGateError::Capture(
                    SnapshotCaptureError::AuthorityUnavailable))));
            assert_eq!(attempt.stage(), CodingResumeStage::Hold);
            assert_eq!((budget.used_native(), budget.used_captures()), (2, 2));
            assert_eq!((probe.calls.len(), build.calls), (before.0 + 4, before.1 + 4));
            assert_eq!(attempt.build_attempts().len(), 2);
            assert!(attempt.build_attempts()[1].capture().is_some());
            assert_eq!(attempt.build_attempts()[1].diagnostic(),
                Some(&SnapshotCaptureError::AuthorityUnavailable));
            assert!(attempt.build_attempts()[1].current().is_err());
            assert!(attempt.current(view).is_err());
            let captures = capture_history(&attempt);
            assert_eq!(native_history(&attempt), natives);

            // 日本語: この負例はerror分類を検査する。既存diagnosticが止めるcapture3は主張しない。
            // English: This negative checks error classification; it does not claim capture3 past a stored diagnostic.
            let calls = (probe.calls.len(), build.calls);
            let mut replacement = CodingResumeCallBudget::checked(2, 2).unwrap();
            let result = attempt.publish_current(plan, &again, &mut port, &mut probe,
                &mut build, &mut replacement, view);
            assert_eq!(result, Err(CodingResumeError::BudgetExhausted),
                "consumed owner slots precede replaying the late Build error; captures={}, probe_calls={} (before {}), snapshot_calls={} (before {})",
                attempt.build_attempts().len(), probe.calls.len(), calls.0, build.calls, calls.1);
            assert_eq!((probe.calls.len(), build.calls), calls);
            assert_eq!(capture_history(&attempt), captures);
            assert_eq!(native_history(&attempt), natives);
            assert_eq!(attempt.build_attempts()[1].diagnostic(),
                Some(&SnapshotCaptureError::AuthorityUnavailable));
            assert_eq!(attempt.stage(), CodingResumeStage::Hold);
            assert!(attempt.current(view).is_err());
            assert_eq!(identity,
                (view.owner_id(), view.run_id(), view.source_revision(), view.reservation_id()));
        }));
    assert_eq!(protected_state(&fixture.root), protected);
}
