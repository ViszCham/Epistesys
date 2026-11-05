//! Proposed runtime audit tests; compilation/execution is separately recorded.
use lc631_core::context::{ContextRevision, SourceRevision, TaskKind};
use lc631_runtime::connection_audit::{
    audit_current_entry, check_reported_registry, AuditError, AuditGap, EdgeId, EdgeObservation,
    MAX_AUDIT_SOURCE_BYTES,
};
use lc631_runtime::{connection_contracts, solve, SolveRequest};

fn context(source: &str) -> ContextRevision {
    ContextRevision::original(SourceRevision::of(source))
}

#[test]
fn tlcx40_empty_missing_extra_duplicate_and_unknown_inventory_are_rejected() {
    assert_eq!(
        check_reported_registry(&[]).unwrap_err(),
        AuditError::RegistryCount
    );
    let contracts = connection_contracts();
    assert_eq!(
        check_reported_registry(&contracts[..19]).unwrap_err(),
        AuditError::RegistryCount
    );
    let mut duplicate = *contracts;
    duplicate[19] = duplicate[0];
    assert_eq!(
        check_reported_registry(&duplicate).unwrap_err(),
        AuditError::DuplicateEdge(EdgeId::checked(1).unwrap())
    );
    let mut unknown = *contracts;
    unknown[0].id = "E21";
    assert_eq!(
        check_reported_registry(&unknown).unwrap_err(),
        AuditError::UnknownEdge
    );
    let mut extra = contracts.to_vec();
    extra.push(contracts[0]);
    assert_eq!(
        check_reported_registry(&extra).unwrap_err(),
        AuditError::RegistryCount
    );
}

#[test]
fn tlcx40_producer_consumer_swaps_are_not_hidden_by_valid_edge_ids() {
    let mut contracts = *connection_contracts();
    contracts[4].producer = "solver_result";
    assert_eq!(
        check_reported_registry(&contracts).unwrap_err(),
        AuditError::EndpointMismatch(EdgeId::checked(5).unwrap())
    );
    contracts = *connection_contracts();
    contracts[4].consumer = "result_validator";
    assert_eq!(
        check_reported_registry(&contracts).unwrap_err(),
        AuditError::EndpointMismatch(EdgeId::checked(5).unwrap())
    );
}

#[test]
fn tlcx40_inventory_order_has_no_effect_on_typed_edge_lookup() {
    let mut contracts = *connection_contracts();
    contracts.reverse();
    let view = check_reported_registry(&contracts).unwrap();
    for n in 1..=20 {
        let id = EdgeId::checked(n).unwrap();
        let contract = view.contract(id);
        assert_eq!(contract.id, connection_contracts()[usize::from(n - 1)].id);
    }
    for invalid in [0, 21, 255] {
        assert_eq!(EdgeId::checked(invalid), Err(AuditError::UnknownEdge));
    }
}

#[test]
fn tlcx40_actual_entry_observation_matches_direct_api_and_never_populates_edge_evidence() {
    let source = "Retain unresolved requirements during a runtime entry audit.";
    for task in [
        TaskKind::Mathematics,
        TaskKind::Reasoning,
        TaskKind::Explanation,
        TaskKind::Coding,
        TaskKind::Media,
    ] {
        let revision = context(source);
        let direct = solve(SolveRequest::checked(source, revision, task).unwrap());
        let audit = audit_current_entry(source, revision, task).unwrap();
        let observed = audit.entry().report();
        assert_eq!(observed.context, direct.context);
        assert_eq!(observed.task, direct.task);
        assert_eq!(observed.status, direct.status);
        assert_eq!(observed.source_bytes, direct.source_bytes);
        assert_eq!(audit.gap(), AuditGap::RequiredEdgeExecutionsUnobserved);
        assert_eq!(audit.registry().contracts().len(), 20);
        for n in 1..=20 {
            assert_eq!(
                audit.edge_observation(EdgeId::checked(n).unwrap()),
                EdgeObservation::Unobserved
            );
        }
        assert!(audit
            .entry()
            .inspect_current(source, revision, task)
            .is_ok());
    }
}

#[test]
fn tlcx40_source_context_task_and_budget_changes_invalidate_entry_observations() {
    let source = "A source-bound runtime audit.";
    let revision = context(source);
    assert_eq!(
        audit_current_entry(source, context("foreign source"), TaskKind::Reasoning).unwrap_err(),
        AuditError::SourceContextMismatch
    );
    assert_eq!(
        audit_current_entry("", context(""), TaskKind::Reasoning).unwrap_err(),
        AuditError::EmptySource
    );
    let oversized = "x".repeat(MAX_AUDIT_SOURCE_BYTES + 1);
    assert_eq!(
        audit_current_entry(&oversized, context(&oversized), TaskKind::Reasoning).unwrap_err(),
        AuditError::SourceBudget
    );
    let audit = audit_current_entry(source, revision, TaskKind::Reasoning).unwrap();
    assert_eq!(
        audit
            .entry()
            .inspect_current("changed", context("changed"), TaskKind::Reasoning),
        Err(AuditError::StaleObservation)
    );
    assert_eq!(
        audit.entry().inspect_current(
            source,
            revision.reconstructed([99; 32]),
            TaskKind::Reasoning
        ),
        Err(AuditError::StaleObservation)
    );
    assert_eq!(
        audit
            .entry()
            .inspect_current(source, revision, TaskKind::Coding),
        Err(AuditError::StaleObservation)
    );
}

#[test]
fn tlcx40_reported_inventory_cannot_construct_an_executed_entry() {
    let registry = check_reported_registry(connection_contracts()).unwrap();
    let first = registry.contract(EdgeId::checked(1).unwrap());
    assert_eq!(first.producer, "host_ingress");
    assert_eq!(first.consumer, "source_store");
    assert_eq!(
        registry.claim_boundary(),
        "declared endpoint shape only; no edge execution evidence"
    );
}
