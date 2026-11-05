//! Authored preparation tests; execution and slice exit remain unobserved.
use lc631_core::measurement_numeric::NumericEvaluation;
use lc631_world::curved_exploration;

use curved_exploration::arithmetic::{Interval, NumericError, Rational};
use curved_exploration::graph::*;
use curved_exploration::*;

fn q(n: i64, d: i64) -> Rational {
    Interval::exact_ratio(n, d).unwrap().lower().clone()
}
fn point(n: i64, d: i64) -> Interval {
    Interval::exact_ratio(n, d).unwrap()
}
fn policy() -> BallPolicy {
    BallPolicy::new(q(1, 1), q(1, 4), q(1, 1), 8).unwrap()
}

#[test]
fn tlcx24_g34_square_crossing_zero_has_nonnegative_lower() {
    let actual = Interval::new(q(-1, 10), q(1, 10))
        .unwrap()
        .square()
        .unwrap();
    assert_eq!(actual, Interval::new(q(0, 1), q(1, 100)).unwrap());
}

#[test]
fn tlcx24_g32_exact_t_is_eight_ninths_and_distance_has_positive_bounds() {
    let prepared = prepare_poincare(
        &policy(),
        &[point(1, 2), point(0, 1)],
        &[point(0, 1), point(1, 2)],
    )
    .unwrap();
    assert_eq!(prepared.t_enclosure(), &point(8, 9));
    assert!(
        matches!(prepared.distance().unwrap(), NumericEvaluation::Complete(bounds)
        if bounds.lower() > &q(0, 1))
    );
    assert_eq!(prepared.curvature(), &q(1, 1));
}

#[test]
fn tlcx24_g21_prepares_one_third_without_fabricating_log_three() {
    let prepared = prepare_poincare(&policy(), &[point(0, 1)], &[point(1, 2)]).unwrap();
    assert_eq!(prepared.t_enclosure(), &point(1, 3));
    assert!(
        matches!(prepared.distance().unwrap(), NumericEvaluation::Complete(bounds)
        if bounds.lower() > &q(1, 1) && bounds.upper() < &q(11, 10))
    );
}

#[test]
fn tlcx24_exact_point_self_distance_zero_and_symmetry() {
    let x = [point(1, 4), point(1, 4)];
    let y = [point(1, 2), point(0, 1)];
    assert_eq!(
        prepare_poincare(&policy(), &x, &x)
            .unwrap()
            .distance()
            .unwrap(),
        NumericEvaluation::Complete(point(0, 1))
    );
    assert_eq!(
        prepare_poincare(&policy(), &x, &y).unwrap().t_enclosure(),
        prepare_poincare(&policy(), &y, &x).unwrap().t_enclosure()
    );
}

#[test]
fn tlcx24_g33_input_uncertainty_remains_an_interval() {
    let x = [Interval::new(q(0, 1), q(1, 10)).unwrap()];
    let prepared = prepare_poincare(&policy(), &x, &[point(1, 2)]).unwrap();
    let lo = prepared.t_enclosure().lower();
    let hi = prepared.t_enclosure().upper();
    // Direct K4 interval enclosure, not the final transcendental-distance golden.
    assert_eq!(lo, &q(16, 75));
    assert_eq!(hi, &q(100, 297));
    assert!(lo < hi);
}

#[test]
fn tlcx24_rejects_ball_boundary_dimensions_and_invalid_configuration() {
    assert_eq!(
        prepare_poincare(&policy(), &[point(1, 1)], &[point(0, 1)]),
        Err(GeometryError::OutsideBallMargin)
    );
    assert_eq!(
        prepare_poincare(&policy(), &[], &[]),
        Err(GeometryError::DimensionMismatch)
    );
    assert_eq!(
        prepare_poincare(&policy(), &[point(0, 1)], &[]),
        Err(GeometryError::DimensionMismatch)
    );
    assert_eq!(
        BallPolicy::new(q(0, 1), q(1, 4), q(1, 1), 1),
        Err(GeometryError::InvalidConfiguration)
    );
    assert_eq!(policy().novelty_scale(), &q(1, 1));
}

#[test]
fn tlcx24_rejects_reversed_and_zero_division_intervals() {
    assert_eq!(
        Interval::new(q(1, 1), q(0, 1)),
        Err(NumericError::ReversedInterval)
    );
    assert_eq!(
        point(1, 1).divide(&Interval::new(q(-1, 1), q(1, 1)).unwrap()),
        Err(NumericError::DivisionByZero)
    );
    assert_eq!(
        Interval::exact_ratio(1, 0),
        Err(NumericError::DivisionByZero)
    );
    assert_eq!(q(1, 2), q(2, 4));
}

#[test]
fn tlcx24_g37_reported_embedding_never_becomes_verified() {
    assert_eq!(
        embedding_readiness(None),
        GeometryUnavailable::EmbeddingPolicyNotRegistered
    );
    let pins = ReportedEmbeddingPins {
        transition_snapshot: [1; 32],
        edge_evidence: [2; 32],
        feature_correspondence: [3; 32],
        objective: [4; 32],
        initialization: [5; 32],
        optimizer_version_seed: [6; 32],
        stopping_budget: [7; 32],
        held_out_checks: [8; 32],
        dimension_curvature_scale_margin_coordinates: [9; 32],
    };
    assert_eq!(
        embedding_readiness(Some(&pins)),
        GeometryUnavailable::EmbeddingPolicyUnverified
    );
    assert_eq!(
        scheduling_readiness(Some(&pins), 1),
        GeometryUnavailable::EmbeddingPolicyUnverified
    );
    assert_eq!(
        scheduling_readiness(None, 0),
        GeometryUnavailable::EmptyVerifiedFrontier
    );
    assert_ne!(
        SchedulerNoveltyPolicyId::default(),
        SchedulerNoveltyPolicyId::NearestVerifiedCandidateV1
    );
    assert_eq!(
        OPERATIONAL_REVISION,
        "OperationalMeasurement/finite-formal-v1.4"
    );
}

fn edge(from: u64, to: u64, slots: u64) -> ReportedTransition {
    ReportedTransition {
        from: NodeId(from),
        to: NodeId(to),
        cost: DirectedTransitionCost::query_slots(slots),
        evidence_payload_digest: [1; 32],
    }
}

#[test]
fn tlcx24_baseline_relabeling_and_direction_are_independent() {
    let a = TransitionGraph::new(
        &[NodeId(1), NodeId(2), NodeId(3)],
        &[edge(1, 2, 4), edge(2, 3, 7)],
    )
    .unwrap();
    let b = TransitionGraph::new(
        &[NodeId(30), NodeId(10), NodeId(20)],
        &[edge(20, 10, 7), edge(30, 20, 4)],
    )
    .unwrap();
    assert_eq!(
        a.baseline_hops(NodeId(1), NodeId(3)).unwrap(),
        b.baseline_hops(NodeId(30), NodeId(10)).unwrap()
    );
    assert_eq!(
        a.baseline_hops(NodeId(3), NodeId(1)).unwrap(),
        BaselineDistance::UnknownNoObservedPath
    );
    assert_eq!(
        a.baseline_hops(NodeId(1), NodeId(1)).unwrap(),
        BaselineDistance::ObservedHops(0)
    );
    assert_eq!(
        a.directed_cost(NodeId(1), NodeId(2))
            .unwrap()
            .unwrap()
            .slots(),
        4
    );
    assert_eq!(a.directed_cost(NodeId(2), NodeId(1)).unwrap(), None);
}

#[test]
fn tlcx24_graph_rejects_empty_alias_missing_evidence_and_overflow_capacity() {
    assert!(matches!(
        TransitionGraph::new(&[], &[]),
        Err(GraphError::EmptyGraph)
    ));
    assert!(matches!(
        TransitionGraph::new(&[NodeId(1), NodeId(1)], &[]),
        Err(GraphError::DuplicateNode)
    ));
    assert!(matches!(
        TransitionGraph::new(&[NodeId(1)], &[edge(1, 2, 1)]),
        Err(GraphError::UnknownNode)
    ));
    assert!(matches!(
        TransitionGraph::new(&[NodeId(1)], &[edge(1, 1, 1), edge(1, 1, 2)]),
        Err(GraphError::DuplicateEdge)
    ));
    let mut missing = edge(1, 1, 1);
    missing.evidence_payload_digest = [0; 32];
    assert!(matches!(
        TransitionGraph::new(&[NodeId(1)], &[missing]),
        Err(GraphError::MissingEvidence)
    ));
    assert!(matches!(
        TransitionGraph::new(&vec![NodeId(1); 2049], &[]),
        Err(GraphError::LimitExceeded)
    ));
}
