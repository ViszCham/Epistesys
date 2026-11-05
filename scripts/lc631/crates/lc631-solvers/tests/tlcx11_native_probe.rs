//! Opt-in concrete strict-improvement probes; no detached report fixtures.
use lc631_solvers::{
    protocol::{sexpr::SExprLimits, SolverProtocol},
    registry::{BackendId, LogicProfile, SolverRegistration, SolverRegistry, SolverResourceCaps},
    runner::ExecutionBudget,
    smt::{
        native_model::execute_native_model,
        native_probe::{execute_native_probe, NativeProbeError, NativeProbeStatus, Unboundedness},
        ObjectiveDirection,
    },
    supervisor_adapter::OwnedPayloadStore,
    typed_query::{BoolExpr, IntExpr, IntSymbolId, SmtEmitLimits, SmtStatusQuery},
};
use lc631_substrate::context_identity::{ContextRevision, SourceRevision};
use lc631_substrate::resource::{
    Cancellation, Deadline, ReportedResourceLease, ResourceAmount, ResourceKind, ResourceLease,
    ResourceLeaseIssuer, ResourceRequest,
};
use std::{path::PathBuf, time::Duration};

fn context() -> ContextRevision {
    ContextRevision::original(SourceRevision::of("tlcx11-native-probe"))
}
fn resource_lease(context: ContextRevision) -> ResourceLease {
    resource_lease_with_cancel(context, Cancellation::default())
}
fn resource_lease_with_cancel(
    context: ContextRevision,
    cancellation: Cancellation,
) -> ResourceLease {
    let req = |kind, amount| ResourceRequest::new(kind, ResourceAmount::checked(amount).unwrap());
    let report = ReportedResourceLease::checked(
        1,
        context.source().digest(),
        &[
            req(ResourceKind::Ram, 128 * 1024 * 1024),
            req(ResourceKind::Output, 2 * 1024 * 1024),
            req(ResourceKind::Child, 1),
            req(ResourceKind::Cleanup, 1024 * 1024),
        ],
        Deadline::after(Duration::from_secs(60)).unwrap(),
        cancellation,
    )
    .unwrap();
    ResourceLeaseIssuer::new()
        .unwrap()
        .issue_admitted(1, report)
        .unwrap()
}
fn budget(ram: u64) -> ExecutionBudget {
    ExecutionBudget::checked(5000, 4096, 65_536, 16_384, 16, 4096, ram, 0, 65_536).unwrap()
}
const WINDOWS_PIN: [u8; 32] = [
    0x5f, 0x32, 0xb2, 0xd2, 0xb9, 0x32, 0x91, 0xb5, 0x6e, 0x3c, 0x10, 0xa4, 0x3b, 0x08, 0x8c, 0xae,
    0x78, 0xac, 0xc8, 0x40, 0x17, 0xeb, 0xff, 0x8f, 0xa7, 0x41, 0xa7, 0xec, 0xee, 0x6c, 0x6c, 0xa5,
];
const LINUX_PIN: [u8; 32] = [
    0xc4, 0x5e, 0x78, 0x7b, 0x5d, 0x1a, 0x3d, 0x94, 0x07, 0x9d, 0x1e, 0x83, 0xb9, 0x8e, 0x23, 0xbb,
    0xc8, 0x84, 0xcc, 0xaf, 0x54, 0x2e, 0xa3, 0x8c, 0x38, 0x14, 0xf3, 0x1a, 0xc2, 0xf7, 0x91, 0x97,
];
fn registry() -> SolverRegistry {
    let path = PathBuf::from(
        std::env::var_os("EPISTESYS_TLCX10_NATIVE_Z3")
            .expect("explicit pinned executable required"),
    );
    assert!(path.is_absolute());
    let pin = match std::env::consts::OS {
        "windows" => WINDOWS_PIN,
        "linux" => LINUX_PIN,
        _ => panic!("unsupported native profile"),
    };
    let caps = SolverResourceCaps {
        max_argv_count: 16,
        max_argv_bytes: 4096,
        max_stdin_bytes: 4096,
        max_stdout_bytes: 65_536,
        max_stderr_bytes: 16_384,
        max_timeout_ms: 5000,
        max_ram_bytes: 64 * 1024 * 1024,
        max_disk_bytes: 0,
        max_cleanup_bytes: 65_536,
        max_child_count: 1,
    };
    let entries = [
        SolverProtocol::SmtLibStatusV1,
        SolverProtocol::SmtLibPayloadWireV2,
    ]
    .into_iter()
    .map(|protocol| {
        SolverRegistration::checked(
            BackendId::Z3,
            LogicProfile::SmtBooleanQfLia,
            protocol,
            path.to_str().unwrap(),
            "5.3.2",
            pin,
            vec![
                "-smt2".into(),
                "-in".into(),
                "-T:2".into(),
                "-memory:64".into(),
            ],
            caps,
        )
        .unwrap()
    })
    .collect();
    SolverRegistry::checked(entries).unwrap()
}
fn query(bounded: bool) -> SmtStatusQuery {
    let assertions = if bounded {
        vec![BoolExpr::LessEqual(
            IntExpr::Constant(3),
            IntExpr::Variable(IntSymbolId::new(0)),
        )]
    } else {
        vec![]
    };
    SmtStatusQuery::checked(
        vec![],
        vec![IntSymbolId::new(0)],
        assertions,
        SmtEmitLimits::checked(4096, 0, 1, 16, 128, 16).unwrap(),
    )
    .unwrap()
}

#[test]
#[ignore = "opt-in pinned native Z3 strict-improvement UNSAT"]
fn tlcx11_native_probe_reports_no_strict_minimum_improvement_not_proof() {
    let context = context().reconstructed([9; 32]);
    let lease = resource_lease(context);
    let view = lease.view();
    let query = query(true);
    let registry = registry();
    let mut store = OwnedPayloadStore::with_resources(1, &view).unwrap();
    let checked = execute_native_model(
        &registry,
        &query,
        context,
        &budget(64 * 1024 * 1024),
        &view,
        &std::env::current_dir().unwrap(),
        &mut store,
        SExprLimits::checked(65_536, 4096, 64).unwrap(),
    )
    .ok()
    .unwrap();
    assert_eq!(checked.model().integer(IntSymbolId::new(0)), Some(3));
    let result = execute_native_probe(
        &registry,
        &checked,
        IntSymbolId::new(0),
        ObjectiveDirection::Minimize,
        &budget(64 * 1024 * 1024),
        &view,
        &std::env::current_dir().unwrap(),
    )
    .unwrap();
    assert_eq!(
        result.status(),
        NativeProbeStatus::SolverReportedNoStrictImprovement
    );
    assert_eq!(result.context(), context);
    assert!(result.insertion().is_ok());
    assert_eq!(result.stored().unwrap().report(), result.report());
    assert_eq!(result.unboundedness(), Unboundedness::NotEstablished);

    let cut = lc631_solvers::smt::native_probe::bounded::consume_bounded_improvement(
        &result,
        &registry,
        &budget(64 * 1024 * 1024),
        &view,
        &std::env::current_dir().unwrap(),
        SExprLimits::default(),
    );
    assert!(matches!(cut, Err(lc631_solvers::smt::native_probe::bounded::BoundedCandidateError::NotReportedImprovement)));
}

#[test]
#[ignore = "opt-in pinned Z3 candidate and cancellation consumption cuts"]
fn tlcx11_native_bounded_probe_consumes_a_real_improved_model_and_gap() {
    use lc631_solvers::smt::native_probe::bounded::{
        consume_bounded_improvement, BoundedCandidateError,
    };
    use lc631_substrate::resource::ResourceError;
    let context = context();
    let cancellation = Cancellation::default();
    let lease = resource_lease_with_cancel(context, cancellation.clone());
    let view = lease.view();
    let registry = registry();
    let i = IntSymbolId::new(0);
    let query = SmtStatusQuery::checked(
        vec![],
        vec![i],
        vec![
            BoolExpr::LessEqual(IntExpr::Constant(-3), IntExpr::Variable(i)),
            BoolExpr::LessEqual(IntExpr::Variable(i), IntExpr::Constant(0)),
        ],
        SmtEmitLimits::checked(4096, 0, 1, 16, 128, 16).unwrap(),
    )
    .unwrap();
    let mut store = OwnedPayloadStore::with_resources(1, &view).unwrap();
    let checked = execute_native_model(
        &registry,
        &query,
        context,
        &budget(64 * 1024 * 1024),
        &view,
        &std::env::current_dir().unwrap(),
        &mut store,
        SExprLimits::checked(65_536, 4096, 64).unwrap(),
    )
    .ok()
    .unwrap();
    let prior = checked.model().integer(i).unwrap();
    let direction = if prior == -3 {
        ObjectiveDirection::Maximize
    } else {
        ObjectiveDirection::Minimize
    };
    let probe = execute_native_probe(
        &registry,
        &checked,
        i,
        direction,
        &budget(64 * 1024 * 1024),
        &view,
        &std::env::current_dir().unwrap(),
    )
    .unwrap();
    assert_eq!(
        probe.status(),
        NativeProbeStatus::SolverReportedImprovementExists
    );
    let rejected = consume_bounded_improvement(
        &probe,
        &registry,
        &budget(64 * 1024 * 1024),
        &view,
        &std::env::current_dir().unwrap(),
        SExprLimits::checked(65_536, 4, 4).unwrap(),
    );
    let Err(BoundedCandidateError::Model(
        lc631_solvers::smt::native_model::NativeModelError::Rejected { observation, .. },
    )) = rejected
    else {
        panic!("actual model parser cut must preserve observation");
    };
    assert!(observation.stdout().starts_with(b"sat"));
    drop(observation);
    let held = view
        .claim_resources(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(127 * 1024 * 1024).unwrap(),
        )])
        .unwrap();
    assert!(matches!(
        consume_bounded_improvement(
            &probe,
            &registry,
            &budget(64 * 1024 * 1024),
            &view,
            &std::env::current_dir().unwrap(),
            SExprLimits::default()
        ),
        Err(BoundedCandidateError::Model(
            lc631_solvers::smt::native_model::NativeModelError::Resource(ResourceError::Exhausted(
                ResourceKind::Ram
            ))
        ))
    ));
    drop(held);
    let candidate = consume_bounded_improvement(
        &probe,
        &registry,
        &budget(64 * 1024 * 1024),
        &view,
        &std::env::current_dir().unwrap(),
        SExprLimits::checked(65_536, 4096, 64).unwrap(),
    )
    .ok()
    .unwrap();
    let value = candidate.value();
    assert!((-3..=0).contains(&value));
    assert_eq!(candidate.stored_reports(), 1);
    assert_eq!(candidate.model().context(), context);
    let expected_gap = match direction {
        ObjectiveDirection::Minimize => {
            assert!(i128::from(value) < prior);
            i128::from(value) + 3
        }
        ObjectiveDirection::Maximize => {
            assert!(i128::from(value) > prior);
            -i128::from(value)
        }
    };
    assert_eq!(
        candidate.gap_upper_bound(),
        u128::try_from(expected_gap).unwrap()
    );
    drop(candidate);
    let foreign = resource_lease(context);
    let foreign_view = foreign.view();
    assert!(matches!(
        consume_bounded_improvement(
            &probe,
            &registry,
            &budget(64 * 1024 * 1024),
            &foreign_view,
            &std::env::current_dir().unwrap(),
            SExprLimits::default()
        ),
        Err(BoundedCandidateError::Resource(ResourceError::ForeignOwner))
    ));
    cancellation.cancel();
    assert!(matches!(
        consume_bounded_improvement(
            &probe,
            &registry,
            &budget(64 * 1024 * 1024),
            &view,
            &std::env::current_dir().unwrap(),
            SExprLimits::default()
        ),
        Err(BoundedCandidateError::Resource(ResourceError::Cancelled))
    ));
}

#[test]
#[ignore = "opt-in pinned native Z3 unbounded objective remains unestablished"]
fn tlcx11_native_probe_sat_does_not_establish_optimality_or_unboundedness() {
    let context = context();
    let lease = resource_lease(context);
    let view = lease.view();
    let query = query(false);
    let registry = registry();
    let mut store = OwnedPayloadStore::with_resources(1, &view).unwrap();
    let checked = execute_native_model(
        &registry,
        &query,
        context,
        &budget(64 * 1024 * 1024),
        &view,
        &std::env::current_dir().unwrap(),
        &mut store,
        SExprLimits::checked(65_536, 4096, 64).unwrap(),
    )
    .ok()
    .unwrap();
    assert_eq!(checked.model().integer(IntSymbolId::new(0)), Some(0));
    let result = execute_native_probe(
        &registry,
        &checked,
        IntSymbolId::new(0),
        ObjectiveDirection::Maximize,
        &budget(64 * 1024 * 1024),
        &view,
        &std::env::current_dir().unwrap(),
    )
    .unwrap();
    assert_eq!(
        result.status(),
        NativeProbeStatus::SolverReportedImprovementExists
    );
    assert_eq!(result.unboundedness(), Unboundedness::NotEstablished);
    let foreign = resource_lease(context);
    let foreign_view = foreign.view();
    let rejected = execute_native_probe(
        &registry,
        &checked,
        IntSymbolId::new(0),
        ObjectiveDirection::Minimize,
        &budget(64 * 1024 * 1024),
        &foreign_view,
        &std::env::current_dir().unwrap(),
    );
    assert!(matches!(rejected, Err(NativeProbeError::Resource(_))));
    let result = execute_native_probe(
        &registry,
        &checked,
        IntSymbolId::new(0),
        ObjectiveDirection::Maximize,
        &budget(128 * 1024 * 1024),
        &view,
        &std::env::current_dir().unwrap(),
    )
    .unwrap();
    assert!(matches!(
        result.status(),
        NativeProbeStatus::Unresolved { .. }
    ));
    assert_eq!(result.unboundedness(), Unboundedness::NotEstablished);
}
