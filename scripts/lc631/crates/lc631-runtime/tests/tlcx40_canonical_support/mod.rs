//! Shared actual source/context/resource fixtures; native execution is opt-in.
use lc631_core::{
    context::{
        ContextLimits, ContextPremise, ContextScope, OriginalContext, PremiseBasis, SourceRevision,
    },
    AuthorityRevision,
};
use lc631_runtime::{
    canonical_solve::{CanonicalServices, DecisionBudget, GpuPolicy},
    explicit_formal::{lower_explicit_formal, FormalLowering},
    resource::{Reservation, ResourceBroker, ResourceLimits, RunResources},
};
use lc631_solvers::{
    protocol::SolverProtocol,
    registry::{BackendId, LogicProfile, SolverRegistration, SolverRegistry, SolverResourceCaps},
    runner::ExecutionBudget,
};
use lc631_substrate::{
    resource::{
        Cancellation, Deadline, ResourceAmount, ResourceKind, ResourceRequest,
        ResourceReservationView,
    },
    source::{SourceSpan, SourceStore},
    Digest32,
};
use std::{path::PathBuf, sync::Arc, time::Duration};

pub const SAT: &str =
    "(set-logic QF_LIA)\n(declare-const x Int)\n(assert (= x (- 3)))\n(check-sat)\n";
pub const OBJECTIVE: &str = "(set-logic QF_LIA)\n(declare-const x Int)\n(assert (<= 0 x))\n(assert (<= x 10))\n(minimize x)\n(check-sat)\n";
pub const UNSUPPORTED: &str =
    "(set-logic QF_LIA)\n(declare-const x Int)\n(future-command x)\n(check-sat)\n";

pub fn d(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}
fn request(kind: ResourceKind, bytes: u64) -> ResourceRequest {
    ResourceRequest::new(kind, ResourceAmount::checked(bytes).unwrap())
}

/// The structural context is built BEFORE the formal parser so its actual
/// canonical interpretation identity can be passed unchanged through every edge.
pub struct Fixture {
    pub source: SourceStore,
    pub context: OriginalContext,
    pub broker: ResourceBroker,
    _run: RunResources,
    pub reservation: Reservation,
    pub cancellation: Cancellation,
}
impl Fixture {
    pub fn new(text: &str) -> Self {
        Self::with_scope(text, ContextScope::declared(d(1), d(2), d(3), d(4)))
    }

    /// Registered evaluation derives scope before the canonical core context.
    pub fn with_scope(text: &str, scope: ContextScope) -> Self {
        let source = SourceStore::new(Arc::from(text), 16 * 1024).unwrap();
        let revision = SourceRevision::of(text);
        let whole = SourceSpan::checked(&source, 0, text.len()).unwrap();
        let premise =
            ContextPremise::from_source(d(10), &source, whole, PremiseBasis::Given, false).unwrap();
        let context = OriginalContext::checked(
            revision,
            scope,
            AuthorityRevision(1),
            vec![premise],
            ContextLimits {
                max_contexts: 2,
                max_premises: 4,
                max_edits: 2,
                max_statement_bytes: 16 * 1024,
            },
        )
        .unwrap();
        let limits = ResourceLimits::uniform(256 * 1024 * 1024).unwrap();
        let broker = ResourceBroker::new(limits).unwrap();
        let cancellation = Cancellation::default();
        let run = broker
            .begin_run(
                1,
                revision.digest(),
                limits,
                Deadline::after(Duration::from_secs(60)).unwrap(),
                cancellation.clone(),
            )
            .unwrap();
        let reservation = run
            .reserve(&[
                request(ResourceKind::Ram, 128 * 1024 * 1024),
                request(ResourceKind::Output, 2 * 1024 * 1024),
                request(ResourceKind::Child, 1),
                request(ResourceKind::Cleanup, 1024 * 1024),
            ])
            .unwrap();
        Self {
            source,
            context,
            broker,
            _run: run,
            reservation,
            cancellation,
        }
    }
}

pub fn execution() -> ExecutionBudget {
    ExecutionBudget::checked(
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
    .unwrap()
}

pub fn lower<'view>(
    fixture: &Fixture,
    view: &'view ResourceReservationView<'_>,
) -> FormalLowering<'view> {
    lower_explicit_formal(
        &fixture.source,
        fixture.context.state().revision(),
        fixture.context.state().scope(),
        view,
    )
    .unwrap()
}

pub fn services<'view, 'lease>(
    fixture: &'view Fixture,
    view: &'view ResourceReservationView<'lease>,
    registry: Option<&'view SolverRegistry>,
    cwd: &'view std::path::Path,
) -> CanonicalServices<'view, 'lease> {
    CanonicalServices {
        registry,
        broker: &fixture.broker,
        current: view,
        execution: execution(),
        parser: lc631_solvers::protocol::sexpr::SExprLimits::checked(65536, 4096, 64).unwrap(),
        working_directory: cwd,
        budget: DecisionBudget::checked(65536).unwrap(),
        optional_gpu: GpuPolicy::NotRequested,
    }
}

/// Selected opt-in tests fail when the executable is missing. They never
/// silently return success and never substitute the current test executable.
pub fn native_registry() -> SolverRegistry {
    let executable = PathBuf::from(
        std::env::var_os("EPISTESYS_TLCX10_NATIVE_Z3")
            .expect("selected native test requires EPISTESYS_TLCX10_NATIVE_Z3"),
    );
    assert!(executable.is_absolute());
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
        other => panic!("native test has no registered OS pin for {other}"),
    };
    let caps = SolverResourceCaps {
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
    };
    let entry = SolverRegistration::checked(
        BackendId::Z3,
        LogicProfile::SmtBooleanQfLia,
        SolverProtocol::SmtLibPayloadWireV2,
        executable.to_str().unwrap(),
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
    .unwrap();
    SolverRegistry::checked(vec![entry]).unwrap()
}
