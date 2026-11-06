//! TEST-FIRST, unrun. Existing actual artifact fixture; missing H2/export is not proof.
#[path = "tlcx22_goal_artifact_support/mod.rs"]
mod actual;
mod tlcx31_goal_clock_floor_support;
use actual::{fixture, BASE, HAND};
use lc631_runtime::{
    contextual_repair::measurement::{paired_gain, ProbeError},
    goal_suite_measurement as g,
};
use lc631_substrate::resource::ResourceError;
use tlcx31_goal_clock_floor_support::*;

#[test]
fn tlcx31_goal_outer200_hidden_owner100_rejects_and_retains_floor_after_failed_row() {
    fixture(|h, current, view| {
        assert_eq!(h.baseline.actual_outputs(), &BASE);
        assert_eq!(h.candidate.actual_outputs(), &HAND);
        let birth = h.baseline.checked_epoch();
        let hash = h.baseline.observation_digest();
        h.baseline.check_current(h.plan, view).unwrap();
        let root = g::ArtifactPairRoot::given(h.root_input(), view).unwrap();
        reset();
        let rejected = with_clocks(current, base200, next100, |cut| {
            g::measure_artifact_pair(&root, h.pair(), cut, view).err()
        });
        // Evaluate ALL real consumers before asserting. A failed first assert
        // must not erase whether owner/gain still forget the external floor.
        let owner = h.baseline.check_current(h.plan, view);
        let gain = paired_gain(h.plan, h.baseline, h.candidate, view).err();
        let unrelated = h.candidate.check_current(h.plan, view);
        assert!(matches!(rejected, Some(g::Error::ClockRollback)));
        assert_eq!(owner, Err(ProbeError::ClockRollback));
        assert_eq!(gain, Some(ProbeError::ClockRollback));
        assert_eq!(unrelated, Ok(()));
        assert_eq!((birth, h.baseline.checked_epoch()), (100, 100));
        assert_eq!(h.baseline.observation_digest(), hash);
        assert!(reads().0 > 0);
    });
}

#[test]
fn tlcx31_goal_candidate_outer200_does_not_pollute_other_roles_clock() {
    fixture(|h, current, view| {
        let root = g::ArtifactPairRoot::given(h.root_input(), view).unwrap();
        reset();
        let rejected = with_clocks(current, base100, next200, |cut| {
            g::measure_artifact_pair(&root, h.pair(), cut, view).err()
        });
        let baseline = h.baseline.check_current(h.plan, view);
        let candidate = h.candidate.check_current(h.plan, view);
        assert!(matches!(rejected, Some(g::Error::ClockRollback)));
        assert_eq!(baseline, Ok(()));
        assert_eq!(candidate, Err(ProbeError::ClockRollback));
        assert!(reads().1 > 0);
    });
}

#[test]
fn tlcx31_goal_observed_outer_expiry400_never_reactivates_hidden_owner100() {
    fixture(|h, current, view| {
        let root = g::ArtifactPairRoot::given(h.root_input(), view).unwrap();
        reset();
        let rejected = with_clocks(current, base400, next100, |cut| {
            g::measure_artifact_pair(&root, h.pair(), cut, view).err()
        });
        let owner = h.baseline.check_current(h.plan, view);
        assert!(
            matches!(rejected, Some(g::Error::Expired)),
            "direct sampled expiry, not arbitrary legacy Stale"
        );
        assert_eq!(owner, Err(ProbeError::ClockRollback));
        assert_eq!(h.baseline.checked_epoch(), 100);
    });
}

#[test]
fn tlcx31_goal_actual_equality100_is_control_not_a_clock_success_stub() {
    fixture(|h, current, view| {
        let root = g::ArtifactPairRoot::given(h.root_input(), view).unwrap();
        reset();
        with_clocks(current, base100, next100, |cut| {
            let owner = g::measure_artifact_pair(&root, h.pair(), cut, view).unwrap();
            owner
                .with_current(&root, h.pair(), cut, view, |v| {
                    assert_eq!(v.baseline_outputs(), &BASE);
                    assert_eq!(v.candidate_outputs(), &HAND);
                    assert_eq!(v.requested_source_goal(), None);
                    assert_eq!(v.source_eligibility(), g::SourceEligibility::Pending);
                })
                .unwrap();
        });
        assert_eq!(h.baseline.check_current(h.plan, view), Ok(()));
        assert_eq!(h.candidate.check_current(h.plan, view), Ok(()));
        assert!(reads().0 > 0 && reads().1 > 0);
    });
}

#[test]
fn tlcx31_goal_new_frame_scope_is_not_legacy_permission_or_clock_admission() {
    fixture(|h, current, view| {
        let root = g::ArtifactPairRoot::given(h.root_input(), view).unwrap();
        reset();
        let old = current.baseline_invocation.authority;
        let frame_scope = "tlcx/framework/purpose-frame/not-the-legacy-probe-scope";
        let authority = lc631_core::ExecutionPermitContext {
            scope: frame_scope,
            ..*old
        };
        let baseline = actual::invocation(&current.baseline_invocation, &authority);
        let candidate = actual::invocation(
            &current.candidate_invocation,
            current.candidate_invocation.authority,
        );
        let mut bad = g::ArtifactCurrent {
            baseline_invocation: baseline,
            candidate_invocation: candidate,
            baseline_build: &mut *current.baseline_build,
            candidate_build: &mut *current.candidate_build,
            baseline_authority: &mut *current.baseline_authority,
            candidate_authority: &mut *current.candidate_authority,
        };
        let rejected = with_clocks(&mut bad, base200, next100, |cut| {
            g::measure_artifact_pair(&root, h.pair(), cut, view).err()
        });
        assert!(matches!(rejected, Some(g::Error::Unauthorized)));
        assert_eq!(
            h.baseline.check_current(h.plan, view),
            Ok(()),
            "unadmitted context cannot set owner floor"
        );
        assert_eq!(
            reads().0,
            0,
            "cheap scope/Test refusal precedes clock sample"
        );
    });
}

#[test]
fn tlcx31_goal_cancel_and_read_quota_keep_resource_causes_before_clock() {
    fixture(|h, current, view| {
        let root = g::ArtifactPairRoot::given(h.root_input(), view).unwrap();
        reset();
        h.cancel.cancel();
        let rejected = with_clocks(current, base200, next100, |cut| {
            g::measure_artifact_pair(&root, h.pair(), cut, view).err()
        });
        assert!(matches!(
            rejected,
            Some(g::Error::Resource(ResourceError::Cancelled))
        ));
        assert_eq!(reads(), (0, 0));
    });
    fixture(|h, current, view| {
        let mut input = h.root_input();
        input.limits.max_current_checks = 1;
        let root = g::ArtifactPairRoot::given(input, view).unwrap();
        with_clocks(current, base100, next100, |cut| {
            let owner = g::measure_artifact_pair(&root, h.pair(), cut, view).unwrap();
            reset();
            let rejected = owner.with_current(&root, h.pair(), cut, view, |_| ());
            assert!(matches!(rejected, Err(g::Error::ReadBudget)));
            assert_eq!(reads(), (0, 0));
        });
    });
}
