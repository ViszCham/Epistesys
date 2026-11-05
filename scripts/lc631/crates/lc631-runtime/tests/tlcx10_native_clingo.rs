//! Explicit native Clingo text-positive and JSON-rejection profile.
//!
//! Placement after review: `lc631-runtime/tests/tlcx10_native_clingo.rs`.
//! From the `scripts/lc631` workspace, set the three environment variables
//! below to authorized absolute file paths and select the two ignored tests:
//! `cargo test -p lc631-runtime --test tlcx10_native_clingo tlcx10_native_clingo_ -- --ignored --test-threads=1`
//! Missing variables or invalid pins are failures when selected; ignoring this
//! test is not a backend observation. No host path is embedded in this file.
//!
//! Current `ClingoStatusV1` accepts text, not JSON. The second case deliberately
//! sends JSON to this text protocol and expects a rejected result, not SAT.
//! JSON-positive support requires a separate explicit protocol and parser.
//! Compilation also requires the companion additive ResultStore resource
//! patch exposing `try_reserve_backing`; no unreserved fallback is intended.
//! Parent-supplied file pins are not process authentication, and hashing
//! caller-supplied package files does not prove which module was imported.

use std::path::{Path, PathBuf};
use std::time::Duration;

use lc631_core::context::SourceRevision;
use lc631_runtime::resource::{
    Cancellation, Deadline, ResourceAmount, ResourceBroker, ResourceKind, ResourceLimits,
    ResourceRequest, ResourceReservationView,
};
use lc631_solvers::{
    protocol::{QueryResultKind, SolverProtocol, ValidationBasis},
    registry::{BackendId, LogicProfile, SolverRegistration, SolverRegistry, SolverResourceCaps},
    result_store::{ResultStore, SolverResultBinding, StoredSolverResult},
    runner::{
        ExecutionBudget, ProcessCleanupState, ProcessRunState, QuerySourceIdentity,
        SolverExecutionReport, SolverExecutionState, SolverQuery,
    },
    supervisor_adapter::{execute_supervised_into_store, hash_registered_executable},
    LogicalResult, Satisfiability,
};

const PYTHON_SHA256: [u8; 32] = if cfg!(windows) {
    [
        0x0b, 0x47, 0x11, 0x33, 0xe1, 0x10, 0xcf, 0xb5, 0x3a, 0x06, 0x1c, 0xad, 0x52, 0x8c, 0xe8,
        0xe5, 0x17, 0xd7, 0xb9, 0xac, 0x41, 0xa0, 0xa3, 0x96, 0xc3, 0x9a, 0xd7, 0x95, 0xa4, 0x87,
        0xfc, 0x14,
    ]
} else {
    [
        0xe5, 0x0d, 0x46, 0x8e, 0x8b, 0x0a, 0xdf, 0xb0, 0x57, 0x33, 0xf5, 0xb8, 0x7b, 0x3c, 0xff,
        0x34, 0x82, 0x9c, 0x4a, 0x8c, 0x1a, 0xea, 0x50, 0xc8, 0x65, 0xaa, 0x8b, 0xdf, 0xe4, 0xbb,
        0x15, 0x0f,
    ]
};
// The legacy PYD variable supplies .pyd on Windows and .so on Linux.
const PYD_SHA256: [u8; 32] = if cfg!(windows) {
    [
        0xde, 0xc5, 0x3c, 0xed, 0x54, 0xfd, 0x12, 0x3d, 0xc5, 0x38, 0xdf, 0x12, 0xff, 0xda, 0x3c,
        0x03, 0x1a, 0x69, 0x06, 0xff, 0xd7, 0xcf, 0xc7, 0x61, 0x60, 0xe3, 0x23, 0xb1, 0x23, 0x07,
        0x6f, 0xd7,
    ]
} else {
    [
        0x39, 0xab, 0x82, 0xbd, 0xaf, 0x2d, 0x38, 0x2a, 0x59, 0x4b, 0xbd, 0x70, 0x75, 0x52, 0xac,
        0xfa, 0x20, 0x16, 0x51, 0xe8, 0xd8, 0x18, 0xa1, 0x74, 0x1f, 0x77, 0x0b, 0xaa, 0x2c, 0x41,
        0x87, 0xdd,
    ]
};
const MAIN_SHA256: [u8; 32] = if cfg!(windows) {
    [
        0x91, 0x27, 0x0e, 0x42, 0xb4, 0x01, 0x87, 0x8a, 0x41, 0x4e, 0x94, 0x7d, 0x4a, 0x16, 0xf5,
        0xa7, 0xf6, 0x3d, 0xb5, 0xb9, 0xb2, 0xcd, 0xac, 0xfc, 0xdf, 0x19, 0xda, 0x91, 0x11, 0x4f,
        0x14, 0x41,
    ]
} else {
    [
        0xce, 0x91, 0x35, 0xfa, 0x69, 0xdf, 0x02, 0xf8, 0x22, 0xd9, 0xe7, 0xff, 0x40, 0x79, 0xa5,
        0xe2, 0x7a, 0xcf, 0xdb, 0x77, 0x4a, 0xb6, 0xe9, 0x43, 0xbe, 0x60, 0xff, 0x2e, 0xb5, 0x9a,
        0x20, 0xc5,
    ]
};
const SOURCE: &str = "p.";

fn required_path(name: &str) -> PathBuf {
    let value = std::env::var_os(name)
        .unwrap_or_else(|| panic!("{name} must be set for the opted-in profile"));
    let path = PathBuf::from(value);
    assert!(path.is_absolute(), "{name} must be an absolute path");
    path
}

fn check_pin(lease: &ResourceReservationView<'_>, path: &Path, expected: [u8; 32], label: &str) {
    let actual = hash_registered_executable(lease, path)
        .unwrap_or_else(|error| panic!("{label} bounded hash failed: {error:?}"));
    assert_eq!(actual, expected, "{label} digest mismatch");
}

fn caps() -> SolverResourceCaps {
    SolverResourceCaps {
        max_argv_count: 16,
        max_argv_bytes: 4096,
        max_stdin_bytes: 4096,
        max_stdout_bytes: 65_536,
        max_stderr_bytes: 16_384,
        max_timeout_ms: 10_000,
        max_ram_bytes: 64 * 1024 * 1024,
        max_disk_bytes: 0,
        max_cleanup_bytes: 65_536,
        max_child_count: 1,
    }
}

fn execute_native(json_output: bool) -> SolverExecutionReport {
    match std::env::consts::OS {
        "windows" | "linux" => (),
        platform => panic!("native profile does not support {platform}"),
    }
    let python = required_path("EPISTESYS_TLCX10_CLINGO_PYTHON");
    let pyd = required_path("EPISTESYS_TLCX10_CLINGO_PYD");
    let main = required_path("EPISTESYS_TLCX10_CLINGO_MAIN");
    let python_text = python
        .to_str()
        .expect("Python path must be UTF-8 for the registry");
    let source_revision = SourceRevision::of(SOURCE).digest();
    let source = QuerySourceIdentity::from_bytes(*source_revision.as_bytes());
    let query = SolverQuery::checked(
        BackendId::Clingo,
        source,
        LogicProfile::AspFiniteGroundNormal,
        QueryResultKind::StableModelExistence,
        SOURCE.as_bytes(),
    )
    .unwrap();
    let limits = ResourceLimits::uniform(256 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let mut run = broker
        .begin_run(
            1,
            source_revision,
            limits,
            Deadline::after(Duration::from_secs(60)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let mut reservation = run
        .reserve(&[
            ResourceRequest::new(
                ResourceKind::Ram,
                ResourceAmount::checked(128 * 1024 * 1024).unwrap(),
            ),
            ResourceRequest::new(
                ResourceKind::Output,
                ResourceAmount::checked(2 * 1024 * 1024).unwrap(),
            ),
            ResourceRequest::new(ResourceKind::Child, ResourceAmount::checked(1).unwrap()),
            ResourceRequest::new(
                ResourceKind::Cleanup,
                ResourceAmount::checked(1024 * 1024).unwrap(),
            ),
        ])
        .unwrap();

    let report = {
        let lease = reservation.lease();
        check_pin(&lease, &python, PYTHON_SHA256, "Python executable");
        check_pin(&lease, &pyd, PYD_SHA256, "Clingo extension");
        check_pin(&lease, &main, MAIN_SHA256, "Clingo main module");
        let mut arguments: Vec<String> =
            vec!["-I".into(), "-B".into(), "-m".into(), "clingo".into()];
        if json_output {
            arguments.push("--outf=2".into());
        }
        arguments.push("-".into());
        let registry = SolverRegistry::checked(vec![SolverRegistration::checked(
            BackendId::Clingo,
            LogicProfile::AspFiniteGroundNormal,
            SolverProtocol::ClingoStatusV1,
            python_text,
            "5.8.2",
            PYTHON_SHA256,
            arguments,
            caps(),
        )
        .unwrap()])
        .unwrap();
        let budget = ExecutionBudget::checked(
            10_000,
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
        let cwd = std::env::current_dir().expect("absolute working directory required");
        assert!(cwd.is_absolute());
        let store_bytes = u64::try_from(std::mem::size_of::<StoredSolverResult>()).unwrap();
        let _store_claim = lease
            .claim_resources(&[ResourceRequest::new(
                ResourceKind::Ram,
                ResourceAmount::checked(store_bytes).unwrap(),
            )])
            .expect("result-store RAM claim failed");
        let mut store = ResultStore::with_capacity(1).unwrap();
        store
            .try_reserve_backing()
            .expect("result-store bounded backing reservation failed");
        let receipt =
            execute_supervised_into_store(&registry, &query, &budget, &lease, &cwd, &mut store)
                .expect("supervisor setup failed");
        let report = receipt.report();
        let insertion = receipt
            .insertion()
            .expect("ResultStore did not consume terminal report");
        let stored = store
            .lookup(SolverResultBinding::for_query(
                &query,
                report.invocation_digest(),
            ))
            .expect("the exact process report was not found");
        assert_eq!(stored.id(), insertion.id());
        assert_eq!(stored.report(), report);
        assert_eq!(store.len(), 1);
        report
    };
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();

    // Diagnose process/resource failure separately from protocol behavior.
    eprintln!("native Clingo json={json_output}: state={:?}, process={:?}, exit={:?}, stdout_bytes={}, stderr_bytes={}",
        report.state(), report.process_state(), report.exit_code(), report.stdout_bytes(), report.stderr_bytes());
    assert_eq!(
        report.process_state(),
        Some(ProcessRunState::Completed),
        "process did not complete"
    );
    assert_eq!(
        report.exit_code(),
        Some(0),
        "current adapter requires exit 0 before protocol parsing"
    );
    assert_eq!(
        report.cleanup_state(),
        ProcessCleanupState::RunnerReportedVerified
    );
    assert!(!report.stdout_truncated() && !report.stderr_truncated());
    assert!(report.stdout_fully_observed() && report.stderr_fully_observed());
    assert!(!report.proof_checked());
    assert!(!report.authority_created());
    assert!(!report.output_commit_allowed());
    report
}

#[test]
#[ignore = "opt-in native Clingo text profile; requires three pinned paths"]
fn tlcx10_native_clingo_text_result_store() {
    let report = execute_native(false);
    assert_eq!(report.state(), SolverExecutionState::SolverReported);
    assert_eq!(
        report.logical_result(),
        Some(LogicalResult::Satisfiability(Satisfiability::Sat))
    );
    assert_eq!(
        report.validation_basis(),
        Some(ValidationBasis::SolverReported)
    );
}

#[test]
#[ignore = "opt-in native Clingo JSON rejection profile; requires three pinned paths"]
fn tlcx10_native_clingo_json_rejected_result_store() {
    let report = execute_native(true);
    assert_eq!(report.state(), SolverExecutionState::ProtocolFailure);
    assert_eq!(report.logical_result(), None);
    assert_eq!(report.validation_basis(), None);
    assert_eq!(report.clingo_output_summary(), None);
}
