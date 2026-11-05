//! H4 negative-first tests only. Central registers this under dgcl_pipeline.
//! The fixed-type worker is deliberately absent until a separate body GO.
use super::goal_input_admission;
use crate::declared_build_capture::{SnapshotCaptureError, SourceReadBinding};
use crate::{
    ConditionEvidenceState, ConditionExpression, ConditionPredicate, ConditionRelation,
    DgclPipelineReport, PlannedCondition,
};
use lc631_substrate::resource::{Cancellation, ResourceError, ResourceKind};
use std::time::{Duration, Instant};
#[path = "goal_input_admission_test_support.rs"]
mod support;
use support::*;

fn limited(pipeline: &DgclPipelineReport) {
    let lease = lease(64 * 1024, Cancellation::default(), None);
    let view = lease.view();
    assert_eq!(
        funded(pipeline, &view),
        Err(SnapshotCaptureError::InventoryLimit)
    );
}

#[test]
fn tlcx22_h4_actual_wide_container_is_rejected_at_known_length() {
    for length in [16_385, 65_537] {
        let mut report = pipeline();
        report.unsupported_operative_region_ids = vec![0; length];
        limited(&report);
    }
}

#[test]
fn tlcx22_h4_actual_nested_ast_string_is_rejected_before_serialization() {
    let mut report = pipeline();
    let parse = report
        .instruction_parses
        .first_mut()
        .expect("actual instruction parse");
    let grammar = std::sync::Arc::make_mut(parse.report.as_mut().unwrap());
    // Mutable compatibility AST is an explicit test mutation, never production
    // preflight. No full report clone/serde/to_value is used to build this cut.
    grammar
        .alternatives
        .first_mut()
        .unwrap()
        .materialize_mut()
        .body = "x".repeat(262_145);
    limited(&report);
}

#[test]
fn tlcx22_h4_actual_recursive_condition_depth_is_rejected_without_recursive_walk() {
    let mut report = pipeline();
    let mut expression = ConditionExpression::Predicate(ConditionPredicate {
        source_span: 0..1,
        text: "p".into(),
    });
    for _ in 0..65 {
        expression = ConditionExpression::Not(Box::new(expression));
    }
    report
        .completion_plan
        .requirements
        .first_mut()
        .unwrap()
        .conditions
        .push(PlannedCondition {
            condition_id: "h4-depth-cut".into(),
            source_span: 0..1,
            source_digest: "given-not-verified".into(),
            marker_span: 0..1,
            marker_digest: "given-not-verified".into(),
            relation: ConditionRelation::Sufficient,
            expression,
            scope_resolved: false,
            truth_state: ConditionEvidenceState::Unknown,
        });
    limited(&report);
}

#[test]
fn tlcx22_h4_actual_aggregate_bytes_and_total_nodes_are_not_per_leaf_only() {
    let mut bytes = pipeline();
    bytes
        .translation_projection
        .envelope
        .domain_payloads
        .first_mut()
        .unwrap()
        .unknowns = vec!["x".repeat(262_144); 4];
    limited(&bytes); // four legal leaves plus actual report strings exceed1MiB

    let mut nodes = pipeline();
    nodes.unsupported_operative_region_ids = vec![0; 16_384];
    nodes
        .requirement_bindings
        .first_mut()
        .unwrap()
        .dependency_language_span_ids = vec![0; 16_384];
    limited(&nodes); // each container legal; actual combined node count is not
}

#[test]
fn tlcx22_h4_exact_leaf_and_container_caps_are_bounds_only_not_identity() {
    let mut leaf = pipeline();
    leaf.program_ir
        .requirements
        .first_mut()
        .unwrap()
        .source_digest = "x".repeat(262_144);
    let lease = lease(64 * 1024, Cancellation::default(), None);
    let view = lease.view();
    assert_eq!(funded(&leaf, &view), Ok(()));
    assert!(!crate::verify_dgcl_pipeline_identity(SOURCE, &leaf));
    let mut width = pipeline();
    width.unsupported_operative_region_ids = vec![0; 16_384];
    assert_eq!(funded(&width, &view), Ok(()));
    assert!(!crate::verify_dgcl_pipeline_identity(SOURCE, &width));
}

#[test]
fn tlcx22_h4_resource_cancellation_deadline_and_funding_are_typed() {
    let report = pipeline();
    let cancellation = Cancellation::default();
    let owner = lease(64 * 1024, cancellation.clone(), None);
    let view = owner.view();
    cancellation.cancel();
    assert_eq!(
        funded(&report, &view),
        Err(SnapshotCaptureError::Resource(ResourceError::Cancelled))
    );

    // Construct the lease live, then wait until the actual deadline; no fake
    // caller expired flag or success callback can substitute for resources.
    let owner = lease(
        64 * 1024,
        Cancellation::default(),
        Some(Instant::now() + Duration::from_millis(20)),
    );
    let view = owner.view();
    while view.remaining().is_ok() {
        std::thread::yield_now();
    }
    assert_eq!(
        funded(&report, &view),
        Err(SnapshotCaptureError::Resource(
            ResourceError::DeadlineExceeded
        ))
    );

    let owner = lease(65_535, Cancellation::default(), None);
    let view = owner.view();
    assert_eq!(
        funded(&report, &view),
        Err(SnapshotCaptureError::Resource(ResourceError::Exhausted(
            ResourceKind::Ram
        )))
    );
}

#[test]
fn tlcx22_h4_normal_actual_45_byte_pipeline_admits_then_real_capture_runs() {
    assert_eq!(SOURCE.len(), 45);
    let report = pipeline();
    assert!(crate::verify_dgcl_pipeline_identity(SOURCE, &report));
    let owner = lease(32 * 1024 * 1024, Cancellation::default(), None);
    let view = owner.view();
    let root = actual_root();
    let binding = binding(&root, &report);
    assert_eq!(binding.check_goal_profile_inputs(&view), Ok(()));
    let (attempt, calls, reads) = actual_capture(&binding, &view);
    let captured = attempt
        .current()
        .expect("actual inventory/capture after H4 admission");
    captured.check_current(&view).unwrap();
    assert_eq!(captured.content_pins().source, view.source_revision());
    assert_eq!(calls, 2);
    assert!(reads > 0);
}

#[test]
fn tlcx22_h4_private_binding_cuts_do_not_invoke_authority_or_inventory() {
    for wide in [true, false] {
        let mut report = pipeline();
        if wide {
            report.unsupported_operative_region_ids = vec![0; 65_537];
        } else {
            report
                .program_ir
                .requirements
                .first_mut()
                .unwrap()
                .source_digest = "x".repeat(262_145);
        }
        // Descriptor metadata is intact; no directory creation/read is needed.
        let root = std::env::temp_dir().join("h4-no-io-descriptor");
        let binding = SourceReadBinding::checked(
            &root,
            SOURCE,
            &report,
            lc631_substrate::Digest32::from_bytes([91; 32]),
            crate::declared_build_capture::declared_capture_digest(
                &report.program_ir.program_digest,
            )
            .unwrap(),
            crate::declared_build_capture::declared_capture_digest(
                &report.completion_plan.plan_digest,
            )
            .unwrap(),
        )
        .unwrap();
        let owner = lease(32 * 1024 * 1024, Cancellation::default(), None);
        let view = owner.view();
        let calls = std::cell::Cell::new(0);
        let mut authority = NeverAuthority(&calls);
        let mut reader = NeverReader(&calls);
        let result = binding.check_goal_profile_inputs(&view).and_then(|()| {
            crate::declared_build_capture::capture_declared_build(
                &binding,
                &mut reader,
                &mut authority,
                &view,
            )
            .current()
            .map(|_| ())
        });
        assert_eq!(result, Err(SnapshotCaptureError::InventoryLimit));
        assert_eq!(calls.get(), 0);
    }
}
