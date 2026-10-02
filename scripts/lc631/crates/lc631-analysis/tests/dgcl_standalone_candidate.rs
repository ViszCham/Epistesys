#![forbid(unsafe_code)]

#[test]
fn public_pipeline_flags_cannot_mint_a_verified_completion_candidate() {
    let source = "Please test the package.";
    let mut pipeline = lc631_analysis::build_dgcl_pipeline(
        source,
        lc631_analysis::DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    pipeline
        .implementation_closure
        .implementation_complete_candidate = true;
    pipeline.implementation_closure.status =
        lc631_analysis::DgclImplementationStatus::ImplementationClosed;
    let candidate = lc631_analysis::build_dgcl_standalone_candidate(
        source,
        &pipeline,
        br#"{"schema_version":"test.v1"}"#,
        "test.v1",
    )
    .unwrap();
    assert!(!candidate.implementation_complete_candidate);
}

use lc631_analysis::{
    build_dgcl_pipeline, build_dgcl_standalone_candidate, DgclBackendSet, DgclImplementationStatus,
    DgclStandaloneCandidateState, HostObservationState,
};

fn pipeline(source: &str) -> lc631_analysis::DgclPipelineReport {
    build_dgcl_pipeline(
        source,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap()
}

#[test]
fn exact_schema_candidate_is_captured_without_becoming_host_delivery() {
    let source = "Please test the package.";
    let pipeline = pipeline(source);
    let bytes = br#"{"schema_version":"test-output.v1","result":"candidate"}"#;
    let candidate =
        build_dgcl_standalone_candidate(source, &pipeline, bytes, "test-output.v1").unwrap();
    assert_eq!(
        candidate.candidate_state,
        DgclStandaloneCandidateState::ExactCandidateCaptured
    );
    assert_ne!(
        candidate.implementation_closure_status,
        DgclImplementationStatus::ImplementationClosed
    );
    assert_eq!(candidate.candidate_text.as_bytes(), bytes);
    assert!(!candidate.host_send_authorized);
    assert!(!candidate.output_commit_allowed);
    assert_eq!(
        candidate.host_observation.precommit,
        HostObservationState::Pending
    );
    assert_eq!(
        candidate.host_observation.post_send,
        HostObservationState::Pending
    );
    assert_eq!(
        candidate.host_observation.sink_delivery,
        HostObservationState::Pending
    );
    assert_eq!(
        candidate.host_observation.durable_replay,
        HostObservationState::Pending
    );
}

#[test]
fn malformed_schema_or_trailing_bytes_are_visible_errors() {
    let source = "Please test the package.";
    let pipeline = pipeline(source);
    assert!(build_dgcl_standalone_candidate(
        source,
        &pipeline,
        br#"{"schema_version":"other.v1"}"#,
        "test-output.v1",
    )
    .is_err());
    assert!(build_dgcl_standalone_candidate(
        source,
        &pipeline,
        b"{\"schema_version\":\"test-output.v1\"} trailing",
        "test-output.v1",
    )
    .is_err());
}

#[test]
fn no_action_candidate_does_not_become_an_output_permission() {
    let source = "> Please test the package.";
    let pipeline = pipeline(source);
    let candidate = build_dgcl_standalone_candidate(
        source,
        &pipeline,
        br#"{"schema_version":"diagnostic.v1"}"#,
        "diagnostic.v1",
    )
    .unwrap();
    assert_eq!(
        candidate.candidate_state,
        DgclStandaloneCandidateState::NoAction
    );
    assert!(!candidate.implementation_complete_candidate);
    assert!(!candidate.host_send_authorized);
    assert!(!candidate.output_commit_allowed);
}
