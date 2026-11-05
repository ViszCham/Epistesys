mod tlcx22_prediction_support;
use lc631_core::{
    context::SourceRevision,
    measurement_numeric::{BigInt, Rational},
    CandidateId,
};
use lc631_runtime::prediction_measurement::*;
use lc631_solvers::temporal::{TemporalError, TemporalNode, TemporalProfile, TemporalQuery, Truth};
use lc631_substrate::resource::Cancellation;
use tlcx22_prediction_support::*;
#[test]
fn tlcx22_prediction_source_query_and_missing_atom_are_not_false_labels() {
    let b = binding(1, SplitRole::Validation);
    let query = query(b);
    let t = trace(Truth::True);
    let z = q(0, 1);
    let p = q(1, 2);
    let w = q(1, 1);
    let (_br, _run, res) = run(Cancellation::default(), b);
    let view = res.lease();
    let limits = ForecastLimits {
        max_node_steps: 1024 * 256,
    };
    let input = Forecast {
        canonical_sample: b"sample",
        baseline: &z,
        candidate: &p,
        weight: &w,
        query: &query,
        trace: &t,
    };
    let mut foreign = b;
    foreign.selection.source = SourceRevision::of("different source");
    assert!(matches!(
        freeze(foreign, &[input], limits, &view),
        Err(Error::Resource(_))
    ));
    let changed = TemporalQuery::checked(
        b.context.reconstructed([99; 32]),
        vec![TemporalNode::Atom(0)],
        0,
        TemporalProfile::ClosedFiniteKleeneV1,
    )
    .unwrap();
    let input = Forecast {
        query: &changed,
        ..input
    };
    assert!(matches!(
        freeze(b, &[input], limits, &view),
        Err(Error::BindingMismatch)
    ));
    let missing = TemporalQuery::checked(
        b.context,
        vec![TemporalNode::Atom(1)],
        0,
        TemporalProfile::ClosedFiniteKleeneV1,
    )
    .unwrap();
    let input = Forecast {
        query: &missing,
        ..input
    };
    let frozen = freeze(b, &[input], limits, &view).unwrap();
    assert!(matches!(
        evaluate(&frozen, &view),
        Err(Error::LabelEvaluation(TemporalError::MissingAtom))
    ));
}
#[test]
fn tlcx22_prediction_canonical_numeric_limits_and_opaque_candidate_relabel() {
    let b = binding(1, SplitRole::Validation);
    let query = query(b);
    let t = trace(Truth::True);
    let z = q(0, 1);
    let p = q(1, 2);
    let w = q(1, 1);
    let (_br, _run, res) = run(Cancellation::default(), b);
    let view = res.lease();
    let limits = ForecastLimits {
        max_node_steps: 1024 * 256,
    };
    let input = Forecast {
        canonical_sample: b"sample",
        baseline: &z,
        candidate: &p,
        weight: &w,
        query: &query,
        trace: &t,
    };
    for bad in [
        Rational::new_raw(1.into(), (-2).into()),
        Rational::new_raw(BigInt::from(1) << 65537usize, 1.into()),
    ] {
        let raw = Forecast {
            baseline: &bad,
            ..input
        };
        assert!(matches!(
            freeze(b, &[raw], limits, &view),
            Err(Error::Numeric(_))
        ));
    }
    assert!(matches!(
        freeze(
            b,
            &[input],
            ForecastLimits {
                max_node_steps: 1024 * 256 + 1
            },
            &view
        ),
        Err(Error::LabelEvaluation(TemporalError::InvalidBudget))
    ));
    let a = freeze(b, &[input], limits, &view).unwrap();
    let mut relabel = b;
    relabel.candidate = CandidateId(987654);
    let c = freeze(relabel, &[input], limits, &view).unwrap();
    assert_eq!(a.historical_digest(), c.historical_digest());
    let oa = evaluate(&a, &view).unwrap();
    let oc = evaluate(&c, &view).unwrap();
    assert_eq!(oa.historical_report(), oc.historical_report());
}

#[test]
fn tlcx22_prediction_actual_not_query_controls_label_not_raw_atom() {
    let b = binding(1, SplitRole::Validation);
    let query = TemporalQuery::checked(
        b.context,
        vec![TemporalNode::Atom(0), TemporalNode::Not(0)],
        1,
        TemporalProfile::ClosedFiniteKleeneV1,
    )
    .unwrap();
    let t = trace(Truth::True);
    let baseline = q(0, 1);
    let candidate = q(1, 2);
    let weight = q(1, 1);
    let input = Forecast {
        canonical_sample: b"Not(True)=False",
        baseline: &baseline,
        candidate: &candidate,
        weight: &weight,
        query: &query,
        trace: &t,
    };
    let (_br, _run, res) = run(Cancellation::default(), b);
    let view = res.lease();
    let frozen = freeze(b, &[input], ForecastLimits { max_node_steps: 2 }, &view).unwrap();
    let owner = evaluate(&frozen, &view).unwrap();
    // y=False: baseline error0 minus candidate error1/4 = -1/4.
    assert_eq!(
        owner.historical_labels(),
        &[LabelState::Observed(Truth::False)]
    );
    assert_eq!(
        owner.historical_report().state,
        lc631_analysis::creativity_measurement::ComponentState::Measured(
            lc631_core::measurement_numeric::RationalInterval::exact_ratio(-1, 4).unwrap()
        )
    );
}
