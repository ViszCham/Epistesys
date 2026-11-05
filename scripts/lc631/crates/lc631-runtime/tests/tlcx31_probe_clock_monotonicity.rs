//! TEST-FIRST: actual existing owners must retain caller-observed epoch maxima.
//! Not host-clock truth, revocation freshness, grants, or continuous watching.
mod tlcx31_probe_clock_support;
use lc631_runtime::contextual_repair::measurement::{
    paired_gain, select_current, OwnedCodingProbeGain, ProbeError,
};
use tlcx31_probe_clock_support::{epochs, fixture};

#[test]
fn tlcx31_clock_n01_observation_200_then_150_rejects_without_rewriting_birth() {
    fixture(|plan, baseline, _, view| {
        epochs(200, 100);
        baseline.check_current(plan, view).unwrap();
        assert_eq!(
            baseline.checked_epoch(),
            100,
            "birth is historical, not the live cursor"
        );
        epochs(150, 100);
        assert_eq!(
            baseline.check_current(plan, view),
            Err(ProbeError::ClockRollback)
        );
    });
}

#[test]
fn tlcx31_clock_n02_expiry_observation_cannot_be_erased_by_150() {
    fixture(|plan, baseline, _, view| {
        epochs(400, 100);
        assert_eq!(baseline.check_current(plan, view), Err(ProbeError::Stale));
        assert_eq!(baseline.checked_epoch(), 100);
        epochs(150, 100);
        assert!(
            matches!(
                baseline.check_current(plan, view),
                Err(ProbeError::ClockRollback | ProbeError::Stale)
            ),
            "observed expiry must not reactivate the same actual owner"
        );
    });
}

#[test]
fn tlcx31_clock_n03_actual_gain_200_then_150_cuts_current_and_selection() {
    fixture(|plan, baseline, candidate, view| {
        let gain = paired_gain(plan, baseline, candidate, view).unwrap();
        assert_eq!(gain.gain_quarters(), 0); // Literal equal-output control.
        epochs(200, 200);
        gain.check_current(plan, view).unwrap();
        epochs(150, 150);
        let current = gain.check_current(plan, view);
        let selected = select_current(plan, &[&gain], view);
        // Evaluate both real consumers before asserting, so selection is not
        // accidentally absent from the RED trace after the first failed assert.
        assert_eq!(current, Err(ProbeError::ClockRollback));
        assert_eq!(selected, Err(ProbeError::ClockRollback));
    });
}

#[test]
fn tlcx31_clock_n04_actual_pair_cannot_reset_its_input_owners_observed_200() {
    fixture(|plan, baseline, candidate, view| {
        epochs(200, 200);
        baseline.check_current(plan, view).unwrap();
        candidate.check_current(plan, view).unwrap();
        epochs(150, 150);
        assert!(
            matches!(
                paired_gain(plan, baseline, candidate, view),
                Err(ProbeError::ClockRollback)
            ),
            "paired_gain must consume live input-owner state"
        );
    });
}

#[test]
fn tlcx31_clock_n05_expired_actual_gain_does_not_reactivate_or_select() {
    fixture(|plan, baseline, candidate, view| {
        let gain = paired_gain(plan, baseline, candidate, view).unwrap();
        epochs(400, 400);
        assert_eq!(gain.check_current(plan, view), Err(ProbeError::Stale));
        epochs(150, 150);
        let current = gain.check_current(plan, view);
        let selected = select_current(plan, &[&gain], view);
        assert!(matches!(
            current,
            Err(ProbeError::ClockRollback | ProbeError::Stale)
        ));
        assert!(matches!(
            selected,
            Err(ProbeError::ClockRollback | ProbeError::Stale)
        ));
    });
}

#[test]
fn tlcx31_clock_p01_monotone_equality_and_role_local_timelines_remain_valid() {
    fixture(|plan, baseline, candidate, view| {
        let gain = paired_gain(plan, baseline, candidate, view).unwrap();
        for (base, next) in [(100, 100), (100, 100), (200, 150), (200, 150), (399, 399)] {
            epochs(base, next);
            baseline.check_current(plan, view).unwrap();
            candidate.check_current(plan, view).unwrap();
            gain.check_current(plan, view).unwrap();
            assert_eq!(select_current(plan, &[&gain], view), Ok(Some(0)));
            assert_eq!(
                (baseline.checked_epoch(), candidate.checked_epoch()),
                (100, 100)
            );
        }
    });
}

#[test]
fn tlcx31_clock_p02_gain_current_auto_traits_are_not_silently_narrowed() {
    // Compile-time contract for the ACTUAL existing type, not a lookalike.
    // Central must observe compilation; this author has not run Cargo/rustc.
    fn send_sync<T: Send + Sync>() {}
    send_sync::<OwnedCodingProbeGain<'static>>();
}
