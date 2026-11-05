mod tlcx22_memory_support;
use lc631_runtime::joint_selection::*;
use lc631_runtime::memory_measurement::*;
use lc631_substrate::resource::Cancellation;
use tlcx22_memory_support::*;

fn row<'a>(
    b: Binding,
    axes: &'a [AxisValue<'a>],
    zero: &'a lc631_core::measurement_numeric::RationalInterval,
    key: &'a [u8],
    digest: lc631_substrate::Digest32,
) -> JointCandidate<'a> {
    JointCandidate {
        id: b.candidate,
        context: b.context,
        pins: b.selection,
        hard_gate: ReportedGate::Pass,
        map_back: ReportedGate::Pass,
        axes,
        total_loss: AxisValue::Exact(zero),
        canonical_key: key,
        measurement_binding: digest,
    }
}
#[test]
fn tlcx22_memory_actual_selection_changes_winner_and_preserves_open_cut() {
    for (cost, open, expected) in [(3, false, 0), (0, false, 1), (0, true, 0)] {
        let a = Fixture::three(vec![bounds(1, 1, 1, 1)], vec![(1, 3, 0)]);
        let b = Fixture::three(vec![bounds(cost, 1, cost, 1)], vec![(1, 3, 0)]);
        let ap = a.pairs();
        let mut bp = b.pairs();
        if open {
            bp.pop();
        }
        let ba = binding(1);
        let bb = binding(2);
        let k = q(1, 1);
        let pa = RegistryPlan::declared(
            ba,
            &a.states,
            &ap,
            StateId(1),
            StateId(3),
            unit(),
            &k,
            budget(),
        );
        let pb = RegistryPlan::declared(
            bb,
            &b.states,
            &bp,
            StateId(1),
            StateId(3),
            unit(),
            &k,
            budget(),
        );
        let cancel = Cancellation::default();
        let (_broker, _run, res) = run(cancel.clone(), ba.selection.source.digest());
        let view = res.lease();
        let oa = execute_owned(&pa, &view).unwrap();
        let ob = execute_owned(&pb, &view).unwrap();
        let goal = AxisKind::Goal(d(99));
        let kinds = [goal, AxisKind::MigrationCost];
        let policy = SelectionPolicy {
            pins: ba.selection,
            axes: &kinds,
            representative_order: None,
            declared_goal_solve_overlap: false,
            max_candidates: 2,
            max_canonical_bytes: 64,
        };
        let zero = bounds(0, 1, 0, 1);
        let aa = [
            AxisValue::Exact(&zero),
            oa.axis_current(&pa, ba.selection, &view).unwrap().value,
        ];
        let ab = [
            AxisValue::Exact(&zero),
            ob.axis_current(&pb, bb.selection, &view).unwrap().value,
        ];
        let rows = [
            row(ba, &aa, &zero, b"a", oa.measurement_binding()),
            row(bb, &ab, &zero, b"b", ob.measurement_binding()),
        ];
        let bindings = [
            MemoryRow {
                owner: &oa,
                plan: &pa,
                row: 0,
                axis: 1,
            },
            MemoryRow {
                owner: &ob,
                plan: &pb,
                row: 1,
                axis: 1,
            },
        ];
        let selected = select_current(
            &policy,
            &rows,
            &bindings,
            SearchCoverage::EvaluatedSet,
            &view,
        )
        .unwrap();
        assert_eq!(
            selected.report_current(&view).unwrap().recommendation,
            Some(expected)
        );
        if open {
            assert_eq!(selected.report_current(&view).unwrap().frontier.len(), 2);
        }
        cancel.cancel();
        assert!(selected.report_current(&view).is_err());
        assert_eq!(selected.historical_report().recommendation, Some(expected));
    }
}
#[test]
fn tlcx22_memory_copied_report_detached_axis_and_uncovered_rows_rejected() {
    let f = Fixture::three(vec![bounds(1, 1, 1, 1)], vec![(1, 3, 0)]);
    let pairs = f.pairs();
    let b = binding(1);
    let k = q(1, 1);
    let plan = RegistryPlan::declared(
        b,
        &f.states,
        &pairs,
        StateId(1),
        StateId(3),
        unit(),
        &k,
        budget(),
    );
    let (_broker, _run, res) = run(Cancellation::default(), b.selection.source.digest());
    let view = res.lease();
    let owner = execute_owned(&plan, &view).unwrap();
    let kinds = [AxisKind::MigrationCost];
    let policy = SelectionPolicy {
        pins: b.selection,
        axes: &kinds,
        representative_order: None,
        declared_goal_solve_overlap: false,
        max_candidates: 2,
        max_canonical_bytes: 64,
    };
    let zero = bounds(0, 1, 0, 1);
    let bindings = [MemoryRow {
        owner: &owner,
        plan: &plan,
        row: 0,
        axis: 0,
    }];
    let detached = owner.historical_report().clone();
    let axes = [project_component_reported(&detached).unwrap().value];
    let rows = [row(b, &axes, &zero, b"a", owner.measurement_binding())];
    assert!(matches!(
        select_current(
            &policy,
            &rows,
            &bindings,
            SearchCoverage::EvaluatedSet,
            &view
        ),
        Err(Error::BindingMismatch)
    ));
    let current = [owner.axis_current(&plan, b.selection, &view).unwrap().value];
    let rows = [row(b, &current, &zero, b"a", owner.measurement_binding())];
    assert!(matches!(
        select_current(&policy, &rows, &[], SearchCoverage::EvaluatedSet, &view),
        Err(Error::InvalidRows)
    ));
    let duplicated = [
        MemoryRow {
            owner: &owner,
            plan: &plan,
            row: 0,
            axis: 0,
        },
        MemoryRow {
            owner: &owner,
            plan: &plan,
            row: 0,
            axis: 0,
        },
    ];
    let rows2 = [
        row(b, &current, &zero, b"a", owner.measurement_binding()),
        row(b, &current, &zero, b"b", owner.measurement_binding()),
    ];
    assert!(matches!(
        select_current(
            &policy,
            &rows2,
            &duplicated,
            SearchCoverage::EvaluatedSet,
            &view
        ),
        Err(Error::InvalidRows)
    ));
    let mut changed = detached.clone();
    changed.binding.result = d(222);
    let copied = [project_component_reported(&changed).unwrap().value];
    let rows = [row(b, &copied, &zero, b"a", owner.measurement_binding())];
    assert!(matches!(
        select_current(
            &policy,
            &rows,
            &bindings,
            SearchCoverage::EvaluatedSet,
            &view
        ),
        Err(Error::BindingMismatch)
    ));
}
