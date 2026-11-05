use lc631_core::context::{
    ContextRevision, DecisionStatus, MissingStage, SourceRevision, TaskKind,
};
use lc631_core::CandidateId;
use lc631_runtime::{connection_contracts, CandidateSet, CandidateSetError, ConnectionState};
use lc631_runtime::{solve, SolveRequest};

#[test]
fn tlcx_01_empty_or_duplicate_candidates_cannot_create_a_positive() {
    assert_eq!(
        CandidateSet::checked(&[], 16).unwrap_err(),
        CandidateSetError::Empty
    );
    assert_eq!(
        CandidateSet::checked(&[CandidateId(1), CandidateId(1)], 16).unwrap_err(),
        CandidateSetError::Duplicate
    );
    assert_eq!(
        CandidateSet::checked(&[CandidateId(1)], 0).unwrap_err(),
        CandidateSetError::InvalidLimit
    );
    let ids = [CandidateId(1), CandidateId(2)];
    assert_eq!(
        CandidateSet::checked(&ids, 1).unwrap_err(),
        CandidateSetError::TooMany
    );
    assert_eq!(
        CandidateSet::checked(&ids, 2_049).unwrap_err(),
        CandidateSetError::InvalidLimit
    );
    assert_eq!(CandidateSet::checked(&ids, 16).unwrap().ids(), &ids);
}

#[test]
fn tlcx_01_twenty_registered_edges_remain_explicitly_unconnected() {
    let edges = connection_contracts();
    assert_eq!(edges.len(), 20);
    let names = edges
        .iter()
        .map(|edge| edge.id)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(names.len(), 20);
    let expected = (1..=20)
        .map(|index| format!("E{index:02}"))
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        names
            .iter()
            .map(|name| (*name).to_owned())
            .collect::<std::collections::BTreeSet<_>>(),
        expected
    );
    assert!(edges
        .iter()
        .all(|edge| edge.state == ConnectionState::NotConnected));
}

#[test]
fn tlcx_01_initial_runtime_reports_the_first_unconnected_stage() {
    let source = "Find a solution without dropping mandatory constraints.";
    let context = ContextRevision::original(SourceRevision::of(source));
    let report = solve(SolveRequest::checked(source, context, TaskKind::Reasoning).unwrap());
    assert_eq!(report.context, context);
    assert_eq!(report.source_bytes, source.len());
    assert_eq!(
        report.status,
        DecisionStatus::NotConnected(MissingStage::ContextLowering)
    );
}

#[test]
fn tlcx_01_initial_runtime_rejects_foreign_source_context() {
    let context = ContextRevision::original(SourceRevision::of("Original instruction."));
    assert!(SolveRequest::checked("Substituted instruction.", context, TaskKind::Coding).is_err());
}
