use lc631_core::measurement_numeric::NumericEvaluation;
use lc631_world::curved_exploration::arithmetic::{point, Interval, Rational};
use lc631_world::curved_exploration::embedding::*;
use lc631_world::curved_exploration::graph::{DirectedTransitionCost, NodeId, ReportedTransition};
use lc631_world::curved_exploration::{
    embedding_readiness, BallPolicy, GeometryUnavailable, ReportedEmbeddingPins,
};

fn q(n: i64, d: i64) -> Rational {
    Rational::new(n.into(), d.into())
}
fn registration(root: NodeId) -> Registration {
    Registration::checked(
        root,
        BallPolicy::new(q(1, 1), q(1, 4), q(1, 1), 1).unwrap(),
        ([1; 32], [2; 32]),
        1_000_000,
    )
    .unwrap()
}
fn nodes(ids: [u64; 4]) -> [NodeBinding<'static>; 4] {
    [
        NodeBinding {
            id: NodeId(ids[0]),
            canonical_content: b"root",
        },
        NodeBinding {
            id: NodeId(ids[1]),
            canonical_content: b"branch-a",
        },
        NodeBinding {
            id: NodeId(ids[2]),
            canonical_content: b"depth-two",
        },
        NodeBinding {
            id: NodeId(ids[3]),
            canonical_content: b"branch-c",
        },
    ]
}
fn edge(a: NodeId, b: NodeId, evidence: u8) -> ReportedTransition {
    ReportedTransition {
        from: a,
        to: b,
        cost: DirectedTransitionCost::query_slots(1),
        evidence_payload_digest: [evidence; 32],
    }
}
fn edges(ids: [u64; 4]) -> [ReportedTransition; 3] {
    [
        edge(NodeId(ids[0]), NodeId(ids[1]), 3),
        edge(NodeId(ids[1]), NodeId(ids[2]), 4),
        edge(NodeId(ids[0]), NodeId(ids[3]), 5),
    ]
}
fn heldout(ids: [u64; 4]) -> [HeldoutRelation; 2] {
    [
        HeldoutRelation {
            from: NodeId(ids[0]),
            to: NodeId(ids[2]),
            expected: LayerRelation::Less,
            evidence_payload_digest: [6; 32],
        },
        HeldoutRelation {
            from: NodeId(ids[1]),
            to: NodeId(ids[3]),
            expected: LayerRelation::Equal,
            evidence_payload_digest: [7; 32],
        },
    ]
}
fn checked<'a>(
    reg: &Registration,
    n: &'a [NodeBinding<'a>],
    e: &[ReportedTransition],
    h: &[HeldoutRelation],
) -> CheckedEmbedding<'a> {
    match produce(reg, n, e, h).unwrap() {
        EmbeddingAttempt::Checked(value) => value,
        EmbeddingAttempt::Pending(reason) => panic!("expected bounded checked profile: {reason:?}"),
    }
}

#[test]
fn tlcx24_embedding_manual_depth_coordinates_and_actual_k8_connection() {
    let ids = [1, 2, 3, 4];
    let n = nodes(ids);
    let e = edges(ids);
    let h = heldout(ids);
    let reg = registration(NodeId(1));
    let value = checked(&reg, &n, &e, &h);
    // Hand arithmetic: r_max=3/16, depths 0,1,2,1 -> 0,3/32,1/8,3/32.
    for (id, expected) in [(1, q(0, 1)), (2, q(3, 32)), (3, q(1, 8)), (4, q(3, 32))] {
        assert_eq!(
            value.coordinate(NodeId(id)).unwrap(),
            &point(expected).unwrap()
        );
    }
    assert_eq!(value.heldout_count(), 2);
    assert_eq!(value.mapping_basis(), MappingBasis::GivenCanonicalContent);
    assert_eq!(
        value.evidence_status(),
        EvidenceStatus::ReportedUnauthenticated
    );
    assert_eq!(reg.dimension(), 1);
    assert_eq!(reg.seed(), 0);
    assert_eq!(reg.algorithm(), Algorithm::RootedShortestLayerRadialV1);
    let self_distance = value.prepare_distance(NodeId(2), NodeId(2)).unwrap();
    assert_eq!(
        self_distance.t_enclosure(),
        &Interval::exact_ratio(0, 1).unwrap()
    );
    assert_eq!(
        self_distance.distance().unwrap(),
        NumericEvaluation::Complete(Interval::exact_ratio(0, 1).unwrap())
    );
    let distance = value
        .prepare_distance(NodeId(1), NodeId(2))
        .unwrap()
        .distance()
        .unwrap();
    match distance {
        NumericEvaluation::Complete(bounds) => assert!(bounds.lower() > &q(0, 1)),
        other => panic!("bounded small reference distance expected complete: {other:?}"),
    }
}

#[test]
fn tlcx24_embedding_ids_and_input_order_do_not_change_coordinates_or_pins() {
    let a = [1, 2, 3, 4];
    let b = [91, 7, 500, 22];
    let na = nodes(a);
    let mut nb = nodes(b);
    let ea = edges(a);
    let mut eb = edges(b);
    let ha = heldout(a);
    let mut hb = heldout(b);
    nb.reverse();
    eb.reverse();
    hb.reverse();
    let first = checked(&registration(NodeId(a[0])), &na, &ea, &ha);
    let second = checked(&registration(NodeId(b[0])), &nb, &eb, &hb);
    assert_eq!(first.pins(), second.pins());
    for (left, right) in a.into_iter().zip(b) {
        assert_eq!(
            first.coordinate(NodeId(left)),
            second.coordinate(NodeId(right))
        );
    }
}

#[test]
fn tlcx24_embedding_graph_shortcut_changes_actual_coordinate_and_snapshot() {
    let ids = [1, 2, 3, 4];
    let n = nodes(ids);
    let e = edges(ids);
    let all = heldout(ids);
    let h = &all[1..]; // sibling equality remains disjoint from both training graphs.
    let reg = registration(NodeId(1));
    let before = checked(&reg, &n, &e, h);
    let mut changed = e.to_vec();
    changed.push(edge(NodeId(1), NodeId(3), 8));
    let after = checked(&reg, &n, &changed, h);
    assert_ne!(before.coordinate(NodeId(3)), after.coordinate(NodeId(3)));
    assert_ne!(before.pins().graph_snapshot, after.pins().graph_snapshot);
    assert_eq!(
        after.coordinate(NodeId(3)).unwrap(),
        &point(q(3, 32)).unwrap()
    );
}

#[test]
fn tlcx24_embedding_cycles_and_root_disconnection_remain_unsupported() {
    let ids = [1, 2, 3, 4];
    let n = nodes(ids);
    let e = edges(ids);
    let h = heldout(ids);
    let reg = registration(NodeId(1));
    let mut cycle = e.to_vec();
    cycle.push(edge(NodeId(3), NodeId(2), 8));
    assert!(matches!(
        produce(&reg, &n, &cycle, &h).unwrap(),
        EmbeddingAttempt::Pending(PendingReason::UnsupportedCycle)
    ));
    assert!(matches!(
        produce(&reg, &n, &e[..2], &h).unwrap(),
        EmbeddingAttempt::Pending(PendingReason::UnsupportedDisconnected)
    ));
    assert!(matches!(
        produce(&registration(NodeId(99)), &n, &e, &h),
        Err(EmbeddingError::UnknownNode)
    ));
}

#[test]
fn tlcx24_embedding_missing_failed_duplicate_and_training_heldout_are_not_checked() {
    let ids = [1, 2, 3, 4];
    let n = nodes(ids);
    let e = edges(ids);
    let h = heldout(ids);
    let reg = registration(NodeId(1));
    assert!(matches!(
        produce(&reg, &n, &e, &[]).unwrap(),
        EmbeddingAttempt::Pending(PendingReason::MissingHeldoutChecks)
    ));
    let mut bad = h;
    bad[0].expected = LayerRelation::Greater;
    assert!(matches!(
        produce(&reg, &n, &e, &bad).unwrap(),
        EmbeddingAttempt::Pending(PendingReason::HeldoutFailed { .. })
    ));
    assert!(matches!(
        produce(&reg, &n, &e, &[h[0], h[0]]),
        Err(EmbeddingError::DuplicateHeldoutPair)
    ));
    let trained = [HeldoutRelation {
        from: NodeId(1),
        to: NodeId(2),
        ..h[0]
    }];
    assert!(matches!(
        produce(&reg, &n, &e, &trained),
        Err(EmbeddingError::HeldoutTrainingOverlap)
    ));
    let reversed = [HeldoutRelation {
        from: NodeId(2),
        to: NodeId(1),
        ..h[0]
    }];
    assert!(matches!(
        produce(&reg, &n, &e, &reversed),
        Err(EmbeddingError::HeldoutTrainingOverlap)
    ));
    let missing = [HeldoutRelation {
        evidence_payload_digest: [0; 32],
        ..h[0]
    }];
    assert!(matches!(
        produce(&reg, &n, &e, &missing),
        Err(EmbeddingError::MissingEvidence)
    ));
}

#[test]
fn tlcx24_embedding_alias_missing_edge_evidence_and_budget_are_detected() {
    let ids = [1, 2, 3, 4];
    let mut n = nodes(ids);
    let e = edges(ids);
    let h = heldout(ids);
    let reg = registration(NodeId(1));
    n[1].canonical_content = n[0].canonical_content;
    assert!(matches!(
        produce(&reg, &n, &e, &h),
        Err(EmbeddingError::DuplicateContent)
    ));
    let n = nodes(ids);
    let mut missing = e;
    missing[0].evidence_payload_digest = [0; 32];
    assert!(matches!(
        produce(&reg, &n, &missing, &h),
        Err(EmbeddingError::Graph(_))
    ));
    let tiny = Registration::checked(
        NodeId(1),
        BallPolicy::new(q(1, 1), q(1, 4), q(1, 1), 1).unwrap(),
        ([1; 32], [2; 32]),
        1,
    )
    .unwrap();
    assert!(matches!(
        produce(&tiny, &n, &e, &h).unwrap(),
        EmbeddingAttempt::Pending(PendingReason::WorkBudget)
    ));
    assert!(matches!(
        Registration::checked(
            NodeId(1),
            BallPolicy::new(q(1, 1), q(1, 4), q(1, 1), 1).unwrap(),
            ([0; 32], [2; 32]),
            1_000_000
        ),
        Err(EmbeddingError::MissingEvidence)
    ));
}

#[test]
fn tlcx24_embedding_policy_and_evidence_changes_invalidate_pins() {
    let ids = [1, 2, 3, 4];
    let n = nodes(ids);
    let e = edges(ids);
    let h = heldout(ids);
    let reg = registration(NodeId(1));
    let a = checked(&reg, &n, &e, &h);
    let changed = Registration::checked(
        NodeId(1),
        BallPolicy::new(q(1, 1), q(1, 4), q(2, 1), 1).unwrap(),
        ([1; 32], [2; 32]),
        1_000_000,
    )
    .unwrap();
    let b = checked(&changed, &n, &e, &h);
    assert_ne!(a.pins().policy, b.pins().policy);
    assert_eq!(a.coordinate(NodeId(3)), b.coordinate(NodeId(3)));
    let mut altered = h;
    altered[0].evidence_payload_digest = [9; 32];
    let c = checked(&reg, &n, &e, &altered);
    assert_ne!(a.pins().heldout_checks, c.pins().heldout_checks);
    let pins = ReportedEmbeddingPins {
        transition_snapshot: [1; 32],
        edge_evidence: [1; 32],
        feature_correspondence: [1; 32],
        objective: [1; 32],
        initialization: [1; 32],
        optimizer_version_seed: [1; 32],
        stopping_budget: [1; 32],
        held_out_checks: [1; 32],
        dimension_curvature_scale_margin_coordinates: [1; 32],
    };
    assert_eq!(
        embedding_readiness(Some(&pins)),
        GeometryUnavailable::EmbeddingPolicyUnverified
    );
}
