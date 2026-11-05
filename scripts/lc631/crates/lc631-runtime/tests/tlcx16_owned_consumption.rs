//! Hand-derived causal/spatial consumer fixtures, not solver-generated gold.
use lc631_core::context::{ContextRevision, ContextScope, SourceRevision};
use lc631_runtime::causal_spatial_observation::*;
use lc631_runtime::facet_ledger::*;
use lc631_runtime::resource::*;
use lc631_solvers::causal_spatial::*;
use lc631_substrate::resource::ResourceClaim;
use std::time::Duration;

fn scope() -> ContextScope {
    let d = Digest32::from_bytes([3; 32]);
    ContextScope::declared(d, d, d, d)
}
fn query(observed: Assignment, intervention: Assignment) -> Query {
    Query {
        observed,
        intervention,
        target: Variable(2),
        value: true,
        work_limit: 4096,
    }
}
fn fixture(body: impl FnOnce(ContextRevision, &ResourceReservationView<'_>, &Cancellation)) {
    fixture_with_ram(1024 * 1024, body)
}
fn fixture_with_ram(
    ram: u64,
    body: impl FnOnce(ContextRevision, &ResourceReservationView<'_>, &Cancellation),
) {
    let source = SourceRevision::of("causal spatial actual consumer");
    let limits = ResourceLimits::uniform(1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let cancel = Cancellation::default();
    let run = broker
        .begin_run(
            1,
            source.digest(),
            limits,
            Deadline::after(Duration::from_secs(30)).unwrap(),
            cancel.clone(),
        )
        .unwrap();
    let mut reservation = run
        .reserve(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(ram).unwrap(),
        )])
        .unwrap();
    {
        let view = reservation.lease();
        body(ContextRevision::original(source), &view, &cancel);
    }
    reservation.release_confirmed().unwrap();
}

#[test]
fn tlcx16_actual_conditioning_and_intervention_are_distinct_and_model_swaps_fail() {
    fixture(|context, view, _| {
        let equations = [
            Equation::Exogenous(0),
            Equation::Table {
                parents: 1,
                values: [2, 0, 0, 0],
            },
            Equation::Table {
                parents: 1,
                values: [2, 0, 0, 0],
            },
        ];
        let model = Scm::new(&equations, 1, &[0, 1], Assignment::empty()).unwrap();
        let conditioned = Request::Causal {
            model: &model,
            query: query(Assignment::new(2, 2).unwrap(), Assignment::empty()),
            context,
            scope: scope(),
        };
        let intervened = Request::Causal {
            model: &model,
            query: query(Assignment::empty(), Assignment::new(2, 2).unwrap()),
            context,
            scope: scope(),
        };
        let mut owner = CausalSpatialObservations::with_resources(2, view).unwrap();
        let first = owner.evaluate(&conditioned, view).unwrap();
        let second = owner.evaluate(&intervened, view).unwrap();
        let Evaluation::Computed(a) = owner
            .lookup(first, &conditioned)
            .unwrap()
            .causal()
            .unwrap()
            .outcome()
        else {
            panic!("conditioning unknown")
        };
        let Evaluation::Computed(b) = owner
            .lookup(second, &intervened)
            .unwrap()
            .causal()
            .unwrap()
            .outcome()
        else {
            panic!("intervention unknown")
        };
        assert_eq!((a.true_worlds, a.false_worlds), (1, 0));
        assert_eq!((b.true_worlds, b.false_worlds), (1, 1));
        assert_ne!(first, second);
        let binding = owner.lookup(first, &conditioned).unwrap().ledger_binding();
        let mut consumed = ledger(binding, scope(), view);
        owner
            .retain_current(
                first,
                &conditioned,
                LedgerTarget {
                    transform: TransformId([8; 32]),
                    property: PropertyId([9; 32]),
                    required_evidence: Revision([10; 32]),
                    next_action: Revision([11; 32]),
                },
                &mut consumed.ledger,
                view,
            )
            .unwrap();
        assert!(matches!(
            consumed.ledger.entries()[0].facet,
            Facet::Uncertainty(_)
        ));
        assert!(owner.lookup(first, &intervened).is_err());
        let changed = Scm::new(&equations, 1, &[0], Assignment::empty()).unwrap();
        let swapped = Request::Causal {
            model: &changed,
            query: query(Assignment::new(2, 2).unwrap(), Assignment::empty()),
            context,
            scope: scope(),
        };
        assert!(owner.lookup(first, &swapped).is_err());
        assert_eq!(owner.len(), 2);
        assert!(matches!(
            owner.evaluate(&intervened, view),
            Err(ObservationError::CapacityReached)
        ));
    });
}

struct OwnedLedger<'view> {
    ledger: FacetLedger,
    _claim: ResourceClaim<'view>,
}
fn ledger<'view>(
    binding: FiniteObservationBinding,
    s: ContextScope,
    view: &'view ResourceReservationView<'_>,
) -> OwnedLedger<'view> {
    let bytes = (std::mem::size_of::<TransformBinding>() + std::mem::size_of::<Entry>()) as u64;
    let claim = view
        .claim_resources(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(bytes).unwrap(),
        )])
        .unwrap();
    let mut ledger = FacetLedger::new(1, 1).unwrap();
    let r = Revision([7; 32]);
    ledger
        .admit_transform(TransformBinding {
            id: TransformId([8; 32]),
            source: binding.source,
            target: r,
            source_context: binding.context,
            target_context: r,
            direction: Direction::Forward,
            scope: ComparisonScope {
                id: ScopeId(*s.comparison().as_bytes()),
                baseline: r,
                root_goal: Revision(*s.goal().as_bytes()),
                probes: Revision(*s.probes().as_bytes()),
                policy: Revision(*s.semantics().as_bytes()),
            },
            producer: r,
            query: binding.query,
            result: binding.result,
        })
        .unwrap();
    OwnedLedger {
        ledger,
        _claim: claim,
    }
}

#[test]
fn tlcx16_actual_unknown_spatial_and_causal_outputs_consume_only_u_and_reject_ledger_cuts() {
    fixture(|context, view, _| {
        let frame = FrameId([4; 32]);
        let spatial = Request::Spatial {
            left: None,
            right: None,
            relation: SpatialRelation::Connected,
            frame,
            context,
            scope: scope(),
        };
        let mut owner = CausalSpatialObservations::with_resources(2, view).unwrap();
        let id = owner.evaluate(&spatial, view).unwrap();
        let observed = owner.lookup(id, &spatial).unwrap();
        assert_eq!(
            observed.spatial().unwrap().outcome(),
            Evaluation::Unknown(UnknownReason::MissingSpatialObject)
        );
        let binding = observed.ledger_binding();
        let target = LedgerTarget {
            transform: TransformId([8; 32]),
            property: PropertyId([9; 32]),
            required_evidence: Revision([10; 32]),
            next_action: Revision([11; 32]),
        };
        let mut actual = ledger(binding, scope(), view);
        owner
            .retain_current(id, &spatial, target, &mut actual.ledger, view)
            .unwrap();
        assert!(matches!(
            actual.ledger.entries()[0].facet,
            Facet::Uncertainty(_)
        ));
        assert_eq!(
            actual
                .ledger
                .unique_defects(actual.ledger.transforms()[0].scope)
                .count(),
            0
        );
        assert_eq!(
            actual
                .ledger
                .unique_gains(actual.ledger.transforms()[0].scope)
                .count(),
            0
        );
        for cut in 0..8 {
            let mut wrong = binding;
            match cut {
                0 => wrong.source = Revision([12; 32]),
                1 => wrong.context = Revision([12; 32]),
                2 => wrong.query = Revision([12; 32]),
                3 => wrong.result = Revision([12; 32]),
                _ => {}
            }
            let s = scope();
            let d = Digest32::from_bytes([12; 32]);
            let changed_scope = match cut {
                4 => ContextScope::declared(d, s.goal(), s.probes(), s.semantics()),
                5 => ContextScope::declared(s.comparison(), d, s.probes(), s.semantics()),
                6 => ContextScope::declared(s.comparison(), s.goal(), d, s.semantics()),
                7 => ContextScope::declared(s.comparison(), s.goal(), s.probes(), d),
                _ => s,
            };
            let mut changed = ledger(wrong, changed_scope, view);
            assert!(owner
                .retain_current(id, &spatial, target, &mut changed.ledger, view)
                .is_err());
            assert!(changed.ledger.entries().is_empty());
        }
        let equations = [Equation::Missing];
        let model = Scm::new(&equations, 0, &[0], Assignment::empty()).unwrap();
        let causal = Request::Causal {
            model: &model,
            query: Query {
                observed: Assignment::empty(),
                intervention: Assignment::empty(),
                target: Variable(0),
                value: true,
                work_limit: 16,
            },
            context,
            scope: scope(),
        };
        let missing = owner.evaluate(&causal, view).unwrap();
        assert_eq!(
            owner
                .lookup(missing, &causal)
                .unwrap()
                .causal()
                .unwrap()
                .outcome(),
            Evaluation::Unknown(UnknownReason::MissingEquation(Variable(0)))
        );
        let mut causal_ledger = ledger(
            owner.lookup(missing, &causal).unwrap().ledger_binding(),
            scope(),
            view,
        );
        owner
            .retain_current(missing, &causal, target, &mut causal_ledger.ledger, view)
            .unwrap();
        assert!(matches!(
            causal_ledger.ledger.entries()[0].facet,
            Facet::Uncertainty(_)
        ));
    });
}

#[test]
fn tlcx16_frame_context_scope_mutations_and_current_cancel_cannot_reuse_history() {
    fixture(|context, view, cancel| {
        let frame = FrameId([4; 32]);
        let rect = Rectangle::new(frame, [0, 0], [2, 2]).unwrap();
        let request = Request::Spatial {
            left: Some(rect),
            right: Some(rect),
            relation: SpatialRelation::Contains,
            frame,
            context,
            scope: scope(),
        };
        let mut owner = CausalSpatialObservations::with_resources(1, view).unwrap();
        let id = owner.evaluate(&request, view).unwrap();
        let wrong_frame = Request::Spatial {
            left: Some(rect),
            right: Some(rect),
            relation: SpatialRelation::Contains,
            frame: FrameId([5; 32]),
            context,
            scope: scope(),
        };
        assert!(owner.lookup(id, &wrong_frame).is_err());
        let changed = Request::Spatial {
            left: Some(rect),
            right: Some(rect),
            relation: SpatialRelation::Contains,
            frame,
            context: context.reconstructed([6; 32]),
            scope: scope(),
        };
        assert!(owner.lookup(id, &changed).is_err());
        let d = Digest32::from_bytes([6; 32]);
        let changed_scope = Request::Spatial {
            left: Some(rect),
            right: Some(rect),
            relation: SpatialRelation::Contains,
            frame,
            context,
            scope: ContextScope::declared(d, d, d, d),
        };
        assert!(owner.lookup(id, &changed_scope).is_err());
        let b = owner.lookup(id, &request).unwrap().ledger_binding();
        let mut l = ledger(b, scope(), view);
        cancel.cancel();
        assert!(owner.lookup(id, &request).is_ok()); // historical only
        assert!(owner.evaluate(&request, view).is_err());
        assert!(owner
            .retain_current(
                id,
                &request,
                LedgerTarget {
                    transform: TransformId([8; 32]),
                    property: PropertyId([9; 32]),
                    required_evidence: Revision([10; 32]),
                    next_action: Revision([11; 32])
                },
                &mut l.ledger,
                view
            )
            .is_err());
        assert!(l.ledger.entries().is_empty());
    });
}

#[test]
fn tlcx16_workspace_refusal_retains_empty_batch_and_refunds_owned_metadata() {
    fixture_with_ram(
        std::mem::size_of::<Observation>() as u64 + 1,
        |context, view, _| {
            let mut owner = CausalSpatialObservations::with_resources(1, view).unwrap();
            let request = Request::Spatial {
                left: None,
                right: None,
                relation: SpatialRelation::Connected,
                frame: FrameId([4; 32]),
                context,
                scope: scope(),
            };
            assert!(matches!(
                owner.evaluate(&request, view),
                Err(ObservationError::Resource(_))
            ));
            assert!(owner.is_empty());
        },
    );
}
