//! 日本語: 接続・断線試験を先に追加。実行前の未登録状態はsemantic REDではない。
//! English: Connection/cut cases added first; unregistered construction is not semantic RED.
use lc631_core::{context::*, AuthorityRevision};
use lc631_runtime::{
    crosslogic_loss::{
        finite_temporal::*, temporal_reprojection::*, ComparisonBinding, PendingReprojection,
        ReprojectionCause,
    },
    finite_results::{FiniteResultError, FiniteResultStore},
    resource::*,
};
use lc631_solvers::temporal::*;
use std::time::Duration;

fn pin(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}
struct Fixture {
    contexts: ContextConsumer,
    binding: ComparisonBinding,
    left: TemporalQuery,
    right: TemporalQuery,
    trace: FiniteTrace,
}
impl Fixture {
    fn new() -> Self {
        let scope = ContextScope::declared(pin(1), pin(2), pin(3), pin(4));
        let original = OriginalContext::checked(
            SourceRevision::of("Given finite temporal source"),
            scope,
            AuthorityRevision(1),
            vec![],
            ContextLimits {
                max_contexts: 4,
                max_premises: 4,
                max_edits: 4,
                max_statement_bytes: 1024,
            },
        )
        .unwrap();
        let context = original.state().revision();
        let binding = ComparisonBinding {
            context,
            scope,
            root_query: pin(5),
            translated_query: pin(6),
            domain: pin(7),
            premises: pin(8),
            projection: pin(9),
        };
        let left = TemporalQuery::checked(
            context,
            vec![TemporalNode::Atom(0)],
            0,
            TemporalProfile::ClosedFiniteKleeneV1,
        )
        .unwrap();
        let right = TemporalQuery::checked(
            context,
            vec![TemporalNode::Atom(0), TemporalNode::Not(0)],
            1,
            TemporalProfile::ClosedFiniteKleeneV1,
        )
        .unwrap();
        let clock = ClockSpec::exact(pin(11), TimeScale::Monotonic, pin(12), pin(13));
        let trace = FiniteTrace::checked(
            vec![TraceStep::new(
                Instant::absolute(clock, 0, TimeUnit::Second).unwrap(),
                vec![Truth::True],
            )],
            TraceClosure::Closed,
        )
        .unwrap();
        Self {
            contexts: ContextConsumer::new(original).unwrap(),
            binding,
            left,
            right,
            trace,
        }
    }
    fn observe<'a>(
        &'a self,
        store: &mut FiniteResultStore<'a, '_>,
        resources: &ResourceReservationView<'_>,
    ) -> (TemporalObservationRequest<'a>, TemporalObservationRequest<'a>) {
        let mut observe = |query: &'a TemporalQuery| {
            let observation = store
                .evaluate_temporal(
                    query,
                    &self.trace,
                    self.binding.context,
                    self.binding.scope,
                    EvaluationBudget::default(),
                    resources,
                )
                .unwrap();
            TemporalObservationRequest {
                observation,
                query,
                trace: &self.trace,
                binding: ComparisonBinding {
                    translated_query: query.digest(),
                    ..self.binding
                },
                dependencies: &[],
            }
        };
        (observe(&self.left), observe(&self.right))
    }
}
fn resources(f: &Fixture, cancel: Cancellation) -> (ResourceBroker, RunResources, Reservation) {
    let broker = ResourceBroker::new(ResourceLimits::uniform(1_048_576).unwrap()).unwrap();
    let run = broker
        .begin_run(
            1,
            f.binding.context.source().digest(),
            ResourceLimits::uniform(1_048_576).unwrap(),
            Deadline::after(Duration::from_secs(60)).unwrap(),
            cancel,
        )
        .unwrap();
    let reservation = run
        .reserve(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(1_048_576).unwrap(),
        )])
        .unwrap();
    (broker, run, reservation)
}
fn pending(pair: &CurrentTemporalPair<'_, '_, '_, '_>) -> PendingReprojection {
    let diagnostic = analyze_temporal_pair(
        pair.left_store,
        pair.left,
        pair.right_store,
        pair.right,
    )
    .unwrap();
    assert_eq!(diagnostic.truths(), (Truth::True, Truth::False));
    let request = diagnostic.reprojection().unwrap();
    assert_eq!(request.cause(), ReprojectionCause::ReportedDisagreement);
    request
        .inspect_current(pair.contexts, pair.left.binding, pair.right.binding)
        .unwrap()
}

#[test]
fn tlcx20_reprojection_rejects_swapped_pending_and_foreign_current_owner() {
    let f = Fixture::new();
    let (_broker, _run, reservation) = resources(&f, Cancellation::default());
    let view = reservation.lease();
    let mut old = FiniteResultStore::with_resources(2, &view).unwrap();
    let foreign = FiniteResultStore::with_resources(2, &view).unwrap();
    let (left, right) = f.observe(&mut old, &view);
    let pair = CurrentTemporalPair {
        contexts: &f.contexts,
        left_store: &old,
        left,
        left_resources: &view,
        right_store: &old,
        right,
        right_resources: &view,
    };
    let pending = pending(&pair);
    let swapped = CurrentTemporalPair {
        left: right,
        right: left,
        ..pair
    };
    assert!(prepare_identity_reprojection(pending, &swapped, &view).is_err());
    let cut = CurrentTemporalPair {
        right_store: &foreign,
        ..pair
    };
    assert!(matches!(
        prepare_identity_reprojection(pending, &cut, &view),
        Err(ReprojectionError::Store(FiniteResultError::BindingMismatch))
    ));
    let (_other_broker, _other_run, other_reservation) = resources(&f, Cancellation::default());
    let other_view = other_reservation.lease();
    let wrong_owner = CurrentTemporalPair {
        left_resources: &other_view,
        ..pair
    };
    assert!(prepare_identity_reprojection(pending, &wrong_owner, &view).is_err());
}

#[test]
fn tlcx20_reprojection_changes_actual_query_and_fresh_feedback_keeps_old_history() {
    let f = Fixture::new();
    let (_broker, _run, reservation) = resources(&f, Cancellation::default());
    let view = reservation.lease();
    let mut old = FiniteResultStore::with_resources(2, &view).unwrap();
    let (left, right) = f.observe(&mut old, &view);
    let pair = CurrentTemporalPair {
        contexts: &f.contexts,
        left_store: &old,
        left,
        left_resources: &view,
        right_store: &old,
        right,
        right_resources: &view,
    };
    let pending = pending(&pair);
    let prepared = prepare_identity_reprojection(pending, &pair, &view).unwrap();
    assert_eq!(prepared.original_request(), pending.request());
    assert_eq!(prepared.original_truths(), (Truth::True, Truth::False));
    assert_eq!(prepared.delta().status(), DeltaStatus::Proposed);
    assert_eq!(prepared.delta().before(), right.query.digest());
    assert_ne!(prepared.query().digest(), right.query.digest());
    assert_eq!(prepared.query().nodes(), &[TemporalNode::Atom(0)]);
    assert_eq!(prepared.query().root(), 0);
    assert_eq!(prepared.query().context(), f.binding.context);
    assert_eq!(prepared.input_basis(), TemporalInputBasis::Given);
    // Cut the dispatch/result edge: old actual observations remain discrepant.
    assert!(analyze_temporal_pair(&old, left, &old, right)
        .unwrap()
        .reprojection()
        .is_ok());
    let mut fresh = FiniteResultStore::with_resources(1, &view).unwrap();
    let result = execute_prepared(
        &prepared,
        &pair,
        &mut fresh,
        EvaluationBudget::default(),
        &view,
    )
    .unwrap();
    assert_eq!(fresh.len(), 1);
    let feedback = consume_current(&prepared, &pair, &result, &fresh, &view).unwrap();
    assert_eq!(feedback.truths(), (Truth::True, Truth::True));
    assert_eq!(
        feedback.correspondence(),
        FiniteCorrespondence::SameFiniteOutcomeUnverified
    );
    assert_eq!(feedback.validation(), FiniteValidation::Unverified);
    assert!(feedback.shares_temporal_evaluator());
    assert!(feedback.reprojection().is_err());
    assert_eq!(old.len(), 2);
    assert_eq!(
        old.lookup_temporal(
            right.observation,
            right.query,
            right.trace,
            right.binding.context,
            right.binding.scope,
        )
        .unwrap()
        .truth(),
        Truth::False
    );
    assert_eq!(prepared.original_right().query.digest(), f.right.digest());
    assert_eq!(prepared.delta().status(), DeltaStatus::Proposed);
    let foreign = FiniteResultStore::with_resources(1, &view).unwrap();
    assert!(consume_current(&prepared, &pair, &result, &foreign, &view).is_err());
}

#[test]
fn tlcx20_reprojection_cancel_cuts_prepare_execution_and_fresh_consumption() {
    let f = Fixture::new();
    let cancel = Cancellation::default();
    let (_broker, _run, reservation) = resources(&f, cancel.clone());
    let view = reservation.lease();
    let mut old = FiniteResultStore::with_resources(2, &view).unwrap();
    let (left, right) = f.observe(&mut old, &view);
    let pair = CurrentTemporalPair {
        contexts: &f.contexts,
        left_store: &old,
        left,
        left_resources: &view,
        right_store: &old,
        right,
        right_resources: &view,
    };
    let pending = pending(&pair);
    let prepared = prepare_identity_reprojection(pending, &pair, &view).unwrap();
    let mut fresh = FiniteResultStore::with_resources(2, &view).unwrap();
    let result = execute_prepared(
        &prepared,
        &pair,
        &mut fresh,
        EvaluationBudget::default(),
        &view,
    )
    .unwrap();
    cancel.cancel();
    assert!(prepare_identity_reprojection(pending, &pair, &view).is_err());
    assert!(execute_prepared(
        &prepared,
        &pair,
        &mut fresh,
        EvaluationBudget::default(),
        &view,
    )
    .is_err());
    assert_eq!(fresh.len(), 1);
    assert!(consume_current(&prepared, &pair, &result, &fresh, &view).is_err());
    assert_eq!(prepared.original_truths(), (Truth::True, Truth::False));
}

#[test]
fn tlcx20_reprojection_stale_binding_and_actual_noop_do_not_create_a_loop() {
    let f = Fixture::new();
    let (_broker, _run, reservation) = resources(&f, Cancellation::default());
    let view = reservation.lease();
    let mut old = FiniteResultStore::with_resources(2, &view).unwrap();
    let (left, right) = f.observe(&mut old, &view);
    let pair = CurrentTemporalPair {
        contexts: &f.contexts,
        left_store: &old,
        left,
        left_resources: &view,
        right_store: &old,
        right,
        right_resources: &view,
    };
    let pending = pending(&pair);
    let changed = CurrentTemporalPair {
        right: TemporalObservationRequest {
            binding: ComparisonBinding {
                root_query: pin(99),
                ..right.binding
            },
            ..right
        },
        ..pair
    };
    assert!(prepare_identity_reprojection(pending, &changed, &view).is_err());
    assert!(analyze_temporal_pair(&old, left, &old, left)
        .unwrap()
        .reprojection()
        .is_err());
    let noop = CurrentTemporalPair { right: left, ..pair };
    assert!(prepare_identity_reprojection(pending, &noop, &view).is_err());
}
