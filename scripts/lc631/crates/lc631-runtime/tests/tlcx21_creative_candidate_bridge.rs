use lc631_core::context::{
    ContextLimits, ContextPremise, ContextScope, OriginalContext, PremiseBasis, SourceRevision,
};
use lc631_core::AuthorityRevision;
use lc631_runtime::creative_candidate_bridge::*;
use lc631_runtime::resource::{ResourceBroker, ResourceLimits, RunResources};
use lc631_solvers::typed_query::{BoolExpr, BoolSymbolId, SmtEmitLimits, SmtStatusQuery};
use lc631_substrate::resource::{
    Cancellation, Deadline, ResourceAmount, ResourceKind, ResourceRequest,
};
use lc631_substrate::source::{SourceSpan, SourceStore};
use lc631_substrate::Digest32;
use std::{sync::Arc, time::Duration};

fn d(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}
fn var(n: u32) -> BoolExpr {
    BoolExpr::Variable(BoolSymbolId::new(n))
}
fn goal() -> SmtStatusQuery {
    SmtStatusQuery::checked(
        vec![BoolSymbolId::new(0), BoolSymbolId::new(1)],
        vec![],
        vec![var(0)],
        SmtEmitLimits::default(),
    )
    .unwrap()
}
fn original(source: &SourceStore) -> OriginalContext {
    OriginalContext::checked(
        SourceRevision::of(source.text()),
        ContextScope::declared(d(1), d(2), d(3), d(4)),
        AuthorityRevision(7),
        vec![
            ContextPremise::from_source(
                d(5),
                source,
                SourceSpan::checked(source, 0, 7).unwrap(),
                PremiseBasis::Given,
                true,
            )
            .unwrap(),
            ContextPremise::from_source(
                d(6),
                source,
                SourceSpan::checked(source, 8, 9).unwrap(),
                PremiseBasis::Given,
                false,
            )
            .unwrap(),
        ],
        ContextLimits {
            max_contexts: 8,
            max_premises: 2,
            max_edits: 1,
            max_statement_bytes: 8192,
        },
    )
    .unwrap()
}
fn limits() -> BridgeLimits {
    BridgeLimits {
        max_candidates: 4,
        max_ast_nodes: 128,
        max_validation_checks: 10000,
        max_payload_bytes: 8192,
    }
}
fn owner(source: &SourceStore, cancel: Cancellation) -> (ResourceBroker, RunResources) {
    let broker = ResourceBroker::new(ResourceLimits::uniform(64 * 1024 * 1024).unwrap()).unwrap();
    let run = broker
        .begin_run(
            1,
            source.id().digest(),
            ResourceLimits::uniform(64 * 1024 * 1024).unwrap(),
            Deadline::after(Duration::from_secs(60)).unwrap(),
            cancel,
        )
        .unwrap();
    (broker, run)
}
fn req(kind: ResourceKind, amount: u64) -> ResourceRequest {
    ResourceRequest::new(kind, ResourceAmount::checked(amount).unwrap())
}
fn fixture<T>(body: impl FnOnce(&SourceStore, &[GivenPremise<'_>], &SmtStatusQuery) -> T) -> T {
    let source = SourceStore::new(Arc::from("p and q\nq"), 128).unwrap();
    let base = BoolExpr::And(vec![var(0), var(1)]);
    let hard = var(1);
    let goal = goal();
    let bindings = [
        GivenPremise {
            id: d(5),
            span: SourceSpan::checked(&source, 0, 7).unwrap(),
            formula: &base,
        },
        GivenPremise {
            id: d(6),
            span: SourceSpan::checked(&source, 8, 9).unwrap(),
            formula: &hard,
        },
    ];
    body(&source, &bindings, &goal)
}

#[test]
fn tlcx21_actual_reframe_generates_context_and_separate_model_checker_accepts() {
    fixture(|source, bindings, goal| {
        let (_b, run) = owner(source, Cancellation::default());
        let reservation = run
            .reserve(&[
                req(ResourceKind::Ram, 32 * 1024 * 1024),
                req(ResourceKind::Output, 1024 * 1024),
            ])
            .unwrap();
        let view = reservation.lease();
        let mut bridge = CreativeCandidateBridge::with_resources(
            source,
            original(source),
            d(5),
            bindings,
            GoalPolicy {
                revision: d(2),
                query: goal,
            },
            limits(),
            &view,
        )
        .unwrap();
        let root = bridge.contexts().original().state().revision();
        let candidate = bridge.generate(Operator::ReframeDeMorgan, &view).unwrap();
        assert_ne!(candidate.prepared().target(), root);
        assert!(!candidate.prepared().is_duplicate());
        assert_eq!(candidate.validation().assignments(), 4);
        assert_eq!(candidate.validation().feasible_models(), 1);
        assert_eq!(
            candidate.validation().boundary(),
            ValidationBoundary::SeparateAlgorithmSharedIrGivenSource
        );
        let query = candidate.project_current(&bridge, &view).unwrap();
        assert_eq!(query.assertions().len(), 2);
        let state = bridge
            .contexts()
            .get(candidate.prepared().target())
            .unwrap();
        assert_eq!(
            state
                .premises()
                .iter()
                .find(|p| p.id() == d(5))
                .unwrap()
                .basis(),
            PremiseBasis::Proposed
        );
        assert_eq!(
            state.declared_delta().unwrap().authority(),
            AuthorityRevision(7)
        );
        assert_eq!(
            bridge.contexts().original().state().premises()[0].basis(),
            PremiseBasis::Given
        );
    });
}

#[test]
fn tlcx21_duplicate_payload_retains_context_aliases_without_new_novelty() {
    fixture(|source, bindings, goal| {
        let (_b, run) = owner(source, Cancellation::default());
        let reservation = run
            .reserve(&[
                req(ResourceKind::Ram, 32 * 1024 * 1024),
                req(ResourceKind::Output, 1024 * 1024),
            ])
            .unwrap();
        let view = reservation.lease();
        let mut bridge = CreativeCandidateBridge::with_resources(
            source,
            original(source),
            d(5),
            bindings,
            GoalPolicy {
                revision: d(2),
                query: goal,
            },
            limits(),
            &view,
        )
        .unwrap();
        let first = bridge.generate(Operator::ReframeDeMorgan, &view).unwrap();
        let alias = bridge.generate(Operator::ReframeDeMorgan, &view).unwrap();
        assert_eq!(first.prepared().target(), alias.prepared().target());
        assert!(alias.prepared().is_duplicate());
        assert_eq!(bridge.proposals().len(), 2);
        assert_eq!(bridge.contexts().states().len(), 2);
        assert_eq!(first.historical_payload(), alias.historical_payload());
    });
}

#[test]
fn tlcx21_existing_source_premise_recombination_is_checked_and_invalid_goal_is_rejected() {
    fixture(|source, bindings, goal| {
        let (_b, run) = owner(source, Cancellation::default());
        let reservation = run
            .reserve(&[
                req(ResourceKind::Ram, 32 * 1024 * 1024),
                req(ResourceKind::Output, 1024 * 1024),
            ])
            .unwrap();
        let view = reservation.lease();
        let mut bridge = CreativeCandidateBridge::with_resources(
            source,
            original(source),
            d(5),
            bindings,
            GoalPolicy {
                revision: d(2),
                query: goal,
            },
            limits(),
            &view,
        )
        .unwrap();
        assert!(matches!(
            bridge.generate(
                Operator::Recombine {
                    partner: d(6),
                    connective: Connective::Or
                },
                &view
            ),
            Err(CreativeError::GoalCounterexample { assignment: 2 })
        ));
        assert_eq!(bridge.proposals().len(), 0);
        assert_eq!(bridge.contexts().states().len(), 1);
        let checked = bridge
            .generate(
                Operator::Recombine {
                    partner: d(6),
                    connective: Connective::And,
                },
                &view,
            )
            .unwrap();
        assert_eq!(checked.validation().feasible_models(), 1);
        assert_eq!(bridge.proposals().len(), 1);
    });
}

#[test]
fn tlcx21_hard_target_foreign_binding_and_changed_goal_policy_are_rejected() {
    fixture(|source, bindings, goal| {
        let (_b, run) = owner(source, Cancellation::default());
        let reservation = run
            .reserve(&[
                req(ResourceKind::Ram, 32 * 1024 * 1024),
                req(ResourceKind::Output, 1024 * 1024),
            ])
            .unwrap();
        let view = reservation.lease();
        assert!(matches!(
            CreativeCandidateBridge::with_resources(
                source,
                original(source),
                d(6),
                bindings,
                GoalPolicy {
                    revision: d(2),
                    query: goal
                },
                limits(),
                &view
            ),
            Err(CreativeError::HardInvariant)
        ));
        assert!(matches!(
            CreativeCandidateBridge::with_resources(
                source,
                original(source),
                d(5),
                bindings,
                GoalPolicy {
                    revision: d(99),
                    query: goal
                },
                limits(),
                &view
            ),
            Err(CreativeError::ScopeMismatch)
        ));
        let bad = [
            GivenPremise {
                id: d(5),
                span: bindings[1].span,
                formula: bindings[0].formula,
            },
            GivenPremise {
                id: d(6),
                span: bindings[1].span,
                formula: bindings[1].formula,
            },
        ];
        assert!(matches!(
            CreativeCandidateBridge::with_resources(
                source,
                original(source),
                d(5),
                &bad,
                GoalPolicy {
                    revision: d(2),
                    query: goal
                },
                limits(),
                &view
            ),
            Err(CreativeError::BindingMismatch)
        ));
    });
}

#[test]
fn tlcx21_validator_budget_cut_does_not_publish_context_or_success() {
    fixture(|source, bindings, goal| {
        let (_b, run) = owner(source, Cancellation::default());
        let reservation = run
            .reserve(&[
                req(ResourceKind::Ram, 32 * 1024 * 1024),
                req(ResourceKind::Output, 1024 * 1024),
            ])
            .unwrap();
        let view = reservation.lease();
        let low = BridgeLimits {
            max_validation_checks: 1,
            ..limits()
        };
        let mut bridge = CreativeCandidateBridge::with_resources(
            source,
            original(source),
            d(5),
            bindings,
            GoalPolicy {
                revision: d(2),
                query: goal,
            },
            low,
            &view,
        )
        .unwrap();
        assert!(matches!(
            bridge.generate(Operator::ReframeDeMorgan, &view),
            Err(CreativeError::Budget)
        ));
        assert_eq!(bridge.contexts().states().len(), 1);
        assert_eq!(bridge.proposals().len(), 0);
    });
}

#[test]
fn tlcx21_cancelled_current_projection_keeps_only_historical_profile_data() {
    fixture(|source, bindings, goal| {
        let cancel = Cancellation::default();
        let (_b, run) = owner(source, cancel.clone());
        let reservation = run
            .reserve(&[
                req(ResourceKind::Ram, 32 * 1024 * 1024),
                req(ResourceKind::Output, 1024 * 1024),
            ])
            .unwrap();
        let view = reservation.lease();
        let mut bridge = CreativeCandidateBridge::with_resources(
            source,
            original(source),
            d(5),
            bindings,
            GoalPolicy {
                revision: d(2),
                query: goal,
            },
            limits(),
            &view,
        )
        .unwrap();
        let candidate = bridge.generate(Operator::ReframeDeMorgan, &view).unwrap();
        cancel.cancel();
        assert!(matches!(
            candidate.project_current(&bridge, &view),
            Err(CreativeError::Resource(_))
        ));
        assert!(!candidate.historical_payload().is_empty());
    });
}

#[test]
fn tlcx21_output_exhaustion_and_pre_generation_cancel_publish_no_context() {
    fixture(|source, bindings, goal| {
        let cancel = Cancellation::default();
        let (_b, run) = owner(source, cancel.clone());
        let ram = 32 * 1024 * 1024;
        let reservation = run
            .reserve(&[req(ResourceKind::Ram, ram), req(ResourceKind::Output, 1)])
            .unwrap();
        let view = reservation.lease();
        let mut bridge = CreativeCandidateBridge::with_resources(
            source,
            original(source),
            d(5),
            bindings,
            GoalPolicy {
                revision: d(2),
                query: goal,
            },
            limits(),
            &view,
        )
        .unwrap();
        assert!(matches!(
            bridge.generate(Operator::ReframeDeMorgan, &view),
            Err(CreativeError::Resource(
                lc631_substrate::resource::ResourceError::Exhausted(ResourceKind::Output)
            ))
        ));
        assert_eq!(bridge.contexts().states().len(), 1);
        assert_eq!(bridge.proposals().len(), 0);
        let returned = view
            .claim_resources(&[req(ResourceKind::Ram, ram - METADATA_RAM)])
            .unwrap();
        drop(returned);
        cancel.cancel();
        assert!(matches!(
            bridge.generate(Operator::ReframeDeMorgan, &view),
            Err(CreativeError::Resource(
                lc631_substrate::resource::ResourceError::Cancelled
            ))
        ));
        assert_eq!(bridge.contexts().states().len(), 1);
        assert_eq!(bridge.proposals().len(), 0);
    });
}

#[test]
fn tlcx21_candidate_and_bridge_drop_release_their_actual_separate_claims() {
    fixture(|source, bindings, goal| {
        let (_b, run) = owner(source, Cancellation::default());
        let ram = METADATA_RAM + VALIDATION_SCRATCH_RAM + CANDIDATE_RAM;
        let reservation = run
            .reserve(&[
                req(ResourceKind::Ram, ram),
                req(ResourceKind::Output, CANDIDATE_OUTPUT),
            ])
            .unwrap();
        let view = reservation.lease();
        let mut bridge = CreativeCandidateBridge::with_resources(
            source,
            original(source),
            d(5),
            bindings,
            GoalPolicy {
                revision: d(2),
                query: goal,
            },
            limits(),
            &view,
        )
        .unwrap();
        let candidate = bridge.generate(Operator::ReframeDeMorgan, &view).unwrap();
        let scratch = view
            .claim_resources(&[req(ResourceKind::Ram, VALIDATION_SCRATCH_RAM)])
            .unwrap();
        assert!(matches!(
            view.claim_resources(&[req(ResourceKind::Ram, 1)]),
            Err(lc631_substrate::resource::ResourceError::Exhausted(
                ResourceKind::Ram
            ))
        ));
        assert!(matches!(
            view.claim_resources(&[req(ResourceKind::Output, 1)]),
            Err(lc631_substrate::resource::ResourceError::Exhausted(
                ResourceKind::Output
            ))
        ));
        drop(scratch);
        drop(candidate);
        let packet_returned = view
            .claim_resources(&[
                req(ResourceKind::Ram, ram - METADATA_RAM),
                req(ResourceKind::Output, CANDIDATE_OUTPUT),
            ])
            .unwrap();
        drop(packet_returned);
        drop(bridge);
        let _all_returned = view
            .claim_resources(&[
                req(ResourceKind::Ram, ram),
                req(ResourceKind::Output, CANDIDATE_OUTPUT),
            ])
            .unwrap();
    });
}
