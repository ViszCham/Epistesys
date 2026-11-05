//! TEST FIRST: intentionally missing additive production APIs, not a body.
//! Planned RED is unresolved `contextual_repair::attempt`/shared-deadline API.
//! Central alone executes compilation/native processes. No success stub exists.
#[path="tlcx31_native_attempt_support/mod.rs"] mod support;
use support::*;
use lc631_core::{Action,CallerOrigin,PrincipalBinding,stable_sha256};
use lc631_runtime::contextual_repair::{attempt::*,measurement::*};
use lc631_runtime::resource::ResourceError;

// Exact proposed producer contract (no implementation in test/support):
// observe_current_attempt<'res>(&CodingProbePlan,&ProbeAttemptInput<'_>,
//   ProbePurpose,Option<&mut dyn CurrentProbeAuthority>,&'res ResourceView)
//   -> ProbeAttempt<'res>.
// Attempt PRIVATE owns Option<ResourceToolResult<'res>> plus optional checked
// observation, diagnostic and NativeStageEffect. current()/observation() are
// sampled/historical only; fresh publication MUST call recheck_current, then
// the independent declared-build gate. Never reconstruct owners from reports.
// CurrentProbeAuthority::current(ProbeAuthorityRequest{purpose,phase,frame,
// execution_scope,source_revision})->ProbeAuthorityView{permit,context,frame}.
// BeforeSpawn is INSIDE the actual native producer AFTER all nested executable
// provenance/hash preparation. AfterChild and BeforePublication are separate.

fn assert_before_effects(attempt:&ProbeAttempt<'_>,fixture:&Fixture) {
    assert_eq!(attempt.effect(),NativeStageEffect::NotAttempted);
    assert!(attempt.diagnostic().is_some());
    assert!(attempt.process().is_none());
    assert!(attempt.observation().is_none());
    assert!(attempt.current().is_err());
    assert!(!fixture.marker.exists(),"no provider or wrong grant must not start a native child");
}

#[test]
fn tlcx31_attempt_default_missing_provider_is_hold_with_zero_native_effects() {
    let fixture=Fixture::new(Output::Correct);
    with_resources(|_,view,_| {
        let attempt=observe_current_attempt(&fixture.plan,&fixture.input(),ProbePurpose::Current,None,view);
        assert_before_effects(&attempt,&fixture);
    });
}

#[test]
fn tlcx31_attempt_external_receipts_real_child_and_paired_owner_are_consumed() {
    let baseline=Fixture::new(Output::Baseline);
    let current=Fixture::new(Output::Correct);
    with_resources(|run,view,cancel| {
        let mut base_authority=baseline.provider(ProbePurpose::Baseline,Action::Test,Cut::None,run,cancel.clone());
        let mut current_authority=current.provider(ProbePurpose::Current,Action::Test,Cut::None,run,cancel);
        let base=observe_current_attempt(&baseline.plan,&baseline.input(),ProbePurpose::Baseline,Some(&mut base_authority),view);
        let mut now=observe_current_attempt(&current.plan,&current.input(),ProbePurpose::Current,Some(&mut current_authority),view);
        assert_eq!(base.effect(),NativeStageEffect::ExecutedObserved);
        assert_eq!(now.effect(),NativeStageEffect::ExecutedObserved);
        assert_eq!(base.current().unwrap().correct_probes(),1);
        assert_eq!(now.current().unwrap().correct_probes(),4);
        assert_eq!(now.process().unwrap().stdout(),b"some:0\nsome:3\nnone\nsome:4294967295\n");
        assert_eq!(now.process().unwrap().reported().exit_code,Some(0));
        let gain=paired_gain(&current.plan,base.current().unwrap(),now.current().unwrap(),view).unwrap();
        assert_eq!(gain.gain_quarters(),3);
        assert_eq!(select_current(&current.plan,&[&gain],view).unwrap(),Some(0));
        now.recheck_current(&current.plan,&current.input(),ProbePurpose::Current,&mut current_authority,view).unwrap();
        assert_eq!(base_authority.calls,vec![(ProbePurpose::Baseline,ProbeAuthorityPhase::BeforeSpawn),(ProbePurpose::Baseline,ProbeAuthorityPhase::AfterChild)]);
        assert_eq!(current_authority.calls,vec![(ProbePurpose::Current,ProbeAuthorityPhase::BeforeSpawn),(ProbePurpose::Current,ProbeAuthorityPhase::AfterChild),(ProbePurpose::Current,ProbeAuthorityPhase::BeforePublication)]);
        assert!(baseline.marker.exists() && current.marker.exists());
    });
}

#[test]
fn tlcx31_attempt_edit_baseline_and_descriptor_grants_do_not_authorize_current_native() {
    for wrong in 0..4 {
        let fixture=Fixture::new(Output::Correct);
        with_resources(|run,view,cancel| {
            let mut authority=match wrong {
                0=>fixture.provider(ProbePurpose::Current,Action::Edit,Cut::None,run,cancel),
                1=>fixture.provider(ProbePurpose::Baseline,Action::Test,Cut::None,run,cancel),
                2=>fixture.provider_for_scope("tlcx/declared-build/read/v1/descriptor-only".into(),Action::Test,Cut::None,run,cancel),
                _=>fixture.provider_for_scope(fixture.registration().execution_scope(&fixture.plan),Action::Test,Cut::None,run,cancel),
            };
            let attempt=observe_current_attempt(&fixture.plan,&fixture.input(),ProbePurpose::Current,Some(&mut authority),view);
            assert_before_effects(&attempt,&fixture);
        });
    }
}

#[test]
fn tlcx31_attempt_wrong_origin_frame_source_build_target_binary_or_goal_never_spawns() {
    for cut in 0..10 {
        let fixture=Fixture::new(Output::Correct);
        with_resources(|run,view,cancel| {
            let mut authority=fixture.provider(ProbePurpose::Current,Action::Test,Cut::None,run,cancel);
            let mut input=fixture.input();
            let mut altered_snapshot=fixture.snapshot.clone();
            let wrong=stable_sha256("different registered artifact");
            let wrong_principal=PrincipalBinding::checked(stable_sha256("different caller principal")).unwrap();
            let wrong_plan=CodingProbePlan::checked_add(digest(b"changed goal"),fixture.plan.policy()).unwrap();
            match cut {
                0=>input.origin.principal=&wrong_principal,
                1=>input.origin.frame=digest(b"different frame"),
                2=>input.source="Different source must not gain authority.",
                3=>{altered_snapshot.build_input_digest=wrong.clone();input.registration.snapshot=&altered_snapshot;}
                4=>input.registration.target_ref="src/unregistered.rs",
                5=>input.registration.executable_digest=&wrong,
                6=>input.origin.caller_origin=CallerOrigin::StandaloneCli,
                8=>input.origin.host_fingerprint=&wrong,
                9=>input.origin.revocation_floor=1,
                _=>{},
            }
            let plan=if cut==7 {&wrong_plan}else{&fixture.plan};
            let attempt=observe_current_attempt(plan,&input,ProbePurpose::Current,Some(&mut authority),view);
            assert_before_effects(&attempt,&fixture);
        });
    }
}

#[test]
fn tlcx31_attempt_valid_parsed_owner_and_real_process_survive_each_late_cut() {
    for cut in [Cut::PostUnavailable,Cut::PostRevoked,Cut::PostClockRollback,Cut::PostFrame,Cut::PostCancel,Cut::PostDeadline] {
        let fixture=Fixture::new(Output::Correct);
        with_resources(|run,view,cancel| {
            let mut authority=fixture.provider(ProbePurpose::Current,Action::Test,cut,run,cancel);
            let attempt=observe_current_attempt(&fixture.plan,&fixture.input(),ProbePurpose::Current,Some(&mut authority),view);
            assert!(attempt.current().is_err(),"late cut {cut:?} must not publish success");
            assert!(attempt.diagnostic().is_some());
            assert_eq!(attempt.effect(),NativeStageEffect::ExecutedObserved);
            assert_eq!(attempt.process().unwrap().stdout(),b"some:0\nsome:3\nnone\nsome:4294967295\n");
            assert_eq!(attempt.observation().unwrap().correct_probes(),4,"valid actual history remains owned on {cut:?}");
            assert!(fixture.marker.exists());
            assert_eq!(authority.calls.len(),2);
            if matches!(cut,Cut::PostCancel) {assert!(matches!(attempt.diagnostic(),Some(ProbeError::Resource(ResourceError::Cancelled))));}
            if matches!(cut,Cut::PostDeadline) {assert!(matches!(attempt.diagnostic(),Some(ProbeError::Resource(ResourceError::DeadlineExceeded))));}
        });
    }
}

#[test]
fn tlcx31_attempt_parse_or_nonzero_exit_retains_process_without_fabricated_observation() {
    for output in [Output::Malformed,Output::Failed] {
        let fixture=Fixture::new(output);
        with_resources(|run,view,cancel| {
            let mut authority=fixture.provider(ProbePurpose::Current,Action::Test,Cut::None,run,cancel);
            let attempt=observe_current_attempt(&fixture.plan,&fixture.input(),ProbePurpose::Current,Some(&mut authority),view);
            assert_eq!(attempt.effect(),NativeStageEffect::ExecutedObserved);
            assert!(attempt.current().is_err());
            assert!(attempt.observation().is_none());
            let process=attempt.process().expect("actual native process owner cannot disappear on parsing/exit failure");
            match output {
                Output::Malformed=>{assert!(matches!(attempt.diagnostic(),Some(ProbeError::InvalidOutput)));assert_eq!(process.stdout(),b"invalid actual native output\n");}
                Output::Failed=>{assert!(matches!(attempt.diagnostic(),Some(ProbeError::ProcessFailed)));assert_eq!(process.reported().exit_code,Some(7));}
                Output::Baseline|Output::Correct=>unreachable!(),
            }
            assert!(fixture.marker.exists());
        });
    }
}

#[test]
fn tlcx31_attempt_publication_resamples_provider_and_keeps_actual_owner_on_rejection() {
    let fixture=Fixture::new(Output::Correct);
    with_resources(|run,view,cancel| {
        let mut authority=fixture.provider(ProbePurpose::Current,Action::Test,Cut::PublicationUnavailable,run,cancel);
        let input=fixture.input();
        let mut attempt=observe_current_attempt(&fixture.plan,&input,ProbePurpose::Current,Some(&mut authority),view);
        assert!(attempt.current().is_ok());
        assert!(attempt.recheck_current(&fixture.plan,&input,ProbePurpose::Current,&mut authority,view).is_err());
        assert!(attempt.current().is_err(),"a previous sampled success is not a current publication permit");
        assert!(attempt.observation().is_some() && attempt.process().is_some());
        assert_eq!(authority.calls.last(),Some(&(ProbePurpose::Current,ProbeAuthorityPhase::BeforePublication)));
    });
}

#[test]
fn tlcx31_attempt_publication_rejects_changed_physical_target_without_another_child() {
    let fixture=Fixture::new(Output::Correct);
    with_resources(|run,view,cancel| {
        let mut authority=fixture.provider(ProbePurpose::Current,Action::Test,Cut::None,run,cancel);
        let input=fixture.input();
        let mut attempt=observe_current_attempt(&fixture.plan,&input,ProbePurpose::Current,Some(&mut authority),view);
        assert!(attempt.current().is_ok());
        let original=std::fs::read_to_string(&fixture.target).unwrap();
        let changed=original.replace("checked_add","checked_sub");
        assert_ne!(original,changed);assert_eq!(original.len(),changed.len());
        std::fs::write(&fixture.target,changed).unwrap();
        assert!(attempt.recheck_current(&fixture.plan,&input,ProbePurpose::Current,&mut authority,view).is_err());
        assert!(attempt.process().is_some() && attempt.observation().is_some());
        assert_eq!(authority.calls.iter().filter(|(_,phase)|*phase==ProbeAuthorityPhase::BeforeSpawn).count(),1);
    });
}
