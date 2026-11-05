//! Test-owned real canonical archive. No reported/native owner is deserialized.
use super::native;
use lc631_core::context::SourceRevision;
use lc631_process_supervisor::NativeDispatchTrace;
use lc631_runtime::{
    canonical_solve::{
        replay::{capture_current, persist_current, with_replay_input, ReplayGoal, ReplayProfile},
        solve_with_services, CanonicalServices, DecisionBudget, GpuPolicy, NativeEvidenceKind,
        SExprLimits,
    },
    resource::{
        Cancellation, Deadline, Reservation, ResourceAmount, ResourceBroker, ResourceKind,
        ResourceLimits, ResourceRequest, ResourceReservationView, RunResources,
    },
};
use lc631_solvers::registry::SolverRegistry;
use std::{
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub fn profile() -> ReplayProfile {
    ReplayProfile::checked(ReplayGoal::NoGoal, false, false).unwrap()
}

pub fn root(label: &str) -> PathBuf {
    std::env::temp_dir().canonicalize().unwrap().join(format!(
        "epistesys-tlcx95-dispatch-{label}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

pub struct Session {
    pub broker: ResourceBroker,
    _run: RunResources,
    pub reservation: Reservation,
    pub cancellation: Cancellation,
}
impl Session {
    pub fn new(duration: Duration) -> Self {
        let limits = ResourceLimits::uniform(256 * 1024 * 1024).unwrap();
        let broker = ResourceBroker::new(limits).unwrap();
        let cancellation = Cancellation::default();
        let run = broker
            .begin_run(
                1,
                SourceRevision::of(native::SAT).digest(),
                limits,
                Deadline::after(duration).unwrap(),
                cancellation.clone(),
            )
            .unwrap();
        let requests = [
            (ResourceKind::Ram, 96 * 1024 * 1024),
            (ResourceKind::Output, 16 * 1024 * 1024),
            (ResourceKind::Disk, 1024 * 1024),
            (ResourceKind::Child, 1),
            (ResourceKind::Cleanup, 1024 * 1024),
        ]
        .map(|(kind, n)| ResourceRequest::new(kind, ResourceAmount::checked(n).unwrap()));
        let reservation = run.reserve(&requests).unwrap();
        Self {
            broker,
            _run: run,
            reservation,
            cancellation,
        }
    }
    pub fn services<'v, 'lease>(
        &'v self,
        view: &'v ResourceReservationView<'lease>,
        registry: Option<&'v SolverRegistry>,
        cwd: &'v Path,
    ) -> CanonicalServices<'v, 'lease> {
        CanonicalServices {
            registry,
            broker: &self.broker,
            current: view,
            execution: native::execution(),
            parser: SExprLimits::checked(65536, 4096, 64).unwrap(),
            working_directory: cwd,
            budget: DecisionBudget::checked(65536).unwrap(),
            optional_gpu: GpuPolicy::NotRequested,
        }
    }
}

pub fn assert_zero<R>(trace: &NativeDispatchTrace<R>) {
    assert!(trace.is_complete(), "unknown/overflow is not zero launches");
    assert!(
        trace.dispatches().is_empty(),
        "preadmission unexpectedly launched a root process"
    );
}
pub fn assert_one<R>(trace: &NativeDispatchTrace<R>) {
    assert!(
        trace.is_complete(),
        "unknown/overflow is not exactly one launch"
    );
    assert_eq!(trace.dispatches().len(), 1);
    assert_ne!(trace.dispatches()[0].process_id(), 0);
    eprintln!(
        "actual supervisor native root pid={}",
        trace.dispatches()[0].process_id()
    );
}

/// Setup is separately observed, not included in the operation's count.
/// The exact registered real Z3 path/digest is checked by native_registry().
pub fn save(root: &Path, registry: &SolverRegistry) {
    let session = Session::new(Duration::from_secs(60));
    let view = session.reservation.lease();
    let cwd = std::env::current_dir().unwrap();
    let trace = lc631_process_supervisor::observe_native_dispatches(|| {
        with_replay_input(native::SAT, profile(), &view, |request| {
            let decision =
                solve_with_services(request, session.services(&view, Some(registry), &cwd))
                    .unwrap();
            assert_eq!(decision.native_kind(), NativeEvidenceKind::ModelRechecked);
            let capture =
                capture_current(&decision, native::SAT, profile(), &session.broker, &view)?;
            persist_current(root, &capture, &view)
        })
    })
    .unwrap();
    assert_one(&trace);
    trace.into_result().unwrap();
    eprintln!("test-owned canonical archive retained: {}", root.display());
}
