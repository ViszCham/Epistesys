use lc631_analysis::logical_lowering::{LogicProfile, LogicalLoweringBudget};
use lc631_core::{
    context::{ContextLimits, ContextRevision, ContextScope, SourceRevision, TaskKind},
    AuthorityRevision,
};
use lc631_runtime::resource::{
    Cancellation, Deadline, ResourceAmount, ResourceBroker, ResourceKind, ResourceLimits,
    ResourceRequest, ResourceReservationView,
};
use lc631_runtime::solve_preparation::{
    prepare, PreparationBudget, PreparationError, PreparationRequest, PreparationState,
    ProvidedPreparation, MAX_PREPARATION_SOURCE_BYTES, PREPARATION_RAM_FLOOR,
};
use lc631_substrate::Digest32;
use std::time::Duration;

const SOURCE: &str = "Please verify the result if ready.";
fn provided(task: TaskKind) -> ProvidedPreparation {
    let d = Digest32::from_bytes([2; 32]);
    ProvidedPreparation {
        scope: ContextScope::declared(d, d, d, d),
        authority_revision: AuthorityRevision(7),
        task,
        logic_profile: LogicProfile::ClassicalPropositionalV1,
        context_limits: ContextLimits {
            max_contexts: 4,
            max_premises: 1024,
            max_edits: 8,
            max_statement_bytes: MAX_PREPARATION_SOURCE_BYTES,
        },
        logical_budget: LogicalLoweringBudget::default(),
    }
}
fn request(source: &str, task: TaskKind) -> PreparationRequest<'_> {
    PreparationRequest {
        source,
        source_revision: SourceRevision::of(source),
        expected_lowered_context: None,
        provided: provided(task),
    }
}
fn resources(source: &str, f: impl FnOnce(&ResourceReservationView<'_>)) {
    let limits = ResourceLimits::uniform(128 * 1024 * 1024).unwrap();
    let owner = ResourceBroker::new(limits).unwrap();
    let mut run = owner
        .begin_run(
            1,
            SourceRevision::of(source).digest(),
            limits,
            Deadline::after(Duration::from_secs(60)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let mut reservation = run
        .reserve(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(64 * 1024 * 1024).unwrap(),
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
fn budget() -> PreparationBudget {
    PreparationBudget::checked(PREPARATION_RAM_FLOOR).unwrap()
}

#[test]
fn tlcx34_prepare_actual_pipeline_context_logical_share_source_and_hold_unknown() {
    resources(SOURCE, |view| {
        let prepared = prepare(request(SOURCE, TaskKind::Coding), budget(), view).unwrap();
        assert_eq!(prepared.source(), SOURCE);
        assert_eq!(
            prepared.expected_context_binding(),
            lc631_runtime::solve_preparation::ExpectedContextBinding::NotProvided
        );
        let projection = prepared.project_current(view).unwrap();
        assert_eq!(
            projection.context().source_store().id(),
            projection.logical().source_store().id()
        );
        assert!(std::ptr::eq(
            projection.context().source_store().text().as_ptr(),
            projection.logical().source_store().text().as_ptr()
        ));
        assert_eq!(
            projection.logical().context_revision(),
            projection.context().original().state().revision()
        );
        assert!(!projection.logical().formulas().is_empty());
        assert!(!projection.logical().obligations().is_empty());
        assert_eq!(prepared.provided().authority_revision, AuthorityRevision(7));
        assert_eq!(prepared.state(), PreparationState::Clarify);
        assert!(!prepared.source_interpretation_verified());
    });
}

#[test]
fn tlcx34_prepare_retains_each_task_kind_without_automatic_coding_or_goal() {
    resources(SOURCE, |view| {
        for task in [
            TaskKind::Mathematics,
            TaskKind::Reasoning,
            TaskKind::Explanation,
            TaskKind::Coding,
            TaskKind::Media,
        ] {
            let prepared = prepare(request(SOURCE, task), budget(), view).unwrap();
            assert_eq!(
                prepared
                    .project_current(view)
                    .unwrap()
                    .logical()
                    .task_kind(),
                task
            );
            assert_eq!(prepared.provided().task, task);
        }
    });
}

#[test]
fn tlcx34_prepare_rejects_source_and_foreign_expected_context_before_construction() {
    resources(SOURCE, |view| {
        let mut r = request(SOURCE, TaskKind::Reasoning);
        r.source_revision = SourceRevision::of("different");
        assert!(matches!(
            prepare(r, budget(), view),
            Err(PreparationError::SourceMismatch)
        ));
        let mut r = request(SOURCE, TaskKind::Reasoning);
        r.expected_lowered_context =
            Some(ContextRevision::original(SourceRevision::of("different")));
        assert!(matches!(
            prepare(r, budget(), view),
            Err(PreparationError::SourceMismatch)
        ));
        assert!(matches!(
            prepare(request("other", TaskKind::Reasoning), budget(), view),
            Err(PreparationError::SourceMismatch)
        ));
    });
}

#[test]
fn tlcx34_prepare_preserves_unsupported_source_spans_and_expects_full_context() {
    let source = "未知の自由文を無断で真理にしない。";
    resources(source, |view| {
        let prepared = prepare(request(source, TaskKind::Explanation), budget(), view).unwrap();
        assert_eq!(prepared.state(), PreparationState::Clarify);
        let p = prepared.project_current(view).unwrap();
        assert!(
            !p.context().unsupported_operative_region_ids().is_empty()
                || !p.logical().residuals().is_empty()
                || p.logical().formulas().is_empty()
        );
        for span in p.logical().source_partition() {
            assert_eq!(span.source_id(), p.logical().source_store().id());
        }
        let actual = p.context().original().state().revision();
        let mut r = request(source, TaskKind::Explanation);
        r.expected_lowered_context = Some(actual.reconstructed([8; 32]));
        assert!(matches!(
            prepare(r, budget(), view),
            Err(PreparationError::ContextMismatch)
        ));
        let mut matched = request(source, TaskKind::Explanation);
        matched.expected_lowered_context = Some(actual);
        let matched = prepare(matched, budget(), view).unwrap();
        assert_eq!(
            matched.expected_context_binding(),
            lc631_runtime::solve_preparation::ExpectedContextBinding::Matched(actual)
        );
    });
}

#[test]
fn tlcx34_prepare_budget_floor_source_limit_and_failed_pipeline_reclaim() {
    assert!(matches!(
        PreparationBudget::checked(1),
        Err(PreparationError::InvalidBudget)
    ));
    resources(SOURCE, |view| {
        let mut r = request(SOURCE, TaskKind::Reasoning);
        r.provided.logical_budget.max_nodes = 1;
        assert!(prepare(r, budget(), view).is_err());
        // Full claim succeeds after failed construction only if its allowance dropped.
        let full = view
            .claim_resources(&[ResourceRequest::new(
                ResourceKind::Ram,
                ResourceAmount::checked(64 * 1024 * 1024).unwrap(),
            )])
            .unwrap();
        drop(full);
        assert!(prepare(request(SOURCE, TaskKind::Reasoning), budget(), view).is_ok());
    });
    let too_large = "x".repeat(MAX_PREPARATION_SOURCE_BYTES + 1);
    resources(&too_large, |view| {
        assert!(matches!(
            prepare(request(&too_large, TaskKind::Reasoning), budget(), view),
            Err(PreparationError::SourceLimit)
        ));
    });
}

#[test]
fn tlcx34_prepare_current_projection_rejects_foreign_resource_and_cancellation() {
    let limits = ResourceLimits::uniform(128 * 1024 * 1024).unwrap();
    let owner = ResourceBroker::new(limits).unwrap();
    let cancel = Cancellation::default();
    let mut run = owner
        .begin_run(
            1,
            SourceRevision::of(SOURCE).digest(),
            limits,
            Deadline::after(Duration::from_secs(60)).unwrap(),
            cancel.clone(),
        )
        .unwrap();
    let mut a = run
        .reserve(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(64 * 1024 * 1024).unwrap(),
        )])
        .unwrap();
    let mut b = run
        .reserve(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(64 * 1024 * 1024).unwrap(),
        )])
        .unwrap();
    {
        let view = a.lease();
        let foreign = b.lease();
        let prepared = prepare(request(SOURCE, TaskKind::Reasoning), budget(), &view).unwrap();
        assert!(matches!(
            prepared.project_current(&foreign),
            Err(PreparationError::ForeignReservation)
        ));
        cancel.cancel();
        assert!(matches!(
            prepared.project_current(&view),
            Err(PreparationError::Resource(_))
        ));
        assert_eq!(prepared.source(), SOURCE);
    }
    a.release_confirmed().unwrap();
    b.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}
