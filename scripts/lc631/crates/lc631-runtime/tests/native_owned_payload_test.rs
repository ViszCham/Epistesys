//! Explicit native owned-payload profile with Windows/Linux byte pins.
//! Set EPISTESYS_TLCX10_NATIVE_Z3 to the authorized pinned executable and run:
//! `cargo test -p lc631-runtime --test native_owned_payload_test tlcx10_native_owned_ -- --ignored --test-threads=1`
//! An absent path fails when selected; newline bytes are never normalized.
//! No proof-symbol identity, semantic validation, authenticated
//! receipt, OS-wide RAM bound or filesystem sandbox is claimed.

use lc631_core::context::SourceRevision;
use lc631_runtime::resource::{
    Cancellation, Deadline, ResourceAmount, ResourceBroker, ResourceError, ResourceKind,
    ResourceLimits, ResourceRequest,
};
use lc631_solvers::{
    protocol::{
        payload::PayloadShape, payload_wire::PayloadWireStatus, QueryResultKind, SolverProtocol,
        ValidationBasis,
    },
    registry::{BackendId, LogicProfile, SolverRegistration, SolverRegistry, SolverResourceCaps},
    result_store::SolverResultBinding,
    runner::{
        ExecutionBudget, ProcessCleanupState, ProcessRunState, QuerySourceIdentity,
        SolverExecutionState, SolverQuery,
    },
    supervisor_adapter::{
        execute_supervised_owned_payload_into_store, hash_registered_executable, OwnedPayloadError,
        OwnedPayloadStore,
    },
};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::time::Duration;

const WINDOWS_PIN: [u8; 32] = [
    0x5f, 0x32, 0xb2, 0xd2, 0xb9, 0x32, 0x91, 0xb5, 0x6e, 0x3c, 0x10, 0xa4, 0x3b, 0x08, 0x8c, 0xae,
    0x78, 0xac, 0xc8, 0x40, 0x17, 0xeb, 0xff, 0x8f, 0xa7, 0x41, 0xa7, 0xec, 0xee, 0x6c, 0x6c, 0xa5,
];
const LINUX_PIN: [u8; 32] = [
    0xc4, 0x5e, 0x78, 0x7b, 0x5d, 0x1a, 0x3d, 0x94, 0x07, 0x9d, 0x1e, 0x83, 0xb9, 0x8e, 0x23, 0xbb,
    0xc8, 0x84, 0xcc, 0xaf, 0x54, 0x2e, 0xa3, 0x8c, 0x38, 0x14, 0xf3, 0x1a, 0xc2, 0xf7, 0x91, 0x97,
];
const MODEL_IN: &str = "(set-option :produce-models true)\n(set-logic QF_LIA)\n(declare-const x Int)\n(assert (= x 2))\n(check-sat)\n(get-model)\n(exit)\n";
const PROOF_IN: &str = "(set-option :produce-proofs true)\n(set-logic QF_LIA)\n(declare-const p Bool)\n(assert p)\n(assert (not p))\n(check-sat)\n(get-proof)\n(exit)\n";
const NO_PROOF_IN: &str = "(set-logic QF_LIA)\n(declare-const p Bool)\n(assert p)\n(assert (not p))\n(check-sat)\n(get-proof)\n(exit)\n";
const RAM: u64 = 128 * 1024 * 1024;
const OUTPUT: u64 = 2 * 1024 * 1024;

fn platform_pin() -> [u8; 32] {
    match std::env::consts::OS {
        "windows" | "linux" => (),
        platform => panic!("native profile does not support {platform}"),
    }
    if cfg!(windows) {
        WINDOWS_PIN
    } else {
        LINUX_PIN
    }
}

// Reconstruct the published witness preimage independently from the owner
// bytes. Report's witness was produced from runner stdout before the copy.
fn stdout_witness(raw: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"epistesys.solver-output-witness.v1\0");
    hash.update(6_u64.to_le_bytes());
    hash.update(b"stdout");
    hash.update((raw.len() as u64).to_le_bytes());
    hash.update((raw.len() as u64).to_le_bytes());
    hash.update([0]); // no truncation
    hash.update(raw);
    hash.finalize().into()
}

fn wire_digest(raw: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"epistesys.tlcx.smt-payload-wire.v2\0");
    hash.update((raw.len() as u64).to_le_bytes());
    hash.update(raw);
    hash.finalize().into()
}

fn caps() -> SolverResourceCaps {
    SolverResourceCaps {
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
    }
}

fn request(kind: ResourceKind, amount: u64) -> ResourceRequest {
    ResourceRequest::new(kind, ResourceAmount::checked(amount).unwrap())
}

fn native_case(
    input: &'static str,
    kind: QueryResultKind,
    status: PayloadWireStatus,
    shape: PayloadShape,
) {
    let pin = platform_pin();
    let executable = PathBuf::from(
        std::env::var_os("EPISTESYS_TLCX10_NATIVE_Z3")
            .expect("EPISTESYS_TLCX10_NATIVE_Z3 is required"),
    );
    assert!(executable.is_absolute());
    let source = SourceRevision::of(input).digest();
    let query = SolverQuery::checked(
        BackendId::Z3,
        QuerySourceIdentity::from_bytes(*source.as_bytes()),
        LogicProfile::SmtBooleanQfLia,
        kind,
        input.as_bytes(),
    )
    .unwrap();
    let limits = ResourceLimits::uniform(256 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let mut run = broker
        .begin_run(
            1,
            source,
            limits,
            Deadline::after(Duration::from_secs(30)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let mut reservation = run
        .reserve(&[
            request(ResourceKind::Ram, RAM),
            request(ResourceKind::Output, OUTPUT),
            request(ResourceKind::Child, 1),
            request(ResourceKind::Cleanup, 1024 * 1024),
        ])
        .unwrap();
    {
        let view = reservation.lease();
        assert_eq!(hash_registered_executable(&view, &executable).unwrap(), pin);
        let reg = SolverRegistration::checked(
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
            caps(),
        )
        .unwrap();
        let registry = SolverRegistry::checked(vec![reg]).unwrap();
        let budget = ExecutionBudget::checked(
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
        .unwrap();
        let cwd = std::env::current_dir().unwrap();
        let (owned, snapshot, snapshot_claim, before_store_drop) = {
            // The resource-bound metadata store dies before the stdout owner.
            let mut store = OwnedPayloadStore::with_resources(1, &view).unwrap();
            let observation = execute_supervised_owned_payload_into_store(
                &registry, &query, &budget, &view, &cwd, &mut store,
            )
            .unwrap();
            let report = observation.report();
            let stored = store
                .lookup(SolverResultBinding::for_query(
                    &query,
                    report.invocation_digest(),
                ))
                .unwrap();
            assert_eq!(stored.id(), observation.insertion().id());
            assert_eq!(stored.report(), report);
            assert_eq!(stored.binding().source(), query.source());
            assert_eq!(stored.binding().query(), query.digest());
            assert_eq!(stored.binding().invocation(), report.invocation_digest());
            assert!(report.invocation_digest().is_some());
            let count = observation.stdout().len();
            let snapshot_claim = view
                .claim_resources(&[
                    request(ResourceKind::Ram, u64::try_from(count).unwrap()),
                    request(ResourceKind::Output, u64::try_from(count).unwrap()),
                ])
                .unwrap();
            let mut snapshot = Vec::new();
            snapshot.try_reserve_exact(count).unwrap();
            assert!(
                snapshot.capacity() <= count,
                "snapshot exceeds its claimed capacity"
            );
            snapshot.extend_from_slice(observation.stdout());
            let before = (
                snapshot.len(),
                stdout_witness(&snapshot),
                wire_digest(&snapshot),
            );
            // Report witness was calculated on runner output before its copy.
            assert_eq!(report.stdout_digest(), Some(before.1));
            assert_eq!(
                report.payload_wire_summary().unwrap().stdout_digest(),
                before.2
            );
            (observation, snapshot, snapshot_claim, before)
        };
        // The function's runner and metadata store have both gone away here.
        assert_eq!(owned.stdout(), snapshot.as_slice());
        assert_eq!(owned.stdout().len(), before_store_drop.0);
        assert_eq!(stdout_witness(owned.stdout()), before_store_drop.1);
        assert_eq!(wire_digest(owned.stdout()), before_store_drop.2);
        let report = owned.report();
        assert_eq!(report.process_state(), Some(ProcessRunState::Completed));
        assert_eq!(report.exit_code(), Some(0));
        assert_eq!(
            report.cleanup_state(),
            ProcessCleanupState::RunnerReportedVerified
        );
        assert_eq!(report.state(), SolverExecutionState::PayloadWireObserved);
        assert_eq!(
            report.validation_basis(),
            Some(ValidationBasis::SolverReported)
        );
        assert_eq!(report.logical_result(), None);
        assert!(
            !report.proof_checked()
                && !report.authority_created()
                && !report.output_commit_allowed()
        );
        let summary = report.payload_wire_summary().unwrap();
        assert_eq!(summary.status(), status);
        assert_eq!(summary.shape(), shape);
        assert_eq!(summary.stdout_digest(), before_store_drop.2);
        let raw = owned.stdout();
        let status_start = summary.status_span().start() as usize;
        let status_end = summary.status_span().end() as usize;
        let payload_start = summary.payload_span().start() as usize;
        let payload_end = summary.payload_span().end() as usize;
        let status_bytes: &[u8] = match status {
            PayloadWireStatus::Sat => b"sat",
            PayloadWireStatus::Unsat => b"unsat",
            PayloadWireStatus::Unknown => panic!("positive case cannot be unknown"),
        };
        assert_eq!(status_start, 0);
        assert_eq!(raw.get(status_start..status_end), Some(status_bytes));
        let gap = raw
            .get(status_end..payload_start)
            .expect("status/payload gap in stdout");
        assert!(gap.starts_with(b"\n") || gap.starts_with(b"\r\n"));
        assert!(gap.iter().all(u8::is_ascii_whitespace));
        let payload = raw
            .get(payload_start..payload_end)
            .expect("payload span in raw stdout");
        assert_eq!(owned.payload(), Some(payload));
        assert_eq!(payload.first(), Some(&b'('));
        assert_eq!(payload.last(), Some(&b')'));
        assert!(raw[payload_end..].iter().all(u8::is_ascii_whitespace));

        // Release the separately-accounted test snapshot before checking
        // whether the owner alone retains the output and RAM charges.
        drop(snapshot);
        drop(snapshot_claim);
        // The test claims each full quota only after the store is dropped.
        // A live owner must retain a nonzero RAM and Output charge.
        assert!(matches!(
            view.claim_resources(&[request(ResourceKind::Ram, RAM)]),
            Err(ResourceError::Exhausted(ResourceKind::Ram))
        ));
        assert!(matches!(
            view.claim_resources(&[request(ResourceKind::Output, OUTPUT)]),
            Err(ResourceError::Exhausted(ResourceKind::Output))
        ));
        drop(owned);
        let ram = view
            .claim_resources(&[request(ResourceKind::Ram, RAM)])
            .unwrap();
        drop(ram);
        let output = view
            .claim_resources(&[request(ResourceKind::Output, OUTPUT)])
            .unwrap();
        drop(output);
    }
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}

#[test]
#[ignore = "opt-in pinned native Z3 owned model"]
fn tlcx10_native_owned_model_survives_runner_and_store_drop() {
    native_case(
        MODEL_IN,
        QueryResultKind::ModelPayload,
        PayloadWireStatus::Sat,
        PayloadShape::Model,
    );
}

#[test]
#[ignore = "opt-in pinned native Z3 owned proof; syntax only"]
fn tlcx10_native_owned_proof_survives_runner_and_store_drop() {
    native_case(
        PROOF_IN,
        QueryResultKind::ProofPayload,
        PayloadWireStatus::Unsat,
        PayloadShape::Proof,
    );
}

#[test]
#[ignore = "opt-in pinned native Z3 proof-disabled report"]
fn tlcx10_native_owned_proof_disabled_preserves_report_without_store() {
    let executable =
        PathBuf::from(std::env::var_os("EPISTESYS_TLCX10_NATIVE_Z3").expect("path required"));
    let pin = platform_pin();
    assert!(executable.is_absolute());
    let source = SourceRevision::of(NO_PROOF_IN).digest();
    let query = SolverQuery::checked(
        BackendId::Z3,
        QuerySourceIdentity::from_bytes(*source.as_bytes()),
        LogicProfile::SmtBooleanQfLia,
        QueryResultKind::ProofPayload,
        NO_PROOF_IN.as_bytes(),
    )
    .unwrap();
    let limits = ResourceLimits::uniform(256 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let mut run = broker
        .begin_run(
            1,
            source,
            limits,
            Deadline::after(Duration::from_secs(30)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let mut reservation = run
        .reserve(&[
            request(ResourceKind::Ram, RAM),
            request(ResourceKind::Output, OUTPUT),
            request(ResourceKind::Child, 1),
            request(ResourceKind::Cleanup, 1024 * 1024),
        ])
        .unwrap();
    {
        let view = reservation.lease();
        assert_eq!(hash_registered_executable(&view, &executable).unwrap(), pin);
        let reg = SolverRegistration::checked(
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
            caps(),
        )
        .unwrap();
        let registry = SolverRegistry::checked(vec![reg]).unwrap();
        let budget = ExecutionBudget::checked(
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
        .unwrap();
        let mut store = OwnedPayloadStore::with_resources(1, &view).unwrap();
        let result = execute_supervised_owned_payload_into_store(
            &registry,
            &query,
            &budget,
            &view,
            &std::env::current_dir().unwrap(),
            &mut store,
        );
        let report = match result {
            Err(OwnedPayloadError::Report(report)) => report,
            _ => panic!("failed proof run must return its terminal report"),
        };
        assert_eq!(report.state(), SolverExecutionState::Failed);
        assert_eq!(report.exit_code(), Some(1));
        assert_eq!(report.payload_wire_status(), Some(PayloadWireStatus::Unsat));
        assert_eq!(report.payload_wire_summary(), None);
        assert_eq!(report.logical_result(), None);
        assert_eq!(report.validation_basis(), None);
        assert!(!report.proof_checked() && !report.authority_created());
        assert_eq!(store.len(), 0);
    }
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}
