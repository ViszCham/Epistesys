//! Actual-owner dynamic-clock TEST-FIRST; all cases unrun by this author.
mod tlcx31_goal_dynamic_clock_support;
mod tlcx31_registered_clock_support;
use lc631_analysis::declared_build_capture::SnapshotCaptureError;
use lc631_runtime::{contextual_repair::measurement::ProbeError, goal_suite_measurement as g};
use tlcx31_goal_dynamic_clock_support::{
    clocks::{self, Event, Lane},
    fixture, BASE, HAND,
};
use Lane::{
    CaptureBase as CB, CaptureNext as CN, DirectBase as DB, DirectNext as DN, HiddenBase as HB,
    HiddenNext as HN,
};

fn repeated(epoch: u64, n: usize) -> Vec<u64> {
    vec![epoch; n]
}
fn no_capture() {
    assert!(clocks::values(CB).is_empty() && clocks::values(CN).is_empty());
}

#[test]
fn fixture_name_collision_keeps_existing_owner_untouched() {
    use tlcx31_goal_dynamic_clock_support::fresh_dir;
    let parent = fresh_dir(
        &std::env::temp_dir(),
        &format!("tlcx-ci-dir-{}", std::process::id()),
    );
    let first = fresh_dir(&parent, "same-clock");
    std::fs::write(first.join("owned-marker"), b"keep original").unwrap();
    let second = fresh_dir(&parent, "same-clock");
    assert_ne!(first, second);
    assert_eq!(
        std::fs::read(first.join("owned-marker")).unwrap(),
        b"keep original"
    );
    assert!(!second.join("owned-marker").exists());
}

#[test]
fn d01_actual_hidden150_after_observed200_rejects_h2_and_same_owner() {
    fixture(|h, current, view| {
        clocks::all(200);
        clocks::script(HB, &[200, 150], 150);
        let root = g::ArtifactPairRoot::given(h.root_input(), view).unwrap();
        let rejected = g::measure_artifact_pair(&root, h.pair(), current, view).err();
        let hidden = clocks::values(HB);
        let direct = clocks::values(DB);
        no_capture();
        let same_owner = h.baseline.check_current(h.plan, view);
        assert!(matches!(rejected, Some(g::Error::ClockRollback)));
        assert_eq!(hidden, [200, 150]);
        assert_eq!(direct, [200, 200, 200]);
        assert_eq!(same_owner, Err(ProbeError::ClockRollback));
        assert_eq!(h.baseline.checked_epoch(), 100);
    });
}

#[test]
fn d02_actual_hidden250_is_not_lost_by_returning_direct200_or_later150() {
    fixture(|h, current, view| {
        clocks::all(200);
        clocks::constant(HB, 250);
        let root = g::ArtifactPairRoot::given(h.root_input(), view).unwrap();
        let rejected = g::measure_artifact_pair(&root, h.pair(), current, view).err();
        let trace = clocks::trace();
        clocks::constant(HB, 150);
        let same_owner = h.baseline.check_current(h.plan, view);
        assert!(matches!(rejected, Some(g::Error::ClockRollback)));
        assert_eq!(
            trace,
            [
                Event {
                    lane: DB,
                    epoch: 200
                },
                Event {
                    lane: HB,
                    epoch: 250
                },
                Event {
                    lane: DB,
                    epoch: 200
                }
            ]
        );
        assert_eq!(same_owner, Err(ProbeError::ClockRollback));
        assert_eq!(clocks::values(HB), [250, 150]);
        no_capture();
    });
}

#[test]
fn d03_first_and_second_actual_physical_check_pre200_post150_are_cuts() {
    // Literal call ordinals from the independent finite trace below. The actual
    // owner clock function changes, not a renamed constant or fabricated DTO.
    for (hidden, direct) in [
        (&[200, 200, 200, 150][..], 4),
        (&[200, 200, 200, 200, 200, 200, 150][..], 6),
    ] {
        fixture(|h, current, view| {
            clocks::all(200);
            clocks::script(HB, hidden, 150);
            let root = g::ArtifactPairRoot::given(h.root_input(), view).unwrap();
            let rejected = g::measure_artifact_pair(&root, h.pair(), current, view).err();
            assert!(matches!(rejected, Some(g::Error::ClockRollback)));
            assert_eq!(clocks::values(HB), hidden);
            assert_eq!(clocks::values(DB), repeated(200, direct));
            no_capture();
            assert_eq!(h.baseline.actual_outputs(), &BASE);
        });
    }
}

#[test]
fn d04_outer_after_first_sample150_rejects_after_two_actual_current_captures() {
    fixture(|h, current, view| {
        clocks::all(200);
        clocks::script(DB, &[200, 200, 200, 200, 200, 200, 200, 200, 150], 150);
        let root = g::ArtifactPairRoot::given(h.root_input(), view).unwrap();
        let rejected = g::measure_artifact_pair(&root, h.pair(), current, view).err();
        assert!(matches!(rejected, Some(g::Error::ClockRollback)));
        assert_eq!(
            clocks::values(DB),
            [200, 200, 200, 200, 200, 200, 200, 200, 150]
        );
        assert_eq!(clocks::values(HB), repeated(200, 9));
        assert_eq!(clocks::values(CB), [200, 200]);
        assert_eq!(clocks::values(CN), [200, 200]);
    });
}

#[test]
fn d05_outer_after_final_sample150_cannot_return_a_stale_token() {
    fixture(|h, current, view| {
        clocks::all(200);
        clocks::script(DB, &[200, 200, 200, 200, 200, 200, 200, 200, 200, 150], 150);
        let root = g::ArtifactPairRoot::given(h.root_input(), view).unwrap();
        let rejected = g::measure_artifact_pair(&root, h.pair(), current, view).err();
        assert!(matches!(rejected, Some(g::Error::ClockRollback)));
        assert_eq!(
            clocks::values(DB),
            [200, 200, 200, 200, 200, 200, 200, 200, 200, 150]
        );
        assert_eq!(clocks::values(HB), repeated(200, 10));
        assert_eq!(clocks::values(CB), [200, 200]);
        assert_eq!(clocks::values(CN), [200, 200]);
    });
}

#[test]
fn d06_capture_provider_real_pre200_post150_preserves_its_original_cause() {
    fixture(|h, current, view| {
        clocks::all(200);
        clocks::script(CB, &[200, 150], 150);
        let attempt = current
            .baseline_build
            .capture_current(current.baseline_authority, view);
        assert!(matches!(
            attempt.current(),
            Err(SnapshotCaptureError::ClockRollback)
        ));
        let actual = attempt
            .capture()
            .expect("inventory actually completed before post-authority refusal");
        assert_eq!(actual.authority_before().checked_epoch, 200);
        assert_eq!(actual.authority_after(), None);
        assert_eq!(clocks::values(CB), [200, 150]);
        assert_eq!(h.baseline.checked_epoch(), 100);
    });
    fixture(|h, current, view| {
        clocks::all(200);
        clocks::script(CB, &[200, 150], 150);
        let root = g::ArtifactPairRoot::given(h.root_input(), view).unwrap();
        let rejected = g::measure_artifact_pair(&root, h.pair(), current, view).err();
        assert!(matches!(
            rejected,
            Some(g::Error::BuildCapture(SnapshotCaptureError::ClockRollback))
        ));
        assert_eq!(clocks::values(CB), [200, 150]);
        assert!(clocks::values(CN).is_empty());
        assert_eq!(clocks::values(DB), repeated(200, 8));
        assert_eq!(clocks::values(HB), repeated(200, 9));
    });
}

#[test]
fn d07_same_local_test_domain_capture100_cannot_pass_after_execution200() {
    use lc631_runtime::goal_clock_domain as registered;
    // The two actual actors are registered into the same domain before capture
    // and native birth. Preserve D07's 200→100 rejection and the four real outputs.
    tlcx31_registered_clock_support::full(|_, root, pair, ports, clocks, _, _, _, view| {
        clocks.all(200);
        clocks.bc.set(100);
        clocks.cc.set(100);
        let before = (clocks.be.calls(), clocks.bc.calls(), clocks.cc.calls());
        let rejected = registered::measure_clock_bound(root, pair, ports, view).err();
        assert!(
            matches!(
                rejected,
                Some(registered::Error::Clock(
                    registered::ClockFault::ClockRollback
                ))
            ),
            "registered capture100 must refuse after actual execution200: {rejected:?}"
        );
        assert!(clocks.be.calls() > before.0);
        assert_eq!(clocks.bc.calls(), before.1 + 1);
        assert_eq!(clocks.cc.calls(), before.2);
        assert_eq!(pair.baseline.actual_outputs(), &BASE);
        assert_eq!(pair.candidate.actual_outputs(), &HAND);
        assert_eq!(pair.baseline.checked_epoch(), 100);
    });
}

// Independent full successful-check order: outer-before2samplers; baseline H2;
// candidate H2; actual baseline/candidate source captures; outer-after2samplers.
// Each H2 has3samplers +2physical(pre/post) +1final hidden check.
const CHECK_ORDER: [Lane; 44] = [
    DB, HB, DB, DN, HN, DN, DB, HB, DB, HB, HB, DB, HB, DB, HB, HB, DB, HB, DB, HB, DN, HN, DN, HN,
    HN, DN, HN, DN, HN, HN, DN, HN, DN, HN, CB, CB, CN, CN, DB, HB, DB, DN, HN, DN,
];
fn expected_two_checks() -> Vec<Event> {
    CHECK_ORDER
        .into_iter()
        .chain(CHECK_ORDER)
        .map(|lane| Event { lane, epoch: 200 })
        .collect()
}
#[test]
fn p01_actual_dynamic200_control_pins_full_measure_and_scoped_read_trace() {
    fixture(|h, current, view| {
        clocks::all(200);
        let root = g::ArtifactPairRoot::given(h.root_input(), view).unwrap();
        let owner = g::measure_artifact_pair(&root, h.pair(), current, view).unwrap();
        assert_eq!(clocks::trace(), expected_two_checks());
        for lane in [DB, HB, DN, HN] {
            assert_eq!(clocks::values(lane), repeated(200, 20));
        }
        assert_eq!(clocks::values(CB), repeated(200, 4));
        assert_eq!(clocks::values(CN), repeated(200, 4));
        clocks::clear_trace();
        owner
            .with_current(&root, h.pair(), current, view, |v| {
                assert_eq!(v.baseline_outputs(), &BASE);
                assert_eq!(v.candidate_outputs(), &HAND);
                assert_eq!(v.requested_source_goal(), None);
                assert_eq!(v.source_eligibility(), g::SourceEligibility::Pending);
            })
            .unwrap();
        assert_eq!(clocks::trace(), expected_two_checks());
        assert_eq!(
            (h.baseline.checked_epoch(), h.candidate.checked_epoch()),
            (100, 100)
        );
    });
}
