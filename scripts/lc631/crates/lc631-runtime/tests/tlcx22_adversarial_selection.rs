mod tlcx22_adversarial_support;
use lc631_core::measurement_numeric::RationalInterval;
use lc631_runtime::{adversarial_measurement::*, joint_selection::*};
use lc631_solvers::typed_query::{BoolExpr, BoolSymbolId};
use lc631_substrate::resource::Cancellation;
use tlcx22_adversarial_support::*;
fn row<'a>(
    b: CandidateBinding,
    pins: SelectionPins,
    axes: &'a [AxisValue<'a>],
    zero: &'a RationalInterval,
    key: &'a [u8],
    digest: lc631_substrate::Digest32,
) -> JointCandidate<'a> {
    JointCandidate {
        id: b.id,
        context: b.context,
        pins,
        hard_gate: ReportedGate::Pass,
        map_back: ReportedGate::Pass,
        axes,
        total_loss: AxisValue::Exact(zero),
        canonical_key: key,
        measurement_binding: digest,
    }
}
#[test]
fn tlcx22_adversarial_actual_pareto_winner_changes_cut_and_consumption_checks() {
    let gamma = theory(&[20], vec![]);
    let reference = v(20);
    let neg = BoolExpr::Not(Box::new(v(20)));
    let weight = q(1, 1);
    let atoms = [SemanticAtom {
        symbol: BoolSymbolId::new(20),
        meaning: b"input",
    }];
    let cases = [AttackCase {
        key: b"fixed-response-agreement",
        atoms: &atoms,
        theory: &gamma,
        reference: &reference,
        weight: &weight,
    }];
    for (swap, cut) in [(false, false), (true, false), (false, true)] {
        let cancel = Cancellation::default();
        let (_br, _run, res) = run(cancel.clone());
        let view = res.lease();
        let mut l = limits();
        if cut {
            l.max_trials = 0;
        }
        let catalog = register(binding(), &cases, l, &view).unwrap();
        let a = candidate(1);
        let b = candidate(2);
        let fa = bind_candidate(
            &catalog,
            a,
            &[Response {
                case: cases[0].key,
                term: if swap { &neg } else { &reference },
            }],
            &view,
        )
        .unwrap();
        let fb = bind_candidate(
            &catalog,
            b,
            &[Response {
                case: cases[0].key,
                term: if swap { &reference } else { &neg },
            }],
            &view,
        )
        .unwrap();
        let oa = execute(&fa, &view).unwrap();
        let ob = execute(&fb, &view).unwrap();
        let kinds = [AxisKind::Goal(d(99)), AxisKind::CounterexampleCost];
        let pins = binding().selection;
        let policy = SelectionPolicy {
            pins,
            axes: &kinds,
            representative_order: None,
            declared_goal_solve_overlap: false,
            max_candidates: 2,
            max_canonical_bytes: 64,
        };
        let zero = RationalInterval::exact_ratio(0, 1).unwrap();
        let aa = [
            AxisValue::Exact(&zero),
            oa.axis_current(&fa, pins, &view).unwrap().value,
        ];
        let ab = [
            AxisValue::Exact(&zero),
            ob.axis_current(&fb, pins, &view).unwrap().value,
        ];
        let rows = [
            row(a, pins, &aa, &zero, b"a", oa.measurement_binding()),
            row(b, pins, &ab, &zero, b"b", ob.measurement_binding()),
        ];
        let owners = [
            AdversarialRow {
                owner: &oa,
                frozen: &fa,
                row: 0,
                axis: 1,
            },
            AdversarialRow {
                owner: &ob,
                frozen: &fb,
                row: 1,
                axis: 1,
            },
        ];
        let selection =
            select_current(&policy, &rows, &owners, SearchCoverage::EvaluatedSet, &view).unwrap();
        let expected = if cut { None } else { Some(usize::from(swap)) };
        assert_eq!(
            selection.report_current(&view).unwrap().recommendation,
            expected
        );
        let (_br2, _run2, res2) = run(Cancellation::default());
        let foreign = res2.lease();
        assert!(matches!(
            selection.report_current(&foreign),
            Err(Error::ForeignReservation)
        ));
        cancel.cancel();
        assert!(selection.report_current(&view).is_err());
        assert_eq!(selection.historical_report().recommendation, expected);
    }
}
#[test]
fn tlcx22_adversarial_report_copy_missing_owner_and_forged_common_catalog_rejected() {
    let gamma = theory(&[], vec![]);
    let yes = BoolExpr::Constant(true);
    let no = BoolExpr::Constant(false);
    let weight = q(1, 1);
    let cases = [AttackCase {
        key: b"case",
        atoms: &[],
        theory: &gamma,
        reference: &yes,
        weight: &weight,
    }];
    let (_br, _run, res) = run(Cancellation::default());
    let view = res.lease();
    let catalog = register(binding(), &cases, limits(), &view).unwrap();
    let a = candidate(1);
    let frozen = bind_candidate(
        &catalog,
        a,
        &[Response {
            case: b"case",
            term: &yes,
        }],
        &view,
    )
    .unwrap();
    let owner = execute(&frozen, &view).unwrap();
    let pins = binding().selection;
    let kinds = [AxisKind::CounterexampleCost];
    let policy = SelectionPolicy {
        pins,
        axes: &kinds,
        representative_order: None,
        declared_goal_solve_overlap: false,
        max_candidates: 2,
        max_canonical_bytes: 64,
    };
    let zero = RationalInterval::exact_ratio(0, 1).unwrap();
    let bindings = [AdversarialRow {
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
        &zero,
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
    let rows = [row(
        a,
        pins,
        &axes,
        &zero,
        b"a",
        owner.measurement_binding(),
    )];
    assert!(matches!(
        select_current(&policy, &rows, &[], SearchCoverage::EvaluatedSet, &view),
        Err(Error::InvalidRows)
    ));
    let changed = [AttackCase {
        reference: &no,
        ..cases[0]
    }];
    let other = register(binding(), &changed, limits(), &view).unwrap();
    let b = candidate(2);
    let fb = bind_candidate(
        &other,
        b,
        &[Response {
            case: b"case",
            term: &yes,
        }],
        &view,
    )
    .unwrap();
    let ob = execute(&fb, &view).unwrap();
    let ab = [ob.axis_current(&fb, pins, &view).unwrap().value];
    let rows = [
        row(a, pins, &axes, &zero, b"a", owner.measurement_binding()),
        row(b, pins, &ab, &zero, b"b", ob.measurement_binding()),
    ];
    let both = [
        AdversarialRow {
            owner: &owner,
            frozen: &frozen,
            row: 0,
            axis: 0,
        },
        AdversarialRow {
            owner: &ob,
            frozen: &fb,
            row: 1,
            axis: 0,
        },
    ];
    assert!(matches!(
        select_current(&policy, &rows, &both, SearchCoverage::EvaluatedSet, &view),
        Err(Error::BindingMismatch)
    ));
}
