//! Parent-central opt-in profile: real pinned external Z3, never portable pass.
use lc631_core::{
    context::{ContextLimits, ContextScope, ContextState, OriginalContext, SourceRevision},
    measurement_numeric::{BigInt, Rational},
    AuthorityRevision,
};
use lc631_runtime::{resource::*, scheduled_solver::*};
use lc631_solvers::{
    protocol::{sexpr::SExprLimits, SolverProtocol},
    registry::{BackendId, LogicProfile, SolverRegistration, SolverRegistry, SolverResourceCaps},
    runner::{ExecutionBudget, ProcessCleanupState},
    typed_query::{BoolExpr, IntExpr, IntSymbolId, SmtEmitLimits, SmtStatusQuery},
};
use lc631_world::exploration_schedule::{
    priority::{PriorityInput, UnitInterval},
    ScheduleBinding, SchedulerNoveltyPolicyId,
};
use std::{path::PathBuf, time::Duration};

fn state() -> ContextState {
    let p = Digest32::from_bytes([1; 32]);
    OriginalContext::checked(
        SourceRevision::of("scheduled-native-selection-fixture"),
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
fn priority(n: i64) -> PriorityInput {
    let r = |x: i64| Rational::from_integer(BigInt::from(x));
    PriorityInput::Discrete {
        uncertainty: UnitInterval::checked(r(0), r(0)).unwrap(),
        novelty: UnitInterval::checked(r(n), r(n)).unwrap(),
    }
}
fn query(n: i64) -> SmtStatusQuery {
    SmtStatusQuery::checked(
        vec![],
        vec![IntSymbolId::new(0)],
        vec![BoolExpr::Equal(
            IntExpr::Variable(IntSymbolId::new(0)),
            IntExpr::Constant(n),
        )],
        SmtEmitLimits::checked(4096, 0, 1, 8, 32, 8).unwrap(),
    )
    .unwrap()
}
fn binding() -> ScheduleBinding {
    ScheduleBinding {
        comparison_scope: [1; 32],
        selection_policy: [2; 32],
        budget_series: [3; 32],
        novelty_policy: SchedulerNoveltyPolicyId::DistanceToFrontierUnionV1,
    }
}
fn registry() -> SolverRegistry {
    let path = PathBuf::from(
        std::env::var_os("EPISTESYS_TLCX10_NATIVE_Z3")
            .expect("parent-authorized native executable required for explicitly ignored profile"),
    );
    assert!(path.is_absolute());
    // Prior platform-local byte pins, not signed image/version/license evidence.
    let pin = match std::env::consts::OS {
        "windows" => [
            0x5f, 0x32, 0xb2, 0xd2, 0xb9, 0x32, 0x91, 0xb5, 0x6e, 0x3c, 0x10, 0xa4, 0x3b, 0x08,
            0x8c, 0xae, 0x78, 0xac, 0xc8, 0x40, 0x17, 0xeb, 0xff, 0x8f, 0xa7, 0x41, 0xa7, 0xec,
            0xee, 0x6c, 0x6c, 0xa5,
        ],
        "linux" => [
            0xc4, 0x5e, 0x78, 0x7b, 0x5d, 0x1a, 0x3d, 0x94, 0x07, 0x9d, 0x1e, 0x83, 0xb9, 0x8e,
            0x23, 0xbb, 0xc8, 0x84, 0xcc, 0xaf, 0x54, 0x2e, 0xa3, 0x8c, 0x38, 0x14, 0xf3, 0x1a,
            0xc2, 0xf7, 0x91, 0x97,
        ],
        other => panic!("unsupported native profile: {other}"),
    };
    SolverRegistry::checked(vec![SolverRegistration::checked(
        BackendId::Z3,
        LogicProfile::SmtBooleanQfLia,
        SolverProtocol::SmtLibPayloadWireV2,
        path.to_str().unwrap(),
        "5.3.2",
        pin,
        vec![
            "-smt2".into(),
            "-in".into(),
            "-T:2".into(),
            "-memory:64".into(),
        ],
        SolverResourceCaps {
            max_argv_count: 16,
            max_argv_bytes: 4096,
            max_stdin_bytes: 4096,
            max_stdout_bytes: 65536,
            max_stderr_bytes: 16384,
            max_timeout_ms: 5000,
            max_ram_bytes: 64 * 1024 * 1024,
            max_disk_bytes: 0,
            max_cleanup_bytes: 65536,
            max_child_count: 1,
        },
    )
    .unwrap()])
    .unwrap()
}
fn native_work<'a>(
    key: &'a [u8],
    q: &'a SmtStatusQuery,
    p: &'a PriorityInput,
    c: &'a ContextState,
    budget: ExecutionBudget,
) -> NativeWork<'a> {
    NativeWork::checked(NativeWorkSpec {
        binding: binding(),
        canonical_key: key,
        priority: p,
        estimated_query_slots: 1,
        query: q,
        context: c,
        budget,
        parser: SExprLimits::checked(65536, 4096, 64).unwrap(),
        demand: NativeDemand::checked([1, 0, 1], 128 * 1024 * 1024, 2 * 1024 * 1024, 1024 * 1024)
            .unwrap(),
    })
    .unwrap()
}

fn experiment(prefer_b: bool) -> (i128, lc631_solvers::runner::QueryDigest) {
    let c = state();
    let limits = ResourceLimits::uniform(256 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let mut run = broker
        .begin_run(
            1,
            c.revision().source().digest(),
            limits,
            Deadline::after(Duration::from_secs(60)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let request = |kind, n| ResourceRequest::new(kind, ResourceAmount::checked(n).unwrap());
    let mut meta = run
        .reserve(&[request(ResourceKind::Ram, SCHEDULER_RAM_BYTES)])
        .unwrap();
    let second = {
        let view = meta.lease();
        let mut scheduler =
            NativeScheduler::with_resources(&broker, &view, binding(), 4, 2, 100).unwrap();
        let a = query(3);
        let b = query(7);
        let pa = priority(i64::from(!prefer_b));
        let pb = priority(i64::from(prefer_b));
        let budget = ExecutionBudget::checked(
            5000,
            4096,
            65536,
            16384,
            16,
            4096,
            64 * 1024 * 1024,
            0,
            65536,
        )
        .unwrap();
        let jobs = [
            native_work(b"a", &a, &pa, &c, budget),
            native_work(b"b", &b, &pb, &c, budget),
        ];
        let registry = registry();
        let cwd = std::env::current_dir().unwrap();
        let mut returned = None;
        for tick in 1..=2 {
            let step = scheduler
                .dispatch_next(
                    &run,
                    &jobs,
                    [0; 32],
                    tick,
                    false,
                    &registry,
                    &cwd,
                    |native| match native {
                        Ok(model) => model
                            .model()
                            .integer(IntSymbolId::new(0))
                            .expect("checked selected model"),
                        Err(_) => panic!("real native model execution or checking rejected"),
                    },
                )
                .unwrap();
            let DispatchStep::Observed(receipt) = step else {
                panic!("actual native observation required")
            };
            assert_eq!(
                receipt.reservation_settlement(),
                &ReservationSettlement::Released
            );
            assert_eq!(receipt.ticket_settlement(), &TicketSettlement::Settled);
            let report = receipt.report().expect("actual native report");
            assert_eq!(
                report.cleanup_state(),
                ProcessCleanupState::RunnerReportedVerified
            );
            assert!(!report.proof_checked());
            assert!(!report.authority_created());
            if tick == 1 {
                assert_eq!(receipt.selected_key(), b"a");
                assert_eq!(*receipt.value(), 3);
            } else {
                assert_eq!(
                    receipt.selected_key(),
                    if prefer_b { &b"b"[..] } else { &b"a"[..] }
                );
                returned = Some((*receipt.value(), report.query()));
            }
            assert_eq!(broker.used(ResourceKind::Child).unwrap(), 0);
        }
        returned.unwrap()
    };
    meta.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
    assert_eq!(broker.used(ResourceKind::Ram).unwrap(), 0);
    second
}

#[test]
#[ignore = "parent-central opt-in pinned native query-selection/real-broker integration"]
fn tlcx25_priority_changes_selected_real_query_and_checked_model_under_same_budget() {
    let (preferred_b, query_b) = experiment(true);
    let (preferred_a, query_a) = experiment(false);
    assert_eq!((preferred_b, preferred_a), (7, 3));
    assert_ne!(query_a, query_b);
}
