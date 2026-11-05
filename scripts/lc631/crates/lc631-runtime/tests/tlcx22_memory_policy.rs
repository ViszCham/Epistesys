//! 日本語: 実ownerの異なる正規化policyを同じ比較に混ぜない回帰fixture。
//! English: Real-owner regression fixtures for mixed normalization policies.
mod tlcx22_memory_support;
use lc631_runtime::{joint_selection::*, memory_measurement::*};
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
fn compare(mode: u8, reject: bool) {
    let a = Fixture::three(vec![bounds(1, 1, 1, 1)], vec![(1, 3, 0)]);
    let b = Fixture::three(vec![bounds(2, 1, 2, 1)], vec![(1, 3, 0)]);
    let ap = a.pairs();
    let mut bp = b.pairs();
    let ba = binding(1);
    let bb = binding(2);
    assert_eq!(ba.selection, bb.selection);
    assert_eq!(ba.context, bb.context);
    let ka = q(1, 1);
    let kb = if mode == 0 { q(100, 1) } else { q(1, 1) };
    let ub = if mode == 1 { CostUnit(d(200)) } else { unit() };
    for pair in &mut bp {
        if let PairDisposition::Allowed { unit, .. } = &mut pair.disposition {
            *unit = ub;
        }
    }
    let steps = if mode == 2 { 999_999 } else { 1_000_000 };
    let pa = RegistryPlan::declared(
        ba,
        &a.states,
        &ap,
        StateId(1),
        StateId(3),
        unit(),
        &ka,
        budget(),
    );
    let pb = RegistryPlan::declared(
        bb,
        &b.states,
        &bp,
        StateId(1),
        StateId(3),
        ub,
        &kb,
        WorkBudget { max_steps: steps },
    );
    let (_br, _run, res) = run(Cancellation::default(), ba.selection.source.digest());
    let view = res.lease();
    let oa = execute_owned(&pa, &view).unwrap();
    let ob = execute_owned(&pb, &view).unwrap();
    // Both graphs and numeric computations really complete before testing comparison.
    assert_eq!(oa.historical_path_state(), PathState::Connected);
    assert_eq!(ob.historical_path_state(), PathState::Connected);
    assert_eq!(
        oa.historical_report().state,
        lc631_analysis::creativity_measurement::ComponentState::Measured(bounds(1, 2, 1, 2))
    );
    assert_eq!(
        ob.historical_report().state,
        lc631_analysis::creativity_measurement::ComponentState::Measured(if mode == 0 {
            bounds(1, 51, 1, 51)
        } else {
            bounds(2, 3, 2, 3)
        })
    );
    let kinds = [AxisKind::Goal(d(99)), AxisKind::MigrationCost];
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
    let owners = [
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
    let result = select_current(&policy, &rows, &owners, SearchCoverage::EvaluatedSet, &view);
    if reject {
        assert!(
            matches!(result, Err(Error::BindingMismatch)),
            "mixed actual policy must not enter current selection"
        );
    } else {
        let selected = result.unwrap();
        assert_eq!(
            selected.report_current(&view).unwrap().recommendation,
            Some(0)
        );
    }
}
#[test]
fn tlcx22_memory_rejects_actual_kappa_mismatch_under_equal_reported_pins() {
    compare(0, true);
}
#[test]
fn tlcx22_memory_rejects_actual_cost_unit_mismatch_under_equal_reported_pins() {
    compare(1, true);
}
#[test]
fn tlcx22_memory_rejects_actual_work_budget_mismatch_under_equal_reported_pins() {
    compare(2, true);
}
#[test]
fn tlcx22_memory_same_policy_allows_different_actual_graphs() {
    compare(3, false);
}
