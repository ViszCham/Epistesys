//! Current/failure cuts use new owned files or provider state, no deletion/overwrite.
mod tlcx22_source_build_support;
use lc631_analysis::source_build_origin as b;
use lc631_substrate::resource::{ResourceError, ResourceKind};
use tlcx22_source_build_support::{authority::Cut, profiles::*, *};

#[test]
#[ignore = "central-only real registration/current Test cuts"]
fn tlcx22_sbo_wrong_registration_fresh_test_expiry_and_epoch_are_not_origins() {
    for mode in [Cut::Revoked, Cut::Expired, Cut::RollbackAfterFirst] {
        let f = Fixture::new(CHECKED);
        f.authority_cut.set(mode);
        f.with_input(None, |input, services, view| {
            let plan = b::BuildPlan::given(input, view).unwrap();
            let failure = b::observe_registered_source_build(&plan, services, view)
                .err()
                .expect("denied epoch/Test");
            assert!(matches!(
                (mode, failure.cause()),
                (Cut::Revoked, b::BuildError::Unauthorized)
                    | (Cut::Expired, b::BuildError::Expired)
                    | (Cut::RollbackAfterFirst, b::BuildError::ClockRollback)
            ));
            assert!(!failure.has_origin());
        });
    }
    let f = Fixture::new(CHECKED);
    f.with_input(None, |mut input, services, view| {
        input.compiler.expected_image = digest(b"not-the-registered-compiler");
        match b::BuildPlan::given(input, view) {
            Err(b::BuildError::CompilerRegistrationMismatch) => {}
            Err(e) => panic!("wrong registration admission {e:?}"),
            Ok(plan) => {
                let failure = b::observe_registered_source_build(&plan, services, view)
                    .err()
                    .expect("wrong compiler");
                assert!(matches!(
                    failure.cause(),
                    b::BuildError::CompilerRegistrationMismatch
                ));
                assert_eq!(failure.observed_child_count(), 0);
            }
        }
    });
    assert!(!f.output.exists());
}

struct StopAtMetadata;
impl b::BuildEventObserver for StopAtMetadata {
    fn observed(&mut self, event: b::BuildEvent<'_>) -> Result<(), b::BuildError> {
        if event.stage() == b::BuildStage::CompilerSysroot {
            return Err(b::BuildError::ConsumerRejected);
        }
        Ok(())
    }
}

struct RevokeBeforeLink(std::rc::Rc<std::cell::Cell<Cut>>);
impl b::BuildEventObserver for RevokeBeforeLink {
    fn observed(&mut self, event: b::BuildEvent<'_>) -> Result<(), b::BuildError> {
        if event.stage() == b::BuildStage::CompilerSysroot {
            self.0.set(Cut::RejectAtLink);
        }
        Ok(())
    }
}
#[test]
#[ignore = "central-only fresh authority immediately before link; earlier metadata is insufficient"]
fn tlcx22_sbo_late_test_loss_and_prebuild_cancel_never_dispatch_link() {
    let f = Fixture::new(CHECKED);
    let mut observer = RevokeBeforeLink(f.authority_cut.clone());
    f.with_input(Some(&mut observer), |input, services, view| {
        let plan = b::BuildPlan::given(input, view).unwrap();
        let failure = b::observe_registered_source_build(&plan, services, view)
            .err()
            .expect("late Test loss");
        assert!(matches!(
            failure.cause(),
            b::BuildError::AuthorityUnavailable
        ));
        assert_eq!(failure.observed_child_count(), 2);
        assert!(!failure.has_origin());
    });
    assert!(!f.output.exists());
    let f = Fixture::new(CHECKED);
    f.with_input(None, |input, services, view| {
        let plan = b::BuildPlan::given(input, view).unwrap();
        f.cancel.cancel();
        let failure = b::observe_registered_source_build(&plan, services, view)
            .err()
            .expect("prebuild cancel");
        assert!(matches!(
            failure.cause(),
            b::BuildError::Resource(ResourceError::Cancelled)
        ));
        assert_eq!(failure.observed_child_count(), 0);
        assert!(!failure.has_origin());
    });
    assert!(!f.output.exists());
}
#[test]
#[ignore = "central-only actual metadata success is not a completed source build"]
fn tlcx22_sbo_actual_metadata_children_without_link_preserve_partial_failure_no_origin() {
    let f = Fixture::new(CHECKED);
    let mut observer = StopAtMetadata;
    f.with_input(Some(&mut observer), |input, services, view| {
        let plan = b::BuildPlan::given(input, view).unwrap();
        let failure = b::observe_registered_source_build(&plan, services, view)
            .err()
            .expect("metadata cut");
        assert!(matches!(failure.cause(), b::BuildError::ConsumerRejected));
        assert_eq!(failure.observed_child_count(), 2);
        assert!(!failure.has_origin());
    });
    assert!(!f.output.exists());
}

struct AppendAfterLink<'a> {
    fixture: &'a Fixture,
    artifact: bool,
}
impl b::BuildEventObserver for AppendAfterLink<'_> {
    fn observed(&mut self, event: b::BuildEvent<'_>) -> Result<(), b::BuildError> {
        if event.stage() == b::BuildStage::Link {
            // Link event is exposed ONLY after the first no-follow artifact
            // fingerprint; otherwise an append before that hash could be blessed.
            assert!(event.artifact_fingerprint().is_some());
            let path = if self.artifact {
                &self.fixture.output
            } else {
                &self.fixture.subject_path
            };
            append_owned(path, b"\n// owned drift cut\n", &self.fixture.root);
        }
        Ok(())
    }
}
#[test]
#[ignore = "central-only postlink actual-source/artifact drift cuts"]
fn tlcx22_sbo_late_source_or_artifact_change_cannot_mint_an_origin() {
    for artifact in [false, true] {
        let f = Fixture::new(CHECKED);
        let mut observer = AppendAfterLink {
            fixture: &f,
            artifact,
        };
        f.with_input(Some(&mut observer), |input, services, view| {
            let plan = b::BuildPlan::given(input, view).unwrap();
            let failure = b::observe_registered_source_build(&plan, services, view)
                .err()
                .expect("late drift");
            assert!(matches!(
                failure.cause(),
                b::BuildError::SourceDrift | b::BuildError::ArtifactDrift
            ));
            assert_eq!(failure.observed_child_count(), 3);
            assert!(!failure.has_origin());
        });
        assert!(
            f.output.exists(),
            "real effects retained; never pretend rollback/delete"
        );
    }
}

#[test]
#[ignore = "central-only actual origin current source/native/cancel/provider cuts"]
fn tlcx22_sbo_current_read_rechecks_source_artifact_and_live_authority_not_cached_report() {
    for cut in 0..4 {
        let f = Fixture::new(CHECKED);
        f.with_input(None, |input, services, view| {
            let plan = b::BuildPlan::given(input, view).unwrap();
            let origin = b::observe_registered_source_build(&plan, services, view)
                .unwrap_or_else(|e| panic!("setup build: {:?}", e.cause()));
            origin
                .with_current(&plan, services, view, |v| assert_eq!(v.source_goal(), None))
                .unwrap();
            match cut {
                0 => append_owned(&f.subject_path, b"\n// current source drift\n", &f.root),
                1 => append_owned(&f.output, b"\nowned current artifact drift\n", &f.root),
                2 => f.cancel.cancel(),
                _ => f.authority_cut.set(Cut::RejectAfterLink),
            }
            let err = match origin.with_current(&plan, services, view, |_| ()) {
                Err(error) => error,
                Ok(_) => panic!("current cut"),
            };
            match cut {
                0 => assert!(matches!(err, b::BuildError::SourceDrift)),
                1 => assert!(matches!(err, b::BuildError::ArtifactDrift)),
                2 => assert!(matches!(
                    err,
                    b::BuildError::Resource(ResourceError::Cancelled)
                )),
                _ => assert!(matches!(err, b::BuildError::AuthorityUnavailable)),
            }
        });
    }
}

#[test]
#[ignore = "central-only funding refusal before compiler allocation/dispatch"]
fn tlcx22_sbo_resource_refusal_has_zero_actual_children_and_no_origin() {
    let f = Fixture::new(CHECKED);
    f.with_input(None, |input, services, view| {
        let plan = b::BuildPlan::given(input, view).unwrap();
        let mut fills = Vec::new();
        let mut refused = false;
        for _ in 0..8192 {
            match view.claim_resources(&[q(ResourceKind::Ram, 64 * 1024)]) {
                Ok(fill) => fills.push(fill),
                Err(ResourceError::Exhausted(ResourceKind::Ram)) => {
                    refused = true;
                    break;
                }
                Err(e) => panic!("unexpected fill {e:?}"),
            }
        }
        assert!(refused);
        let failure = b::observe_registered_source_build(&plan, services, view)
            .err()
            .expect("funding cut");
        assert!(matches!(
            failure.cause(),
            b::BuildError::Resource(ResourceError::Exhausted(ResourceKind::Ram))
        ));
        assert_eq!(failure.observed_child_count(), 0);
        assert!(!failure.has_origin());
        drop(fills);
    });
    assert!(!f.output.exists());
}

#[test]
#[ignore = "central-only two actual builds cannot exchange source/artifact frames"]
fn tlcx22_sbo_foreign_actual_artifact_and_origin_never_rebind_to_another_source_plan() {
    let a = Fixture::new(CHECKED);
    let other = Fixture::new(WRAPPING);
    a.with_input(None, |input_a, services_a, view_a| {
        let plan_a = b::BuildPlan::given(input_a, view_a).unwrap();
        let origin_a = b::observe_registered_source_build(&plan_a, services_a, view_a)
            .unwrap_or_else(|e| panic!("actual A: {:?}", e.cause()));
        other.with_input(None, |input_b, services_b, view_b| {
            let plan_b = b::BuildPlan::given(input_b, view_b).unwrap();
            let origin_b = b::observe_registered_source_build(&plan_b, services_b, view_b)
                .unwrap_or_else(|e| panic!("actual B: {:?}", e.cause()));
            assert_ne!(a.root, other.root);
            assert_ne!(a.subject.id(), other.subject.id());
            assert!(origin_b
                .with_current(&plan_a, services_a, view_a, |_| ())
                .is_err());
            assert!(origin_a
                .with_current(&plan_b, services_b, view_b, |_| ())
                .is_err());
        });
    });
}
