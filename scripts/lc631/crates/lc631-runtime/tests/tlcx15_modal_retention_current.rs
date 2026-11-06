use lc631_core::context::{ContextRevision, ContextScope, SourceRevision};
use lc631_runtime::facet_ledger::{
    ComparisonScope, Direction, Facet, FacetLedger, FiniteObservationBinding, GapReason,
    PropertyId, Revision, ScopeId, TransformBinding, TransformId,
};
use lc631_runtime::finite_modal_observation::{
    ModalError, ModalLedgerRequest, ModalObservationId, ModalObservationStore, ModalRequest,
};
use lc631_runtime::resource::{
    Cancellation, Deadline, ResourceAmount, ResourceBroker, ResourceError, ResourceKind,
    ResourceLimits, ResourceRequest,
};
use lc631_solvers::finite_modal::{Formula, Frame, Model, Profile};
use lc631_substrate::Digest32;
use std::time::Duration;

const SOURCE: &str = "modal retention after successful evaluation";
const TRANSFORM: TransformId = TransformId([5; 32]);

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

fn ledger(binding: FiniteObservationBinding) -> FacetLedger {
    let mut ledger = FacetLedger::new(1, 1).unwrap();
    ledger
        .admit_transform(TransformBinding {
            id: TRANSFORM,
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
    ledger
}

fn retention(id: ModalObservationId) -> ModalLedgerRequest {
    ModalLedgerRequest {
        observation: id,
        context: context(),
        scope: scope(),
        transform: TRANSFORM,
        property: PropertyId([10; 32]),
        required_evidence: Revision([11; 32]),
        next_action: Revision([12; 32]),
    }
}

#[test]
fn tlcx15_cancel_after_actual_evaluation_blocks_new_ledger_entry_but_keeps_history() {
    let limits = ResourceLimits::uniform(1_048_576).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let cancel = Cancellation::default();
    let mut run = broker
        .begin_run(
            1,
            context().source().digest(),
            limits,
            Deadline::after(Duration::from_secs(30)).unwrap(),
            cancel.clone(),
        )
        .unwrap();
    let mut reservation = run
        .reserve(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(32768).unwrap(),
        )])
        .unwrap();
    {
        let view = reservation.lease();
        let rows = [1];
        let atoms = [1];
        let nodes = [Formula::Atom(0), Formula::Ought(0)];
        let model =
            Model::new(Frame::new(Profile::DeonticSerialKd, &rows).unwrap(), &atoms).unwrap();
        let mut store = ModalObservationStore::with_resources(1, &view).unwrap();
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
                &view,
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
        let binding = store.binding(id, context(), scope()).unwrap();

        // A live positive proves this exact observation and scope can be consumed.
        let mut before_cancel = ledger(binding);
        store.retain(retention(id), &mut before_cancel).unwrap();
        assert!(matches!(
            before_cancel.entries(),
            [entry] if matches!(entry.facet, Facet::Uncertainty(gap)
                if gap.reason == GapReason::Unverified)
        ));

        let mut after_cancel = ledger(binding);
        let declared = after_cancel.transforms().to_vec();
        cancel.cancel();
        assert_eq!(view.check(), Err(ResourceError::Cancelled));
        assert_eq!(store.len(), 1);
        assert_eq!(store.binding(id, context(), scope()).unwrap(), binding);
        assert_eq!(
            store
                .lookup(id, context(), scope())
                .unwrap()
                .evaluation()
                .at_world(0),
            Ok(true)
        );

        let result = store.retain(retention(id), &mut after_cancel);
        assert_eq!(result, Err(ModalError::Resource(ResourceError::Cancelled)));
        assert!(after_cancel.entries().is_empty());
        assert_eq!(after_cancel.transforms(), declared.as_slice());
        assert_eq!(before_cancel.entries().len(), 1);
    }
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
    assert_eq!(broker.used(ResourceKind::Ram).unwrap(), 0);
}
