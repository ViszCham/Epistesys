use lc631_core::context::{ContextRevision, ContextScope, SourceRevision};
use lc631_runtime::{
    four_valued_observation::{FourValuedError, FourValuedObservationStore, FourValuedRequest},
    resource::*,
};
use lc631_solvers::four_valued::{Atom, Literal, Rule, Truth};
use std::time::Duration;

fn context() -> ContextRevision {
    ContextRevision::original(SourceRevision::of("four valued owned input"))
}
fn scope() -> ContextScope {
    let d = Digest32::from_bytes([3; 32]);
    ContextScope::declared(d, d, d, d)
}
fn owner(cancel: Cancellation, ram: u64) -> (ResourceBroker, RunResources, Reservation) {
    let limits = ResourceLimits::uniform(2 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let run = broker
        .begin_run(
            1,
            context().source().digest(),
            limits,
            Deadline::after(Duration::from_secs(30)).unwrap(),
            cancel,
        )
        .unwrap();
    let reservation = run
        .reserve(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(ram).unwrap(),
        )])
        .unwrap();
    (broker, run, reservation)
}
fn request<'a>(facts: &'a [Literal], rules: &'a [Rule<'a>]) -> FourValuedRequest<'a> {
    FourValuedRequest {
        atoms: 3,
        facts,
        rules,
        context: context(),
        scope: scope(),
        query: [4; 32],
    }
}

#[test]
fn tlcx14_actual_evaluation_consumes_both_neither_and_local_dependencies() {
    let p = Atom::new(0).unwrap();
    let q = Atom::new(1).unwrap();
    let unrelated = Atom::new(2).unwrap();
    let facts = [Literal::positive(p), Literal::negative(p)];
    let body = [Literal::positive(p)];
    let rules = [Rule {
        body: &body,
        head: Literal::positive(q),
    }];
    let req = request(&facts, &rules);
    let (broker, _run, mut reservation) = owner(Cancellation::default(), 1024 * 1024);
    {
        let view = reservation.lease();
        let mut store = FourValuedObservationStore::with_resources(&broker, 2, &view).unwrap();
        let id = store.evaluate(req, 1000, &view).unwrap();
        let report = store.current(id, req, &view).unwrap();
        assert_eq!(report.truth(p).unwrap(), Truth::Both);
        assert_eq!(report.truth(q).unwrap(), Truth::Supported);
        assert_eq!(report.truth(unrelated).unwrap(), Truth::Neither);
        assert_eq!(report.contradiction_scope(p).unwrap(), Some(1));
        assert_eq!(report.dependencies(Literal::positive(q)).unwrap(), Some(1));
        let changed = context().reconstructed([9; 32]);
        assert!(matches!(
            store.historical(
                id,
                FourValuedRequest {
                    context: changed,
                    ..req
                }
            ),
            Err(FourValuedError::BindingMismatch)
        ));
        let changed_facts = [Literal::negative(p)];
        assert!(matches!(
            store.historical(
                id,
                FourValuedRequest {
                    facts: &changed_facts,
                    ..req
                }
            ),
            Err(FourValuedError::BindingMismatch)
        ));
        let foreign_scope = ContextScope::declared(
            Digest32::from_bytes([7; 32]),
            scope().goal(),
            scope().probes(),
            scope().semantics(),
        );
        assert!(matches!(
            store.historical(
                id,
                FourValuedRequest {
                    scope: foreign_scope,
                    ..req
                }
            ),
            Err(FourValuedError::BindingMismatch)
        ));
        assert_eq!(store.len(), 1);
    }
    reservation.release_confirmed().unwrap();
}

#[test]
fn tlcx14_exhausted_evaluation_does_not_publish_neither_or_partial_both() {
    let p = Atom::new(0).unwrap();
    let facts = [Literal::positive(p), Literal::negative(p)];
    let req = request(&facts, &[]);
    let (broker, _run, reservation) = owner(Cancellation::default(), 1024 * 1024);
    let view = reservation.lease();
    let mut store = FourValuedObservationStore::with_resources(&broker, 1, &view).unwrap();
    assert!(matches!(
        store.evaluate(req, 1, &view),
        Err(FourValuedError::Evaluation(
            lc631_solvers::four_valued::Error::ResourceExhausted
        ))
    ));
    assert!(store.is_empty());
    let id = store.evaluate(req, 10, &view).unwrap();
    assert_eq!(
        store.current(id, req, &view).unwrap().truth(p).unwrap(),
        Truth::Both
    );
}

#[test]
fn tlcx14_foreign_broker_lease_and_capacity_are_rejected_before_publication() {
    let (broker, _run, reservation) = owner(Cancellation::default(), 4096);
    let view = reservation.lease();
    assert!(matches!(
        FourValuedObservationStore::with_resources(&broker, 4096, &view),
        Err(FourValuedError::Resource(ResourceError::Exhausted(
            ResourceKind::Ram
        )))
    ));
    let (foreign, _foreign_run, other) = owner(Cancellation::default(), 1024 * 1024);
    let other_view = other.lease();
    assert!(matches!(
        FourValuedObservationStore::with_resources(&foreign, 1, &view),
        Err(FourValuedError::Resource(ResourceError::ForeignOwner))
    ));
    let facts = [];
    let req = request(&facts, &[]);
    let mut store = FourValuedObservationStore::with_resources(&broker, 1, &view).unwrap();
    assert!(matches!(
        store.evaluate(req, 10, &other_view),
        Err(FourValuedError::Resource(ResourceError::ForeignOwner))
    ));
    assert!(store.is_empty());
}

#[test]
fn tlcx14_cancellation_blocks_current_consumption_but_preserves_historical_bits() {
    let p = Atom::new(0).unwrap();
    let facts = [Literal::positive(p), Literal::negative(p)];
    let req = request(&facts, &[]);
    let cancel = Cancellation::default();
    let (broker, _run, reservation) = owner(cancel.clone(), 1024 * 1024);
    let view = reservation.lease();
    let mut store = FourValuedObservationStore::with_resources(&broker, 1, &view).unwrap();
    let id = store.evaluate(req, 10, &view).unwrap();
    cancel.cancel();
    assert_eq!(
        store.historical(id, req).unwrap().truth(p).unwrap(),
        Truth::Both
    );
    assert!(matches!(
        store.current(id, req, &view),
        Err(FourValuedError::Resource(ResourceError::Cancelled))
    ));
}

fn transform(
    binding: lc631_runtime::facet_ledger::FiniteObservationBinding,
    scope: ContextScope,
) -> lc631_runtime::facet_ledger::TransformBinding {
    use lc631_runtime::facet_ledger::*;
    let rev = Revision([8; 32]);
    TransformBinding {
        id: TransformId([9; 32]),
        source: binding.source,
        target: rev,
        source_context: binding.context,
        target_context: rev,
        direction: Direction::Forward,
        scope: ComparisonScope {
            id: ScopeId(*scope.comparison().as_bytes()),
            baseline: rev,
            root_goal: Revision(*scope.goal().as_bytes()),
            probes: Revision(*scope.probes().as_bytes()),
            policy: Revision(*scope.semantics().as_bytes()),
        },
        producer: rev,
        query: binding.query,
        result: binding.result,
    }
}

#[test]
fn tlcx14_actual_four_values_reach_only_unverified_uncertainty() {
    use lc631_runtime::facet_ledger::*;
    let p = Atom::new(0).unwrap();
    for (facts, expected) in [
        (vec![], Truth::Neither),
        (vec![Literal::positive(p)], Truth::Supported),
        (vec![Literal::negative(p)], Truth::Refuted),
        (
            vec![Literal::positive(p), Literal::negative(p)],
            Truth::Both,
        ),
    ] {
        let req = request(&facts, &[]);
        let (broker, _run, reservation) = owner(Cancellation::default(), 1024 * 1024);
        let view = reservation.lease();
        let mut store = FourValuedObservationStore::with_resources(&broker, 1, &view).unwrap();
        let id = store.evaluate(req, 100, &view).unwrap();
        assert_eq!(
            store.current(id, req, &view).unwrap().truth(p).unwrap(),
            expected
        );
        let binding = store.binding(id, req).unwrap();
        let declared = transform(binding, scope());
        let mut ledger = FacetLedger::new(1, 1).unwrap();
        ledger.admit_transform(declared).unwrap();
        assert_eq!(
            store
                .retain_unverified(
                    id,
                    req,
                    &view,
                    &mut ledger,
                    declared.id,
                    PropertyId([10; 32]),
                    Revision([11; 32]),
                    Revision([12; 32])
                )
                .unwrap(),
            binding
        );
        assert!(matches!(
            ledger.entries()[0].facet,
            Facet::Uncertainty(Gap {
                reason: GapReason::Unverified,
                ..
            })
        ));
        assert_eq!(ledger.unique_defects(declared.scope).count(), 0);
        assert_eq!(ledger.unique_gains(declared.scope).count(), 0);
    }
}

#[test]
fn tlcx14_ledger_cuts_reject_real_observation_scope_and_lineage_swaps() {
    use lc631_runtime::facet_ledger::*;
    let p = Atom::new(0).unwrap();
    let facts = [Literal::positive(p), Literal::negative(p)];
    let req = request(&facts, &[]);
    let cancel = Cancellation::default();
    let (broker, _run, reservation) = owner(cancel.clone(), 1024 * 1024);
    let view = reservation.lease();
    let mut store = FourValuedObservationStore::with_resources(&broker, 1, &view).unwrap();
    let id = store.evaluate(req, 100, &view).unwrap();
    let binding = store.binding(id, req).unwrap();
    for cut in 0..8 {
        let mut declared = transform(binding, scope());
        let wrong = Revision([13; 32]);
        match cut {
            0 => declared.scope.id = ScopeId(wrong.0),
            1 => declared.scope.root_goal = wrong,
            2 => declared.scope.probes = wrong,
            3 => declared.scope.policy = wrong,
            4 => declared.source = wrong,
            5 => declared.source_context = wrong,
            6 => declared.query = wrong,
            _ => declared.result = wrong,
        }
        let mut ledger = FacetLedger::new(1, 1).unwrap();
        ledger.admit_transform(declared).unwrap();
        let result = store.retain_unverified(
            id,
            req,
            &view,
            &mut ledger,
            declared.id,
            PropertyId([10; 32]),
            wrong,
            wrong,
        );
        assert_eq!(
            result,
            Err(FourValuedError::Ledger(if cut < 4 {
                LedgerError::ScopeMismatch
            } else {
                LedgerError::RevisionMismatch
            })),
            "cut {cut}"
        );
        assert!(ledger.entries().is_empty());
    }
    let declared = transform(binding, scope());
    let mut ledger = FacetLedger::new(1, 1).unwrap();
    ledger.admit_transform(declared).unwrap();
    cancel.cancel();
    assert!(matches!(
        store.retain_unverified(
            id,
            req,
            &view,
            &mut ledger,
            declared.id,
            PropertyId([10; 32]),
            Revision([11; 32]),
            Revision([12; 32])
        ),
        Err(FourValuedError::Resource(ResourceError::Cancelled))
    ));
    assert!(ledger.entries().is_empty());
}
