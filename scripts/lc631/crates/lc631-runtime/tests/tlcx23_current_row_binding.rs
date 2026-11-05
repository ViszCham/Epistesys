//! Actual owned formal projection must match the row consumed by selection.
#[path = "tlcx23_formal_support/mod.rs"]
mod support;
use lc631_core::measurement_numeric::RationalInterval as Interval;
use lc631_core::CandidateId;
use lc631_runtime::formal_measurement::{owned, Direction};
use lc631_runtime::joint_selection::formal::*;
use lc631_runtime::joint_selection::*;
use lc631_substrate::resource::{Cancellation, ResourceError};
use support::*;

fn selection(pins: SelectionPins, axes: &[AxisKind]) -> SelectionPolicy<'_> {
    SelectionPolicy {
        pins,
        axes,
        representative_order: None,
        declared_goal_solve_overlap: false,
        max_candidates: 2,
        max_canonical_bytes: 4096,
    }
}
fn row<'a>(
    id: u64,
    context: lc631_core::context::ContextRevision,
    pins: SelectionPins,
    values: &'a [AxisValue<'a>],
    zero: &'a Interval,
    binding: lc631_substrate::Digest32,
) -> JointCandidate<'a> {
    JointCandidate {
        id: CandidateId(id),
        context,
        pins,
        hard_gate: ReportedGate::Pass,
        map_back: ReportedGate::Pass,
        axes: values,
        total_loss: AxisValue::Exact(zero),
        canonical_key: if id == 1 { b"good" } else { b"bad" },
        measurement_binding: binding,
    }
}

#[test]
fn tlcx23_current_row_binding_consumes_real_axis_and_rejects_row_swaps() {
    cases(|c| {
        let cancel = Cancellation::default();
        let (_broker, _run, reservation) = setup(&c, cancel.clone());
        let view = reservation.lease();
        let gs = owned::measure_owned(
            c.plan,
            c.good,
            Direction::Soundness,
            c.good_obs,
            work(),
            &view,
        )
        .unwrap();
        let gc = owned::measure_owned(
            c.plan,
            c.good,
            Direction::Completeness,
            c.good_obs,
            work(),
            &view,
        )
        .unwrap();
        let bs = owned::measure_owned(
            c.plan,
            c.bad,
            Direction::Soundness,
            c.bad_obs,
            work(),
            &view,
        )
        .unwrap();
        let bc = owned::measure_owned(
            c.plan,
            c.bad,
            Direction::Completeness,
            c.bad_obs,
            work(),
            &view,
        )
        .unwrap();
        let one = Interval::exact_ratio(1, 1).unwrap();
        let zero = Interval::exact_ratio(0, 1).unwrap();
        let reduction = policy(pins(c.plan), one.lower(), one.lower(), &view);
        let pins = reduction.pins();
        let good = reduce_formal_owned(&reduction, &gs, &gc, c.plan, c.good, &view).unwrap();
        let bad = reduce_formal_owned(&reduction, &bs, &bc, c.plan, c.bad, &view).unwrap();
        let (_, good_value) = good
            .project_current(&reduction, pins, c.plan, c.good, &view)
            .unwrap();
        let (_, bad_value) = bad
            .project_current(&reduction, pins, c.plan, c.bad, &view)
            .unwrap();
        let axes = [AxisKind::Goal(d(40)), AxisKind::SemanticLoss];
        let selected = selection(pins, &axes);
        let good_values = [AxisValue::Exact(&one), good_value];
        let bad_values = [AxisValue::Exact(&one), bad_value];
        let mut rows = [
            row(1, c.good.context(), pins, &good_values, &zero, d(60)),
            row(2, c.bad.context(), pins, &bad_values, &zero, d(61)),
        ];
        good.check_selection_row_current(
            &reduction,
            &selected,
            &rows[0],
            1,
            d(60),
            c.plan,
            c.good,
            &view,
        )
        .unwrap();
        bad.check_selection_row_current(
            &reduction,
            &selected,
            &rows[1],
            1,
            d(61),
            c.plan,
            c.bad,
            &view,
        )
        .unwrap();
        let report = select_joint_reported(&selected, &rows, SearchCoverage::EvaluatedSet).unwrap();
        assert_eq!(report.frontier, vec![0]);
        assert_eq!(report.recommendation, Some(0));
        rows[1].context = c.good.context();
        assert!(matches!(
            bad.check_selection_row_current(
                &reduction,
                &selected,
                &rows[1],
                1,
                d(61),
                c.plan,
                c.bad,
                &view
            ),
            Err(FormalAxisError::BindingMismatch)
        ));
        rows[1].context = c.bad.context();
        rows[1].measurement_binding = d(60);
        assert!(matches!(
            bad.check_selection_row_current(
                &reduction,
                &selected,
                &rows[1],
                1,
                d(61),
                c.plan,
                c.bad,
                &view
            ),
            Err(FormalAxisError::BindingMismatch)
        ));
        rows[1].measurement_binding = d(61);
        assert!(matches!(
            bad.check_selection_row_current(
                &reduction,
                &selected,
                &rows[1],
                0,
                d(61),
                c.plan,
                c.bad,
                &view
            ),
            Err(FormalAxisError::BindingMismatch)
        ));
        assert!(matches!(
            bad.check_selection_row_current(
                &reduction,
                &selected,
                &rows[1],
                usize::MAX,
                d(61),
                c.plan,
                c.bad,
                &view
            ),
            Err(FormalAxisError::BindingMismatch)
        ));
        let copied = bad.historical_bounds().clone();
        let forged_values = [AxisValue::Exact(&one), AxisValue::Exact(&copied)];
        let forged = row(2, c.bad.context(), pins, &forged_values, &zero, d(61));
        assert_eq!(copied, *bad.historical_bounds());
        assert!(matches!(
            bad.check_selection_row_current(
                &reduction,
                &selected,
                &forged,
                1,
                d(61),
                c.plan,
                c.bad,
                &view
            ),
            Err(FormalAxisError::BindingMismatch)
        ));
        cancel.cancel();
        assert!(matches!(
            bad.check_selection_row_current(
                &reduction,
                &selected,
                &rows[1],
                1,
                d(61),
                c.plan,
                c.bad,
                &view
            ),
            Err(FormalAxisError::Resource(ResourceError::Cancelled))
        ));
    });
}

#[test]
fn tlcx23_current_row_binding_preserves_missing_bounds_and_exactness_tag() {
    cases(|c| {
        let (_broker, _run, reservation) = setup(&c, Cancellation::default());
        let view = reservation.lease();
        let (sound, complete) = missing_pair(&c, &view);
        let one = Interval::exact_ratio(1, 1).unwrap();
        let zero = Interval::exact_ratio(0, 1).unwrap();
        let reduction = policy(pins(c.plan), one.lower(), one.lower(), &view);
        let pins = reduction.pins();
        let axis =
            reduce_formal_owned(&reduction, &sound, &complete, c.plan, c.good, &view).unwrap();
        let (_, value) = axis
            .project_current(&reduction, pins, c.plan, c.good, &view)
            .unwrap();
        let axes = [AxisKind::Goal(d(40)), AxisKind::SemanticLoss];
        let selected = selection(pins, &axes);
        let values = [AxisValue::Exact(&one), value];
        let current = row(1, c.good.context(), pins, &values, &zero, d(60));
        axis.check_selection_row_current(
            &reduction,
            &selected,
            &current,
            1,
            d(60),
            c.plan,
            c.good,
            &view,
        )
        .unwrap();
        assert_eq!(
            axis.historical_bounds(),
            &Interval::from_ratio(0, 1, 1, 1).unwrap()
        );
        let relabeled = [
            AxisValue::Exact(&one),
            AxisValue::Estimated(axis.historical_bounds()),
        ];
        let wrong_tag = row(1, c.good.context(), pins, &relabeled, &zero, d(60));
        assert!(matches!(
            axis.check_selection_row_current(
                &reduction,
                &selected,
                &wrong_tag,
                1,
                d(60),
                c.plan,
                c.good,
                &view
            ),
            Err(FormalAxisError::BindingMismatch)
        ));
    });
}
