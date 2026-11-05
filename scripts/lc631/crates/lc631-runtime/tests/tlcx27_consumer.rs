use lc631_core::context::{ContextRevision, ContextScope, SourceRevision};
use lc631_runtime::contextual_distillation::{consumer::*, *};
use lc631_substrate::Digest32;

fn edge(a: u8, b: u8) -> RelationEdit {
    RelationEdit {
        from: RelationNode(a),
        to: RelationNode(b),
    }
}

fn engine(status: BasisAvailability) -> ContextualDistillation {
    let context = ContextRevision::original(SourceRevision::of("immutable given DAG"));
    let p = Digest32::from_bytes([3; 32]);
    let scope = ContextScope::declared(p, p, p, p);
    ContextualDistillation::new(
        ImmutableRelationBasis::checked(
            context,
            scope,
            5,
            &[edge(0, 1), edge(1, 2), edge(2, 3)],
            status,
        )
        .unwrap(),
        DistillationLimits {
            max_epochs: 8,
            max_proposals: 8,
            max_relations: 16,
        },
    )
    .unwrap()
}

fn add(state: &mut ContextualDistillation, relation: RelationEdit) -> NextRelationQuery {
    state
        .epoch(&[RelationProposal {
            context: state.basis().context(),
            scope: state.basis().scope(),
            edit: relation,
        }])
        .unwrap()
        .next_query
        .unwrap()
}

#[test]
fn tlcx27_consumer_negative_stale_state_cannot_reuse_request() {
    let mut state = engine(BasisAvailability::Given);
    let request = add(&mut state, edge(0, 2));
    add(&mut state, edge(0, 3));
    assert!(matches!(
        revalidate_next_query(&state, request, 1000),
        Err(RelationConsumerHold::StaleState)
    ));
}

#[test]
fn tlcx27_consumer_negative_binding_swaps_are_separate_holds() {
    let mut state = engine(BasisAvailability::Given);
    let request = add(&mut state, edge(0, 2));
    let mut swapped = request;
    swapped.context = ContextRevision::original(SourceRevision::of("foreign source"));
    assert!(matches!(
        revalidate_next_query(&state, swapped, 1000),
        Err(RelationConsumerHold::ForeignContext)
    ));
    swapped = request;
    let p = Digest32::from_bytes([9; 32]);
    swapped.scope = ContextScope::declared(p, p, p, p);
    assert!(matches!(
        revalidate_next_query(&state, swapped, 1000),
        Err(RelationConsumerHold::ForeignScope)
    ));
    swapped = request;
    swapped.basis_digest = p;
    assert!(matches!(
        revalidate_next_query(&state, swapped, 1000),
        Err(RelationConsumerHold::ForeignBasis)
    ));
}

#[test]
fn tlcx27_consumer_negative_existing_membership_and_unadmitted_relation_are_not_new_queries() {
    let mut state = engine(BasisAvailability::Given);
    let mut request = add(&mut state, edge(0, 2));
    request.relation = edge(0, 1);
    assert!(matches!(
        revalidate_next_query(&state, request, 1000),
        Err(RelationConsumerHold::NotDerivedRelation)
    ));
    request.relation = edge(0, 3); // Reachable in basis, but not admitted to this state.
    assert!(matches!(
        revalidate_next_query(&state, request, 1000),
        Err(RelationConsumerHold::RelationNotAdmitted)
    ));
    request.relation = edge(3, 0);
    assert!(matches!(
        revalidate_next_query(&state, request, 1000),
        Err(RelationConsumerHold::Refuted)
    ));
}

#[test]
fn tlcx27_consumer_negative_checker_cut_is_resource_hold_without_state_mutation() {
    let mut state = engine(BasisAvailability::Given);
    let request = add(&mut state, edge(0, 3));
    let before = state.state_revision();
    assert!(matches!(
        revalidate_next_query(&state, request, 0),
        Err(RelationConsumerHold::ResourceExhausted)
    ));
    assert!(matches!(
        revalidate_next_query(&state, request, 1),
        Err(RelationConsumerHold::ResourceExhausted)
    ));
    assert_eq!(state.state_revision(), before);
    assert_eq!(state.epochs_used(), 1);
}

#[test]
fn tlcx27_consumer_negative_unavailable_and_unsupported_basis_stay_visible() {
    let state = engine(BasisAvailability::Unavailable);
    let request = NextRelationQuery {
        context: state.basis().context(),
        scope: state.basis().scope(),
        state_revision: state.state_revision(),
        basis_digest: state.basis().digest(),
        relation: edge(0, 2),
        basis: ValidationBasis::GivenFiniteReachability,
    };
    assert!(matches!(
        revalidate_next_query(&state, request, 1000),
        Err(RelationConsumerHold::NeedsEvidence)
    ));
    let state = engine(BasisAvailability::Unsupported);
    let request = NextRelationQuery {
        state_revision: state.state_revision(),
        basis_digest: state.basis().digest(),
        ..request
    };
    assert!(matches!(
        revalidate_next_query(&state, request, 1000),
        Err(RelationConsumerHold::Unsupported)
    ));
}

#[test]
fn tlcx27_consumer_state_query_recheck_path_and_cut_are_causally_connected() {
    let mut state = engine(BasisAvailability::Given);
    let original = state.state_revision();
    let request = add(&mut state, edge(0, 3));
    assert_ne!(original, request.state_revision);
    let checked = revalidate_next_query(&state, request, 1000).unwrap();
    let path = consume_dependency_path(checked).unwrap();
    assert_eq!(
        path.nodes(),
        &[
            RelationNode(0),
            RelationNode(1),
            RelationNode(2),
            RelationNode(3)
        ]
    );
    assert_eq!(path.request(), request);
    assert_eq!(path.input_basis(), RelationInputBasis::Given);
    assert_eq!(
        path.independence(),
        RelationIndependence::SharedBasisUnassessed
    );
    assert!(path.edge_visits() > 0);
    assert!(path
        .nodes()
        .windows(2)
        .all(|w| state.basis().edges().contains(&RelationEdit {
            from: w[0],
            to: w[1]
        })));
    assert!(matches!(
        revalidate_next_query(&state, request, 0),
        Err(RelationConsumerHold::ResourceExhausted)
    ));
    let membership_only = state
        .epoch(&[RelationProposal {
            context: request.context,
            scope: request.scope,
            edit: edge(0, 1),
        }])
        .unwrap();
    assert!(membership_only.next_query.is_none());
    assert_eq!(membership_only.added, 0);
}

#[test]
fn tlcx27_consumer_path_changes_with_admitted_relation() {
    let mut state = engine(BasisAvailability::Given);
    let first = add(&mut state, edge(0, 2));
    let path =
        consume_dependency_path(revalidate_next_query(&state, first, 1000).unwrap()).unwrap();
    assert_eq!(
        path.nodes(),
        &[RelationNode(0), RelationNode(1), RelationNode(2)]
    );
    let second = add(&mut state, edge(1, 3));
    let path =
        consume_dependency_path(revalidate_next_query(&state, second, 1000).unwrap()).unwrap();
    assert_eq!(
        path.nodes(),
        &[RelationNode(1), RelationNode(2), RelationNode(3)]
    );
    assert_ne!(first.state_revision, second.state_revision);
}

#[test]
fn tlcx27_consumer_invalid_node_request_cannot_panic_checker() {
    let mut state = engine(BasisAvailability::Given);
    let mut request = add(&mut state, edge(0, 2));
    request.relation = edge(255, 254);
    assert!(matches!(
        revalidate_next_query(&state, request, 1000),
        Err(RelationConsumerHold::InvalidRelation)
    ));
}
