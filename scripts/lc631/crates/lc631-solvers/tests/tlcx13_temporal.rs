use lc631_solvers::temporal::*;
use lc631_substrate::context_identity::{ContextRevision, SourceRevision};
use lc631_substrate::Digest32;

fn clock() -> ClockSpec {
    ClockSpec::exact(
        Digest32::from_bytes([1; 32]),
        TimeScale::Monotonic,
        Digest32::from_bytes([2; 32]),
        Digest32::from_bytes([3; 32]),
    )
}

fn instant(value: i64) -> Instant {
    Instant::absolute(clock(), value, TimeUnit::Second).unwrap()
}

fn trace(rows: Vec<Vec<Truth>>) -> FiniteTrace {
    let steps = rows
        .into_iter()
        .enumerate()
        .map(|(i, values)| TraceStep::new(instant(i as i64), values))
        .collect();
    FiniteTrace::checked(steps, TraceClosure::Closed).unwrap()
}

fn query(nodes: Vec<TemporalNode>, root: u16) -> TemporalQuery {
    TemporalQuery::checked(
        ContextRevision::original(SourceRevision::of("temporal")),
        nodes,
        root,
        TemporalProfile::ClosedFiniteKleeneV1,
    )
    .unwrap()
}

#[test]
fn tlcx13_negative_empty_reversed_and_loop_traces() {
    assert_eq!(
        FiniteTrace::checked(vec![], TraceClosure::Closed).unwrap_err(),
        TemporalError::EmptyTrace
    );
    let steps = vec![
        TraceStep::new(instant(2), vec![Truth::True]),
        TraceStep::new(instant(1), vec![Truth::True]),
    ];
    assert_eq!(
        FiniteTrace::checked(steps, TraceClosure::Closed).unwrap_err(),
        TemporalError::NonIncreasingTime
    );
    let steps = vec![TraceStep::new(instant(0), vec![Truth::True])];
    assert_eq!(
        FiniteTrace::checked(steps, TraceClosure::LoopTo(0)).unwrap_err(),
        TemporalError::UnsupportedLoop
    );
}

#[test]
fn tlcx13_negative_future_guarantee_and_unknown_are_not_true() {
    let q = query(vec![TemporalNode::Atom(0), TemporalNode::StrongNext(0)], 1);
    let t = trace(vec![vec![Truth::True]]);
    let result = evaluate(&q, &t, EvaluationBudget::default()).unwrap();
    assert_eq!(result.truth(), Truth::False);
    assert_eq!(result.counterexample().unwrap().steps().len(), 1);
    let q = query(vec![TemporalNode::Atom(0), TemporalNode::Always(0)], 1);
    assert_eq!(
        evaluate(
            &q,
            &trace(vec![vec![Truth::Unknown]]),
            EvaluationBudget::default()
        )
        .unwrap()
        .truth(),
        Truth::Unknown
    );
}

#[test]
fn tlcx13_negative_wrong_clock_and_interval_boundaries() {
    let wrong = ClockSpec::exact(
        Digest32::from_bytes([9; 32]),
        TimeScale::Monotonic,
        Digest32::from_bytes([2; 32]),
        Digest32::from_bytes([3; 32]),
    );
    let other = Instant::absolute(wrong, 2, TimeUnit::Second).unwrap();
    assert_eq!(
        Interval::checked(instant(0), other).unwrap_err(),
        TemporalError::ClockMismatch
    );
    assert_eq!(
        Interval::checked(instant(1), instant(1)).unwrap_err(),
        TemporalError::EmptyOrReversedInterval
    );
    let a = Interval::checked(instant(0), instant(1)).unwrap();
    let b = Interval::checked(instant(1), instant(2)).unwrap();
    assert!(!a.before(b).unwrap());
    assert!(a.meets(b).unwrap());
    assert!(!b.after(a).unwrap());
}

#[test]
fn tlcx13_negative_foreign_forward_node_and_budget() {
    let context = ContextRevision::original(SourceRevision::of("temporal"));
    assert_eq!(
        TemporalQuery::checked(
            context,
            vec![TemporalNode::Not(0)],
            0,
            TemporalProfile::ClosedFiniteKleeneV1
        )
        .unwrap_err(),
        TemporalError::InvalidNodeReference
    );
    let q = query(vec![TemporalNode::Atom(0)], 0);
    assert_eq!(
        evaluate(
            &q,
            &trace(vec![vec![Truth::True]]),
            EvaluationBudget::checked(0).unwrap()
        )
        .unwrap_err(),
        TemporalError::ResourceExhausted
    );
    assert_eq!(
        evaluate(&q, &trace(vec![vec![]]), EvaluationBudget::default()).unwrap_err(),
        TemporalError::MissingAtom
    );
}

#[test]
fn tlcx13_positive_absolute_relative_and_unit_conversion() {
    let base = instant(10);
    let relative = Instant::relative(base, 500, TimeUnit::Millisecond).unwrap();
    let absolute = Instant::absolute(clock(), 10_500, TimeUnit::Millisecond).unwrap();
    assert_eq!(relative, absolute);
    let duration = Interval::checked(base, relative)
        .unwrap()
        .duration_nanoseconds()
        .unwrap();
    assert_eq!(duration, 500_000_000);
    assert_eq!(
        Instant::absolute(clock(), i64::MAX, TimeUnit::Second)
            .unwrap()
            .nanoseconds(),
        i128::from(i64::MAX) * 1_000_000_000
    );
}

#[test]
fn tlcx13_positive_strong_until_endpoint_and_full_counterexample() {
    let q = query(
        vec![
            TemporalNode::Atom(0),
            TemporalNode::Atom(1),
            TemporalNode::Until { left: 0, right: 1 },
        ],
        2,
    );
    let good = trace(vec![
        vec![Truth::True, Truth::False],
        vec![Truth::False, Truth::True],
    ]);
    assert_eq!(
        evaluate(&q, &good, EvaluationBudget::default())
            .unwrap()
            .truth(),
        Truth::True
    );
    let bad = trace(vec![
        vec![Truth::True, Truth::False],
        vec![Truth::True, Truth::False],
    ]);
    let result = evaluate(&q, &bad, EvaluationBudget::default()).unwrap();
    assert_eq!(result.truth(), Truth::False);
    assert_eq!(result.counterexample().unwrap(), &bad);
    assert_eq!(
        result.logical_result(),
        lc631_solvers::LogicalResult::ModelCheck {
            support: lc631_solvers::Support::Refuted,
            traces: lc631_solvers::TraceDomain::Finite
        }
    );
    assert_eq!(result.context(), q.context());
    assert_eq!(result.query_digest(), q.digest());
    assert_eq!(result.trace_digest(), bad.digest());
}

#[test]
fn tlcx13_independent_exhaustive_until_oracle() {
    // Independent existential-prefix definition; production uses reverse recurrence.
    let q = query(
        vec![
            TemporalNode::Atom(0),
            TemporalNode::Atom(1),
            TemporalNode::Until { left: 0, right: 1 },
        ],
        2,
    );
    for length in 1..=4 {
        for assignment in 0..(1u32 << (length * 2)) {
            let rows: Vec<Vec<Truth>> = (0..length)
                .map(|i| {
                    vec![
                        if assignment & (1 << (2 * i)) != 0 {
                            Truth::True
                        } else {
                            Truth::False
                        },
                        if assignment & (1 << (2 * i + 1)) != 0 {
                            Truth::True
                        } else {
                            Truth::False
                        },
                    ]
                })
                .collect();
            let expected = (0..length)
                .any(|j| rows[j][1] == Truth::True && (0..j).all(|k| rows[k][0] == Truth::True));
            let result = evaluate(&q, &trace(rows), EvaluationBudget::default()).unwrap();
            assert_eq!(
                result.truth(),
                if expected { Truth::True } else { Truth::False }
            );
        }
    }
}

#[test]
fn tlcx13_negative_unsupported_profiles_precision_and_open_prefix() {
    let steps = vec![TraceStep::new(instant(0), vec![Truth::True])];
    assert_eq!(
        FiniteTrace::checked(steps, TraceClosure::OpenPrefix).unwrap_err(),
        TemporalError::UnsupportedOpenPrefix
    );
    let context = ContextRevision::original(SourceRevision::of("temporal"));
    assert_eq!(
        TemporalQuery::checked(
            context,
            vec![TemporalNode::Constant(true)],
            0,
            TemporalProfile::InfiniteLtl
        )
        .unwrap_err(),
        TemporalError::UnsupportedInfiniteProfile
    );
    assert_eq!(
        ClockSpec::checked(
            Digest32::from_bytes([1; 32]),
            TimeScale::Monotonic,
            Digest32::from_bytes([2; 32]),
            Digest32::from_bytes([3; 32]),
            1
        )
        .unwrap_err(),
        TemporalError::UnsupportedPrecision
    );
}

#[test]
fn tlcx13_positive_kleene_not_and_or_and_suffix_operators() {
    // Direct truth-table goldens, not generated through production operations.
    let states = [Truth::False, Truth::Unknown, Truth::True];
    let and = [
        Truth::False,
        Truth::False,
        Truth::False,
        Truth::False,
        Truth::Unknown,
        Truth::Unknown,
        Truth::False,
        Truth::Unknown,
        Truth::True,
    ];
    let or = [
        Truth::False,
        Truth::Unknown,
        Truth::True,
        Truth::Unknown,
        Truth::Unknown,
        Truth::True,
        Truth::True,
        Truth::True,
        Truth::True,
    ];
    for (i, a) in states.iter().enumerate() {
        for (j, b) in states.iter().enumerate() {
            let t = trace(vec![vec![*a, *b]]);
            for (node, expected) in [
                (TemporalNode::And(0, 1), and[i * 3 + j]),
                (TemporalNode::Or(0, 1), or[i * 3 + j]),
            ] {
                let q = query(vec![TemporalNode::Atom(0), TemporalNode::Atom(1), node], 2);
                assert_eq!(
                    evaluate(&q, &t, EvaluationBudget::default())
                        .unwrap()
                        .truth(),
                    expected
                );
            }
        }
        let q = query(vec![TemporalNode::Atom(0), TemporalNode::Not(0)], 1);
        let expected = [Truth::True, Truth::Unknown, Truth::False][i];
        assert_eq!(
            evaluate(&q, &trace(vec![vec![*a]]), EvaluationBudget::default())
                .unwrap()
                .truth(),
            expected
        );
    }
    let t = trace(vec![vec![Truth::False], vec![Truth::True]]);
    let q = query(vec![TemporalNode::Atom(0), TemporalNode::Eventually(0)], 1);
    assert_eq!(
        evaluate(&q, &t, EvaluationBudget::default())
            .unwrap()
            .truth(),
        Truth::True
    );
    let q = query(vec![TemporalNode::Atom(0), TemporalNode::Always(0)], 1);
    assert_eq!(
        evaluate(&q, &t, EvaluationBudget::default())
            .unwrap()
            .truth(),
        Truth::False
    );
}

#[test]
fn tlcx13_negative_caps_and_nonuniform_trace() {
    assert_eq!(
        FiniteTrace::checked(
            vec![TraceStep::new(instant(0), vec![Truth::True; 257])],
            TraceClosure::Closed
        )
        .unwrap_err(),
        TemporalError::AtomLimitExceeded
    );
    let steps = vec![
        TraceStep::new(instant(0), vec![Truth::True]),
        TraceStep::new(instant(1), vec![]),
    ];
    assert_eq!(
        FiniteTrace::checked(steps, TraceClosure::Closed).unwrap_err(),
        TemporalError::InconsistentAtomWidth
    );
    let steps = (0..257)
        .map(|i| TraceStep::new(instant(i), vec![]))
        .collect();
    assert_eq!(
        FiniteTrace::checked(steps, TraceClosure::Closed).unwrap_err(),
        TemporalError::TraceLimitExceeded
    );
    assert_eq!(
        EvaluationBudget::checked(1024 * 256 + 1).unwrap_err(),
        TemporalError::InvalidBudget
    );
}

#[test]
fn tlcx13_positive_exact_budget_and_source_context_binding() {
    let q = query(vec![TemporalNode::Constant(true)], 0);
    let t = trace(vec![vec![]]);
    let r = evaluate(&q, &t, EvaluationBudget::checked(1).unwrap()).unwrap();
    assert_eq!(r.truth(), Truth::True);
    assert_eq!(r.node_steps(), 1);
    assert_eq!(r.source(), q.context().source());
    assert!(r.counterexample().is_none());
    let alternate = TemporalQuery::checked(
        q.context().reconstructed([9; 32]),
        vec![TemporalNode::Constant(true)],
        0,
        TemporalProfile::ClosedFiniteKleeneV1,
    )
    .unwrap();
    let other = evaluate(&alternate, &t, EvaluationBudget::default()).unwrap();
    // Interpretation identity is part of the query hash, not just the report.
    assert_ne!(r.context(), other.context());
    assert_ne!(r.query_digest(), other.query_digest());
}

#[test]
fn tlcx13_positive_shift_and_unit_invariance_with_trace_invalidation() {
    let values = vec![vec![Truth::False], vec![Truth::True]];
    let original = trace(values.clone());
    let shifted = FiniteTrace::checked(
        values
            .into_iter()
            .enumerate()
            .map(|(i, v)| {
                TraceStep::new(
                    Instant::absolute(clock(), 100_000 + i as i64 * 1_000, TimeUnit::Millisecond)
                        .unwrap(),
                    v,
                )
            })
            .collect(),
        TraceClosure::Closed,
    )
    .unwrap();
    let q = query(vec![TemporalNode::Atom(0), TemporalNode::Eventually(0)], 1);
    let a = evaluate(&q, &original, EvaluationBudget::default()).unwrap();
    let b = evaluate(&q, &shifted, EvaluationBudget::default()).unwrap();
    assert_eq!(a.truth(), b.truth());
    assert_ne!(a.trace_digest(), b.trace_digest());
    assert_eq!(
        Interval::checked(instant(0), instant(1))
            .unwrap()
            .duration_nanoseconds()
            .unwrap(),
        Interval::checked(shifted.steps()[0].instant(), shifted.steps()[1].instant())
            .unwrap()
            .duration_nanoseconds()
            .unwrap()
    );
}
