use lc631_core::{context::*, AuthorityRevision};

#[test]
fn tlcx25_foreign_source_and_logical_deadline_stop_before_native_dispatch() {
    let c = state();
    let foreign = OriginalContext::checked(
        SourceRevision::of("foreign scheduled source"),
        c.scope(),
        AuthorityRevision(1),
        vec![],
        ContextLimits {
            max_contexts: 8,
            max_premises: 8,
            max_edits: 8,
            max_statement_bytes: 4096,
        },
    )
    .unwrap()
    .state()
    .clone();
    let (broker, run) = broker_run(
        c.revision().source().digest(),
        32 * 1024 * 1024,
        Cancellation::default(),
    );
    let mut meta = run
        .reserve(&[request(ResourceKind::Ram, SCHEDULER_RAM_BYTES)])
        .unwrap();
    {
        let view = meta.lease();
        let mut scheduler =
            NativeScheduler::with_resources(&broker, &view, binding(), 4, 1, 100).unwrap();
        let q = query(true);
        let p = PriorityInput::Missing(MissingPriorityReason::Unknown);
        let jobs = [work(b"a", &q, &foreign, &p)];
        let registry = no_spawn_registry();
        let cwd = std::env::current_dir().unwrap();
        assert!(matches!(
            scheduler.dispatch_next(&run, &jobs, [0; 32], 1, false, &registry, &cwd, |_| ()),
            Err(DispatchError::SourceMismatch)
        ));
        assert_eq!(scheduler.slots_consumed(), 0);
        let jobs = [work(b"a", &q, &c, &p)];
        let step = scheduler
            .dispatch_next(&run, &jobs, [0; 32], 100, false, &registry, &cwd, |_| ())
            .unwrap();
        assert!(matches!(
            step,
            DispatchStep::Stopped(lc631_world::exploration_schedule::StopReason::Deadline)
        ));
        assert_eq!(scheduler.slots_consumed(), 0);
        assert_eq!(broker.used(ResourceKind::Child).unwrap(), 0);
    }
    meta.release_confirmed().unwrap();
}
use lc631_runtime::resource::*;
use lc631_runtime::scheduled_solver::*;
use lc631_solvers::{
    protocol::{sexpr::SExprLimits, SolverProtocol},
    registry::{BackendId, LogicProfile, SolverRegistration, SolverRegistry, SolverResourceCaps},
    runner::ExecutionBudget,
    typed_query::{BoolExpr, SmtEmitLimits, SmtStatusQuery},
};
use lc631_world::exploration_schedule::{
    priority::{MissingPriorityReason, PriorityInput},
    ScheduleBinding, SchedulerNoveltyPolicyId,
};
use std::time::Duration;

fn state() -> ContextState {
    let p = Digest32::from_bytes([1; 32]);
    OriginalContext::checked(
        SourceRevision::of("scheduled solver fixture"),
        ContextScope::declared(p, p, p, p),
        AuthorityRevision(1),
        vec![],
        ContextLimits {
            max_contexts: 8,
            max_premises: 8,
            max_edits: 8,
            max_statement_bytes: 4096,
        },
    )
    .unwrap()
    .state()
    .clone()
}
fn binding() -> ScheduleBinding {
    ScheduleBinding {
        comparison_scope: [1; 32],
        selection_policy: [2; 32],
        budget_series: [3; 32],
        novelty_policy: SchedulerNoveltyPolicyId::DistanceToFrontierUnionV1,
    }
}
fn request(kind: ResourceKind, n: u64) -> ResourceRequest {
    ResourceRequest::new(kind, ResourceAmount::checked(n).unwrap())
}
fn budget() -> ExecutionBudget {
    ExecutionBudget::checked(1000, 4096, 4096, 1024, 16, 4096, 1024 * 1024, 0, 65536).unwrap()
}
fn query(value: bool) -> SmtStatusQuery {
    SmtStatusQuery::checked(
        vec![],
        vec![],
        vec![BoolExpr::Constant(value)],
        SmtEmitLimits::checked(4096, 0, 0, 4, 8, 4).unwrap(),
    )
    .unwrap()
}
fn work<'a>(
    key: &'a [u8],
    q: &'a SmtStatusQuery,
    c: &'a ContextState,
    p: &'a PriorityInput,
) -> NativeWork<'a> {
    NativeWork::checked(NativeWorkSpec {
        binding: binding(),
        canonical_key: key,
        priority: p,
        estimated_query_slots: 1,
        query: q,
        context: c,
        budget: budget(),
        parser: SExprLimits::checked(4096, 128, 16).unwrap(),
        demand: NativeDemand::checked([1, 0, 1], 8 * 1024 * 1024, 16384, 65536).unwrap(),
    })
    .unwrap()
}
fn broker_run(
    source: Digest32,
    limit: u64,
    cancel: Cancellation,
) -> (ResourceBroker, RunResources) {
    let limits = ResourceLimits::uniform(limit).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let run = broker
        .begin_run(
            1,
            source,
            limits,
            Deadline::after(Duration::from_secs(30)).unwrap(),
            cancel,
        )
        .unwrap();
    (broker, run)
}
fn no_spawn_registry() -> SolverRegistry {
    let caps = SolverResourceCaps {
        max_argv_count: 16,
        max_argv_bytes: 4096,
        max_stdin_bytes: 4096,
        max_stdout_bytes: 4096,
        max_stderr_bytes: 1024,
        max_timeout_ms: 1000,
        max_ram_bytes: 1024 * 1024,
        max_disk_bytes: 0,
        max_cleanup_bytes: 65536,
        max_child_count: 1,
    };
    SolverRegistry::checked(vec![SolverRegistration::checked(
        BackendId::Z3,
        LogicProfile::SmtBooleanQfLia,
        SolverProtocol::SmtLibPayloadWireV2,
        std::env::current_exe().unwrap().to_str().unwrap(),
        "wrong-pin-never-spawn",
        [1; 32],
        vec![],
        caps,
    )
    .unwrap()])
    .unwrap()
}

#[test]
fn tlcx25_missing_backend_is_explicit_and_consumes_no_dispatch_ticket() {
    let c = state();
    let (broker, mut run) = broker_run(
        c.revision().source().digest(),
        32 * 1024 * 1024,
        Cancellation::default(),
    );
    let mut meta = run
        .reserve(&[request(ResourceKind::Ram, SCHEDULER_RAM_BYTES)])
        .unwrap();
    {
        let view = meta.lease();
        let mut scheduler =
            NativeScheduler::with_resources(&broker, &view, binding(), 4, 1, 100).unwrap();
        let q = query(true);
        let p = PriorityInput::Missing(MissingPriorityReason::Unknown);
        let jobs = [work(b"a", &q, &c, &p)];
        // Valid non-Z3 declaration only; this fixture backend is never executed.
        let other_backend = SolverRegistry::checked(vec![SolverRegistration::checked(
            BackendId::Clingo,
            LogicProfile::AspFiniteGroundNormal,
            SolverProtocol::ClingoStatusV1,
            std::env::current_exe().unwrap().to_str().unwrap(),
            "fixture-declared-not-executed",
            [1; 32],
            vec![],
            no_spawn_registry().entries()[0].caps(),
        )
        .unwrap()])
        .unwrap();
        let step = scheduler
            .dispatch_next(
                &run,
                &jobs,
                [0; 32],
                1,
                false,
                &other_backend,
                &std::env::current_dir().unwrap(),
                |_| panic!("backend unavailable"),
            )
            .unwrap();
        assert!(matches!(
            step,
            DispatchStep::Unavailable(UnavailableReason::Registry(
                lc631_solvers::registry::SolverRegistryError::UnsupportedProfile
            ))
        ));
        assert_eq!(scheduler.slots_consumed(), 0);
        assert_eq!(broker.used(ResourceKind::Child).unwrap(), 0);
    }
    meta.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}

#[test]
fn tlcx25_real_broker_exhaustion_blocks_ticket_without_spawning_or_refunding_it() {
    let c = state();
    let (broker, _run) = broker_run(
        c.revision().source().digest(),
        SCHEDULER_RAM_BYTES,
        Cancellation::default(),
    );
    let run = &_run;
    let mut meta = run
        .reserve(&[request(ResourceKind::Ram, SCHEDULER_RAM_BYTES)])
        .unwrap();
    {
        let view = meta.lease();
        let mut scheduler =
            NativeScheduler::with_resources(&broker, &view, binding(), 4, 1, 100).unwrap();
        let q = query(true);
        let p = PriorityInput::Missing(MissingPriorityReason::Unknown);
        let jobs = [work(b"a", &q, &c, &p)];
        let registry = no_spawn_registry();
        let result = scheduler.dispatch_next(
            run,
            &jobs,
            [0; 32],
            1,
            false,
            &registry,
            &std::env::current_dir().unwrap(),
            |_| (),
        );
        assert!(matches!(
            result,
            Err(DispatchError::Resource(ResourceError::Exhausted(
                ResourceKind::Ram
            )))
        ));
        assert_eq!(scheduler.slots_consumed(), 1);
        let next = scheduler
            .dispatch_next(
                run,
                &jobs,
                [0; 32],
                2,
                false,
                &registry,
                &std::env::current_dir().unwrap(),
                |_| (),
            )
            .unwrap();
        assert!(matches!(next, DispatchStep::Stopped(_)));
        assert_eq!(broker.used(ResourceKind::Child).unwrap(), 0);
    }
    meta.release_confirmed().unwrap();
}

#[test]
fn tlcx25_real_cancellation_and_foreign_metadata_owner_reject_before_ticket() {
    let c = state();
    let cancel = Cancellation::default();
    let (broker, run) = broker_run(
        c.revision().source().digest(),
        32 * 1024 * 1024,
        cancel.clone(),
    );
    let mut meta = run
        .reserve(&[request(ResourceKind::Ram, SCHEDULER_RAM_BYTES)])
        .unwrap();
    {
        let view = meta.lease();
        let (foreign, _) = broker_run(
            c.revision().source().digest(),
            32 * 1024 * 1024,
            Cancellation::default(),
        );
        assert!(matches!(
            NativeScheduler::with_resources(&foreign, &view, binding(), 4, 1, 100),
            Err(DispatchError::Resource(ResourceError::ForeignOwner))
        ));
        let mut scheduler =
            NativeScheduler::with_resources(&broker, &view, binding(), 4, 1, 100).unwrap();
        let q = query(true);
        let p = PriorityInput::Missing(MissingPriorityReason::Unknown);
        let jobs = [work(b"a", &q, &c, &p)];
        cancel.cancel();
        assert!(matches!(
            scheduler.dispatch_next(
                &run,
                &jobs,
                [0; 32],
                1,
                false,
                &no_spawn_registry(),
                &std::env::current_dir().unwrap(),
                |_| ()
            ),
            Err(DispatchError::Resource(ResourceError::Cancelled))
        ));
        assert_eq!(scheduler.slots_consumed(), 0);
    }
    meta.release_confirmed().unwrap();
}

#[test]
fn tlcx25_duplicate_key_cannot_switch_to_a_different_native_query() {
    let c = state();
    let (broker, run) = broker_run(
        c.revision().source().digest(),
        32 * 1024 * 1024,
        Cancellation::default(),
    );
    let mut meta = run
        .reserve(&[request(ResourceKind::Ram, SCHEDULER_RAM_BYTES)])
        .unwrap();
    {
        let view = meta.lease();
        let mut scheduler =
            NativeScheduler::with_resources(&broker, &view, binding(), 4, 1, 100).unwrap();
        let a = query(true);
        let b = query(false);
        let p = PriorityInput::Missing(MissingPriorityReason::Unknown);
        let jobs = [work(b"same", &a, &c, &p), work(b"same", &b, &c, &p)];
        assert!(matches!(
            scheduler.dispatch_next(
                &run,
                &jobs,
                [0; 32],
                1,
                false,
                &no_spawn_registry(),
                &std::env::current_dir().unwrap(),
                |_| ()
            ),
            Err(DispatchError::ConflictingWork)
        ));
        assert_eq!(scheduler.slots_consumed(), 0);
    }
    meta.release_confirmed().unwrap();
}

#[test]
fn tlcx25_gpu_and_undersized_native_demands_are_not_cpu_quota_claims() {
    assert!(matches!(
        NativeDemand::checked([1, 1, 1], 1, 1, 1),
        Err(DispatchError::GpuUnsupported)
    ));
    assert!(matches!(
        NativeDemand::checked([0, 0, 1], 1, 1, 1),
        Err(DispatchError::InvalidDemand)
    ));
    let c = state();
    let q = query(true);
    let p = PriorityInput::Missing(MissingPriorityReason::Unknown);
    let spec = NativeWorkSpec {
        binding: binding(),
        canonical_key: b"a",
        priority: &p,
        estimated_query_slots: 1,
        query: &q,
        context: &c,
        budget: budget(),
        parser: SExprLimits::checked(4096, 128, 16).unwrap(),
        demand: NativeDemand::checked([1, 0, 1], 1, 1, 1).unwrap(),
    };
    assert!(matches!(
        NativeWork::checked(spec),
        Err(DispatchError::InvalidDemand)
    ));
}
