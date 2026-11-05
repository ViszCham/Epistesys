//! Real finite AST -> owned K1 -> explicit reduction -> reported selection cuts.
#[path = "tlcx23_formal_support/mod.rs"]
mod support;
use lc631_core::measurement_numeric::RationalInterval as Interval;
use lc631_core::CandidateId;
use lc631_runtime::formal_measurement::{self as m, owned};
use lc631_runtime::joint_selection::{formal::*, *};
use lc631_substrate::resource::{Cancellation, ResourceError};
use support::*;

#[test]
fn tlcx38_direction_swap_cut_restoration_and_nonuniform_reduction_hand_oracle() {
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
        // Γs=Top,φs=p; Γt=not-p,φt=p. Soundness is preserved; completeness fails.
        assert_eq!(
            sound.historical().bounds(),
            &Interval::exact_ratio(0, 1).unwrap()
        );
        assert_eq!(
            complete.historical().bounds(),
            &Interval::exact_ratio(1, 1).unwrap()
        );
        let one = Interval::exact_ratio(1, 1).unwrap();
        let three = Interval::exact_ratio(3, 1).unwrap();
        let reduction = policy(pins(c.plan), one.lower(), three.lower(), &view);
        let swapped = reduce_formal_owned(&reduction, &complete, &sound, c.plan, c.bad, &view);
        assert!(matches!(
            swapped,
            Err(FormalAxisError::Owned(owned::OwnedError::BindingMismatch))
        ));
        let restored =
            reduce_formal_owned(&reduction, &sound, &complete, c.plan, c.bad, &view).unwrap();
        assert_eq!(
            restored.historical_bounds(),
            &Interval::exact_ratio(3, 4).unwrap()
        );
        let missing = owned::measure_owned(
            c.plan,
            c.bad,
            m::Direction::Completeness,
            &[],
            work(),
            &view,
        )
        .unwrap();
        let cut = reduce_formal_owned(&reduction, &sound, &missing, c.plan, c.bad, &view).unwrap();
        assert_eq!(
            cut.historical_bounds(),
            &Interval::from_ratio(0, 1, 3, 4).unwrap()
        );
        assert_ne!(cut.historical_bounds(), restored.historical_bounds());
    });
}

#[test]
fn tlcx38_copied_numeric_axis_is_not_the_current_owned_axis() {
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
        let reduction = policy(pins(c.plan), one.lower(), one.lower(), &view);
        let axis =
            reduce_formal_owned(&reduction, &sound, &complete, c.plan, c.bad, &view).unwrap();
        let (kind, live) = axis
            .project_current(&reduction, reduction.pins(), c.plan, c.bad, &view)
            .unwrap();
        let copied = axis.historical_bounds().clone();
        let kinds = [AxisKind::Goal(d(90)), kind];
        let values = [AxisValue::Exact(&one), live];
        let selection = SelectionPolicy {
            pins: reduction.pins(),
            axes: &kinds,
            representative_order: None,
            declared_goal_solve_overlap: false,
            max_candidates: 2,
            max_canonical_bytes: 4096,
        };
        let measurement_binding = d(73); // Explicit Given ledger/measurement pin, not candidate ID.
        let mut row = JointCandidate {
            id: CandidateId(5),
            context: c.bad.context(),
            pins: reduction.pins(),
            hard_gate: ReportedGate::Pass,
            map_back: ReportedGate::Pass,
            axes: &values,
            total_loss: AxisValue::Exact(&copied),
            canonical_key: b"given/bad/theory",
            measurement_binding,
        };
        axis.check_selection_row_current(
            &reduction,
            &selection,
            &row,
            1,
            measurement_binding,
            c.plan,
            c.bad,
            &view,
        )
        .unwrap();
        assert!(matches!(
            axis.check_selection_row_current(
                &reduction,
                &selection,
                &row,
                1,
                d(74),
                c.plan,
                c.bad,
                &view
            ),
            Err(FormalAxisError::BindingMismatch)
        ));
        let detached = [AxisValue::Exact(&one), AxisValue::Exact(&copied)];
        row.axes = &detached;
        assert!(matches!(
            axis.check_selection_row_current(
                &reduction,
                &selection,
                &row,
                1,
                measurement_binding,
                c.plan,
                c.bad,
                &view
            ),
            Err(FormalAxisError::BindingMismatch)
        ));
        row.axes = &values;
        axis.check_selection_row_current(
            &reduction,
            &selection,
            &row,
            1,
            measurement_binding,
            c.plan,
            c.bad,
            &view,
        )
        .unwrap();
    });
}

#[test]
fn tlcx38_actual_formal_loss_changes_reported_frontier_and_missing_gate_is_visible() {
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
        let zero = Interval::exact_ratio(0, 1).unwrap();
        let reduction = policy(pins(c.plan), one.lower(), one.lower(), &view);
        let good = reduce_formal_owned(&reduction, &gs, &gc, c.plan, c.good, &view).unwrap();
        let bad = reduce_formal_owned(&reduction, &bs, &bc, c.plan, c.bad, &view).unwrap();
        let (_, gv) = good
            .project_current(&reduction, reduction.pins(), c.plan, c.good, &view)
            .unwrap();
        let (_, bv) = bad
            .project_current(&reduction, reduction.pins(), c.plan, c.bad, &view)
            .unwrap();
        let kinds = [AxisKind::Goal(d(90)), AxisKind::SemanticLoss];
        let ga = [AxisValue::Exact(&one), gv];
        let ba = [AxisValue::Exact(&one), bv];
        let selection = SelectionPolicy {
            pins: reduction.pins(),
            axes: &kinds,
            representative_order: None,
            declared_goal_solve_overlap: false,
            max_candidates: 2,
            max_canonical_bytes: 4096,
        };
        let mut rows = [
            JointCandidate {
                id: CandidateId(4),
                context: c.good.context(),
                pins: reduction.pins(),
                hard_gate: ReportedGate::Pass,
                map_back: ReportedGate::Pass,
                axes: &ga,
                total_loss: AxisValue::Exact(&zero),
                canonical_key: b"good",
                measurement_binding: c.good.id(),
            },
            JointCandidate {
                id: CandidateId(5),
                context: c.bad.context(),
                pins: reduction.pins(),
                hard_gate: ReportedGate::Pass,
                map_back: ReportedGate::Pass,
                axes: &ba,
                total_loss: AxisValue::Exact(&zero),
                canonical_key: b"bad",
                measurement_binding: c.bad.id(),
            },
        ];
        let actual =
            select_joint_reported(&selection, &rows, SearchCoverage::EvaluatedSet).unwrap();
        assert_eq!(actual.recommendation, Some(0));
        assert_eq!(actual.dispositions[1], Disposition::Dominated { by: 0 });
        rows[0].hard_gate = ReportedGate::NeedsEvidence;
        let cut = select_joint_reported(&selection, &rows, SearchCoverage::EvaluatedSet).unwrap();
        assert_eq!(cut.recommendation, Some(1));
        assert_eq!(cut.dispositions[0], Disposition::NeedsEvidence);
        rows[0].hard_gate = ReportedGate::Pass;
        assert_eq!(
            select_joint_reported(&selection, &rows, SearchCoverage::EvaluatedSet)
                .unwrap()
                .recommendation,
            Some(0)
        );
    });
}

#[test]
fn tlcx38_cancelled_owned_projection_fails_while_historical_numbers_remain() {
    cases(|c| {
        let cancel = Cancellation::default();
        let (_b, _run, reservation) = setup(&c, cancel.clone());
        let view = reservation.lease();
        let sound = owned::measure_owned(
            c.plan,
            c.good,
            m::Direction::Soundness,
            c.good_obs,
            work(),
            &view,
        )
        .unwrap();
        let complete = owned::measure_owned(
            c.plan,
            c.good,
            m::Direction::Completeness,
            c.good_obs,
            work(),
            &view,
        )
        .unwrap();
        let one = Interval::exact_ratio(1, 1).unwrap();
        let reduction = policy(pins(c.plan), one.lower(), one.lower(), &view);
        let axis =
            reduce_formal_owned(&reduction, &sound, &complete, c.plan, c.good, &view).unwrap();
        cancel.cancel();
        assert!(matches!(
            axis.project_current(&reduction, reduction.pins(), c.plan, c.good, &view),
            Err(FormalAxisError::Resource(ResourceError::Cancelled))
        ));
        assert_eq!(
            axis.historical_bounds(),
            &Interval::exact_ratio(0, 1).unwrap()
        );
    });
}
