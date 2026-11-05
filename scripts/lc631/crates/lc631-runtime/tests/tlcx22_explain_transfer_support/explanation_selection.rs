//! Current-consumer tests: existing reported goals/gates remain reported.
use super::*;
#[test]
fn tlcx22_explain_selection_changes_and_current_cut_cannot_replay_old_winner() {
    let f = Fixture::explain();
    let p = f.projection(false, false);
    let bad = f.projection(true, true);
    let good = f.projection(true, false);
    let cases = [f.case(b"p", &f.p, &p), f.case(b"q", &f.q, &bad)];
    let ri = root_input(&cases);
    let cancel = Cancellation::default();
    let (_br, _run, res) = run(cancel.clone());
    let view = res.lease();
    let root = e::capture_explain(ri, &view).unwrap();
    let ra = [response(b"p", &p), response(b"q", &bad)];
    let rb = [response(b"p", &p), response(b"q", &good)];
    let ia = input(1, &ra);
    let ib = input(2, &rb);
    let a = e::admit_explain(&root, ia, &view).unwrap();
    let b = e::admit_explain(&root, ib, &view).unwrap();
    let oa = e::measure_explain(&a, &view).unwrap();
    let ob = e::measure_explain(&b, &view).unwrap();
    let ea = e::Expected {
        root: ri,
        candidate: ia,
    };
    let eb = e::Expected {
        root: ri,
        candidate: ib,
    };
    let zero = RationalInterval::exact_ratio(0, 1).unwrap();
    let one = RationalInterval::exact_ratio(1, 1).unwrap();
    let kinds = [AxisKind::Goal(d(8)), AxisKind::ExplainGain];
    let aa = [
        AxisValue::Exact(&one),
        oa.axis_current(&a, ea, ri.binding.selection, &view)
            .unwrap()
            .value,
    ];
    let ab = [
        AxisValue::Exact(&one),
        ob.axis_current(&b, eb, ri.binding.selection, &view)
            .unwrap()
            .value,
    ];
    let rows = [
        JointCandidate {
            id: ia.binding.id,
            context: ia.binding.context,
            pins: ri.binding.selection,
            hard_gate: ReportedGate::Pass,
            map_back: ReportedGate::Pass,
            axes: &aa,
            total_loss: AxisValue::Exact(&zero),
            canonical_key: b"proof-a",
            measurement_binding: oa.measurement_binding(),
        },
        JointCandidate {
            id: ib.binding.id,
            context: ib.binding.context,
            pins: ri.binding.selection,
            hard_gate: ReportedGate::Pass,
            map_back: ReportedGate::Pass,
            axes: &ab,
            total_loss: AxisValue::Exact(&zero),
            canonical_key: b"proof-b",
            measurement_binding: ob.measurement_binding(),
        },
    ];
    let policy = SelectionPolicy {
        pins: ri.binding.selection,
        axes: &kinds,
        representative_order: None,
        declared_goal_solve_overlap: false,
        max_candidates: 64,
        max_canonical_bytes: 4096,
    };
    let bindings = [
        e::ExplainRow {
            owner: &oa,
            candidate: &a,
            expected: ea,
            row: 0,
            axis: 1,
        },
        e::ExplainRow {
            owner: &ob,
            candidate: &b,
            expected: eb,
            row: 1,
            axis: 1,
        },
    ];
    let selection = e::select_explain_current(
        &policy,
        &rows,
        &bindings,
        SearchCoverage::EvaluatedSet,
        &view,
    )
    .unwrap();
    assert_eq!(
        selection.report_current(&view).unwrap().recommendation,
        Some(1)
    );
    assert!(matches!(
        e::select_explain_current(
            &policy,
            &rows,
            &bindings[..1],
            SearchCoverage::EvaluatedSet,
            &view
        ),
        Err(e::Error::InvalidRows)
    ));
    // A detached equal scalar is not an owner borrow; identity must be checked.
    let detached = RationalInterval::exact_ratio(1, 2).unwrap();
    let forged_axes = [AxisValue::Exact(&one), AxisValue::Exact(&detached)];
    let forged = [JointCandidate {
        id: ib.binding.id,
        context: ib.binding.context,
        pins: ri.binding.selection,
        hard_gate: ReportedGate::Pass,
        map_back: ReportedGate::Pass,
        axes: &forged_axes,
        total_loss: AxisValue::Exact(&zero),
        canonical_key: b"proof-b",
        measurement_binding: ob.measurement_binding(),
    }];
    let forged_binding = [e::ExplainRow {
        owner: &ob,
        candidate: &b,
        expected: eb,
        row: 0,
        axis: 1,
    }];
    assert!(matches!(
        e::select_explain_current(
            &policy,
            &forged,
            &forged_binding,
            SearchCoverage::EvaluatedSet,
            &view
        ),
        Err(e::Error::BindingMismatch)
    ));
    let hard = [JointCandidate {
        id: ib.binding.id,
        context: ib.binding.context,
        pins: ri.binding.selection,
        hard_gate: ReportedGate::Fail,
        map_back: ReportedGate::Pass,
        axes: &ab,
        total_loss: AxisValue::Exact(&zero),
        canonical_key: b"proof-b",
        measurement_binding: ob.measurement_binding(),
    }];
    let denied = e::select_explain_current(
        &policy,
        &hard,
        &forged_binding,
        SearchCoverage::EvaluatedSet,
        &view,
    )
    .unwrap();
    assert_eq!(denied.report_current(&view).unwrap().recommendation, None);
    cancel.cancel();
    assert!(matches!(
        selection.report_current(&view),
        Err(e::Error::Resource(ResourceError::Cancelled))
    ));
}
