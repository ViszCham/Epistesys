//! Actual owner -> existing selection, not raw reported gain injection.
use super::*;
#[test]
fn tlcx22_transfer_selection_three_quarters_has_effect_and_hard_cut_stays_hard() {
    let f = Fixture::transfer();
    let cases = tf::cases(&f);
    let entries = tf::entries(&f);
    let ri = tf::root(&cases, &entries);
    let cancel = Cancellation::default();
    let (_br, _run, res) = run(cancel.clone());
    let view = res.lease();
    let root = t::capture_transfer(ri, &view).unwrap();
    let ia = tf::input(1, t::Program::ExtractV1);
    let ib = tf::input(2, t::Program::ComposeV1);
    let a = t::admit_transfer(&root, ia, &view).unwrap();
    let b = t::admit_transfer(&root, ib, &view).unwrap();
    let oa = t::execute_transfer(&a, &view).unwrap();
    let ob = t::execute_transfer(&b, &view).unwrap();
    let ea = t::Expected {
        root: ri,
        candidate: ia,
    };
    let eb = t::Expected {
        root: ri,
        candidate: ib,
    };
    let zero = RationalInterval::exact_ratio(0, 1).unwrap();
    let one = RationalInterval::exact_ratio(1, 1).unwrap();
    let kinds = [AxisKind::Goal(d(8)), AxisKind::TransferGain];
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
            canonical_key: b"extract-v1",
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
            canonical_key: b"compose-v1",
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
    let owners = [
        t::TransferRow {
            owner: &oa,
            candidate: &a,
            expected: ea,
            row: 0,
            axis: 1,
        },
        t::TransferRow {
            owner: &ob,
            candidate: &b,
            expected: eb,
            row: 1,
            axis: 1,
        },
    ];
    let selected =
        t::select_transfer_current(&policy, &rows, &owners, SearchCoverage::EvaluatedSet, &view)
            .unwrap();
    assert_eq!(
        selected.report_current(&view).unwrap().recommendation,
        Some(1)
    );
    assert!(matches!(
        t::select_transfer_current(
            &policy,
            &rows,
            &owners[..1],
            SearchCoverage::EvaluatedSet,
            &view
        ),
        Err(t::Error::InvalidRows)
    ));
    let wrong = [
        t::TransferRow {
            owner: &ob,
            candidate: &a,
            expected: ea,
            row: 0,
            axis: 1,
        },
        owners[1],
    ];
    assert!(t::select_transfer_current(
        &policy,
        &rows,
        &wrong,
        SearchCoverage::EvaluatedSet,
        &view
    )
    .is_err());
    let hard = [JointCandidate {
        id: ib.binding.id,
        context: ib.binding.context,
        pins: ri.binding.selection,
        hard_gate: ReportedGate::Fail,
        map_back: ReportedGate::Pass,
        axes: &ab,
        total_loss: AxisValue::Exact(&zero),
        canonical_key: b"compose-v1",
        measurement_binding: ob.measurement_binding(),
    }];
    let one_owner = [t::TransferRow {
        owner: &ob,
        candidate: &b,
        expected: eb,
        row: 0,
        axis: 1,
    }];
    let denied = t::select_transfer_current(
        &policy,
        &hard,
        &one_owner,
        SearchCoverage::EvaluatedSet,
        &view,
    )
    .unwrap();
    assert_eq!(denied.report_current(&view).unwrap().recommendation, None);
    cancel.cancel();
    assert!(matches!(
        selected.report_current(&view),
        Err(t::Error::Resource(ResourceError::Cancelled))
    ));
}
