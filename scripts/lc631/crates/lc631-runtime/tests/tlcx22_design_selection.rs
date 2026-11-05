mod tlcx22_design_support;
use lc631_core::measurement_numeric::RationalInterval;
use lc631_runtime::{design_measurement::*, joint_selection::*};
use lc631_substrate::resource::Cancellation;
use tlcx22_design_support::*;
fn row<'a>(
    b: CandidateBinding,
    pins: SelectionPins,
    axes: &'a [AxisValue<'a>],
    z: &'a RationalInterval,
    key: &'a [u8],
    binding: lc631_substrate::Digest32,
) -> JointCandidate<'a> {
    JointCandidate {
        id: b.id,
        context: b.context,
        pins,
        hard_gate: ReportedGate::Pass,
        map_back: ReportedGate::Pass,
        axes,
        total_loss: AxisValue::Exact(z),
        canonical_key: key,
        measurement_binding: binding,
    }
}
#[test]
fn tlcx22_design_actual_current_pareto_winner_change_and_consumption() {
    let f = Fixture::gold();
    for swap in [false, true] {
        let cancel = Cancellation::default();
        let (_br, _run, res) = run(cancel.clone());
        let view = res.lease();
        let suite = register(binding(), &[f.workload(b"case")], limits(), &view).unwrap();
        let good = improved();
        let baseline = &f.baseline;
        let a = candidate(1);
        let b = candidate(2);
        let fa = bind_candidate(
            &suite,
            a,
            &[PriorityProgram {
                case: b"case",
                order: if swap { baseline } else { &good },
            }],
            &view,
        )
        .unwrap();
        let fb = bind_candidate(
            &suite,
            b,
            &[PriorityProgram {
                case: b"case",
                order: if swap { &good } else { baseline },
            }],
            &view,
        )
        .unwrap();
        let oa = execute(&fa, &view).unwrap();
        let ob = execute(&fb, &view).unwrap();
        let pins = binding().selection;
        let kinds = [AxisKind::Goal(d(99)), AxisKind::DesignGain];
        let policy = SelectionPolicy {
            pins,
            axes: &kinds,
            representative_order: None,
            declared_goal_solve_overlap: false,
            max_candidates: 2,
            max_canonical_bytes: 64,
        };
        let z = RationalInterval::exact_ratio(0, 1).unwrap();
        let aa = [
            AxisValue::Exact(&z),
            oa.axis_current(&fa, pins, &view).unwrap().value,
        ];
        let ab = [
            AxisValue::Exact(&z),
            ob.axis_current(&fb, pins, &view).unwrap().value,
        ];
        let rows = [
            row(a, pins, &aa, &z, b"a", oa.measurement_binding()),
            row(b, pins, &ab, &z, b"b", ob.measurement_binding()),
        ];
        let owners = [
            DesignRow {
                owner: &oa,
                frozen: &fa,
                row: 0,
                axis: 1,
            },
            DesignRow {
                owner: &ob,
                frozen: &fb,
                row: 1,
                axis: 1,
            },
        ];
        let selected =
            select_current(&policy, &rows, &owners, SearchCoverage::EvaluatedSet, &view).unwrap();
        assert_eq!(
            selected.report_current(&view).unwrap().recommendation,
            Some(usize::from(swap))
        );
        let (_br2, _run2, res2) = run(Cancellation::default());
        let foreign = res2.lease();
        assert!(matches!(
            selected.report_current(&foreign),
            Err(Error::ForeignReservation)
        ));
        cancel.cancel();
        assert!(selected.report_current(&view).is_err());
        assert_eq!(
            selected.historical_report().recommendation,
            Some(usize::from(swap))
        );
    }
}
#[test]
fn tlcx22_design_private_owner_rejects_report_copies_uncovered_and_duplicate_rows() {
    let f = Fixture::gold();
    let (_br, _run, res) = run(Cancellation::default());
    let view = res.lease();
    let suite = register(binding(), &[f.workload(b"case")], limits(), &view).unwrap();
    let good = improved();
    let a = candidate(1);
    let frozen = bind_candidate(
        &suite,
        a,
        &[PriorityProgram {
            case: b"case",
            order: &good,
        }],
        &view,
    )
    .unwrap();
    let owner = execute(&frozen, &view).unwrap();
    let pins = binding().selection;
    let kinds = [AxisKind::DesignGain];
    let policy = SelectionPolicy {
        pins,
        axes: &kinds,
        representative_order: None,
        declared_goal_solve_overlap: false,
        max_candidates: 2,
        max_canonical_bytes: 64,
    };
    let z = RationalInterval::exact_ratio(0, 1).unwrap();
    let bindings = [DesignRow {
        owner: &owner,
        frozen: &frozen,
        row: 0,
        axis: 0,
    }];
    let copy = owner.historical_report().clone();
    let detached = [project_component_reported(&copy).unwrap().value];
    let rows = [row(
        a,
        pins,
        &detached,
        &z,
        b"a",
        owner.measurement_binding(),
    )];
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
    let axes = [owner.axis_current(&frozen, pins, &view).unwrap().value];
    let rows = [row(a, pins, &axes, &z, b"a", owner.measurement_binding())];
    assert!(matches!(
        select_current(&policy, &rows, &[], SearchCoverage::EvaluatedSet, &view),
        Err(Error::InvalidRows)
    ));
    let rows = [
        row(a, pins, &axes, &z, b"a", owner.measurement_binding()),
        row(a, pins, &axes, &z, b"b", owner.measurement_binding()),
    ];
    let dup = [
        DesignRow {
            owner: &owner,
            frozen: &frozen,
            row: 0,
            axis: 0,
        },
        DesignRow {
            owner: &owner,
            frozen: &frozen,
            row: 0,
            axis: 0,
        },
    ];
    assert!(matches!(
        select_current(&policy, &rows, &dup, SearchCoverage::EvaluatedSet, &view),
        Err(Error::InvalidRows)
    ));
}
#[test]
fn tlcx22_design_actual_common_suite_prevents_mixed_workloads_maps_and_budgets() {
    for cut in 0..10 {
        let a = Fixture::gold();
        let mut b = Fixture::gold();
        let mut bb = binding();
        let mut l = limits();
        if cut == 0 {
            b.upper = q(7, 1);
        }
        if cut == 1 {
            b.accepted_high = q(21, 1);
        }
        if cut == 2 {
            b.weight = q(2, 1);
        }
        if cut == 4 {
            b.baseline = improved().to_vec();
        }
        if cut == 6 {
            bb.oracle = d(200);
        }
        if cut == 7 {
            l.max_events_per_run = 31;
        }
        if cut == 8 {
            b.jobs[0].duration_ticks = 4;
        }
        if cut == 9 {
            b.dependencies = vec![(JobId(11), JobId(33))];
        }
        let wa = a.workload(b"case");
        let mut wb = b.workload(b"case");
        if cut == 3 {
            wb.workers = 1;
        }
        if cut == 5 {
            let unit = TickUnit(d(201));
            wb.unit = unit;
            wb.accepted.unit = unit;
            wb.normalization.unit = unit;
        }
        // TWO suites + TWO frozen programs + TWO results retain 16 MiB output.
        // Reserve the selector's exact extra 64 KiB; RAM/run limits stay fixed.
        let (_br, _run, res) = run_with_output(
            binding(),
            Cancellation::default(),
            16 * 1024 * 1024 + 64 * 1024,
        );
        let view = res.lease();
        let sa = register(binding(), &[wa], limits(), &view).unwrap();
        let sb = register(bb, &[wb], l, &view).unwrap();
        assert_ne!(
            sa.historical_digest(),
            sb.historical_digest(),
            "cut {cut} must change actual immutable suite"
        );
        let good = improved();
        let ca = candidate(1);
        let cb = candidate(2);
        let fa = bind_candidate(
            &sa,
            ca,
            &[PriorityProgram {
                case: b"case",
                order: &good,
            }],
            &view,
        )
        .unwrap();
        let fb = bind_candidate(
            &sb,
            cb,
            &[PriorityProgram {
                case: b"case",
                order: &good,
            }],
            &view,
        )
        .unwrap();
        let oa = execute(&fa, &view).unwrap();
        let ob = execute(&fb, &view).unwrap();
        let pins = binding().selection;
        assert_eq!(pins, bb.selection);
        let kinds = [AxisKind::DesignGain];
        let policy = SelectionPolicy {
            pins,
            axes: &kinds,
            representative_order: None,
            declared_goal_solve_overlap: false,
            max_candidates: 2,
            max_canonical_bytes: 64,
        };
        let z = RationalInterval::exact_ratio(0, 1).unwrap();
        let aa = [oa.axis_current(&fa, pins, &view).unwrap().value];
        let ab = [ob.axis_current(&fb, pins, &view).unwrap().value];
        let rows = [
            row(ca, pins, &aa, &z, b"a", oa.measurement_binding()),
            row(cb, pins, &ab, &z, b"b", ob.measurement_binding()),
        ];
        let owners = [
            DesignRow {
                owner: &oa,
                frozen: &fa,
                row: 0,
                axis: 0,
            },
            DesignRow {
                owner: &ob,
                frozen: &fb,
                row: 1,
                axis: 0,
            },
        ];
        let actual = select_current(&policy, &rows, &owners, SearchCoverage::EvaluatedSet, &view);
        assert!(
            matches!(&actual, Err(Error::BindingMismatch)),
            "cut {cut}: actual error {:?}",
            actual.as_ref().err()
        );
    }
}
