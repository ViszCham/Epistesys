mod tlcx22_prediction_support;
use lc631_core::measurement_numeric::RationalInterval;
use lc631_runtime::joint_selection::*;
use lc631_runtime::prediction_measurement::*;
use lc631_solvers::temporal::Truth;
use lc631_substrate::resource::Cancellation;
use tlcx22_prediction_support::*;
fn row<'a>(
    b: ForecastBinding,
    axes: &'a [AxisValue<'a>],
    zero: &'a RationalInterval,
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
fn tlcx22_prediction_real_current_selection_winner_change_unknown_cut_and_consumption() {
    for (prob, unknown, expected) in [(0, false, 0), (1, false, 1), (1, true, 0)] {
        let a = binding(1, SplitRole::Validation);
        let mut b = binding(2, SplitRole::Validation);
        b.candidate_model = d(101);
        let qa = query(a);
        let qb = query(b);
        let ta = trace(if unknown { Truth::Unknown } else { Truth::True });
        let tb = trace(if unknown { Truth::Unknown } else { Truth::True });
        let zero = q(0, 1);
        let half = q(1, 2);
        let other = q(prob, 1);
        let one = q(1, 1);
        let ia = Forecast {
            canonical_sample: b"sample",
            baseline: &zero,
            candidate: &half,
            weight: &one,
            query: &qa,
            trace: &ta,
        };
        let ib = Forecast {
            canonical_sample: b"sample",
            baseline: &zero,
            candidate: &other,
            weight: &one,
            query: &qb,
            trace: &tb,
        };
        let cancel = Cancellation::default();
        let (_br, _run, res) = run(cancel.clone(), a);
        let view = res.lease();
        let limits = ForecastLimits {
            max_node_steps: 1024 * 256,
        };
        let fa = freeze(a, &[ia], limits, &view).unwrap();
        let fb = freeze(b, &[ib], limits, &view).unwrap();
        let oa = evaluate(&fa, &view).unwrap();
        let ob = evaluate(&fb, &view).unwrap();
        let kinds = [AxisKind::Goal(d(99)), AxisKind::PredictionGain];
        let policy = SelectionPolicy {
            pins: a.selection,
            axes: &kinds,
            representative_order: None,
            declared_goal_solve_overlap: false,
            max_candidates: 2,
            max_canonical_bytes: 64,
        };
        let z = RationalInterval::exact_ratio(0, 1).unwrap();
        let aa = [
            AxisValue::Exact(&z),
            oa.axis_current(&fa, a.selection, &view).unwrap().value,
        ];
        let ab = [
            AxisValue::Exact(&z),
            ob.axis_current(&fb, b.selection, &view).unwrap().value,
        ];
        let rows = [
            row(a, &aa, &z, b"a", oa.measurement_binding()),
            row(b, &ab, &z, b"b", ob.measurement_binding()),
        ];
        let bindings = [
            PredictionRow {
                owner: &oa,
                frozen: &fa,
                row: 0,
                axis: 1,
            },
            PredictionRow {
                owner: &ob,
                frozen: &fb,
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
            if unknown { None } else { Some(expected) }
        );
        if unknown {
            assert_eq!(selected.report_current(&view).unwrap().frontier.len(), 2);
        }
        let (_br2, _run2, res2) = run(Cancellation::default(), a);
        let view2 = res2.lease();
        assert!(matches!(
            selected.report_current(&view2),
            Err(Error::ForeignReservation)
        ));
        cancel.cancel();
        assert!(selected.report_current(&view).is_err());
        assert_eq!(
            selected.historical_report().recommendation,
            if unknown { None } else { Some(expected) }
        );
    }
}
#[test]
fn tlcx22_prediction_copied_report_missing_duplicate_rows_and_policy_cuts() {
    let b = binding(1, SplitRole::Validation);
    let query = query(b);
    let trace = trace(Truth::True);
    let zero = q(0, 1);
    let half = q(1, 2);
    let one = q(1, 1);
    let input = Forecast {
        canonical_sample: b"sample",
        baseline: &zero,
        candidate: &half,
        weight: &one,
        query: &query,
        trace: &trace,
    };
    let (_br, _run, res) = run(Cancellation::default(), b);
    let view = res.lease();
    let frozen = freeze(
        b,
        &[input],
        ForecastLimits {
            max_node_steps: 1024 * 256,
        },
        &view,
    )
    .unwrap();
    let owner = evaluate(&frozen, &view).unwrap();
    let kinds = [AxisKind::PredictionGain];
    let policy = SelectionPolicy {
        pins: b.selection,
        axes: &kinds,
        representative_order: None,
        declared_goal_solve_overlap: false,
        max_candidates: 2,
        max_canonical_bytes: 64,
    };
    let z = RationalInterval::exact_ratio(0, 1).unwrap();
    let bindings = [PredictionRow {
        owner: &owner,
        frozen: &frozen,
        row: 0,
        axis: 0,
    }];
    let copy = owner.historical_report().clone();
    let bad = [project_component_reported(&copy).unwrap().value];
    let rows = [row(b, &bad, &z, b"a", owner.measurement_binding())];
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
    let axes = [owner
        .axis_current(&frozen, b.selection, &view)
        .unwrap()
        .value];
    let rows = [row(b, &axes, &z, b"a", owner.measurement_binding())];
    assert!(matches!(
        select_current(&policy, &rows, &[], SearchCoverage::EvaluatedSet, &view),
        Err(Error::InvalidRows)
    ));
    let duplicate = [
        PredictionRow {
            owner: &owner,
            frozen: &frozen,
            row: 0,
            axis: 0,
        },
        PredictionRow {
            owner: &owner,
            frozen: &frozen,
            row: 0,
            axis: 0,
        },
    ];
    let rows2 = [
        row(b, &axes, &z, b"a", owner.measurement_binding()),
        row(b, &axes, &z, b"b", owner.measurement_binding()),
    ];
    assert!(matches!(
        select_current(
            &policy,
            &rows2,
            &duplicate,
            SearchCoverage::EvaluatedSet,
            &view
        ),
        Err(Error::InvalidRows)
    ));
    let mut changed = b.selection;
    changed.policy = d(223);
    assert!(matches!(
        owner.axis_current(&frozen, changed, &view),
        Err(Error::BindingMismatch)
    ));
}
#[test]
fn tlcx22_prediction_split_roles_and_local_budget_are_frozen_policy() {
    for role in [SplitRole::Training, SplitRole::FinalResearchHoldout] {
        let b = binding(1, role);
        let query = query(b);
        let trace = trace(Truth::True);
        let zero = q(0, 1);
        let half = q(1, 2);
        let one = q(1, 1);
        let input = Forecast {
            canonical_sample: b"sample",
            baseline: &zero,
            candidate: &half,
            weight: &one,
            query: &query,
            trace: &trace,
        };
        let (_br, _run, res) = run(Cancellation::default(), b);
        let view = res.lease();
        let f = freeze(
            b,
            &[input],
            ForecastLimits {
                max_node_steps: 1024 * 256,
            },
            &view,
        )
        .unwrap();
        if role == SplitRole::FinalResearchHoldout {
            assert!(matches!(
                evaluate(&f, &view),
                Err(Error::SplitNotSelectable)
            ));
        } else {
            let owner = evaluate(&f, &view).unwrap();
            assert!(matches!(
                owner.axis_current(&f, b.selection, &view),
                Err(Error::SplitNotSelectable)
            ));
        }
    }
    let b = binding(1, SplitRole::Validation);
    let query = query(b);
    let trace = trace(Truth::True);
    let zero = q(0, 1);
    let half = q(1, 2);
    let one = q(1, 1);
    let input = Forecast {
        canonical_sample: b"sample",
        baseline: &zero,
        candidate: &half,
        weight: &one,
        query: &query,
        trace: &trace,
    };
    let (_br, _run, res) = run(Cancellation::default(), b);
    let view = res.lease();
    let a = freeze(b, &[input], ForecastLimits { max_node_steps: 0 }, &view).unwrap();
    let c = freeze(b, &[input], ForecastLimits { max_node_steps: 1 }, &view).unwrap();
    assert_ne!(a.historical_digest(), c.historical_digest());
    let oa = evaluate(&a, &view).unwrap();
    let oc = evaluate(&c, &view).unwrap();
    assert!(matches!(
        oa.historical_report().state,
        lc631_analysis::creativity_measurement::ComponentState::Unknown(_)
    ));
    assert!(matches!(
        oc.historical_report().state,
        lc631_analysis::creativity_measurement::ComponentState::Measured(_)
    ));
}

#[test]
fn tlcx22_prediction_actual_common_profile_rejects_forged_equal_reported_pins() {
    // Every altered input below retains equal public SelectionPins. The private
    // comparison identity must detect the actual baseline/label/policy change.
    for cut in 0..8 {
        let a = binding(1, SplitRole::Validation);
        let mut b = binding(2, SplitRole::Validation);
        if cut == 3 {
            b.split_digest = d(200);
        }
        if cut == 6 {
            b.baseline_model = d(201);
        }
        if cut == 7 {
            b.oracle = d(202);
        }
        let qa = query(a);
        let qb = if cut == 5 {
            lc631_solvers::temporal::TemporalQuery::checked(
                b.context,
                vec![
                    lc631_solvers::temporal::TemporalNode::Atom(0),
                    lc631_solvers::temporal::TemporalNode::Not(0),
                ],
                1,
                lc631_solvers::temporal::TemporalProfile::ClosedFiniteKleeneV1,
            )
            .unwrap()
        } else {
            query(b)
        };
        let ta = trace(Truth::True);
        let tb = trace(if cut == 0 { Truth::False } else { Truth::True });
        let zero = q(0, 1);
        let half = q(1, 2);
        let one = q(1, 1);
        let two = q(2, 1);
        let ia = Forecast {
            canonical_sample: b"sample",
            baseline: &zero,
            candidate: &half,
            weight: &one,
            query: &qa,
            trace: &ta,
        };
        let ib = Forecast {
            canonical_sample: b"sample",
            baseline: if cut == 2 { &half } else { &zero },
            candidate: &one,
            weight: if cut == 1 { &two } else { &one },
            query: &qb,
            trace: &tb,
        };
        let (_br, _run, res) = run(Cancellation::default(), a);
        let view = res.lease();
        let fa = freeze(
            a,
            &[ia],
            ForecastLimits {
                max_node_steps: 1024 * 256,
            },
            &view,
        )
        .unwrap();
        let fb = freeze(
            b,
            &[ib],
            ForecastLimits {
                max_node_steps: if cut == 4 { 0 } else { 1024 * 256 },
            },
            &view,
        )
        .unwrap();
        let oa = evaluate(&fa, &view).unwrap();
        let ob = evaluate(&fb, &view).unwrap();
        let kinds = [AxisKind::PredictionGain];
        let policy = SelectionPolicy {
            pins: a.selection,
            axes: &kinds,
            representative_order: None,
            declared_goal_solve_overlap: false,
            max_candidates: 2,
            max_canonical_bytes: 64,
        };
        let z = RationalInterval::exact_ratio(0, 1).unwrap();
        let aa = [oa.axis_current(&fa, a.selection, &view).unwrap().value];
        let ab = [ob.axis_current(&fb, b.selection, &view).unwrap().value];
        let rows = [
            row(a, &aa, &z, b"a", oa.measurement_binding()),
            row(b, &ab, &z, b"b", ob.measurement_binding()),
        ];
        let bindings = [
            PredictionRow {
                owner: &oa,
                frozen: &fa,
                row: 0,
                axis: 0,
            },
            PredictionRow {
                owner: &ob,
                frozen: &fb,
                row: 1,
                axis: 0,
            },
        ];
        assert!(
            matches!(
                select_current(
                    &policy,
                    &rows,
                    &bindings,
                    SearchCoverage::EvaluatedSet,
                    &view
                ),
                Err(Error::BindingMismatch)
            ),
            "cut {cut}"
        );
    }
}
