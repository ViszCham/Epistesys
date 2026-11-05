//! 日本語: 閉有限profileの未検査境界を独立定義で照合する。
//! English: Check additional closed-finite boundaries against an independent definition.
use lc631_solvers::temporal::*;
use lc631_solvers::{LogicalResult, Support, TraceDomain};
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

fn trace(rows: &[Vec<Truth>]) -> FiniteTrace {
    FiniteTrace::checked(
        rows.iter()
            .enumerate()
            .map(|(index, values)| {
                TraceStep::new(
                    Instant::absolute(clock(), index as i64, TimeUnit::Second).unwrap(),
                    values.clone(),
                )
            })
            .collect(),
        TraceClosure::Closed,
    )
    .unwrap()
}

fn query(nodes: Vec<TemporalNode>, root: u16) -> TemporalQuery {
    TemporalQuery::checked(
        ContextRevision::original(SourceRevision::of("balanced finite temporal")),
        nodes,
        root,
        TemporalProfile::ClosedFiniteKleeneV1,
    )
    .unwrap()
}

#[test]
fn tlcx13_balanced_negative_unknown_until_is_not_a_boolean_default() {
    let q = query(
        vec![
            TemporalNode::Atom(0),
            TemporalNode::Atom(1),
            TemporalNode::Until { left: 0, right: 1 },
        ],
        2,
    );
    let cases = [
        (vec![vec![Truth::True, Truth::Unknown]], Truth::Unknown),
        (vec![vec![Truth::Unknown, Truth::False]], Truth::False),
        (
            vec![
                vec![Truth::Unknown, Truth::False],
                vec![Truth::False, Truth::True],
            ],
            Truth::Unknown,
        ),
        (
            vec![
                vec![Truth::False, Truth::Unknown],
                vec![Truth::True, Truth::True],
            ],
            Truth::Unknown,
        ),
        (vec![vec![Truth::Unknown, Truth::True]], Truth::True),
    ];
    for (rows, expected) in cases {
        let t = trace(&rows);
        let r = evaluate(&q, &t, EvaluationBudget::default()).unwrap();
        assert_eq!(r.truth(), expected);
        assert_eq!(r.counterexample().is_some(), expected == Truth::False);
        if expected == Truth::Unknown {
            assert_eq!(
                r.logical_result(),
                LogicalResult::ModelCheck {
                    support: Support::Neither,
                    traces: TraceDomain::Finite,
                }
            );
        }
    }
}

#[test]
fn tlcx13_balanced_exhaustive_ternary_until_uses_witness_prefix_oracle() {
    let states = [Truth::False, Truth::Unknown, Truth::True];
    let q = query(
        vec![
            TemporalNode::Atom(0),
            TemporalNode::Atom(1),
            TemporalNode::Until { left: 0, right: 1 },
        ],
        2,
    );
    let mut checked = 0;
    for length in 1..=3_usize {
        for assignment in 0..3_usize.pow((2 * length) as u32) {
            let mut digits = assignment;
            let mut ranks = Vec::new();
            let rows: Vec<Vec<Truth>> = (0..length)
                .map(|_| {
                    let left = digits % 3;
                    digits /= 3;
                    let right = digits % 3;
                    digits /= 3;
                    ranks.push([left, right]);
                    vec![states[left], states[right]]
                })
                .collect();
            // Independent finite disjunction of witness/prefix conjunctions.
            // Production instead fills each Until suffix by reverse recurrence.
            let expected_rank = (0..length)
                .map(|witness| {
                    (0..witness)
                        .map(|prior| ranks[prior][0])
                        .fold(ranks[witness][1], usize::min)
                })
                .max()
                .unwrap();
            let result = evaluate(&q, &trace(&rows), EvaluationBudget::default()).unwrap();
            assert_eq!(result.truth(), states[expected_rank]);
            checked += 1;
        }
    }
    assert_eq!(checked, 819);
}

#[test]
fn tlcx13_balanced_nested_strong_next_exposes_closed_trace_endpoint() {
    let t = trace(&[vec![Truth::True], vec![Truth::True], vec![Truth::True]]);
    let globally_next = query(
        vec![
            TemporalNode::Atom(0),
            TemporalNode::StrongNext(0),
            TemporalNode::Always(1),
        ],
        2,
    );
    let r = evaluate(&globally_next, &t, EvaluationBudget::checked(9).unwrap()).unwrap();
    assert_eq!(r.truth(), Truth::False);
    assert_eq!(r.counterexample().unwrap(), &t);
    assert_eq!(r.evaluated_at(), 0);
    assert_eq!(r.node_steps(), 9);
    assert_eq!(
        evaluate(&globally_next, &t, EvaluationBudget::checked(8).unwrap()).unwrap_err(),
        TemporalError::ResourceExhausted
    );
    let three_next = query(
        vec![
            TemporalNode::Atom(0),
            TemporalNode::StrongNext(0),
            TemporalNode::StrongNext(1),
            TemporalNode::StrongNext(2),
        ],
        3,
    );
    assert_eq!(
        evaluate(&three_next, &t, EvaluationBudget::default())
            .unwrap()
            .truth(),
        Truth::False
    );
}

#[test]
fn tlcx13_balanced_negative_all_clock_dimensions_require_explicit_correspondence() {
    let id = |n| Digest32::from_bytes([n; 32]);
    let original = clock();
    let variants = [
        ClockSpec::exact(
            id(9),
            original.scale(),
            original.origin(),
            original.reference(),
        ),
        ClockSpec::exact(
            original.clock(),
            TimeScale::Tai,
            original.origin(),
            original.reference(),
        ),
        ClockSpec::exact(
            original.clock(),
            original.scale(),
            id(9),
            original.reference(),
        ),
        ClockSpec::exact(original.clock(), original.scale(), original.origin(), id(9)),
    ];
    let first = Instant::absolute(original, 0, TimeUnit::Second).unwrap();
    let interval = Interval::checked(
        first,
        Instant::relative(first, 1, TimeUnit::Second).unwrap(),
    )
    .unwrap();
    for variant in variants {
        let other = Instant::absolute(variant, 2, TimeUnit::Second).unwrap();
        assert_eq!(
            Interval::checked(first, other).unwrap_err(),
            TemporalError::ClockMismatch
        );
        let other_interval = Interval::checked(
            other,
            Instant::relative(other, 1, TimeUnit::Second).unwrap(),
        )
        .unwrap();
        assert_eq!(
            interval.before(other_interval).unwrap_err(),
            TemporalError::ClockMismatch
        );
        assert_eq!(
            interval.meets(other_interval).unwrap_err(),
            TemporalError::ClockMismatch
        );
        assert_eq!(
            interval.after(other_interval).unwrap_err(),
            TemporalError::ClockMismatch
        );
    }
}

#[test]
fn tlcx13_balanced_absolute_relative_offsets_do_not_shift_elapsed_duration() {
    let a = Instant::absolute(clock(), 1_000, TimeUnit::Second).unwrap();
    let b = Instant::relative(a, -500, TimeUnit::Millisecond).unwrap();
    assert_eq!(
        b,
        Instant::absolute(clock(), 999_500, TimeUnit::Millisecond).unwrap()
    );
    let shifted_a = Instant::relative(a, 123, TimeUnit::Second).unwrap();
    let shifted_b = Instant::relative(b, 123, TimeUnit::Second).unwrap();
    assert_ne!(b, shifted_b);
    assert_eq!(
        Interval::checked(b, a)
            .unwrap()
            .duration_nanoseconds()
            .unwrap(),
        500_000_000
    );
    assert_eq!(
        Interval::checked(shifted_b, shifted_a)
            .unwrap()
            .duration_nanoseconds()
            .unwrap(),
        500_000_000
    );
    let low = Instant::absolute(clock(), i64::MIN, TimeUnit::Second).unwrap();
    let high = Instant::absolute(clock(), i64::MAX, TimeUnit::Second).unwrap();
    let expected = (i128::from(i64::MAX) - i128::from(i64::MIN)) * 1_000_000_000;
    assert_eq!(
        Interval::checked(low, high)
            .unwrap()
            .duration_nanoseconds()
            .unwrap(),
        expected
    );
    assert_eq!(
        Instant::relative(high, i64::MAX, TimeUnit::Second)
            .unwrap()
            .nanoseconds(),
        i128::from(i64::MAX) * 2 * 1_000_000_000
    );
}
