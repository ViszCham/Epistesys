//! AuthorityRevision is context metadata here, never an execution Grant.
use super::{capture_current, ReplayError, ReplayGoal, ReplayProfile};
use crate::{
    canonical_solve::{
        solve_with_services, BackendId, CanonicalRequest, CanonicalServices, DecisionBudget,
        ExecutionBudget, GpuPolicy, LogicProfile, SExprLimits, SolverProtocol, SolverRegistration,
        SolverRegistry, SolverResourceCaps, SourceSpan, SourceStore,
    },
    explicit_formal::{lower_explicit_formal, FormalLowering},
    resource::{
        Cancellation, Deadline, ResourceAmount, ResourceBroker, ResourceKind, ResourceLimits,
        ResourceRequest,
    },
};
use lc631_core::{
    context::{ContextLimits, ContextPremise, OriginalContext, PremiseBasis, SourceRevision},
    AuthorityRevision,
};
use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
const SOURCE: &str =
    "(set-logic QF_LIA)\n(declare-const x Int)\n(assert (= x (- 3)))\n(check-sat)\n";

fn registry() -> SolverRegistry {
    let executable = std::env::var("EPISTESYS_TLCX10_NATIVE_Z3").expect("actual Z3 required");
    let pin = if cfg!(windows) {
        "5f32b2d2b93291b56e3c10a43b088cae78acc84017ebff8fa741a7ecee6c6ca5"
    } else if cfg!(target_os = "linux") {
        "c45e787b5d1a3d94079d1e83b98e23bbc884ccaf542ea38c3814f31ac2f79197"
    } else {
        panic!("no selected native profile")
    };
    let mut hash = [0; 32];
    for (i, b) in hash.iter_mut().enumerate() {
        *b = u8::from_str_radix(&pin[i * 2..i * 2 + 2], 16).unwrap();
    }
    SolverRegistry::checked(vec![SolverRegistration::checked(
        BackendId::Z3,
        LogicProfile::SmtBooleanQfLia,
        SolverProtocol::SmtLibPayloadWireV2,
        &executable,
        "5.3.2",
        hash,
        vec!["-in".into(), "-smt2".into()],
        SolverResourceCaps {
            max_argv_count: 16,
            max_argv_bytes: 4096,
            max_stdin_bytes: 16384,
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
fn context(
    store: &SourceStore,
    profile: ReplayProfile,
    authority: u64,
    basis: PremiseBasis,
) -> OriginalContext {
    let premise = ContextPremise::from_source(
        SourceRevision::of("epistesys.cli.formal.v1\0whole-given-source").digest(),
        store,
        SourceSpan::checked(store, 0, SOURCE.len()).unwrap(),
        basis,
        false,
    )
    .unwrap();
    OriginalContext::checked(
        SourceRevision::of(SOURCE),
        profile.scope(SOURCE).unwrap(),
        AuthorityRevision(authority),
        vec![premise],
        ContextLimits {
            max_contexts: 1,
            max_premises: 1,
            max_edits: 1,
            max_statement_bytes: 16384,
        },
    )
    .unwrap()
}

#[test]
#[ignore = "actual Z3: all-Given custom AuthorityRevision is reachable but not fixed-profile resumable"]
fn tlcx95_custom_authority_context_capture_rejects_before_write() {
    let registry = registry();
    let profile = ReplayProfile::checked(ReplayGoal::NoGoal, false, false).unwrap();
    let limits = ResourceLimits::uniform(256 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let mut run = broker
        .begin_run(
            95,
            SourceRevision::of(SOURCE).digest(),
            limits,
            Deadline::after(Duration::from_secs(60)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let request = |kind, n| ResourceRequest::new(kind, ResourceAmount::checked(n).unwrap());
    let mut reservation = run
        .reserve(&[
            request(ResourceKind::Ram, 96 * 1024 * 1024),
            request(ResourceKind::Output, 16 * 1024 * 1024),
            request(ResourceKind::Child, 1),
            request(ResourceKind::Cleanup, 1024 * 1024),
        ])
        .unwrap();
    {
        let view = reservation.lease();
        let _metadata = view
            .claim_resources(&[request(ResourceKind::Ram, 1024 * 1024)])
            .unwrap();
        let source = SourceStore::new(Arc::from(SOURCE), 16384).unwrap();
        let baseline = context(&source, profile, 0, PremiseBasis::Given);
        let custom = context(&source, profile, 1, PremiseBasis::Given);
        assert_ne!(baseline.state().revision(), custom.state().revision());
        let cwd = std::env::current_dir().unwrap();
        for (context, supported) in [(&baseline, true), (&custom, false)] {
            let lowered = lower_explicit_formal(
                &source,
                context.state().revision(),
                context.state().scope(),
                &view,
            )
            .unwrap();
            let FormalLowering::Checked(problem) = lowered else {
                panic!("reachable checked fragment")
            };
            let request =
                CanonicalRequest::formal(&problem, context.state(), profile.goal_policy()).unwrap();
            let decision = solve_with_services(
                request,
                CanonicalServices {
                    registry: Some(&registry),
                    broker: &broker,
                    current: &view,
                    execution: ExecutionBudget::checked(
                        5000,
                        16384,
                        65536,
                        16384,
                        16,
                        4096,
                        64 * 1024 * 1024,
                        0,
                        65536,
                    )
                    .unwrap(),
                    parser: SExprLimits::checked(65536, 4096, 64).unwrap(),
                    working_directory: &cwd,
                    budget: DecisionBudget::checked(65536).unwrap(),
                    optional_gpu: GpuPolicy::NotRequested,
                },
            )
            .unwrap();
            assert!(decision.native_model().is_some());
            let root = std::env::temp_dir().join(format!(
                "epistesys-tlcx95-context-{}-{}-{}",
                std::process::id(),
                supported,
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            let capture = capture_current(&decision, SOURCE, profile, &broker, &view);
            eprintln!(
                "authority_metadata_supported={supported}; capture_error={:?}; root_absent={}",
                capture.as_ref().err(),
                !root.exists()
            );
            if supported {
                assert!(capture.is_ok());
            } else {
                assert!(matches!(capture, Err(ReplayError::Binding)));
            }
            assert!(!root.exists());
        }
    }
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}
