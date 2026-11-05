use lc631_core::context::{ContextRevision, ContextScope, SourceRevision};
use lc631_runtime::facet_ledger::{
    ComparisonScope, Direction, Facet, FacetLedger, GapReason, PropertyId, Revision, ScopeId,
    TransformBinding, TransformId,
};
use lc631_runtime::finite_modal_observation::{
    ModalError, ModalLedgerRequest, ModalObservationStore, ModalRequest,
};
use lc631_runtime::resource::{
    Cancellation, Deadline, ResourceAmount, ResourceBroker, ResourceKind, ResourceLimits,
    ResourceRequest, ResourceReservationView,
};
use lc631_solvers::finite_modal::{Formula, Frame, Model, Profile, ValidationBasis};
use lc631_substrate::Digest32;
use std::time::Duration;

const SOURCE: &str = "finite modal source";
fn context() -> ContextRevision {
    ContextRevision::original(SourceRevision::of(SOURCE))
}
fn scope() -> ContextScope {
    ContextScope::declared(
        Digest32::from_bytes([1; 32]),
        Digest32::from_bytes([2; 32]),
        Digest32::from_bytes([3; 32]),
        Digest32::from_bytes([4; 32]),
    )
}
fn resources(f: impl FnOnce(&ResourceReservationView<'_>)) {
    let limits = ResourceLimits::uniform(1_048_576).unwrap();
    let owner = ResourceBroker::new(limits).unwrap();
    let mut run = owner
        .begin_run(
            1,
            context().source().digest(),
            limits,
            Deadline::after(Duration::from_secs(30)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let mut reservation = run
        .reserve(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(1_048_576).unwrap(),
        )])
        .unwrap();
    {
        let view = reservation.lease();
        f(&view);
    }
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
    assert_eq!(owner.used(ResourceKind::Ram).unwrap(), 0);
}

#[test]
fn tlcx15_modal_actual_evaluation_is_consumed_and_context_scope_swaps_rejected() {
    resources(|view| {
        let rows = [3, 2];
        let atoms = [2];
        let nodes = [Formula::Atom(0), Formula::Not(0), Formula::Or(0, 1)];
        let model = Model::new(
            Frame::new(Profile::IntuitionisticPreorder, &rows).unwrap(),
            &atoms,
        )
        .unwrap();
        let mut store = ModalObservationStore::with_resources(2, view).unwrap();
        let id = store
            .evaluate(
                ModalRequest {
                    source: SOURCE,
                    context: context(),
                    scope: scope(),
                    model,
                    nodes: &nodes,
                    root: 2,
                },
                view,
            )
            .unwrap();
        let observed = store.lookup(id, context(), scope()).unwrap();
        assert_eq!(observed.evaluation().at_world(0), Ok(false));
        assert_eq!(observed.evaluation().at_world(1), Ok(true));
        assert_eq!(
            observed.validation_basis(),
            ValidationBasis::EvaluatorReported
        );
        assert_eq!(
            store
                .lookup(id, context().reconstructed([9; 32]), scope())
                .unwrap_err(),
            ModalError::ContextMismatch
        );
        let swapped = ContextScope::declared(
            Digest32::from_bytes([8; 32]),
            scope().goal(),
            scope().probes(),
            scope().semantics(),
        );
        assert_eq!(
            store.lookup(id, context(), swapped).unwrap_err(),
            ModalError::ScopeMismatch
        );
        let other = ModalObservationStore::with_resources(1, view).unwrap();
        assert_eq!(
            other.lookup(id, context(), scope()).unwrap_err(),
            ModalError::ForeignObservation
        );
        assert_eq!(store.len(), 1);
    });
}

#[test]
fn tlcx15_modal_model_cut_changes_result_in_same_scope_without_truth_promotion() {
    resources(|view| {
        let rows = [1];
        let yes = [1];
        let no = [0];
        let nodes = [Formula::Atom(0), Formula::Box(0)];
        let mut store = ModalObservationStore::with_resources(2, view).unwrap();
        let mut ids = Vec::new();
        for atoms in [&yes[..], &no[..]] {
            let model = Model::new(Frame::new(Profile::ModalK, &rows).unwrap(), atoms).unwrap();
            ids.push(
                store
                    .evaluate(
                        ModalRequest {
                            source: SOURCE,
                            context: context(),
                            scope: scope(),
                            model,
                            nodes: &nodes,
                            root: 1,
                        },
                        view,
                    )
                    .unwrap(),
            );
        }
        let left = store.lookup(ids[0], context(), scope()).unwrap();
        let right = store.lookup(ids[1], context(), scope()).unwrap();
        assert_eq!(left.evaluation().at_world(0), Ok(true));
        assert_eq!(right.evaluation().at_world(0), Ok(false));
        assert_ne!(left.model_digest(), right.model_digest());
        assert_ne!(
            store.binding(ids[0], context(), scope()).unwrap().result,
            store.binding(ids[1], context(), scope()).unwrap().result
        );
    });
}

#[test]
fn tlcx15_modal_result_reaches_real_ledger_only_as_unverified_gap() {
    resources(|view| {
        let rows = [1];
        let atoms = [1];
        let nodes = [Formula::Atom(0), Formula::Ought(0)];
        let model =
            Model::new(Frame::new(Profile::DeonticSerialKd, &rows).unwrap(), &atoms).unwrap();
        let mut store = ModalObservationStore::with_resources(1, view).unwrap();
        let id = store
            .evaluate(
                ModalRequest {
                    source: SOURCE,
                    context: context(),
                    scope: scope(),
                    model,
                    nodes: &nodes,
                    root: 1,
                },
                view,
            )
            .unwrap();
        let binding = store.binding(id, context(), scope()).unwrap();
        let mut ledger = FacetLedger::new(1, 1).unwrap();
        let transform = TransformId([5; 32]);
        ledger
            .admit_transform(TransformBinding {
                id: transform,
                source: binding.source,
                target: Revision([6; 32]),
                source_context: binding.context,
                target_context: Revision([7; 32]),
                direction: Direction::Forward,
                scope: ComparisonScope {
                    id: ScopeId(*scope().comparison().as_bytes()),
                    baseline: Revision([8; 32]),
                    root_goal: Revision(*scope().goal().as_bytes()),
                    probes: Revision(*scope().probes().as_bytes()),
                    policy: Revision(*scope().semantics().as_bytes()),
                },
                producer: Revision([9; 32]),
                query: binding.query,
                result: binding.result,
            })
            .unwrap();
        store
            .retain(
                ModalLedgerRequest {
                    observation: id,
                    context: context(),
                    scope: scope(),
                    transform,
                    property: PropertyId([10; 32]),
                    required_evidence: Revision([11; 32]),
                    next_action: Revision([12; 32]),
                },
                &mut ledger,
            )
            .unwrap();
        assert!(
            matches!(ledger.entries()[0].facet,Facet::Uncertainty(gap) if gap.reason==GapReason::Unverified)
        );
        assert_eq!(ledger.entries().len(), 1);
    });
}

#[test]
fn tlcx15_modal_source_capacity_and_invalid_formula_do_not_publish() {
    resources(|view| {
        let rows = [1];
        let atoms = [1];
        let nodes = [Formula::Atom(0)];
        let model = Model::new(Frame::new(Profile::ModalK, &rows).unwrap(), &atoms).unwrap();
        let mut store = ModalObservationStore::with_resources(1, view).unwrap();
        let request = ModalRequest {
            source: SOURCE,
            context: context(),
            scope: scope(),
            model,
            nodes: &nodes,
            root: 0,
        };
        assert_eq!(
            store
                .evaluate(
                    ModalRequest {
                        source: "different",
                        ..request
                    },
                    view
                )
                .unwrap_err(),
            ModalError::SourceMismatch
        );
        assert!(store
            .evaluate(ModalRequest { root: 2, ..request }, view)
            .is_err());
        assert!(store.is_empty());
        store.evaluate(request, view).unwrap();
        assert_eq!(
            store.evaluate(request, view).unwrap_err(),
            ModalError::CapacityReached
        );
        assert_eq!(store.len(), 1);
    });
}

#[test]
fn tlcx15_modal_foreign_reservation_and_cancellation_do_not_publish() {
    let limits = ResourceLimits::uniform(1_048_576).unwrap();
    let owner = ResourceBroker::new(limits).unwrap();
    let cancel = Cancellation::default();
    let mut run = owner
        .begin_run(
            1,
            context().source().digest(),
            limits,
            Deadline::after(Duration::from_secs(30)).unwrap(),
            cancel.clone(),
        )
        .unwrap();
    let mut a = run
        .reserve(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(32768).unwrap(),
        )])
        .unwrap();
    let mut b = run
        .reserve(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(32768).unwrap(),
        )])
        .unwrap();
    {
        let view = a.lease();
        let other = b.lease();
        let rows = [1];
        let atoms = [1];
        let nodes = [Formula::Atom(0)];
        let model = Model::new(Frame::new(Profile::ModalK, &rows).unwrap(), &atoms).unwrap();
        let request = ModalRequest {
            source: SOURCE,
            context: context(),
            scope: scope(),
            model,
            nodes: &nodes,
            root: 0,
        };
        let mut store = ModalObservationStore::with_resources(1, &view).unwrap();
        assert_eq!(
            store.evaluate(request, &other).unwrap_err(),
            ModalError::Resource(lc631_runtime::resource::ResourceError::ForeignOwner)
        );
        cancel.cancel();
        assert_eq!(
            store.evaluate(request, &view).unwrap_err(),
            ModalError::Resource(lc631_runtime::resource::ResourceError::Cancelled)
        );
        assert!(store.is_empty());
    }
    a.release_confirmed().unwrap();
    b.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
    assert_eq!(owner.used(ResourceKind::Ram).unwrap(), 0);
}

#[test]
fn tlcx15_all_declared_profiles_flow_through_same_actual_store() {
    resources(|view| {
        let rows = [1];
        let atoms = [1];
        let mut store = ModalObservationStore::with_resources(4, view).unwrap();
        for (profile, operator) in [
            (Profile::ModalK, Formula::Box(0)),
            (Profile::EpistemicS5SingleAgent, Formula::Know(0)),
            (Profile::IntuitionisticPreorder, Formula::Implies(0, 0)),
            (Profile::DeonticSerialKd, Formula::Ought(0)),
        ] {
            let nodes = [Formula::Atom(0), operator];
            let model = Model::new(Frame::new(profile, &rows).unwrap(), &atoms).unwrap();
            let id = store
                .evaluate(
                    ModalRequest {
                        source: SOURCE,
                        context: context(),
                        scope: scope(),
                        model,
                        nodes: &nodes,
                        root: 1,
                    },
                    view,
                )
                .unwrap();
            assert_eq!(
                store
                    .lookup(id, context(), scope())
                    .unwrap()
                    .evaluation()
                    .at_world(0),
                Ok(true)
            );
        }
    });
}
