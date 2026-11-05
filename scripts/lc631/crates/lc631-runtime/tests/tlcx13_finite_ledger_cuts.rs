//! Independently cut the ledger checks after a successful actual store lookup.
use lc631_core::context::{ContextRevision, ContextScope, SourceRevision};
use lc631_runtime::facet_ledger::*;
use lc631_runtime::finite_results::{
    FiniteLedgerError, FiniteResultId, FiniteResultStore, TemporalLedgerRequest,
};
use lc631_runtime::resource::{
    Cancellation, Deadline, ResourceAmount, ResourceBroker, ResourceKind, ResourceLimits,
    ResourceRequest,
};
use lc631_solvers::temporal::*;
use lc631_substrate::Digest32;
use std::time::Duration;

fn fixture(
    truth: Truth,
    body: impl FnOnce(
        &FiniteResultStore<'_, '_>,
        FiniteResultId,
        &TemporalQuery,
        &FiniteTrace,
        ContextRevision,
        ContextScope,
    ),
) {
    let source = SourceRevision::of("finite ledger cut profile");
    let context = ContextRevision::original(source);
    let d = |n| Digest32::from_bytes([n; 32]);
    let scope = ContextScope::declared(d(1), d(2), d(3), d(4));
    let query = TemporalQuery::checked(
        context,
        vec![TemporalNode::Atom(0)],
        0,
        TemporalProfile::ClosedFiniteKleeneV1,
    )
    .unwrap();
    let clock = ClockSpec::exact(d(5), TimeScale::Monotonic, d(6), d(7));
    let trace = FiniteTrace::checked(
        vec![TraceStep::new(
            Instant::absolute(clock, 0, TimeUnit::Second).unwrap(),
            vec![truth],
        )],
        TraceClosure::Closed,
    )
    .unwrap();
    let limits = ResourceLimits::uniform(1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let run = broker
        .begin_run(
            1,
            source.digest(),
            limits,
            Deadline::after(Duration::from_secs(30)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let mut reservation = run
        .reserve(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(1024 * 1024).unwrap(),
        )])
        .unwrap();
    {
        let view = reservation.lease();
        let mut store = FiniteResultStore::with_resources(1, &view).unwrap();
        let id = store
            .evaluate_temporal(
                &query,
                &trace,
                context,
                scope,
                EvaluationBudget::default(),
                &view,
            )
            .unwrap();
        body(&store, id, &query, &trace, context, scope);
    }
    reservation.release_confirmed().unwrap();
}

fn transform(binding: FiniteObservationBinding, scope: ContextScope) -> TransformBinding {
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
fn tlcx13_ledger_scope_and_revision_cuts_are_reached_after_valid_store_lookup() {
    fixture(Truth::True, |store, id, query, trace, context, scope| {
        let binding = store
            .temporal_binding(id, query, trace, context, scope)
            .unwrap();
        for cut in 0..8 {
            let mut changed = transform(binding, scope);
            let wrong = Revision([10; 32]);
            match cut {
                0 => changed.scope.id = ScopeId(wrong.0),
                1 => changed.scope.root_goal = wrong,
                2 => changed.scope.probes = wrong,
                3 => changed.scope.policy = wrong,
                4 => changed.source = wrong,
                5 => changed.source_context = wrong,
                6 => changed.query = wrong,
                _ => changed.result = wrong,
            }
            let mut ledger = FacetLedger::new(1, 1).unwrap();
            ledger.admit_transform(changed).unwrap();
            let result = store.retain_temporal_observation(
                TemporalLedgerRequest {
                    observation: id,
                    query,
                    trace,
                    context,
                    scope,
                    transform: changed.id,
                    property: PropertyId([11; 32]),
                    required_evidence: wrong,
                    next_action: wrong,
                },
                &mut ledger,
            );
            let expected = if cut < 4 {
                LedgerError::ScopeMismatch
            } else {
                LedgerError::RevisionMismatch
            };
            assert_eq!(
                result,
                Err(FiniteLedgerError::Ledger(expected)),
                "cut {cut}"
            );
            assert!(ledger.entries().is_empty());
        }
    });
}

#[test]
fn tlcx13_true_false_unknown_observations_all_remain_unverified_not_defects_or_gains() {
    for truth in [Truth::False, Truth::Unknown, Truth::True] {
        fixture(truth, |store, id, query, trace, context, scope| {
            let binding = store
                .temporal_binding(id, query, trace, context, scope)
                .unwrap();
            let transform = transform(binding, scope);
            let mut ledger = FacetLedger::new(1, 1).unwrap();
            ledger.admit_transform(transform).unwrap();
            store
                .retain_temporal_observation(
                    TemporalLedgerRequest {
                        observation: id,
                        query,
                        trace,
                        context,
                        scope,
                        transform: transform.id,
                        property: PropertyId([11; 32]),
                        required_evidence: Revision([12; 32]),
                        next_action: Revision([13; 32]),
                    },
                    &mut ledger,
                )
                .unwrap();
            assert!(matches!(
                ledger.entries()[0].facet,
                Facet::Uncertainty(Gap {
                    reason: GapReason::Unverified,
                    ..
                })
            ));
            assert_eq!(ledger.unique_defects(transform.scope).count(), 0);
            assert_eq!(ledger.unique_gains(transform.scope).count(), 0);
            assert_eq!(
                store
                    .lookup_temporal(id, query, trace, context, scope)
                    .unwrap()
                    .truth(),
                truth
            );
        });
    }
}
