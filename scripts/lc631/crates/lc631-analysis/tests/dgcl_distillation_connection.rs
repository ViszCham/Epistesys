use lc631_analysis::*;

#[test]
fn parsed_constraint_materialization_reaches_ir_tl_plan_and_candidate_identity() {
    let source = "Please test then publish.";
    let pipeline = build_dgcl_pipeline(
        source,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    let projection = &pipeline.program_ir.structural_distillation;
    assert!(projection
        .run
        .as_ref()
        .unwrap()
        .epochs
        .iter()
        .any(|epoch| epoch.state_changed));
    assert!(!projection.constraint_witnesses.is_empty());
    assert_eq!(
        pipeline.translation_projection.program_ir_digest,
        pipeline.program_ir.program_digest
    );
    assert_eq!(
        pipeline.completion_plan.program_ir_digest,
        pipeline.program_ir.program_digest
    );
    assert!(!pipeline.authority_created);
    assert!(
        !pipeline
            .implementation_closure
            .implementation_complete_candidate
    );
    let mut cut = pipeline.clone();
    cut.program_ir
        .structural_distillation
        .constraint_witnesses
        .clear();
    assert!(!verify_dgcl_pipeline_identity(source, &cut));
    assert!(build_dgcl_standalone_candidate(
        source,
        &cut,
        b"{\"schema_version\":\"candidate.v1\"}",
        "candidate.v1"
    )
    .is_err());
}

#[test]
fn unresolved_or_unverified_edges_do_not_gain_truth_or_completion_from_geometry() {
    for source in [
        "Please test and publish.",
        "Please publish if unknown.",
        "Please publish if A and B or C.",
    ] {
        let pipeline = build_dgcl_pipeline(
            source,
            DgclBackendSet {
                english: None,
                japanese: None,
            },
        )
        .unwrap();
        assert!(pipeline
            .program_ir
            .structural_distillation
            .constraint_witnesses
            .iter()
            .all(|edge| !edge.semantic_truth_claim && !edge.authority_created));
        assert!(
            !pipeline
                .implementation_closure
                .implementation_complete_candidate
        );
        assert!(!pipeline.authority_created);
    }
}

#[test]
fn multiple_constraints_on_one_pair_keep_the_exact_materialized_evidence_origin() {
    let source = "Please publish then do not publish.";
    let pipeline = build_dgcl_pipeline(
        source,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    let mut requirements = pipeline.program_ir.requirements.clone();
    let ast = &mut requirements[0].alternatives[0].ast;
    assert!(ast.constraint_graph.edges.len() >= 2);
    // Explicit adversarial typed-input fixture: two separately retained
    // candidate constraints share a pair. This is not a semantic gold label.
    for edge in &mut ast.constraint_graph.edges {
        edge.state = InstructionEdgeState::Candidate;
    }
    assert!(
        distill_dgcl_constraints(source, &requirements).is_err(),
        "a stale AST digest must not produce witnesses"
    );
    requirements[0].alternatives[0].parser_ast_digest = lc631_core::stable_sha256(
        &serde_json::to_string(&requirements[0].alternatives[0].ast).unwrap(),
    );
    let projection = distill_dgcl_constraints(source, &requirements).unwrap();
    for witness in &projection.constraint_witnesses {
        // Geometry selects a pair, while the discrete validator materializes
        // the first exact source relation; a pair-map must not overwrite it
        // with a later conflict/requires edge sharing the endpoints.
        assert_eq!(witness.constraint_edge_index, 0);
    }
    assert!(!projection.constraint_witnesses.is_empty());
    assert!(
        !pipeline
            .implementation_closure
            .implementation_complete_candidate
    );
}
