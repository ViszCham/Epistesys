//! Actual native capture and actual local write; cancellation is injected only
//! after the synced write has returned. This is not an external-host event.
use super::{persist_with_after_open, persist_with_after_write};
use crate::canonical_solve::{
    replay::{
        capture_current, with_replay_input, ReplayError, ReplayGoal, ReplayPostWriteCause,
        ReplayProfile,
    },
    solve_with_services, BackendId, CanonicalServices, DecisionBudget, ExecutionBudget, GpuPolicy,
    LogicProfile, SExprLimits, SolverProtocol, SolverRegistration, SolverRegistry,
    SolverResourceCaps,
};
use crate::resource::{
    Cancellation, Deadline, ResourceAmount, ResourceBroker, ResourceKind, ResourceLimits,
    ResourceRequest,
};
use lc631_core::context::SourceRevision;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[test]
#[ignore = "actual pinned Z3 and deterministic after-write cancellation"]
fn tlcx95_after_write_cancel_preserves_frame_and_may_have_applied_cause() {
    late_cancel(false);
}
#[test]
#[ignore = "actual pinned Z3 and deterministic after-open cancellation"]
fn tlcx95_after_open_cancel_preserves_coordination_and_may_have_applied_cause() {
    late_cancel(true);
}
fn late_cancel(after_open: bool) {
    let source = "(set-logic QF_LIA)\n(declare-const x Int)\n(assert (= x (- 3)))\n(check-sat)\n";
    let profile = ReplayProfile::checked(ReplayGoal::NoGoal, false, false).unwrap();
    let root = std::env::temp_dir().join(format!(
        "epistesys-tlcx95-after-write-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    assert!(!root.exists());
    let executable =
        std::env::var("EPISTESYS_TLCX10_NATIVE_Z3").expect("actual native Z3 is required");
    let pin = if cfg!(windows) {
        "5f32b2d2b93291b56e3c10a43b088cae78acc84017ebff8fa741a7ecee6c6ca5"
    } else if cfg!(target_os = "linux") {
        "c45e787b5d1a3d94079d1e83b98e23bbc884ccaf542ea38c3814f31ac2f79197"
    } else {
        panic!("native profile is unavailable")
    };
    let mut hash = [0; 32];
    for (index, byte) in hash.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&pin[index * 2..index * 2 + 2], 16).unwrap();
    }
    let entry = SolverRegistration::checked(
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
    .unwrap();
    let registry = SolverRegistry::checked(vec![entry]).unwrap();
    let limits = ResourceLimits::uniform(256 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let cancellation = Cancellation::default();
    let mut run = broker
        .begin_run(
            95,
            SourceRevision::of(source).digest(),
            limits,
            Deadline::after(Duration::from_secs(60)).unwrap(),
            cancellation.clone(),
        )
        .unwrap();
    let request = |kind, n| ResourceRequest::new(kind, ResourceAmount::checked(n).unwrap());
    let mut reservation = run
        .reserve(&[
            request(ResourceKind::Ram, 96 * 1024 * 1024),
            request(ResourceKind::Output, 16 * 1024 * 1024),
            request(ResourceKind::Disk, 1024 * 1024),
            request(ResourceKind::Child, 1),
            request(ResourceKind::Cleanup, 1024 * 1024),
        ])
        .unwrap();
    {
        let view = reservation.lease();
        let cwd = std::env::current_dir().unwrap();
        with_replay_input(source, profile, &view, |request| {
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
            let captured = capture_current(&decision, source, profile, &broker, &view).unwrap();
            let result = if after_open {
                persist_with_after_open(&root, &captured, &view, cancellation.clone())
            } else {
                persist_with_after_write(&root, &captured, &view, cancellation.clone())
            };
            let file = root.join(super::super::FILE);
            if after_open {
                assert!(root.is_dir());
                assert!(root.join(".resource-owner-v1.lock").is_file());
                assert!(!file.exists());
                eprintln!(
                    "after-open result={result:?}; root/lock exist, segment absent; retained={}",
                    root.display()
                );
                assert!(matches!(
                    result,
                    Err(ReplayError::PostOpenMayHaveApplied(
                        ReplayPostWriteCause::Resource(
                            lc631_substrate::resource::ResourceError::Cancelled
                        )
                    ))
                ));
            } else {
                let bytes =
                    std::fs::read(&file).expect("physical frame survives late cancellation");
                lc631_substrate::codec::decode_envelope_borrowed(&bytes).unwrap();
                eprintln!(
                    "after-write result={result:?}; physical_bytes={}; retained={}",
                    bytes.len(),
                    file.display()
                );
                assert!(matches!(
                    result,
                    Err(ReplayError::PostWriteMayHaveApplied(
                        ReplayPostWriteCause::Resource(
                            lc631_substrate::resource::ResourceError::Cancelled
                        )
                    ))
                ));
            }
            Ok(())
        })
        .unwrap();
    }
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}
