//! 日本語: CR-BRIDGEの先行試験。成功stubなし。
//! English: Test-first CR-BRIDGE; no success stub.
//! Missing API is planned API RED, not semantic RED.
mod tlcx22_bridge_actual_support;
use lc631_analysis::creativity_measurement::{
    ComponentMetric, IndependenceStatus, InputBasis, SemanticValidation,
};
use lc631_runtime::{bridge_gain_measurement as b, compatibility_measurement as c};
use lc631_solvers::typed_query::{IntSymbolId, SmtEmitLimits, SmtStatusQuery};
use lc631_substrate::resource::{ResourceError, ResourceKind};
use tlcx22_bridge_actual_support::*;

#[test]
fn tlcx22_bridge_actual_pair_half_gain_uses_eight_fresh_owners_not_parent_reports() {
    fixture(NORMAL, 2 * 1024 * 1024, |fx, cases, view, _| {
        let base = fx.forward(true);
        let bi = Fixture::inverse(&base);
        let cand = fx.forward(false);
        let ci = Fixture::inverse(&cand);
        let root =
            b::BridgeRoot::given(root_input(fx.input(cases), spec(&base, &bi), 48), view).unwrap();
        let bp = root.baseline_program(view).unwrap();
        let cp = root.candidate_program(spec(&cand, &ci), view).unwrap();
        let bb = root.prepare(&bp, view).unwrap();
        let cb = root.prepare(&cp, view).unwrap();
        assert_eq!(root.work_used_current(view).unwrap(), 32);
        for branch in [&bb, &cb] {
            assert_eq!(
                bounds(&branch.parent_report_current(view).unwrap().state),
                &unknown()
            );
            assert_eq!(branch.hard_current(view).unwrap(), c::HardState::Pass);
            let trials: &[c::Trial<'_>] = branch.trials_current(view).unwrap();
            assert_eq!(trials.len(), 4);
            assert!(trials.iter().all(|t| t.type_check() == c::Check::Pass
                && t.roundtrip_check() == c::Check::Pass
                && t.use_check() == c::Check::Unknown));
        }
        let bu = bb
            .execute_uses(view)
            .unwrap_or_else(|_| panic!("baseline fresh use failed"));
        let cu = cb
            .execute_uses(view)
            .unwrap_or_else(|_| panic!("candidate fresh use failed"));
        assert_eq!(root.work_used_current(view).unwrap(), 48);
        let mut actual_stages = 0;
        // The table is literal; no production helper computes expected success.
        for (case, (p, q, baseline_u, candidate_u)) in HAND.into_iter().enumerate() {
            assert_eq!((p, q), (fx.values[case][1].value, fx.values[case][2].value));
            for (batch, expected_u) in [(&bu, baseline_u), (&cu, candidate_u)] {
                let native: &c::OwnedUse<'_, '_, '_, '_, '_> =
                    batch.use_owner_current(case, view).unwrap();
                assert_eq!(
                    native.outcome_current(view).unwrap(),
                    c::UseOutcome::Executed {
                        case,
                        source_value: p,
                        target_value: expected_u,
                    }
                );
                actual_stages += native.actual_use_count();
                let (sm, tm) = native.models_current(view).unwrap();
                assert!(std::ptr::eq(sm.query(), &fx.source_theory));
                assert!(std::ptr::eq(tm.query(), &fx.target_theory));
                assert_eq!(sm.boolean(fx.source_symbols[0]), Some(true));
                assert_eq!(tm.boolean(fx.target_symbols[0]), Some(true));
                assert_eq!(sm.boolean(fx.source_symbols[1]), Some(p));
                assert_eq!(tm.boolean(fx.target_symbols[1]), Some(expected_u));
                if case == 0 {
                    let false_success =
                        lc631_analysis::creativity_measurement::RationalInterval::from_ratio(
                            1, 4, 1, 1,
                        )
                        .unwrap();
                    assert_eq!(
                        bounds(&native.report_current(view).unwrap().state),
                        &false_success
                    );
                }
            }
        }
        assert_eq!(actual_stages, 16);
        let gain = b::measure_pair(&root, &bu, &cu, view).unwrap();
        let report = gain.report_current(&root, &cp, view).unwrap();
        assert_eq!(report.metric, ComponentMetric::Bridge);
        assert_eq!(bounds(&report.state), &ratio(1, 2));
        assert_eq!(report.basis, InputBasis::Given);
        assert_eq!(report.validation, SemanticValidation::Unassessed);
        assert_eq!(report.independence, IndependenceStatus::SharedDependency);
        // Parent reports are immutable unknowns, so report subtraction cannot yield this exact half.
        for branch in [&bb, &cb] {
            assert_eq!(
                bounds(&branch.parent_report_current(view).unwrap().state),
                &unknown()
            );
        }
        assert_eq!(root.work_used_current(view).unwrap(), 48);
    });
}

#[test]
fn tlcx22_bridge_signed_direction_alias_and_explicit_role_rename_invariance() {
    assert_eq!(measured(NORMAL, true, false), ratio(1, 2));
    assert_eq!(measured(NORMAL, false, true), ratio(-1, 2));
    assert_eq!(measured(NORMAL, true, true), ratio(0, 1));
    assert_eq!(measured(NORMAL, false, false), ratio(0, 1));
    // Explicit source/target role bijection, not opaque ID/order distance.
    assert_eq!(measured(RENAMED, true, false), ratio(1, 2));
    assert_eq!(measured(RENAMED, false, true), ratio(-1, 2));
}

#[test]
fn tlcx22_bridge_edge_alias_duplicate_registration_cannot_increase_usable_queries() {
    fixture(NORMAL, 2 * 1024 * 1024, |fx, cases, view, _| {
        let map = fx.forward(true);
        let inv = Fixture::inverse(&map);
        let root =
            b::BridgeRoot::given(root_input(fx.input(cases), spec(&map, &inv), 48), view).unwrap();
        let bp = root.baseline_program(view).unwrap();
        let alias = root.candidate_program(spec(&map, &inv), view).unwrap();
        // More registered program occurrences, no new executable semantic map.
        let _extra_alias = root.candidate_program(spec(&map, &inv), view).unwrap();
        let bb = root.prepare(&bp, view).unwrap();
        let cb = root.prepare(&alias, view).unwrap();
        let bu = bb
            .execute_uses(view)
            .unwrap_or_else(|_| panic!("base use failed"));
        let cu = cb
            .execute_uses(view)
            .unwrap_or_else(|_| panic!("alias use failed"));
        let gain = b::measure_pair(&root, &bu, &cu, view).unwrap();
        assert_eq!(
            bounds(&gain.report_current(&root, &alias, view).unwrap().state),
            &ratio(0, 1)
        );
        for case in [1, 2] {
            assert_eq!(
                cu.use_owner_current(case, view)
                    .unwrap()
                    .outcome_current(view)
                    .unwrap(),
                c::UseOutcome::Executed {
                    case,
                    source_value: HAND[case].0,
                    target_value: HAND[case].2
                }
            );
        }
        // Multiplicity is not a fourth slot or a larger query denominator.
        let repeated = [map[0], map[1], map[2], map[2]];
        assert!(matches!(
            root.candidate_program(spec(&repeated, &inv), view),
            Err(b::BridgeError::Compatibility(c::Error::InvalidInput))
        ));
    });
}

#[test]
fn tlcx22_bridge_root_freezes_oracle_goal_queries_and_complete_case_universe() {
    fixture(NORMAL, 2 * 1024 * 1024, |fx, cases, view, _| {
        let map = fx.forward(true);
        let inv = Fixture::inverse(&map);
        let input = fx.input(cases);
        for work in [0, 49, u64::MAX] {
            assert!(matches!(
                b::BridgeRoot::given(root_input(input, spec(&map, &inv), work), view),
                Err(b::BridgeError::InvalidLimits)
            ));
        }
        let mut altered = input;
        altered.pins.definition = d(99);
        assert!(matches!(
            b::BridgeRoot::given(root_input(altered, spec(&map, &inv), 48), view),
            Err(b::BridgeError::OracleMismatch)
        ));
        let mut altered = input;
        altered.pins.oracle = d(99);
        assert!(matches!(
            b::BridgeRoot::given(root_input(altered, spec(&map, &inv), 48), view),
            Err(b::BridgeError::OracleMismatch)
        ));
        let mut altered = input;
        altered.root_goal = &fx.source_use;
        assert!(matches!(
            b::BridgeRoot::given(root_input(altered, spec(&map, &inv), 48), view),
            Err(b::BridgeError::Compatibility(c::Error::InvalidInput))
        ));
        let mut altered = input;
        altered.target_use = &fx.target_theory;
        assert!(matches!(
            b::BridgeRoot::given(root_input(altered, spec(&map, &inv), 48), view),
            Err(b::BridgeError::Compatibility(c::Error::InvalidInput))
        ));
        for weight in [0, 2] {
            let mut weighted = fx.cases();
            weighted[0].weight = weight;
            let mut altered = input;
            altered.cases = &weighted;
            assert!(matches!(
                b::BridgeRoot::given(root_input(altered, spec(&map, &inv), 48), view),
                Err(b::BridgeError::Compatibility(c::Error::InvalidInput))
            ));
        }
        let duplicate = [cases[0], cases[1], cases[2], cases[0]];
        for invalid in [&cases[..3], &duplicate[..], &[cases[0]; 5][..]] {
            let mut altered = input;
            altered.cases = invalid;
            assert!(matches!(
                b::BridgeRoot::given(root_input(altered, spec(&map, &inv), 48), view),
                Err(b::BridgeError::Compatibility(c::Error::InvalidInput))
            ));
        }
        let integer = SmtStatusQuery::checked(
            fx.source_symbols.to_vec(),
            vec![IntSymbolId::new(0)],
            fx.source_theory.assertions().to_vec(),
            SmtEmitLimits::default(),
        )
        .unwrap();
        let mut unsupported = input;
        unsupported.source_theory = &integer;
        assert!(matches!(
            b::BridgeRoot::given(root_input(unsupported, spec(&map, &inv), 48), view),
            Err(b::BridgeError::Compatibility(c::Error::Unsupported(
                c::Unsupported::Integer
            )))
        ));
    });
}

#[test]
fn tlcx22_bridge_current_root_program_role_and_reservation_cannot_be_rebound() {
    fixture(NORMAL, 2 * 1024 * 1024, |fx, cases, view, _| {
        let map = fx.forward(true);
        let inv = Fixture::inverse(&map);
        let good = fx.forward(false);
        let gi = Fixture::inverse(&good);
        let root =
            b::BridgeRoot::given(root_input(fx.input(cases), spec(&map, &inv), 48), view).unwrap();
        let other =
            b::BridgeRoot::given(root_input(fx.input(cases), spec(&map, &inv), 48), view).unwrap();
        let bp = root.baseline_program(view).unwrap();
        let cp = root.candidate_program(spec(&good, &gi), view).unwrap();
        let alias = root.candidate_program(spec(&good, &gi), view).unwrap();
        let foreign_program = other.candidate_program(spec(&good, &gi), view).unwrap();
        assert!(matches!(
            root.prepare(&foreign_program, view),
            Err(b::BridgeError::BindingMismatch)
        ));
        let bb = root.prepare(&bp, view).unwrap();
        let cb = root.prepare(&cp, view).unwrap();
        let bu = bb
            .execute_uses(view)
            .unwrap_or_else(|_| panic!("base use failed"));
        let cu = cb
            .execute_uses(view)
            .unwrap_or_else(|_| panic!("candidate use failed"));
        assert!(matches!(
            b::measure_pair(&root, &cu, &bu, view),
            Err(b::BridgeError::RoleMismatch)
        ));
        assert!(matches!(
            b::measure_pair(&other, &bu, &cu, view),
            Err(b::BridgeError::BindingMismatch)
        ));
        let gain = b::measure_pair(&root, &bu, &cu, view).unwrap();
        assert!(matches!(
            gain.report_current(&root, &alias, view),
            Err(b::BridgeError::BindingMismatch)
        ));
        fixture(NORMAL, 2 * 1024 * 1024, |_, _, foreign_view, _| {
            assert!(matches!(
                gain.report_current(&root, &cp, foreign_view),
                Err(b::BridgeError::ForeignReservation)
            ));
            assert!(matches!(
                cu.use_owner_current(0, foreign_view),
                Err(b::BridgeError::ForeignReservation)
            ));
        });
    });
}

#[test]
fn tlcx22_bridge_cancelled_current_owner_does_not_restore_cached_scalar() {
    fixture(NORMAL, 2 * 1024 * 1024, |fx, cases, view, cancel| {
        let map = fx.forward(true);
        let inv = Fixture::inverse(&map);
        let good = fx.forward(false);
        let gi = Fixture::inverse(&good);
        let root =
            b::BridgeRoot::given(root_input(fx.input(cases), spec(&map, &inv), 48), view).unwrap();
        let bp = root.baseline_program(view).unwrap();
        let cp = root.candidate_program(spec(&good, &gi), view).unwrap();
        let bb = root.prepare(&bp, view).unwrap();
        let cb = root.prepare(&cp, view).unwrap();
        let bu = bb
            .execute_uses(view)
            .unwrap_or_else(|_| panic!("base use failed"));
        let cu = cb
            .execute_uses(view)
            .unwrap_or_else(|_| panic!("candidate use failed"));
        let gain = b::measure_pair(&root, &bu, &cu, view).unwrap();
        assert_eq!(
            bounds(&gain.report_current(&root, &cp, view).unwrap().state),
            &ratio(1, 2)
        );
        cancel.cancel();
        assert!(matches!(
            gain.report_current(&root, &cp, view),
            Err(b::BridgeError::Resource(ResourceError::Cancelled))
        ));
        assert!(matches!(
            cu.use_owner_current(0, view),
            Err(b::BridgeError::Resource(ResourceError::Cancelled))
        ));
        assert!(matches!(
            b::measure_pair(&root, &bu, &cu, view),
            Err(b::BridgeError::Resource(ResourceError::Cancelled))
        ));
    });
}

#[test]
fn tlcx22_bridge_hard_failure_is_not_a_soft_zero_or_positive_gain() {
    fixture(NORMAL, 2 * 1024 * 1024, |fx, cases, view, _| {
        let map = fx.forward(true);
        let inv = Fixture::inverse(&map);
        let mut broken = fx.forward(false);
        broken[0].target = fx.target_symbols[1];
        broken[1].target = fx.target_symbols[0];
        let inverse = Fixture::inverse(&broken);
        let root =
            b::BridgeRoot::given(root_input(fx.input(cases), spec(&map, &inv), 48), view).unwrap();
        let cp = root
            .candidate_program(spec(&broken, &inverse), view)
            .unwrap();
        let cb = root.prepare(&cp, view).unwrap();
        assert_eq!(cb.hard_current(view).unwrap(), c::HardState::Fail);
        let failure = cb
            .execute_uses(view)
            .err()
            .expect("hard failure cannot make a complete batch");
        assert!(matches!(
            failure.cause(),
            b::BridgeError::HardInvariantFailed
        ));
        assert_eq!(failure.completed_use_count(), 0);
    });
}

#[test]
fn tlcx22_bridge_partial_47_slot_cut_retains_three_uses_no_scalar_and_no_refund() {
    fixture(NORMAL, 2 * 1024 * 1024, |fx, cases, view, _| {
        let map = fx.forward(true);
        let inv = Fixture::inverse(&map);
        let good = fx.forward(false);
        let gi = Fixture::inverse(&good);
        let root =
            b::BridgeRoot::given(root_input(fx.input(cases), spec(&map, &inv), 47), view).unwrap();
        let bp = root.baseline_program(view).unwrap();
        let cp = root.candidate_program(spec(&good, &gi), view).unwrap();
        let bb = root.prepare(&bp, view).unwrap();
        let cb = root.prepare(&cp, view).unwrap();
        let _bu = bb
            .execute_uses(view)
            .unwrap_or_else(|_| panic!("baseline fits 40 slots"));
        let failure = cb
            .execute_uses(view)
            .err()
            .expect("47 slots cannot complete the 48-slot pair");
        assert!(matches!(failure.cause(), b::BridgeError::WorkBudget));
        assert_eq!(failure.completed_use_count(), 3);
        assert_eq!(failure.scalar(), None);
        assert_eq!(root.work_used_current(view).unwrap(), 46);
        for case in 0..3 {
            let native: &c::OwnedUse<'_, '_, '_, '_, '_> =
                failure.use_owner_current(case, view).unwrap();
            assert_eq!(native.actual_use_count(), 2);
            assert!(native.models_current(view).is_ok());
        }
        drop(failure);
        assert_eq!(root.work_used_current(view).unwrap(), 46);
        let retry = cb
            .execute_uses(view)
            .err()
            .expect("cut work is never refunded");
        assert!(matches!(retry.cause(), b::BridgeError::WorkBudget));
        assert_eq!(retry.completed_use_count(), 0);
        assert_eq!(retry.scalar(), None);
    });
}

#[test]
fn tlcx22_bridge_missing_preparation_and_resource_cut_are_not_measured_zero() {
    fixture(NORMAL, 2 * 1024 * 1024, |fx, cases, view, _| {
        let map = fx.forward(true);
        let inv = Fixture::inverse(&map);
        let root =
            b::BridgeRoot::given(root_input(fx.input(cases), spec(&map, &inv), 15), view).unwrap();
        let bp = root.baseline_program(view).unwrap();
        let bb = root.prepare(&bp, view).unwrap();
        assert_eq!(
            bounds(&bb.parent_report_current(view).unwrap().state),
            &unknown()
        );
        let failure = bb
            .execute_uses(view)
            .err()
            .expect("missing guard/model cannot be used");
        assert!(matches!(
            failure.cause(),
            b::BridgeError::MissingPreparation
        ));
        assert_eq!(failure.completed_use_count(), 0);
        assert_eq!(failure.scalar(), None);
        assert_eq!(root.work_used_current(view).unwrap(), 15);
    });
    fixture(NORMAL, 1, |fx, cases, view, _| {
        let map = fx.forward(true);
        let inv = Fixture::inverse(&map);
        let root =
            b::BridgeRoot::given(root_input(fx.input(cases), spec(&map, &inv), 48), view).unwrap();
        let bp = root.baseline_program(view).unwrap();
        assert!(matches!(
            root.prepare(&bp, view),
            Err(b::BridgeError::Resource(ResourceError::Exhausted(
                ResourceKind::Output
            )))
        ));
        assert_eq!(root.work_used_current(view).unwrap(), 0);
    });
}
