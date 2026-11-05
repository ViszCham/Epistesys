//! Actual private finite-result lookup -> advisory typed reprojection.
use lc631_core::{context::*, AuthorityRevision};
use lc631_runtime::crosslogic_loss::{finite_temporal::*, ComparisonBinding, ReprojectionCause};
use lc631_runtime::finite_results::{FiniteResultError, FiniteResultStore};
use lc631_runtime::resource::*;
use lc631_solvers::temporal::*;
use std::time::Duration;

fn pin(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}
fn fixture() -> (ContextConsumer, ComparisonBinding) {
    let scope = ContextScope::declared(pin(1), pin(2), pin(3), pin(4));
    let original = OriginalContext::checked(
        SourceRevision::of("actual finite bridge"),
        scope,
        AuthorityRevision(1),
        vec![],
        ContextLimits {
            max_contexts: 8,
            max_premises: 8,
            max_edits: 8,
            max_statement_bytes: 1024,
        },
    )
    .unwrap();
    let binding = ComparisonBinding {
        context: original.state().revision(),
        scope,
        root_query: pin(5),
        translated_query: pin(6),
        domain: pin(7),
        premises: pin(8),
        projection: pin(9),
    };
    (ContextConsumer::new(original).unwrap(), binding)
}
fn query(binding: ComparisonBinding, node: TemporalNode) -> TemporalQuery {
    TemporalQuery::checked(
        binding.context,
        vec![node],
        0,
        TemporalProfile::ClosedFiniteKleeneV1,
    )
    .unwrap()
}
fn trace() -> FiniteTrace {
    trace_with(Truth::Unknown)
}
fn trace_with(value: Truth) -> FiniteTrace {
    let clock = ClockSpec::exact(pin(11), TimeScale::Monotonic, pin(12), pin(13));
    FiniteTrace::checked(
        vec![TraceStep::new(
            Instant::absolute(clock, 0, TimeUnit::Second).unwrap(),
            vec![value],
        )],
        TraceClosure::Closed,
    )
    .unwrap()
}
fn resources(binding: ComparisonBinding) -> (ResourceBroker, RunResources, Reservation) {
    let owner = ResourceBroker::new(ResourceLimits::uniform(65_536).unwrap()).unwrap();
    let run = owner
        .begin_run(
            1,
            binding.context.source().digest(),
            ResourceLimits::uniform(65_536).unwrap(),
            Deadline::after(Duration::from_secs(60)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let reservation = run
        .reserve(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(65_536).unwrap(),
        )])
        .unwrap();
    (owner, run, reservation)
}
fn request<'a>(
    id: lc631_runtime::finite_results::FiniteResultId,
    query: &'a TemporalQuery,
    trace: &'a FiniteTrace,
    mut binding: ComparisonBinding,
    dependencies: &'a [Digest32],
) -> TemporalObservationRequest<'a> {
    binding.translated_query = query.digest();
    TemporalObservationRequest {
        observation: id,
        query,
        trace,
        binding,
        dependencies,
    }
}

#[test]
fn tlcx20_actual_finite_disagreement_becomes_current_typed_request() {
    let (contexts, binding) = fixture();
    let yes = query(binding, TemporalNode::Constant(true));
    let no = query(binding, TemporalNode::Constant(false));
    let trace = trace();
    let (_owner, mut run, mut reservation) = resources(binding);
    {
        let view = reservation.lease();
        let mut store = FiniteResultStore::with_resources(2, &view).unwrap();
        let a = store
            .evaluate_temporal(
                &yes,
                &trace,
                binding.context,
                binding.scope,
                EvaluationBudget::default(),
                &view,
            )
            .unwrap();
        let b = store
            .evaluate_temporal(
                &no,
                &trace,
                binding.context,
                binding.scope,
                EvaluationBudget::default(),
                &view,
            )
            .unwrap();
        let left = request(a, &yes, &trace, binding, &[]);
        let right = request(b, &no, &trace, binding, &[]);
        let diagnostic = analyze_temporal_pair(&store, left, &store, right).unwrap();
        assert_eq!(diagnostic.truths(), (Truth::True, Truth::False));
        assert_eq!(diagnostic.validation(), FiniteValidation::Unverified);
        let reprojection = diagnostic.reprojection().unwrap();
        assert_eq!(
            reprojection.cause(),
            ReprojectionCause::ReportedDisagreement
        );
        assert_eq!(
            reprojection
                .inspect_current(&contexts, left.binding, right.binding)
                .unwrap()
                .state(),
            "NeedsEvidence/ReprojectorNotConnected"
        );
        let mut stale = left.binding;
        stale.translated_query = pin(99);
        assert!(reprojection
            .inspect_current(&contexts, stale, right.binding)
            .is_err());
    }
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}

#[test]
fn tlcx20_actual_unknown_and_shared_dependency_never_become_agreement_proof() {
    let (_, binding) = fixture();
    let unknown = query(binding, TemporalNode::Atom(0));
    let trace = trace();
    let deps = [pin(21)];
    let (_owner, mut run, mut reservation) = resources(binding);
    {
        let view = reservation.lease();
        let mut store = FiniteResultStore::with_resources(1, &view).unwrap();
        let id = store
            .evaluate_temporal(
                &unknown,
                &trace,
                binding.context,
                binding.scope,
                EvaluationBudget::default(),
                &view,
            )
            .unwrap();
        let input = request(id, &unknown, &trace, binding, &deps);
        let diagnostic = analyze_temporal_pair(&store, input, &store, input).unwrap();
        assert_eq!(diagnostic.truths(), (Truth::Unknown, Truth::Unknown));
        assert_eq!(
            diagnostic.reprojection().unwrap().cause(),
            ReprojectionCause::UnknownOutcome
        );
        assert_eq!(
            diagnostic.dependencies(),
            lc631_runtime::crosslogic_loss::DependencyStatus::ReportedSharedDependency
        );
        assert_eq!(diagnostic.validation(), FiniteValidation::Unverified);
    }
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}

#[test]
fn tlcx20_actual_store_owner_and_translated_query_cuts_are_rejected() {
    let (_, binding) = fixture();
    let yes = query(binding, TemporalNode::Constant(true));
    let trace = trace();
    let changed_trace = trace_with(Truth::True);
    let (_owner, mut run, mut reservation) = resources(binding);
    {
        let view = reservation.lease();
        let mut store = FiniteResultStore::with_resources(1, &view).unwrap();
        let foreign = FiniteResultStore::with_resources(1, &view).unwrap();
        let id = store
            .evaluate_temporal(
                &yes,
                &trace,
                binding.context,
                binding.scope,
                EvaluationBudget::default(),
                &view,
            )
            .unwrap();
        let good = request(id, &yes, &trace, binding, &[]);
        assert!(matches!(
            analyze_temporal_pair(&foreign, good, &store, good),
            Err(FiniteBridgeError::Store(FiniteResultError::BindingMismatch))
        ));
        let mut wrong = good;
        wrong.binding.translated_query = pin(99);
        assert!(matches!(
            analyze_temporal_pair(&store, wrong, &store, good),
            Err(FiniteBridgeError::TranslatedQueryMismatch)
        ));
        wrong = good;
        wrong.trace = &changed_trace;
        assert!(matches!(
            analyze_temporal_pair(&store, wrong, &store, good),
            Err(FiniteBridgeError::Store(FiniteResultError::BindingMismatch))
        ));
        wrong = good;
        wrong.binding.context = ContextRevision::original(SourceRevision::of("swapped"));
        assert!(matches!(
            analyze_temporal_pair(&store, wrong, &store, good),
            Err(FiniteBridgeError::Store(FiniteResultError::ContextMismatch))
        ));
        wrong = good;
        wrong.binding.scope = ContextScope::declared(pin(99), pin(2), pin(3), pin(4));
        assert!(matches!(
            analyze_temporal_pair(&store, wrong, &store, good),
            Err(FiniteBridgeError::Store(FiniteResultError::BindingMismatch))
        ));
        let diagnostic = analyze_temporal_pair(&store, good, &store, good).unwrap();
        assert_eq!(
            diagnostic.correspondence(),
            FiniteCorrespondence::SameFiniteOutcomeUnverified
        );
        assert!(diagnostic.reprojection().is_err());
    }
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}

#[test]
fn tlcx20_distinct_actual_traces_override_equal_declared_premise_pins() {
    let (_, binding) = fixture();
    let yes = query(binding, TemporalNode::Constant(true));
    let first = trace();
    let changed = trace_with(Truth::True);
    let (_owner, mut run, mut reservation) = resources(binding);
    {
        let view = reservation.lease();
        let mut store = FiniteResultStore::with_resources(2, &view).unwrap();
        let a = store
            .evaluate_temporal(
                &yes,
                &first,
                binding.context,
                binding.scope,
                EvaluationBudget::default(),
                &view,
            )
            .unwrap();
        let b = store
            .evaluate_temporal(
                &yes,
                &changed,
                binding.context,
                binding.scope,
                EvaluationBudget::default(),
                &view,
            )
            .unwrap();
        let diagnostic = analyze_temporal_pair(
            &store,
            request(a, &yes, &first, binding, &[]),
            &store,
            request(b, &yes, &changed, binding, &[]),
        )
        .unwrap();
        // The independent expectation is concrete: different input records
        // cannot be equal premises merely because both constant queries pass.
        assert_eq!(diagnostic.truths(), (Truth::True, Truth::True));
        assert_eq!(
            diagnostic.reprojection().unwrap().cause(),
            ReprojectionCause::PremiseMismatch
        );
        assert!(diagnostic.shares_temporal_evaluator());
        assert_eq!(diagnostic.validation(), FiniteValidation::Unverified);
    }
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}
