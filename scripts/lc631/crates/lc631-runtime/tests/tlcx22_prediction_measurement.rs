mod tlcx22_prediction_support;
use lc631_analysis::creativity_measurement::{
    ComponentState, IndependenceStatus, InputBasis, MeasurementGap, SemanticValidation,
};
use lc631_core::measurement_numeric::{Rational, RationalInterval};
use lc631_runtime::prediction_measurement::*;
use lc631_solvers::temporal::Truth;
use lc631_substrate::resource::Cancellation;
use tlcx22_prediction_support::*;
#[test]
fn tlcx22_prediction_g16_and_weighted_mixed_independent_handgold() {
    let b = binding(1, SplitRole::Validation);
    let query = query(b);
    let t = trace(Truth::True);
    let f = trace(Truth::False);
    // G16: (0-1)^2-(1/2-1)^2=3/4.
    // Mixed: true weight1 gives +3/4; false weight3 gives -1/4 =>0 / total4.
    for mixed in [false, true] {
        let p0 = q(0, 1);
        let pz = q(1, 2);
        let w = q(1, 1);
        let w3 = q(3, 1);
        let mut inputs = vec![Forecast {
            canonical_sample: b"case-true",
            baseline: &p0,
            candidate: &pz,
            weight: &w,
            query: &query,
            trace: &t,
        }];
        if mixed {
            inputs.push(Forecast {
                canonical_sample: b"case-false",
                baseline: &p0,
                candidate: &pz,
                weight: &w3,
                query: &query,
                trace: &f,
            });
        }
        let (_br, _run, res) = run(Cancellation::default(), b);
        let view = res.lease();
        let frozen = freeze(
            b,
            &inputs,
            ForecastLimits {
                max_node_steps: 1024 * 256,
            },
            &view,
        )
        .unwrap();
        let owner = evaluate(&frozen, &view).unwrap();
        let expected = if mixed {
            RationalInterval::exact_ratio(0, 1).unwrap()
        } else {
            RationalInterval::exact_ratio(3, 4).unwrap()
        };
        assert_eq!(
            owner.report_current(&frozen, &view).unwrap().state,
            ComponentState::Measured(expected)
        );
        assert_eq!(owner.fixed_weight(), &q(if mixed { 4 } else { 1 }, 1));
        assert_eq!(owner.historical_report().basis, InputBasis::Given);
        assert_eq!(
            owner.historical_report().validation,
            SemanticValidation::Unassessed
        );
        assert_eq!(
            owner.historical_report().independence,
            IndependenceStatus::SharedDependency
        );
        assert_eq!(
            owner.historical_labels()[0],
            LabelState::Observed(if mixed { Truth::False } else { Truth::True })
        );
    }
}
#[test]
fn tlcx22_prediction_unknown_and_budget_cut_keep_original_denominator() {
    let b = binding(1, SplitRole::Validation);
    let query = query(b);
    let t = trace(Truth::True);
    let unknown = trace(Truth::Unknown);
    let p0 = q(0, 1);
    let pz = q(1, 2);
    let w = q(1, 1);
    let w3 = q(3, 1);
    for budget_cut in [false, true] {
        let inputs = [
            Forecast {
                canonical_sample: b"a",
                baseline: &p0,
                candidate: &pz,
                weight: &w,
                query: &query,
                trace: &t,
            },
            Forecast {
                canonical_sample: b"b",
                baseline: &p0,
                candidate: &pz,
                weight: &w3,
                query: &query,
                trace: &unknown,
            },
        ];
        let (_br, _run, res) = run(Cancellation::default(), b);
        let view = res.lease();
        let frozen = freeze(
            b,
            &inputs,
            ForecastLimits {
                max_node_steps: if budget_cut { 0 } else { 1024 * 256 },
            },
            &view,
        )
        .unwrap();
        let owner = evaluate(&frozen, &view).unwrap();
        assert_eq!(owner.fixed_weight(), &q(4, 1));
        assert_eq!(
            owner.historical_report().state,
            ComponentState::Unknown(MeasurementGap::MissingObservation)
        );
        assert!(owner
            .historical_labels()
            .iter()
            .any(|l| matches!(l, LabelState::Unknown(_))));
    }
}
#[test]
fn tlcx22_prediction_raw_numeric_weight_duplicate_and_source_cuts() {
    let b = binding(1, SplitRole::Validation);
    let query = query(b);
    let trace = trace(Truth::True);
    let zero = q(0, 1);
    let half = q(1, 2);
    let one = q(1, 1);
    let (_br, _run, res) = run(Cancellation::default(), b);
    let view = res.lease();
    for bad in [
        Rational::new_raw(1.into(), 0.into()),
        Rational::new_raw(2.into(), 4.into()),
        q(-1, 1),
        q(2, 1),
    ] {
        let inputs = [Forecast {
            canonical_sample: b"a",
            baseline: &bad,
            candidate: &half,
            weight: &one,
            query: &query,
            trace: &trace,
        }];
        assert!(freeze(
            b,
            &inputs,
            ForecastLimits {
                max_node_steps: 1024 * 256
            },
            &view
        )
        .is_err());
    }
    for bad in [q(0, 1), q(-1, 1)] {
        let inputs = [Forecast {
            canonical_sample: b"a",
            baseline: &zero,
            candidate: &half,
            weight: &bad,
            query: &query,
            trace: &trace,
        }];
        assert!(matches!(
            freeze(
                b,
                &inputs,
                ForecastLimits {
                    max_node_steps: 1024 * 256
                },
                &view
            ),
            Err(Error::InvalidWeight)
        ));
    }
    let input = Forecast {
        canonical_sample: b"a",
        baseline: &zero,
        candidate: &half,
        weight: &one,
        query: &query,
        trace: &trace,
    };
    assert!(matches!(
        freeze(
            b,
            &[input, input],
            ForecastLimits {
                max_node_steps: 1024 * 256
            },
            &view
        ),
        Err(Error::DuplicateSample)
    ));
    let mut bad = b;
    bad.probe_set = d(222);
    assert!(matches!(
        freeze(
            bad,
            &[input],
            ForecastLimits {
                max_node_steps: 1024 * 256
            },
            &view
        ),
        Err(Error::BindingMismatch)
    ));
    let foreign = binding(1, SplitRole::Training);
    let (_br2, _run2, res2) = run(Cancellation::default(), foreign);
    let view2 = res2.lease();
    let frozen = freeze(
        b,
        &[input],
        ForecastLimits {
            max_node_steps: 1024 * 256,
        },
        &view,
    )
    .unwrap();
    assert!(matches!(
        evaluate(&frozen, &view2),
        Err(Error::ForeignReservation)
    ));
}
#[test]
fn tlcx22_prediction_frozen_owned_values_current_cancellation_and_foreign_frozen() {
    let b = binding(1, SplitRole::Validation);
    let query = query(b);
    let trace = trace(Truth::True);
    let mut p = q(1, 2);
    let zero = q(0, 1);
    let one = q(1, 1);
    let cancel = Cancellation::default();
    let (_br, _run, res) = run(cancel.clone(), b);
    let view = res.lease();
    let frozen = {
        let input = Forecast {
            canonical_sample: b"a",
            baseline: &zero,
            candidate: &p,
            weight: &one,
            query: &query,
            trace: &trace,
        };
        freeze(
            b,
            &[input],
            ForecastLimits {
                max_node_steps: 1024 * 256,
            },
            &view,
        )
        .unwrap()
    };
    p = q(1, 1);
    assert_eq!(p, q(1, 1));
    let owner = evaluate(&frozen, &view).unwrap();
    assert_eq!(
        owner.historical_report().state,
        ComponentState::Measured(RationalInterval::exact_ratio(3, 4).unwrap())
    );
    let input = Forecast {
        canonical_sample: b"a",
        baseline: &zero,
        candidate: &p,
        weight: &one,
        query: &query,
        trace: &trace,
    };
    let other = freeze(
        b,
        &[input],
        ForecastLimits {
            max_node_steps: 1024 * 256,
        },
        &view,
    )
    .unwrap();
    assert!(matches!(
        owner.report_current(&other, &view),
        Err(Error::BindingMismatch)
    ));
    cancel.cancel();
    assert!(owner.report_current(&frozen, &view).is_err());
    assert!(matches!(
        owner.historical_report().state,
        ComponentState::Measured(_)
    ));
}
