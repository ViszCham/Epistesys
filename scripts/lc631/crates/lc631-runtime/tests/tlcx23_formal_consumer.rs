//! Actual finite AST/owned K1 -> bidirectional axis -> selection/cut/restoration.
#[path = "tlcx23_formal_support/mod.rs"]
mod support;
use lc631_core::measurement_numeric::{BigInt, Rational, RationalInterval as Interval};
use lc631_core::CandidateId;
use lc631_runtime::formal_measurement::owned;
use lc631_runtime::joint_selection::formal::BothDirectionReduction as Reduction;
use lc631_runtime::joint_selection::formal::*;
use lc631_runtime::joint_selection::*;
use lc631_runtime::{formal_boolean as f, formal_measurement as m};
use lc631_substrate::resource::Cancellation;
use support::*;

#[test]
fn tlcx23_formal_owned_k1_changes_frontier() {
    cases(|c| {
        let (_b, _run, reservation) = setup(&c, Cancellation::default());
        let view = reservation.lease();
        let gs = owned::measure_owned(
            c.plan,
            c.good,
            m::Direction::Soundness,
            c.good_obs,
            work(),
            &view,
        )
        .unwrap();
        let gc = owned::measure_owned(
            c.plan,
            c.good,
            m::Direction::Completeness,
            c.good_obs,
            work(),
            &view,
        )
        .unwrap();
        let bs = owned::measure_owned(
            c.plan,
            c.bad,
            m::Direction::Soundness,
            c.bad_obs,
            work(),
            &view,
        )
        .unwrap();
        let bc = owned::measure_owned(
            c.plan,
            c.bad,
            m::Direction::Completeness,
            c.bad_obs,
            work(),
            &view,
        )
        .unwrap();
        let one = Interval::exact_ratio(1, 1).unwrap();
        let policy = policy(pins(c.plan), one.lower(), one.lower(), &view);
        let p = policy.pins();
        let good = reduce_formal_owned(&policy, &gs, &gc, c.plan, c.good, &view).unwrap();
        let bad = reduce_formal_owned(&policy, &bs, &bc, c.plan, c.bad, &view).unwrap();
        assert_eq!(
            good.historical_bounds(),
            &Interval::exact_ratio(0, 1).unwrap()
        );
        assert_eq!(
            bad.historical_bounds(),
            &Interval::exact_ratio(1, 2).unwrap()
        );
        assert_eq!(bad.mapping_basis(), f::MappingBasis::Given);
        assert_eq!(
            bad.source_interpretation(),
            f::SourceInterpretation::Unverified
        );
        assert_eq!(
            bad.checker_independence(),
            f::CheckerIndependence::UnassessedSharedIr
        );
        let (semantic, gv) = good
            .project_current(&policy, p, c.plan, c.good, &view)
            .unwrap();
        let (_, bv) = bad
            .project_current(&policy, p, c.plan, c.bad, &view)
            .unwrap();
        let axes = [AxisKind::Goal(d(40)), semantic];
        let ga = [AxisValue::Exact(&one), gv];
        let ba = [AxisValue::Exact(&one), bv];
        let zero = Interval::exact_ratio(0, 1).unwrap();
        let selection = SelectionPolicy {
            pins: p,
            axes: &axes,
            representative_order: None,
            declared_goal_solve_overlap: false,
            max_candidates: 2,
            max_canonical_bytes: 4096,
        };
        let rows = [
            JointCandidate {
                id: CandidateId(4),
                context: c.good.context(),
                pins: p,
                hard_gate: ReportedGate::Pass,
                map_back: ReportedGate::Pass,
                axes: &ga,
                total_loss: AxisValue::Exact(&zero),
                canonical_key: b"good/theory-top/given",
                measurement_binding: d(4),
            },
            JointCandidate {
                id: CandidateId(5),
                context: c.bad.context(),
                pins: p,
                hard_gate: ReportedGate::Pass,
                map_back: ReportedGate::Pass,
                axes: &ba,
                total_loss: AxisValue::Exact(&zero),
                canonical_key: b"bad/theory-not-p/given",
                measurement_binding: d(5),
            },
        ];
        let report =
            select_joint_reported(&selection, &rows, SearchCoverage::EvaluatedSet).unwrap();
        assert_eq!(report.recommendation, Some(0));
        assert_eq!(report.frontier, vec![0]);
        assert_eq!(report.dispositions[1], Disposition::Dominated { by: 0 });
        // Cut only the actual completeness observation; fixed scope/goal/weights
        // and the soundness producer remain unchanged.
        let absent = owned::measure_owned(
            c.plan,
            c.bad,
            m::Direction::Completeness,
            &[],
            work(),
            &view,
        )
        .unwrap();
        let cut = reduce_formal_owned(&policy, &bs, &absent, c.plan, c.bad, &view).unwrap();
        assert_eq!(
            cut.historical_bounds(),
            &Interval::from_ratio(0, 1, 1, 2).unwrap()
        );
        let (_, cut_value) = cut
            .project_current(&policy, p, c.plan, c.bad, &view)
            .unwrap();
        let cut_values = [AxisValue::Exact(&one), cut_value];
        let [good_row, bad_row] = rows;
        let cut_rows = [
            good_row,
            JointCandidate {
                axes: &cut_values,
                ..bad_row
            },
        ];
        let report =
            select_joint_reported(&selection, &cut_rows, SearchCoverage::EvaluatedSet).unwrap();
        assert_eq!(report.frontier, vec![0, 1]);
        let [good_row, bad_row] = cut_rows;
        let restored = [
            good_row,
            JointCandidate {
                axes: &ba,
                ..bad_row
            },
        ];
        let report =
            select_joint_reported(&selection, &restored, SearchCoverage::EvaluatedSet).unwrap();
        assert_eq!(report.frontier, vec![0]);
    });
}

#[test]
fn tlcx23_formal_missing_bounds_and_explicit_weights() {
    cases(|c| {
        let (_b, _run, reservation) = setup(&c, Cancellation::default());
        let view = reservation.lease();
        let sound = owned::measure_owned(
            c.plan,
            c.bad,
            m::Direction::Soundness,
            c.bad_obs,
            work(),
            &view,
        )
        .unwrap();
        let complete = owned::measure_owned(
            c.plan,
            c.bad,
            m::Direction::Completeness,
            c.bad_obs,
            work(),
            &view,
        )
        .unwrap();
        let one = Interval::exact_ratio(1, 1).unwrap();
        let three = Interval::exact_ratio(3, 1).unwrap();
        let policy = policy(pins(c.plan), one.lower(), three.lower(), &view);
        let p = policy.pins();
        let axis = reduce_formal_owned(&policy, &sound, &complete, c.plan, c.bad, &view).unwrap();
        assert_eq!(
            axis.historical_bounds(),
            &Interval::exact_ratio(3, 4).unwrap()
        );
        let (ms, mc) = missing_pair(&c, &view);
        let missing = reduce_formal_owned(&policy, &ms, &mc, c.plan, c.good, &view).unwrap();
        assert_eq!(
            missing.historical_bounds(),
            &Interval::from_ratio(0, 1, 1, 1).unwrap()
        );
        assert!(matches!(
            missing
                .project_current(&policy, p, c.plan, c.good, &view)
                .unwrap()
                .1,
            AxisValue::Bounded(_)
        ));
    });
}

#[test]
fn tlcx23_formal_invalid_weights_and_policy_binding() {
    cases(|c| {
        let (_b, _run, reservation) = setup(&c, Cancellation::default());
        let view = reservation.lease();
        let (s, k) = missing_pair(&c, &view);
        let one = Rational::from_integer(BigInt::from(1));
        let registered = policy(pins(c.plan), &one, &one, &view);
        let p = registered.pins();
        let bad = [
            Rational::from_integer(BigInt::from(0)),
            Rational::from_integer(BigInt::from(-1)),
            Rational::new_raw(BigInt::from(2), BigInt::from(2)),
            Rational::from_integer(BigInt::from(1) << 65_536usize),
        ];
        for weight in &bad {
            let policy = Reduction::declared_registered(p, p.policy, d(2), weight, &one);
            assert!(reduce_formal_owned(&policy, &s, &k, c.plan, c.good, &view).is_err());
        }
        let policy = Reduction::declared_registered(p, d(99), d(2), &one, &one);
        assert!(matches!(
            reduce_formal_owned(&policy, &s, &k, c.plan, c.good, &view),
            Err(FormalAxisError::PolicyMismatch)
        ));
        let wrong_definition = Reduction::declared_registered(p, p.policy, d(98), &one, &one);
        assert!(matches!(
            reduce_formal_owned(&wrong_definition, &s, &k, c.plan, c.good, &view),
            Err(FormalAxisError::PolicyMismatch)
        ));
        let changed = Rational::from_integer(BigInt::from(99));
        let collision = Reduction::declared_registered(p, p.policy, d(2), &one, &changed);
        assert!(matches!(
            reduce_formal_owned(&collision, &s, &k, c.plan, c.good, &view),
            Err(FormalAxisError::PolicyMismatch)
        ));
    });
}
