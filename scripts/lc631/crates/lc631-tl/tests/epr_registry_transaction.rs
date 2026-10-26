use lc631_tl::{ProjectionDefectGraphV3, SourceLedgerId, TargetClaim, TargetClaimId};

#[test]
fn epr_05_rejected_target_replacement_preserves_the_entire_graph() {
    let mut graph = ProjectionDefectGraphV3::new(SourceLedgerId(1), "source-v1", "source").unwrap();
    let original =
        TargetClaim::checked(TargetClaimId(1), "candidate", "answer", "v1", "A").unwrap();
    graph.add_target_claim(original).unwrap();
    let before = graph.clone();

    let replacement =
        TargetClaim::checked(TargetClaimId(1), "candidate", "answer", "v2", "B").unwrap();
    assert!(graph.add_target_claim(replacement).is_err());
    assert_eq!(
        graph, before,
        "rejection must not replace an existing claim"
    );

    let distinct = TargetClaim::checked(TargetClaimId(2), "candidate", "other", "v2", "C").unwrap();
    assert!(graph.add_target_claim(distinct).is_ok());
}
