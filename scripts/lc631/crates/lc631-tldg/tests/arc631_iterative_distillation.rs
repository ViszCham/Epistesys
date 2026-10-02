use lc631_tldg::{
    analyze, build_structural_kernel, run_mutual_distillation, AnchorState, CpuGeometryBackend,
    DistillationStopReason, ParseBudget, RelationKind, RelationState, SyntaxRelation,
};

#[test]
fn arc631_16_max_epochs_controls_real_revisioned_backflow_iterations() {
    let artifact = analyze("ambiguous input").unwrap();
    let mut kernel = build_structural_kernel(&artifact).unwrap();
    kernel.anchors.last_mut().unwrap().state = AnchorState::Unknown;
    let run = run_mutual_distillation(
        kernel,
        &artifact,
        ParseBudget {
            max_epochs: 3,
            max_proposals: 128,
        },
        &CpuGeometryBackend,
    )
    .unwrap();
    assert_eq!(run.epochs.len(), 1);
    assert_eq!(run.stop_reason, DistillationStopReason::NoProgress);
    assert!(!run.epochs[0].state_changed);
    assert_eq!(run.epochs[0].input_revision, run.epochs[0].output_revision);
    assert!(run.epochs[0].materialized_relations_added == 0);
    assert!(run.reprojection_requests.is_empty());
    assert!(run.max_epochs_respected);
    assert!(!run
        .final_payload
        .geometry
        .as_ref()
        .unwrap()
        .proposals
        .is_empty());
    assert!(run.final_payload.kernel.materialized_relations.is_empty());
    assert!(run
        .epochs
        .iter()
        .all(|epoch| !epoch.grammar_mutated && !epoch.thresholds_mutated));
}

#[test]
fn verified_materialization_changes_the_next_epoch_input_revision() {
    let mut artifact = analyze("alpha beta gamma").unwrap();
    let source = artifact.syntax.nodes[0].id;
    let target = artifact.syntax.nodes[1].id;
    artifact.syntax.relations.push(SyntaxRelation {
        from: source,
        to: target,
        kind: RelationKind::DependencyCandidate,
        state: RelationState::Verified,
        evidence: "fixture-verified-source-edge.v1".into(),
    });
    let kernel = build_structural_kernel(&artifact).unwrap();
    let run = run_mutual_distillation(
        kernel,
        &artifact,
        ParseBudget {
            max_epochs: 3,
            max_proposals: 128,
        },
        &CpuGeometryBackend,
    )
    .unwrap();
    assert_eq!(run.epochs.len(), 2);
    assert!(run.epochs[0].state_changed);
    assert_eq!(run.epochs[0].materialized_relations_added, 1);
    assert_eq!(run.epochs[1].input_revision, run.epochs[0].output_revision);
    assert!(!run.epochs[1].state_changed);
    assert_eq!(run.epochs[1].input_revision, run.epochs[1].output_revision);
    assert_eq!(run.stop_reason, DistillationStopReason::NoProgress);
    assert_eq!(run.final_payload.kernel.materialized_relations.len(), 1);
    assert_eq!(run.reprojection_requests.len(), 1);
    assert_eq!(
        run.reprojection_requests[0].next_input_revision,
        run.epochs[0].output_revision
    );
    let geometry = run.final_payload.geometry.as_ref().unwrap();
    assert!(!geometry
        .proposals
        .iter()
        .any(|proposal| { proposal.source == source && proposal.target == target }));
    assert_eq!(geometry.source_kernel_digest, run.epochs[1].input_revision);
    assert_eq!(run.epochs[0].output_revision, run.epochs[1].input_revision);
    assert!(!run.final_backflow.grammar_mutated);
    assert!(!run.final_backflow.thresholds_mutated);
}
