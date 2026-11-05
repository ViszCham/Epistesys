//! Explicit native tests require the same opt-in executable pin as TLCX-10.
use lc631_solvers::{
    protocol::{sexpr::SExprLimits, SolverProtocol},
    registry::{BackendId, LogicProfile, SolverRegistration, SolverRegistry, SolverResourceCaps},
    runner::ExecutionBudget,
    smt::native_model::{execute_native_model, NativeModelError},
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
    ContextRevision::original(SourceRevision::of("tlcx11-native-model"))
}

fn lease(context: ContextRevision, ram: u64) -> ResourceLease {
    let request =
        |kind, amount| ResourceRequest::new(kind, ResourceAmount::checked(amount).unwrap());
    let report = ReportedResourceLease::checked(
        1,
        context.source().digest(),
        &[
            request(ResourceKind::Ram, ram),
            request(ResourceKind::Output, 2 * 1024 * 1024),
            request(ResourceKind::Child, 1),
            request(ResourceKind::Cleanup, 1024 * 1024),
        ],
        Deadline::after(Duration::from_secs(30)).unwrap(),
        Cancellation::default(),
    )
    .unwrap();
    ResourceLeaseIssuer::new()
        .unwrap()
        .issue_admitted(1, report)
        .unwrap()
}

fn query() -> SmtStatusQuery {
    SmtStatusQuery::checked(
        vec![],
        vec![IntSymbolId::new(0)],
        vec![BoolExpr::Equal(
            IntExpr::Variable(IntSymbolId::new(0)),
            IntExpr::Constant(-3),
        )],
        SmtEmitLimits::checked(4096, 0, 1, 16, 128, 16).unwrap(),
    )
    .unwrap()
}

fn budget() -> ExecutionBudget {
    ExecutionBudget::checked(
        5000,
        4096,
        65_536,
        16_384,
        16,
        4096,
        64 * 1024 * 1024,
        0,
        65_536,
    )
    .unwrap()
}

// Syntactically valid descriptor only. The deliberately wrong digest means it
// cannot dispatch even if a preflight guard regresses; it is not a backend test.
fn preflight_registry() -> SolverRegistry {
    let path = std::env::current_exe().unwrap();
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
    SolverRegistry::checked(vec![SolverRegistration::checked(
        BackendId::Z3,
        LogicProfile::SmtBooleanQfLia,
        SolverProtocol::SmtLibPayloadWireV2,
        path.to_str().unwrap(),
        "preflight-never-dispatch",
        [1; 32],
        vec!["-in".into()],
        caps,
    )
    .unwrap()])
    .unwrap()
}

#[test]
fn tlcx11_native_source_mismatch_is_rejected_before_dispatch() {
    let context = context();
    let lease = lease(context, 128 * 1024 * 1024);
    let view = lease.view();
    let mut store = OwnedPayloadStore::with_resources(1, &view).unwrap();
    let foreign = ContextRevision::original(SourceRevision::of("foreign"));
    let query = query();
    let result = execute_native_model(
        &preflight_registry(),
        &query,
        foreign,
        &budget(),
        &view,
        &std::env::current_dir().unwrap(),
        &mut store,
        SExprLimits::default(),
    );
    assert!(matches!(result, Err(NativeModelError::SourceMismatch)));
    assert!(store.is_empty());
}

#[test]
fn tlcx11_native_parser_capacity_is_admitted_before_dispatch() {
    let context = context();
    let lease = lease(context, 512 * 1024);
    let view = lease.view();
    let mut store = OwnedPayloadStore::with_resources(1, &view).unwrap();
    let query = query();
    let result = execute_native_model(
        &preflight_registry(),
        &query,
        context,
        &budget(),
        &view,
        &std::env::current_dir().unwrap(),
        &mut store,
        SExprLimits::default(),
    );
    assert!(matches!(result, Err(NativeModelError::Resource(_))));
    assert!(store.is_empty());
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
        caps,
    )
    .unwrap()])
    .unwrap()
}

#[test]
#[ignore = "opt-in pinned native Z3 typed model execution"]
fn tlcx11_native_model_is_checked_and_owned_after_store_drop() {
    let context = context().reconstructed([7; 32]);
    let lease = lease(context, 128 * 1024 * 1024);
    let view = lease.view();
    let query = query();
    let checked = {
        let mut store = OwnedPayloadStore::with_resources(1, &view).unwrap();
        let result = execute_native_model(
            &registry(),
            &query,
            context,
            &budget(),
            &view,
            &std::env::current_dir().unwrap(),
            &mut store,
            SExprLimits::checked(65_536, 4096, 64).unwrap(),
        );
        assert!(result.is_ok());
        assert_eq!(store.len(), 1);
        result.ok().unwrap()
    };
    assert_eq!(checked.context(), context);
    assert_eq!(checked.model().integer(IntSymbolId::new(0)), Some(-3));
    assert!(checked.observation().stdout().starts_with(b"sat"));
    assert!(checked.observation().invocation().is_some());
    assert!(lease.close_confirmed().is_err());
    drop(checked);
    lease.close_confirmed().unwrap();
}

#[test]
#[ignore = "opt-in pinned native Z3 retained semantic-parser rejection"]
fn tlcx11_native_model_rejection_retains_exact_owned_stdout() {
    let context = context();
    let lease = lease(context, 128 * 1024 * 1024);
    let view = lease.view();
    let mut store = OwnedPayloadStore::with_resources(1, &view).unwrap();
    let query = query();
    let result = execute_native_model(
        &registry(),
        &query,
        context,
        &budget(),
        &view,
        &std::env::current_dir().unwrap(),
        &mut store,
        SExprLimits::checked(65_536, 4, 4).unwrap(),
    );
    let Err(NativeModelError::Rejected { observation, .. }) = result else {
        panic!("bounded parser must reject")
    };
    assert_eq!(store.len(), 1);
    assert!(observation.stdout().starts_with(b"sat"));
    assert!(observation
        .payload()
        .unwrap()
        .windows(10)
        .any(|window| window == b"define-fun"));
}
