use lc631_core::{
    context::{
        ContextAdmission, ContextApplicability, ContextDelta, ContextLimits, ContextPremise,
        ContextRevision, ContextScope, OriginalContext, PremiseBasis, PremiseEdit, SourceRevision,
    },
    AuthorityRevision,
};
use lc631_runtime::{
    facet_ledger::{Facet, FacetLedger, GapReason, LedgerError},
    four_valued_context::{
        BranchError, BranchInputs, BranchObservation, BranchPlan, SourceMeaning,
    },
    resource::*,
};
use lc631_solvers::four_valued::{Atom, Truth};
use lc631_world::context_graph::ContextGraph;
use std::{sync::Arc, time::Duration};

fn d(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}
fn source() -> SourceRevision {
    SourceRevision::of("explicit finite signed Horn model")
}
fn scope() -> ContextScope {
    ContextScope::declared(d(1), d(2), d(3), d(4))
}
fn atom(n: u8) -> Atom {
    Atom::new(n).unwrap()
}
fn graph_with(rows: &[(&str, PremiseBasis, bool)]) -> ContextGraph {
    let premises = rows
        .iter()
        .enumerate()
        .map(|(i, (text, basis, changeable))| {
            ContextPremise::reported(d(10 + i as u8), Arc::from(*text), *basis, *changeable)
        })
        .collect();
    ContextGraph::new(
        OriginalContext::checked(
            source(),
            scope(),
            AuthorityRevision(7),
            premises,
            ContextLimits {
                max_contexts: 8,
                max_premises: 256,
                max_edits: 256,
                max_statement_bytes: 16 * 1024,
            },
        )
        .unwrap(),
        8,
    )
    .unwrap()
}
fn graph() -> ContextGraph {
    graph_with(&[
        ("+0", PremiseBasis::Given, false),
        ("-0", PremiseBasis::Given, true),
        ("+1 <- +0", PremiseBasis::Given, false),
    ])
}
fn owner(cancel: Cancellation) -> (ResourceBroker, RunResources, Reservation) {
    let limits = ResourceLimits::uniform(8 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let run = broker
        .begin_run(
            1,
            source().digest(),
            limits,
            Deadline::after(Duration::from_secs(30)).unwrap(),
            cancel,
        )
        .unwrap();
    let reservation = run
        .reserve(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(4 * 1024 * 1024).unwrap(),
        )])
        .unwrap();
    (broker, run, reservation)
}
fn child(graph: &mut ContextGraph) -> ContextRevision {
    let parent = graph.original().state().revision();
    let delta = ContextDelta::proposed(
        parent,
        scope(),
        AuthorityRevision(7),
        ContextApplicability::Unconditional,
        vec![PremiseEdit::Retract { id: d(11) }],
    );
    match graph.propose(delta).unwrap() {
        ContextAdmission::Inserted(revision) => revision,
        ContextAdmission::Duplicate(_) => panic!("fixture must create an actual child"),
    }
}

#[test]
fn tlcx14_context_actual_premises_query_owner_and_same_ledger_are_consumed() {
    let graph = graph();
    let root = graph.original().state().revision();
    let (broker, _run, reservation) = owner(Cancellation::default());
    let view = reservation.lease();
    let plan = BranchPlan::prepare(&graph, root, scope(), 3, atom(0), &view).unwrap();
    let inputs = BranchInputs::prepare(&plan, &view).unwrap();
    let observed = BranchObservation::execute(&inputs, &broker, 1000, &view).unwrap();
    assert_eq!(
        observed
            .truth_current(&graph, root, atom(0), &view)
            .unwrap(),
        Truth::Both
    );
    let report = observed
        .report_current(&graph, root, atom(0), &view)
        .unwrap();
    assert_eq!(report.truth(atom(1)).unwrap(), Truth::Supported);
    assert_eq!(report.truth(atom(2)).unwrap(), Truth::Neither);
    assert_eq!(report.contradiction_scope(atom(0)).unwrap(), Some(1));
    assert_eq!(observed.source_meaning(), SourceMeaning::SourceUnverified);
    assert_eq!(observed.actual_evaluations(), 1);
    let mut ledger = FacetLedger::new(4, 16).unwrap();
    let written = observed.record(&mut ledger, &view).unwrap();
    assert!(written.context_transform().is_none());
    assert_eq!(ledger.transforms().len(), 1);
    assert_eq!(ledger.entries().len(), 1);
    assert!(matches!(ledger.entries()[0].facet, Facet::Uncertainty(g)
        if g.reason == GapReason::Unverified));
    assert_eq!(ledger.for_transform(written.result_transform()).count(), 1);
    assert_eq!(
        ledger.unique_defects(ledger.transforms()[0].scope).count(),
        0
    );
    assert_eq!(ledger.unique_gains(ledger.transforms()[0].scope).count(), 0);
}

#[test]
fn tlcx14_context_retraction_changes_raw_result_and_keeps_delta_u_separate() {
    let mut graph = graph();
    let root = graph.original().state().revision();
    let next = child(&mut graph);
    let (broker, _run, reservation) = owner(Cancellation::default());
    let view = reservation.lease();
    let original = BranchPlan::prepare(&graph, root, scope(), 3, atom(0), &view).unwrap();
    let original_inputs = BranchInputs::prepare(&original, &view).unwrap();
    let before = BranchObservation::execute(&original_inputs, &broker, 1000, &view).unwrap();
    let changed = BranchPlan::prepare(&graph, next, scope(), 3, atom(0), &view).unwrap();
    let changed_inputs = BranchInputs::prepare(&changed, &view).unwrap();
    let after = BranchObservation::execute(&changed_inputs, &broker, 1000, &view).unwrap();
    assert_eq!(
        before.truth_current(&graph, root, atom(0), &view).unwrap(),
        Truth::Both
    );
    assert_eq!(
        after.truth_current(&graph, next, atom(0), &view).unwrap(),
        Truth::Supported
    );
    let mut ledger = FacetLedger::new(6, 16).unwrap();
    before.record(&mut ledger, &view).unwrap();
    let recorded = after.record(&mut ledger, &view).unwrap();
    let context = recorded.context_transform().unwrap();
    assert_ne!(context, recorded.result_transform());
    assert!(ledger
        .for_transform(context)
        .any(|e| matches!(e.facet, Facet::Delta(_))));
    assert!(ledger
        .for_transform(context)
        .any(|e| matches!(e.facet, Facet::Uncertainty(_))));
    assert_eq!(ledger.for_transform(recorded.result_transform()).count(), 1);
    assert!(ledger
        .for_transform(recorded.result_transform())
        .all(|e| matches!(e.facet, Facet::Uncertainty(_))));
    ledger
        .check_path(&[context, recorded.result_transform()])
        .unwrap();
    assert_eq!(graph.get(root).unwrap().premises().len(), 3);
    assert_eq!(graph.get(next).unwrap().premises().len(), 2);
}

#[test]
fn tlcx14_context_foreign_membership_scope_and_typed_query_cuts_fail() {
    let graph = graph();
    let foreign = graph_with(&[("+2", PremiseBasis::Given, true)]);
    let root = graph.original().state().revision();
    let (broker, _run, reservation) = owner(Cancellation::default());
    let view = reservation.lease();
    assert!(matches!(
        BranchPlan::prepare(
            &graph,
            foreign.original().state().revision(),
            scope(),
            3,
            atom(0),
            &view
        ),
        Err(BranchError::UnknownContext)
    ));
    let wrong = ContextScope::declared(d(5), scope().goal(), scope().probes(), scope().semantics());
    assert!(matches!(
        BranchPlan::prepare(&graph, root, wrong, 3, atom(0), &view),
        Err(BranchError::ScopeMismatch)
    ));
    assert!(matches!(
        BranchPlan::prepare(&graph, root, scope(), 3, atom(3), &view),
        Err(BranchError::UndeclaredAtom)
    ));
    let plan = BranchPlan::prepare(&graph, root, scope(), 3, atom(0), &view).unwrap();
    let inputs = BranchInputs::prepare(&plan, &view).unwrap();
    let observed = BranchObservation::execute(&inputs, &broker, 1000, &view).unwrap();
    assert!(matches!(
        observed.truth_current(&foreign, root, atom(0), &view),
        Err(BranchError::ForeignGraph)
    ));
    assert!(matches!(
        observed.truth_current(&graph, root, atom(1), &view),
        Err(BranchError::QueryMismatch)
    ));
}

#[test]
fn tlcx14_context_unsupported_premise_syntax_and_basis_never_disappear() {
    let (_broker, _run, reservation) = owner(Cancellation::default());
    let view = reservation.lease();
    for text in [
        "p",
        "not p",
        "+01",
        "+0 <- ",
        "+0 <- +1,",
        "+0 <- +1 <- +2",
        "+0\n+1",
        "+0 <- not +1",
        "+0 ; prose",
        "+64",
    ] {
        let graph = graph_with(&[(text, PremiseBasis::Given, true)]);
        assert!(
            BranchPlan::prepare(
                &graph,
                graph.original().state().revision(),
                scope(),
                3,
                atom(0),
                &view
            )
            .is_err(),
            "unsupported premise {text:?}"
        );
    }
    for basis in [
        PremiseBasis::Observed,
        PremiseBasis::Derived,
        PremiseBasis::Refuted,
        PremiseBasis::Conflicted,
    ] {
        let graph = graph_with(&[("+0", basis, true)]);
        assert!(matches!(
            BranchPlan::prepare(
                &graph,
                graph.original().state().revision(),
                scope(),
                3,
                atom(0),
                &view
            ),
            Err(BranchError::UnsupportedBasis)
        ));
    }
}

#[test]
fn tlcx14_context_query_and_actual_premise_changes_change_bound_payloads() {
    let graph = graph();
    let root = graph.original().state().revision();
    let (broker, _run, reservation) = owner(Cancellation::default());
    let view = reservation.lease();
    let a = BranchPlan::prepare(&graph, root, scope(), 3, atom(0), &view).unwrap();
    let b = BranchPlan::prepare(&graph, root, scope(), 3, atom(1), &view).unwrap();
    assert_ne!(a.query_digest(), b.query_digest());
    let changed = graph_with(&[("+0", PremiseBasis::Given, false)]);
    let c = BranchPlan::prepare(
        &changed,
        changed.original().state().revision(),
        scope(),
        3,
        atom(0),
        &view,
    )
    .unwrap();
    assert_ne!(a.query_digest(), c.query_digest());
    let a_inputs = BranchInputs::prepare(&a, &view).unwrap();
    let c_inputs = BranchInputs::prepare(&c, &view).unwrap();
    let a_observed = BranchObservation::execute(&a_inputs, &broker, 1000, &view).unwrap();
    let c_observed = BranchObservation::execute(&c_inputs, &broker, 1000, &view).unwrap();
    let mut ledger = FacetLedger::new(4, 8).unwrap();
    let a_recorded = a_observed.record(&mut ledger, &view).unwrap();
    let c_recorded = c_observed.record(&mut ledger, &view).unwrap();
    assert_ne!(
        ledger
            .transform(a_recorded.result_transform())
            .unwrap()
            .scope
            .baseline,
        ledger
            .transform(c_recorded.result_transform())
            .unwrap()
            .scope
            .baseline
    );
}

#[test]
fn tlcx14_context_cancel_budget_and_foreign_lease_cannot_publish() {
    let graph = graph();
    let root = graph.original().state().revision();
    let cancel = Cancellation::default();
    let (broker, _run, reservation) = owner(cancel.clone());
    let view = reservation.lease();
    let plan = BranchPlan::prepare(&graph, root, scope(), 3, atom(0), &view).unwrap();
    let inputs = BranchInputs::prepare(&plan, &view).unwrap();
    assert!(matches!(
        BranchObservation::execute(&inputs, &broker, 1, &view),
        Err(BranchError::Observation(_))
    ));
    let observed = BranchObservation::execute(&inputs, &broker, 1000, &view).unwrap();
    let (_other_broker, _other_run, other) = owner(Cancellation::default());
    assert!(matches!(
        observed.truth_current(&graph, root, atom(0), &other.lease()),
        Err(BranchError::Resource(ResourceError::ForeignOwner))
    ));
    cancel.cancel();
    assert_eq!(
        observed
            .historical_report()
            .unwrap()
            .truth(atom(0))
            .unwrap(),
        Truth::Both
    );
    let mut ledger = FacetLedger::new(4, 16).unwrap();
    assert!(matches!(
        observed.record(&mut ledger, &view),
        Err(BranchError::Resource(ResourceError::Cancelled))
    ));
    assert!(ledger.transforms().is_empty());
    assert!(ledger.entries().is_empty());
}

#[test]
fn tlcx14_context_atomic_ledger_capacity_and_duplicate_failure_keep_old_events() {
    let mut graph = graph();
    let next = child(&mut graph);
    let (broker, _run, reservation) = owner(Cancellation::default());
    let view = reservation.lease();
    let plan = BranchPlan::prepare(&graph, next, scope(), 3, atom(0), &view).unwrap();
    let inputs = BranchInputs::prepare(&plan, &view).unwrap();
    let observed = BranchObservation::execute(&inputs, &broker, 1000, &view).unwrap();
    for (transforms, entries) in [(1, 8), (8, 1)] {
        let mut ledger = FacetLedger::new(transforms, entries).unwrap();
        assert!(matches!(
            observed.record(&mut ledger, &view),
            Err(BranchError::Ledger(LedgerError::Capacity))
        ));
        assert!(ledger.transforms().is_empty());
        assert!(ledger.entries().is_empty());
    }
    let mut ledger = FacetLedger::new(8, 16).unwrap();
    observed.record(&mut ledger, &view).unwrap();
    let old_transforms = ledger.transforms().to_vec();
    let old_entries = ledger.entries().to_vec();
    assert!(matches!(
        observed.record(&mut ledger, &view),
        Err(BranchError::Ledger(LedgerError::DuplicateTransform))
    ));
    assert_eq!(ledger.transforms(), old_transforms);
    assert_eq!(ledger.entries(), old_entries);

    let root = graph.original().state().revision();
    let root_plan = BranchPlan::prepare(&graph, root, scope(), 3, atom(0), &view).unwrap();
    let root_inputs = BranchInputs::prepare(&root_plan, &view).unwrap();
    let root_observed = BranchObservation::execute(&root_inputs, &broker, 1000, &view).unwrap();
    let mut seeded = FacetLedger::new(2, 16).unwrap();
    root_observed.record(&mut seeded, &view).unwrap();
    let old_transforms = seeded.transforms().to_vec();
    let old_entries = seeded.entries().to_vec();
    assert!(matches!(
        observed.record(&mut seeded, &view),
        Err(BranchError::Ledger(LedgerError::Capacity))
    ));
    assert_eq!(seeded.transforms(), old_transforms);
    assert_eq!(seeded.entries(), old_entries);
}

#[test]
fn tlcx14_context_empty_model_and_declared_assumptions_preserve_raw_states_and_basis() {
    let (broker, _run, reservation) = owner(Cancellation::default());
    let view = reservation.lease();
    for (rows, expected) in [
        (vec![], Truth::Neither),
        (vec![("+0", PremiseBasis::Assumed, true)], Truth::Supported),
        (vec![("-0", PremiseBasis::Proposed, true)], Truth::Refuted),
    ] {
        let graph = graph_with(&rows);
        let root = graph.original().state().revision();
        let plan = BranchPlan::prepare(&graph, root, scope(), 1, atom(0), &view).unwrap();
        assert_eq!(plan.context_state().premises().len(), rows.len());
        if let Some(row) = rows.first() {
            assert_eq!(plan.context_state().premises()[0].basis(), row.1);
        }
        let inputs = BranchInputs::prepare(&plan, &view).unwrap();
        let observed = BranchObservation::execute(&inputs, &broker, 1000, &view).unwrap();
        assert_eq!(
            observed
                .truth_current(&graph, root, atom(0), &view)
                .unwrap(),
            expected
        );
        assert_eq!(observed.source_meaning(), SourceMeaning::SourceUnverified);
        assert_eq!(observed.actual_evaluations(), 1);
    }
}

#[test]
fn tlcx14_context_converged_current_model_keeps_distinct_original_baselines() {
    let mut a = graph_with(&[
        ("+0", PremiseBasis::Given, false),
        ("-0", PremiseBasis::Given, true),
    ]);
    let next = child(&mut a);
    let b = graph_with(&[("+0", PremiseBasis::Given, false)]);
    let root = b.original().state().revision();
    assert_eq!(next, root);
    let (broker, _run, reservation) = owner(Cancellation::default());
    let view = reservation.lease();
    let ap = BranchPlan::prepare(&a, next, scope(), 1, atom(0), &view).unwrap();
    let bp = BranchPlan::prepare(&b, root, scope(), 1, atom(0), &view).unwrap();
    assert_eq!(ap.query_digest(), bp.query_digest());
    let ai = BranchInputs::prepare(&ap, &view).unwrap();
    let bi = BranchInputs::prepare(&bp, &view).unwrap();
    let ao = BranchObservation::execute(&ai, &broker, 1000, &view).unwrap();
    let bo = BranchObservation::execute(&bi, &broker, 1000, &view).unwrap();
    let mut ledger = FacetLedger::new(4, 8).unwrap();
    let ar = ao.record(&mut ledger, &view).unwrap();
    let br = bo.record(&mut ledger, &view).unwrap();
    assert_ne!(ar.result_transform(), br.result_transform());
    assert_ne!(
        ledger
            .transform(ar.result_transform())
            .unwrap()
            .scope
            .baseline,
        ledger
            .transform(br.result_transform())
            .unwrap()
            .scope
            .baseline
    );
    assert_eq!(ledger.transforms().len(), 3);
}
