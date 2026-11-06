//! Public resource-owner and fixed-live-deadline cuts; no injected owners/reports.
//! Live deadline narrowing remains prospective: the disconnected resource/deadline
//! module and its missing substrate API are recorded in C3-validation-allocation-v2.
//! The registered missing executable exercises admission without a native child.
use lc631_core::{context::*, AuthorityRevision};
use lc631_runtime::{resource::*, scheduled_solver::*};
use lc631_solvers::{
    protocol::{sexpr::SExprLimits, SolverProtocol},
    registry::{BackendId, LogicProfile, SolverRegistration, SolverRegistry, SolverResourceCaps},
    runner::{ExecutionBudget, ProcessCleanupState, SolverExecutionState},
    smt::native_model::NativeModelError,
    supervisor_adapter::OwnedPayloadError,
    typed_query::{BoolExpr, IntExpr, IntSymbolId, SmtEmitLimits, SmtStatusQuery},
};
use lc631_world::exploration_schedule::{
    priority::{MissingPriorityReason, PriorityInput},
    ScheduleBinding, SchedulerNoveltyPolicyId,
};
use std::{cell::Cell, time::{Duration, Instant, SystemTime, UNIX_EPOCH}};

const WORK_RAM: u64 = 8 * 1024 * 1024;
const WORK_OUTPUT: u64 = 16_384;
const WORK_CLEANUP: u64 = 65_536;

fn context() -> ContextState {
    let pin = Digest32::from_bytes([1; 32]);
    OriginalContext::checked(
        SourceRevision::of("beta7 actual resource owner/deadline fixture"),
        ContextScope::declared(pin, pin, pin, pin),
        AuthorityRevision(1),
        vec![],
        ContextLimits { max_contexts: 8, max_premises: 8, max_edits: 8, max_statement_bytes: 4096 },
    ).unwrap().state().clone()
}
fn binding() -> ScheduleBinding {
    ScheduleBinding {
        comparison_scope: [1; 32], selection_policy: [2; 32], budget_series: [3; 32],
        novelty_policy: SchedulerNoveltyPolicyId::DistanceToFrontierUnionV1,
    }
}
fn request(kind: ResourceKind, amount: u64) -> ResourceRequest {
    ResourceRequest::new(kind, ResourceAmount::checked(amount).unwrap())
}
fn usage(broker: &ResourceBroker) -> [u64; 6] {
    ResourceKind::ALL.map(|kind| broker.used(kind).unwrap())
}
fn work_charge() -> [u64; 6] {
    [WORK_RAM, 0, WORK_OUTPUT, 0, 1, WORK_CLEANUP]
}
fn add_usage(before: [u64; 6], charge: [u64; 6]) -> [u64; 6] {
    std::array::from_fn(|i| before[i].checked_add(charge[i]).unwrap())
}
fn begin(
    broker: &ResourceBroker, source: Digest32, limits: ResourceLimits,
    deadline: Deadline,
) -> RunResources {
    broker.begin_run(1, source, limits, deadline, Cancellation::default()).unwrap()
}
fn query() -> SmtStatusQuery {
    SmtStatusQuery::checked(
        vec![], vec![IntSymbolId::new(0)],
        vec![BoolExpr::Equal(IntExpr::Variable(IntSymbolId::new(0)), IntExpr::Constant(3))],
        SmtEmitLimits::checked(4096, 0, 1, 8, 32, 8).unwrap(),
    ).unwrap()
}
fn work<'a>(query: &'a SmtStatusQuery, context: &'a ContextState, priority: &'a PriorityInput) -> NativeWork<'a> {
    NativeWork::checked(NativeWorkSpec {
        binding: binding(), canonical_key: b"actual-owner-work", priority,
        estimated_query_slots: 1, query, context,
        budget: ExecutionBudget::checked(1000, 4096, 4096, 1024, 16, 4096, 1024 * 1024, 0, WORK_CLEANUP).unwrap(),
        parser: SExprLimits::checked(4096, 128, 16).unwrap(),
        demand: NativeDemand::checked([1, 0, 1], WORK_RAM, WORK_OUTPUT, WORK_CLEANUP).unwrap(),
    }).unwrap()
}
fn missing_executable_registry() -> SolverRegistry {
    // Registration is available; the actual unique executable path is absent.
    // This is a descriptor, never a fabricated successful process observation.
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let path = std::env::temp_dir().join(format!(
        "epistesys-beta7-owner-no-executable-{}-{nonce:x}{}",
        std::process::id(), std::env::consts::EXE_SUFFIX,
    ));
    assert!(path.is_absolute());
    assert!(matches!(std::fs::symlink_metadata(&path), Err(error) if error.kind() == std::io::ErrorKind::NotFound));
    SolverRegistry::checked(vec![SolverRegistration::checked(
        BackendId::Z3, LogicProfile::SmtBooleanQfLia, SolverProtocol::SmtLibPayloadWireV2,
        path.to_str().unwrap(), "declared-missing-not-a-native-positive", [1; 32], vec![],
        SolverResourceCaps {
            max_argv_count: 16, max_argv_bytes: 4096, max_stdin_bytes: 4096,
            max_stdout_bytes: 4096, max_stderr_bytes: 1024, max_timeout_ms: 1000,
            max_ram_bytes: 1024 * 1024, max_disk_bytes: 0,
            max_cleanup_bytes: WORK_CLEANUP, max_child_count: 1,
        },
    ).unwrap()]).unwrap()
}

#[test]
fn foreign_run_with_equal_labels_refuses_before_ticket_and_foreign_charge() {
    let context = context();
    let source = context.revision().source().digest();
    let limits = ResourceLimits::uniform(32 * 1024 * 1024).unwrap();
    // Two new() roots, not clones or same-label replacement leases.
    let broker_a = ResourceBroker::new(limits).unwrap();
    let broker_b = ResourceBroker::new(limits).unwrap();
    let mut run_a = begin(&broker_a, source, limits, Deadline::after(Duration::from_secs(30)).unwrap());
    let mut run_b = begin(&broker_b, source, limits, Deadline::after(Duration::from_secs(30)).unwrap());
    let mut metadata = run_a.reserve(&[request(ResourceKind::Ram, SCHEDULER_RAM_BYTES)]).unwrap();
    let mut foreign_probe = run_b.reserve(&[request(ResourceKind::Ram, 1)]).unwrap();
    {
        let view = metadata.lease();
        let foreign = foreign_probe.lease();
        assert_eq!(view.run_id(), foreign.run_id());
        assert_eq!(view.source_revision(), foreign.source_revision());
        assert_eq!(view.reservation_id(), foreign.reservation_id());
        assert_ne!(view.owner_id(), foreign.owner_id());
        let mut scheduler = NativeScheduler::with_resources(&broker_a, &view, binding(), 4, 1, 100).unwrap();
        let query = query();
        let priority = PriorityInput::Missing(MissingPriorityReason::Unknown);
        let works = [work(&query, &context, &priority)];
        let registry = missing_executable_registry();
        let before_a = usage(&broker_a);
        let before_b = usage(&broker_b);
        let consumers = Cell::new(0usize);
        let result = scheduler.dispatch_next(
            &run_b, &works, [0; 32], 1, false, &registry,
            &std::env::current_dir().unwrap(), |_| consumers.set(consumers.get() + 1),
        );
        let refused = matches!(&result, Err(DispatchError::Resource(ResourceError::ForeignOwner)));
        // Collect the distinguishing witness together. An error string alone is
        // insufficient, and Drop is not a foreign-reservation refund.
        assert_eq!(
            (refused, scheduler.slots_consumed(), consumers.get(), usage(&broker_a), usage(&broker_b)),
            (true, 0, 0, before_a, before_b),
            "foreign run must refuse before effects; actual_error={:?}", result.as_ref().err(),
        );
    }
    // Teardown follows the main assertion so retained foreign charge cannot
    // replace the intended RED with a close_confirmed failure.
    metadata.release_confirmed().unwrap();
    foreign_probe.release_confirmed().unwrap();
    run_a.close_confirmed().unwrap();
    run_b.close_confirmed().unwrap();
    assert_eq!(usage(&broker_a), [0; 6]);
    assert_eq!(usage(&broker_b), [0; 6]);
}

#[test]
fn own_run_reaches_registered_missing_executable_with_actual_settlement() {
    let context = context();
    let limits = ResourceLimits::uniform(32 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let mut run = begin(&broker, context.revision().source().digest(), limits, Deadline::after(Duration::from_secs(30)).unwrap());
    let mut metadata = run.reserve(&[request(ResourceKind::Ram, SCHEDULER_RAM_BYTES)]).unwrap();
    let retained;
    {
        let view = metadata.lease();
        let mut scheduler = NativeScheduler::with_resources(&broker, &view, binding(), 4, 1, 100).unwrap();
        let query = query();
        let priority = PriorityInput::Missing(MissingPriorityReason::Unknown);
        let works = [work(&query, &context, &priority)];
        let before = usage(&broker);
        let consumers = Cell::new(0usize);
        let step = scheduler.dispatch_next(
            &run, &works, [0; 32], 1, false, &missing_executable_registry(),
            &std::env::current_dir().unwrap(), |native| {
                consumers.set(consumers.get() + 1);
                matches!(native, Err(NativeModelError::Execution(OwnedPayloadError::Report(report)))
                    if report.state() == SolverExecutionState::NotStarted
                    && report.cleanup_state() == ProcessCleanupState::RunnerReportedNotStarted)
            },
        ).unwrap();
        let DispatchStep::Observed(receipt) = step else { panic!("valid own run must reach the registered native facade") };
        assert_eq!((scheduler.slots_consumed(), consumers.get()), (1, 1));
        assert_eq!(receipt.selected_key(), b"actual-owner-work");
        assert!(*receipt.value(), "require an actual typed NotStarted report, never infer it from None");
        let report = receipt.report().expect("retain the actual missing-executable report");
        assert_eq!(report.state(), SolverExecutionState::NotStarted);
        assert_eq!(report.cleanup_state(), ProcessCleanupState::RunnerReportedNotStarted);
        assert!(!report.authority_created());
        retained = match receipt.reservation_settlement() {
            ReservationSettlement::Released => {
                assert_eq!(receipt.ticket_settlement(), &TicketSettlement::Settled);
                assert_eq!(usage(&broker), before);
                false
            }
            ReservationSettlement::Retained(_) => {
                assert_eq!(receipt.ticket_settlement(), &TicketSettlement::Retained);
                assert_eq!(usage(&broker), add_usage(before, work_charge()));
                true
            }
        };
    }
    metadata.release_confirmed().unwrap();
    if retained {
        assert_eq!(usage(&broker), work_charge());
        assert_eq!(run.close_confirmed(), Err(ResourceError::CleanupUnverified));
    } else {
        run.close_confirmed().unwrap();
        assert_eq!(usage(&broker), [0; 6]);
    }
}

#[test]
fn legitimate_own_run_exhaustion_still_consumes_its_issued_ticket() {
    let context = context();
    let limits = ResourceLimits::uniform(SCHEDULER_RAM_BYTES).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let mut run = begin(&broker, context.revision().source().digest(), limits, Deadline::after(Duration::from_secs(30)).unwrap());
    let mut metadata = run.reserve(&[request(ResourceKind::Ram, SCHEDULER_RAM_BYTES)]).unwrap();
    {
        let view = metadata.lease();
        let mut scheduler = NativeScheduler::with_resources(&broker, &view, binding(), 4, 1, 100).unwrap();
        let query = query();
        let priority = PriorityInput::Missing(MissingPriorityReason::Unknown);
        let works = [work(&query, &context, &priority)];
        let registry = missing_executable_registry();
        let before = usage(&broker);
        let consumers = Cell::new(0usize);
        let result = scheduler.dispatch_next(
            &run, &works, [0; 32], 1, false, &registry,
            &std::env::current_dir().unwrap(), |_| consumers.set(consumers.get() + 1),
        );
        assert!(matches!(result, Err(DispatchError::Resource(ResourceError::Exhausted(ResourceKind::Ram)))));
        assert_eq!((scheduler.slots_consumed(), consumers.get(), usage(&broker)), (1, 0, before));
        let next = scheduler.dispatch_next(
            &run, &works, [0; 32], 2, false, &registry,
            &std::env::current_dir().unwrap(), |_| consumers.set(consumers.get() + 1),
        ).unwrap();
        assert!(matches!(next, DispatchStep::Stopped(_)));
        assert_eq!((scheduler.slots_consumed(), consumers.get(), usage(&broker)), (1, 0, before));
    }
    metadata.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
    assert_eq!(usage(&broker), [0; 6]);
}

#[test]
fn existing_live_deadline_expiry_refuses_before_ticket_or_new_charge() {
    let context = context();
    let query = query();
    let priority = PriorityInput::Missing(MissingPriorityReason::Unknown);
    let works = [work(&query, &context, &priority)];
    let registry = missing_executable_registry();
    let cwd = std::env::current_dir().unwrap();
    let limits = ResourceLimits::uniform(32 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    // The view is genuinely admitted while live, then the SAME fixed deadline
    // expires. This does not claim a public live-deadline narrowing API exists.
    let expires_at = Instant::now() + Duration::from_secs(1);
    let mut run = begin(&broker, context.revision().source().digest(), limits, Deadline::at(expires_at));
    let mut metadata = run.reserve(&[request(ResourceKind::Ram, SCHEDULER_RAM_BYTES)])
        .expect("admit the actual fixture reservation before its fixed deadline");
    {
        let view = metadata.lease();
        let mut scheduler = NativeScheduler::with_resources(&broker, &view, binding(), 4, 1, 100)
            .expect("construct the scheduler while its actual metadata is live");
        view.check().expect("the retained view must first be live");
        let before = usage(&broker);
        std::thread::sleep(expires_at.saturating_duration_since(Instant::now()) + Duration::from_millis(5));
        assert_eq!(view.check(), Err(ResourceError::DeadlineExceeded));
        assert_eq!(view.remaining(), Err(ResourceError::DeadlineExceeded));
        assert_eq!(broker.validate_lease(&view, view.run_id(), view.source_revision(), ResourceKind::Ram), Err(ResourceError::DeadlineExceeded));
        let consumers = Cell::new(0usize);
        let result = scheduler.dispatch_next(
            &run, &works, [0; 32], 1, false, &registry, &cwd,
            |_| consumers.set(consumers.get() + 1),
        );
        assert!(matches!(result, Err(DispatchError::Resource(ResourceError::DeadlineExceeded))));
        assert_eq!((scheduler.slots_consumed(), consumers.get(), usage(&broker)), (0, 0, before));
        let reserve = run.reserve(&[request(ResourceKind::Ram, 1)]);
        assert!(matches!(reserve, Err(ResourceError::DeadlineExceeded)));
        assert_eq!(usage(&broker), before);
    }
    // No child or remaining live claim exists; explicit closure is separate
    // from the failed deadline check, rather than an assumed Drop refund.
    metadata.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
    assert_eq!(usage(&broker), [0; 6]);
}
