//! 日本語: 実owner由来の固定1:3 totalをcurrent選択へ。Given goal/gatesは別境界。
//! English: Real-owner fixed 1:3 totals in current selection; goals/gates stay Given.
mod tlcx19_checked_tl_support;
use lc631_core::CandidateId;
use lc631_runtime::{checked_tl_reduction as r, joint_selection as j};
use lc631_runtime::spatiotemporal_selection::{self as s, RowOwners, SpatialOwner, TemporalOwner};
use lc631_solvers::temporal::{EvaluationBudget, Truth};
use tlcx19_checked_tl_support::*;

#[derive(Clone, Copy, Eq, PartialEq)]
enum Case {
    Real,
    FlipActual,
    ReverseReportedTotals,
    DetachedEqualTotal,
    SwapReduction,
    EqualOtherRoot,
    EqualOtherOwner,
    MissingReduction,
    DuplicateRow,
    WrongBaseline,
    WrongVariant,
    CancelBefore,
    CancelAfter,
    ForeignAfter,
    Bounded,
    Unsupported,
}

fn total_axis(v: Option<&lc631_core::measurement_numeric::RationalInterval>) -> j::AxisValue<'_> {
    match v {
        Some(v) if v.is_exact() => j::AxisValue::Exact(v),
        Some(v) => j::AxisValue::Bounded(v),
        None => j::AxisValue::Unavailable(j::MissingAxis::Unsupported),
    }
}

fn exercise(case: Case) {
    let q = query();
    let good = trace(Truth::True);
    let changed = trace(if case == Case::Bounded || case == Case::WrongVariant {
        Truth::Unknown
    } else {
        Truth::False
    });
    let source = temporal(&q, &good);
    let other = temporal(&q, &changed);
    let original_space = spatial(false);
    let changed_space = spatial(true);
    let tp = time_policy(source, source);
    let unsupported = case == Case::Unsupported;
    let sp = space_policy(original_space, original_space, unsupported);
    let cancel = Cancellation::default();
    let (_broker, _run, reservation) = run(cancel.clone());
    let view = reservation.lease();
    let root = r::RootPlan::capture(source, original_space, &tp, &sp, &view).unwrap();
    let (at, asp, bt, bsp) = if case == Case::FlipActual {
        (source, changed_space, other, original_space)
    } else {
        (other, original_space, source, changed_space)
    };
    let ca = r::TupleCandidate::capture(&root, at, asp).unwrap();
    let cb = r::TupleCandidate::capture(&root, bt, bsp).unwrap();
    let atp = time_policy(source, at);
    let btp = time_policy(source, bt);
    let aspp = space_policy(original_space, asp, unsupported);
    let bspp = space_policy(original_space, bsp, unsupported);
    let ta = TemporalOwner::produce(&atp, source, at, &view, &view, EvaluationBudget::default()).unwrap();
    let tb = TemporalOwner::produce(&btp, source, bt, &view, &view, EvaluationBudget::default()).unwrap();
    let sa = SpatialOwner::produce(&aspp, original_space, asp, &view, &view).unwrap();
    let sb = SpatialOwner::produce(&bspp, original_space, bsp, &view, &view).unwrap();
    let ra = r::reduce_current(&root, &ca, r::Owners { temporal: Some(&ta), spatial: Some(&sa) }, &view).unwrap();
    let rb = r::reduce_current(&root, &cb, r::Owners { temporal: Some(&tb), spatial: Some(&sb) }, &view).unwrap();
    let total_a = ra.current(&root, &ca, &view).unwrap().total();
    let total_b = rb.current(&root, &cb, &view).unwrap().total();
    if !unsupported {
        let expected_a = if case == Case::FlipActual {
            bounds(3, 4, 3, 4)
        } else if case == Case::Bounded || case == Case::WrongVariant {
            bounds(0, 1, 1, 4)
        } else {
            bounds(1, 4, 1, 4)
        };
        assert_eq!(total_a, Some(&expected_a));
        assert_eq!(total_b, Some(&if case == Case::FlipActual { bounds(1, 4, 1, 4) } else { bounds(3, 4, 3, 4) }));
    } else {
        assert!(total_a.is_none() && total_b.is_none());
        assert_eq!(ra.historical().gaps()[1], Some(r::NoScalar::Unavailable(j::MissingAxis::Unsupported)));
    }
    // An equal-content fresh owner is not the exact owner retained by ra.
    let mirror = TemporalOwner::produce(&atp, source, at, &view, &view, EvaluationBudget::default()).unwrap();
    let row_ta = if case == Case::EqualOtherOwner { &mirror } else { &ta };
    let owners = [
        RowOwners { row: 0, temporal: Some(row_ta), spatial: Some(&sa) },
        RowOwners { row: 1, temporal: Some(&tb), spatial: Some(&sb) },
    ];
    let other_root = r::RootPlan::capture(source, original_space, &tp, &sp, &view).unwrap();
    assert_eq!(other_root.digest(), root.digest());
    let other_ca = r::TupleCandidate::capture(&other_root, at, asp).unwrap();
    let other_ra = r::reduce_current(&other_root, &other_ca, r::Owners { temporal: Some(&ta), spatial: Some(&sa) }, &view).unwrap();
    let mut reductions = [
        r::ReductionRow { row: 0, owner: &ra },
        r::ReductionRow { row: 1, owner: &rb },
    ];
    if case == Case::SwapReduction {
        reductions[0].owner = &rb;
        reductions[1].owner = &ra;
    }
    if case == Case::EqualOtherRoot { reductions[0].owner = &other_ra; }
    if case == Case::DuplicateRow { reductions[1].row = 0; }
    let mut pins = j::SelectionPins {
        source: context().source(), scope: scope(), baseline: root.digest(),
        hard_invariants: d(91), policy: root.digest(), goal_profile: Some(d(92)),
        evidence_revision: d(93),
    };
    if case == Case::WrongBaseline { pins.baseline = d(99); }
    let goal = bounds(1, 1, 1, 1);
    let axes_a = [j::AxisValue::Exact(&goal), row_ta.axis_current(&view).unwrap(), sa.axis_current(&view).unwrap()];
    let axes_b = [j::AxisValue::Exact(&goal), tb.axis_current(&view).unwrap(), sb.axis_current(&view).unwrap()];
    let detached = total_a.cloned();
    let mut rows = [
        j::JointCandidate { id: CandidateId(1), context: context(), pins,
            hard_gate: j::ReportedGate::Pass, map_back: j::ReportedGate::Pass,
            axes: &axes_a, total_loss: total_axis(total_a), canonical_key: b"z",
            measurement_binding: owners[0].binding_digest() },
        j::JointCandidate { id: CandidateId(2), context: context(), pins,
            hard_gate: j::ReportedGate::Pass, map_back: j::ReportedGate::Pass,
            axes: &axes_b, total_loss: total_axis(total_b), canonical_key: b"a",
            measurement_binding: owners[1].binding_digest() },
    ];
    if case == Case::ReverseReportedTotals {
        rows[0].total_loss = total_axis(total_b);
        rows[1].total_loss = total_axis(total_a);
    }
    if case == Case::DetachedEqualTotal { rows[0].total_loss = total_axis(detached.as_ref()); }
    if case == Case::WrongVariant { rows[0].total_loss = j::AxisValue::Exact(total_a.unwrap()); }
    let kinds = [j::AxisKind::Goal(d(92)), j::AxisKind::TemporalLoss, j::AxisKind::SpatialLoss];
    let policy = j::SelectionPolicy { pins, axes: &kinds, representative_order: None,
        declared_goal_solve_overlap: false, max_candidates: 2, max_canonical_bytes: 64 };
    if case == Case::ReverseReportedTotals {
        // Existing API verifies axes and accepts Given totals by its contract.
        // This is a control for the need for the NEW stronger connection.
        let declared = s::select_current(&policy, &rows, &owners, j::SearchCoverage::EvaluatedSet, &view).unwrap();
        assert_eq!(declared.report_current(&view).unwrap().recommendation, Some(1));
    }
    if case == Case::CancelBefore { cancel.cancel(); }
    let reduction_slice = if case == Case::MissingReduction { &reductions[..1] } else { &reductions[..] };
    let result = r::select_current_reduced(&policy, &rows, &owners, reduction_slice, j::SearchCoverage::EvaluatedSet, &view);
    if matches!(case, Case::ReverseReportedTotals | Case::DetachedEqualTotal | Case::SwapReduction
        | Case::EqualOtherRoot | Case::EqualOtherOwner | Case::MissingReduction | Case::DuplicateRow
        | Case::WrongBaseline | Case::WrongVariant | Case::CancelBefore) {
        assert!(result.is_err());
        return;
    }
    let selected = result.unwrap();
    let report = selected.report_current(&view).unwrap();
    if unsupported {
        assert!(report.recommendation.is_none());
    } else {
        assert_eq!(report.frontier.len(), 2);
        assert_eq!(report.recommendation, Some(usize::from(case == Case::FlipActual)));
    }
    if case == Case::CancelAfter {
        cancel.cancel();
        assert!(selected.report_current(&view).is_err());
        assert_eq!(selected.historical_report().recommendation, Some(0));
    }
    if case == Case::ForeignAfter {
        let (_other_broker, _other_run, other_reservation) = run(Cancellation::default());
        let foreign = other_reservation.lease();
        assert!(selected.report_current(&foreign).is_err());
        assert_eq!(selected.report_current(&view).unwrap().recommendation, Some(0));
    }
}

#[test]
fn tlcx19_reduced_total_rejects_detached_reversed_and_wrong_variant() {
    for c in [Case::ReverseReportedTotals, Case::DetachedEqualTotal, Case::WrongVariant] { exercise(c); }
}
#[test]
fn tlcx19_reduced_total_rejects_owner_root_row_and_baseline_swaps() {
    for c in [Case::SwapReduction, Case::EqualOtherRoot, Case::EqualOtherOwner,
        Case::MissingReduction, Case::DuplicateRow, Case::WrongBaseline] { exercise(c); }
}
#[test]
fn tlcx19_reduced_total_real_tradeoff_recommendation_changes_with_actual_models() {
    exercise(Case::Real);
    exercise(Case::FlipActual);
}
#[test]
fn tlcx19_reduced_total_current_cancellation_and_foreign_lease() {
    for c in [Case::CancelBefore, Case::CancelAfter, Case::ForeignAfter] { exercise(c); }
}
#[test]
fn tlcx19_reduced_total_bounded_unknown_and_unsupported_stay_typed() {
    exercise(Case::Bounded);
    exercise(Case::Unsupported);
}
