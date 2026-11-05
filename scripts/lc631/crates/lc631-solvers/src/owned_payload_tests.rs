//! Synthetic contract tests for owned payloads and their resource boundaries.
//! Native test-only seams remain crate-private; real probes are separate.
use crate::protocol::payload::{parse_payload_syntax, PayloadShape};
use crate::protocol::sexpr::SExprLimits;
use crate::protocol::{
    query_protocol_compatibility, QueryProtocolCompatibility, QueryResultKind, SolverProtocol,
};
use crate::registry::{BackendId, LogicProfile, SolverRegistration, SolverRegistry};
use crate::result_store::{ResultStore, StoredSolverResult};
use crate::runner::{
    execute_registered, execute_registered_native_for_test, LiveResourceReservation,
    ProcessFailure, ProcessRequest, ProcessRunState, RunnerReportedProcessObservation,
    SolverExecutionState, SolverProcessRunner, SolverQuery,
};
use crate::supervisor_adapter::owned_test_hooks;
use crate::supervisor_adapter::{
    execute_supervised_owned_payload_into_store, finish_owned_for_test, OwnedPayloadError,
    OwnedPayloadStore, OwnedPayloadStoreInitError,
};
use lc631_substrate::resource::{
    Cancellation, Deadline, ReportedResourceLease, ResourceAmount, ResourceError, ResourceKind,
    ResourceLeaseIssuer, ResourceRequest,
};
use std::time::Duration;

mod fixture {
    use crate::registry::SolverResourceCaps;
    use crate::runner::{ExecutionBudget, QuerySourceIdentity};
    use lc631_substrate::resource::{
        Cancellation, Deadline, ReportedResourceLease, ResourceAmount, ResourceKind, ResourceLease,
        ResourceLeaseIssuer, ResourceRequest,
    };
    use std::time::Duration;

    pub(super) fn source() -> QuerySourceIdentity {
        QuerySourceIdentity::from_bytes([0x2a; 32])
    }
    pub(super) fn caps() -> SolverResourceCaps {
        SolverResourceCaps {
            max_argv_count: 16,
            max_argv_bytes: 4096,
            max_stdin_bytes: 4096,
            max_stdout_bytes: 4096,
            max_stderr_bytes: 1024,
            max_timeout_ms: 5000,
            max_ram_bytes: 16 * 1024 * 1024,
            max_disk_bytes: 4096,
            max_cleanup_bytes: 65_536,
            max_child_count: 1,
        }
    }
    pub(super) fn budget(disk: u64) -> ExecutionBudget {
        ExecutionBudget::checked(
            1000,
            4096,
            4096,
            1024,
            16,
            4096,
            8 * 1024 * 1024,
            disk,
            65_536,
        )
        .unwrap()
    }
    pub(super) fn resource_owner(
        source: QuerySourceIdentity,
    ) -> (ResourceLeaseIssuer, ResourceLease) {
        let issuer = ResourceLeaseIssuer::new().unwrap();
        let req =
            |kind, amount| ResourceRequest::new(kind, ResourceAmount::checked(amount).unwrap());
        let report = ReportedResourceLease::checked(
            1,
            lc631_substrate::Digest32::from_bytes(*source.as_bytes()),
            &[
                req(ResourceKind::Ram, 32 * 1024 * 1024),
                req(ResourceKind::Output, 2 * 1024 * 1024),
                req(ResourceKind::Child, 1),
                req(ResourceKind::Cleanup, 1024 * 1024),
            ],
            Deadline::after(Duration::from_secs(30)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
        let lease = issuer.issue_admitted(1, report).unwrap();
        (issuer, lease)
    }
}

struct SyntheticRunner {
    calls: usize,
    stdout: &'static [u8],
}
impl SolverProcessRunner for SyntheticRunner {
    fn run<'a>(
        &'a mut self,
        request: ProcessRequest<'a>,
        _view: &'a dyn LiveResourceReservation,
    ) -> Result<RunnerReportedProcessObservation<'a>, ProcessFailure> {
        self.calls += 1;
        Ok(RunnerReportedProcessObservation {
            state: ProcessRunState::Completed,
            source: request.source,
            query: request.query,
            invocation: request.invocation,
            observed_arguments: request.fixed_arguments,
            executable_digest: *request.expected_executable_digest,
            exit_code: Some(0),
            stdout: self.stdout,
            stderr: b"",
            stdout_truncated: false,
            stderr_truncated: false,
            cleanup_verified: true,
        })
    }
}

struct MustNotDispatch;
impl SolverProcessRunner for MustNotDispatch {
    fn run<'a>(
        &'a mut self,
        _request: ProcessRequest<'a>,
        _view: &'a dyn LiveResourceReservation,
    ) -> Result<RunnerReportedProcessObservation<'a>, ProcessFailure> {
        panic!("unsupported mapping must be rejected before dispatch")
    }
}

struct IncompleteRunner {
    state: ProcessRunState,
    stdout_truncated: bool,
    stderr_truncated: bool,
    cleanup_verified: bool,
}

impl SolverProcessRunner for IncompleteRunner {
    fn run<'a>(
        &'a mut self,
        request: ProcessRequest<'a>,
        _view: &'a dyn LiveResourceReservation,
    ) -> Result<RunnerReportedProcessObservation<'a>, ProcessFailure> {
        Ok(RunnerReportedProcessObservation {
            state: self.state,
            source: request.source,
            query: request.query,
            invocation: request.invocation,
            observed_arguments: request.fixed_arguments,
            executable_digest: *request.expected_executable_digest,
            exit_code: (self.state == ProcessRunState::Completed).then_some(0),
            stdout: b"sat\r\n()\r\n",
            stderr: b"",
            stdout_truncated: self.stdout_truncated,
            stderr_truncated: self.stderr_truncated,
            cleanup_verified: self.cleanup_verified,
        })
    }
}

fn query(kind: QueryResultKind) -> SolverQuery<'static> {
    SolverQuery::checked(
        BackendId::Z3,
        fixture::source(),
        LogicProfile::SmtBooleanQfLia,
        kind,
        b"test query",
    )
    .unwrap()
}

fn registry(protocol: SolverProtocol, executable: &str) -> SolverRegistry {
    SolverRegistry::checked(vec![SolverRegistration::checked(
        BackendId::Z3,
        LogicProfile::SmtBooleanQfLia,
        protocol,
        executable,
        "fixture-only-not-a-real-solver",
        [0x51; 32],
        Vec::new(),
        fixture::caps(),
    )
    .unwrap()])
    .unwrap()
}

#[test]
fn tlcx10_r2_v1_protocol_compatibility_is_unchanged() {
    for kind in [
        QueryResultKind::ModelPayload,
        QueryResultKind::ProofPayload,
        QueryResultKind::CounterexamplePayload,
    ] {
        assert_eq!(
            query_protocol_compatibility(SolverProtocol::SmtLibPayloadV1, kind),
            QueryProtocolCompatibility::Supported
        );
    }
    assert_eq!(
        query_protocol_compatibility(
            SolverProtocol::SmtLibPayloadV1,
            QueryResultKind::Satisfiability
        ),
        QueryProtocolCompatibility::Unsupported
    );
    assert_eq!(
        query_protocol_compatibility(
            SolverProtocol::SmtLibStatusV1,
            QueryResultKind::Satisfiability
        ),
        QueryProtocolCompatibility::Supported
    );
}

fn request(kind: ResourceKind, amount: u64) -> ResourceRequest {
    ResourceRequest::new(kind, ResourceAmount::checked(amount).unwrap())
}

#[test]
fn tlcx10_r2_store_metadata_claim_stays_live_until_store_drop() {
    let (_issuer, lease) = fixture::resource_owner(fixture::source());
    let view = lease.view();
    let ram_limit = view.amount(ResourceKind::Ram).unwrap();
    let store = OwnedPayloadStore::with_resources(1, &view).unwrap();
    assert_eq!(store.len(), 0);
    assert!(matches!(
        view.claim_resources(&[request(ResourceKind::Ram, ram_limit)]),
        Err(ResourceError::Exhausted(ResourceKind::Ram))
    ));
    drop(store);
    let full = view
        .claim_resources(&[request(ResourceKind::Ram, ram_limit)])
        .unwrap();
    drop(full);
    lease.close_confirmed().unwrap();
}

#[test]
fn tlcx10_r2_store_rejects_unreserved_and_exhausted_ram() {
    let issuer = ResourceLeaseIssuer::new().unwrap();
    let source = fixture::source();
    let lease_report = ReportedResourceLease::checked(
        1,
        lc631_substrate::Digest32::from_bytes(*source.as_bytes()),
        &[
            request(ResourceKind::Output, 4096),
            request(ResourceKind::Child, 1),
            request(ResourceKind::Cleanup, 65_536),
        ],
        Deadline::after(Duration::from_secs(30)).unwrap(),
        Cancellation::default(),
    )
    .unwrap();
    let lease = issuer.issue_admitted(1, lease_report).unwrap();
    let view = lease.view();
    assert!(matches!(
        OwnedPayloadStore::with_resources(1, &view),
        Err(OwnedPayloadStoreInitError::Resource(
            ResourceError::Unreserved(ResourceKind::Ram)
        ))
    ));
    lease.close_confirmed().unwrap();

    let (_issuer, lease) = fixture::resource_owner(source);
    let view = lease.view();
    let ram_limit = view.amount(ResourceKind::Ram).unwrap();
    let held = view
        .claim_resources(&[request(ResourceKind::Ram, ram_limit)])
        .unwrap();
    assert!(matches!(
        OwnedPayloadStore::with_resources(1, &view),
        Err(OwnedPayloadStoreInitError::Resource(
            ResourceError::Exhausted(ResourceKind::Ram)
        ))
    ));
    drop(held);
    let store = OwnedPayloadStore::with_resources(1, &view).unwrap();
    assert_eq!(store.len(), 0);
    drop(store);
    lease.close_confirmed().unwrap();
}

#[test]
fn tlcx10_r2_store_rejects_same_source_foreign_live_reservation() {
    let source = fixture::source();
    let (_issuer_a, lease_a) = fixture::resource_owner(source);
    let (_issuer_b, lease_b) = fixture::resource_owner(source);
    let view_a = lease_a.view();
    let view_b = lease_b.view();
    let mut store = OwnedPayloadStore::with_resources(1, &view_a).unwrap();
    let q = query(QueryResultKind::ModelPayload);
    let exe = std::env::current_exe().unwrap();
    let registry = registry(SolverProtocol::SmtLibPayloadWireV2, exe.to_str().unwrap());
    let result = execute_supervised_owned_payload_into_store(
        &registry,
        &q,
        &fixture::budget(0),
        &view_b,
        &std::env::current_dir().unwrap(),
        &mut store,
    );
    assert!(matches!(
        result,
        Err(OwnedPayloadError::Resource(ResourceError::ForeignOwner))
    ));
    assert_eq!(store.len(), 0);
    drop(store);
    lease_a.close_confirmed().unwrap();
    lease_b.close_confirmed().unwrap();
}

#[test]
fn tlcx10_r2_native_scratch_exhaustion_is_rejected_before_runner() {
    let source = fixture::source();
    let (_issuer, lease) = fixture::resource_owner(source);
    let view = lease.view();
    let all_ram = view.amount(ResourceKind::Ram).unwrap();
    let held = view
        .claim_resources(&[request(ResourceKind::Ram, all_ram)])
        .unwrap();
    let exe = std::env::current_exe().unwrap();
    let registry = registry(SolverProtocol::SmtLibPayloadWireV2, exe.to_str().unwrap());
    let q = query(QueryResultKind::ModelPayload);
    // The panic runner is an observable dispatch sentinel; it cannot establish
    // allocator-call counts without a separate allocator hook.
    let report = execute_registered_native_for_test(
        &registry,
        &q,
        &fixture::budget(0),
        &view,
        &mut MustNotDispatch,
    );
    assert_eq!(report.state(), SolverExecutionState::ResourceRejected);
    assert_eq!(report.logical_result(), None);
    assert_eq!(report.validation_basis(), None);
    assert_eq!(report.payload_wire_status(), None);
    assert_eq!(report.payload_wire_summary(), None);
    drop(held);
    lease.close_confirmed().unwrap();
}

#[test]
fn tlcx10_r2_owned_finalizer_preserves_incomplete_reports_and_store() {
    let source = fixture::source();
    let (_issuer, lease) = fixture::resource_owner(source);
    let view = lease.view();
    let mut store = OwnedPayloadStore::with_resources(1, &view).unwrap();
    let exe = std::env::current_exe().unwrap();
    let registry = registry(SolverProtocol::SmtLibPayloadWireV2, exe.to_str().unwrap());
    let q = query(QueryResultKind::ModelPayload);
    let budget = fixture::budget(0);
    let raw = b"sat\r\n()\r\n";
    for (state, stdout_truncated, stderr_truncated, cleanup, expected) in [
        (
            ProcessRunState::Completed,
            true,
            false,
            true,
            SolverExecutionState::OutputTruncated,
        ),
        (
            ProcessRunState::Completed,
            false,
            true,
            true,
            SolverExecutionState::OutputTruncated,
        ),
        (
            ProcessRunState::Completed,
            false,
            false,
            false,
            SolverExecutionState::CleanupUnknown,
        ),
        (
            ProcessRunState::TimedOut,
            false,
            false,
            true,
            SolverExecutionState::TimedOut,
        ),
    ] {
        let mut runner = IncompleteRunner {
            state,
            stdout_truncated,
            stderr_truncated,
            cleanup_verified: cleanup,
        };
        let report = execute_registered_native_for_test(&registry, &q, &budget, &view, &mut runner);
        assert_eq!(report.state(), expected);
        assert_eq!(report.logical_result(), None);
        assert_eq!(report.validation_basis(), None);
        let result = finish_owned_for_test(&q, &budget, &view, &mut store, report, raw);
        match result {
            Err(OwnedPayloadError::Report(preserved)) => assert_eq!(preserved, report),
            _ => panic!("incomplete report must be returned unchanged without an owner"),
        }
        assert_eq!(store.len(), 0);
    }
    drop(store);
    lease.close_confirmed().unwrap();
}

#[test]
fn tlcx10_r2_owned_copy_rejects_unreserved_and_exhausted_output_without_store_insert() {
    let source = fixture::source();
    let q = query(QueryResultKind::ModelPayload);
    let exe = std::env::current_exe().unwrap().join("must-not-dispatch");
    // A path nested under the executable file cannot launch a process. This
    // checks typed denial/store state, not whether a launch was attempted.
    let registry = registry(SolverProtocol::SmtLibPayloadWireV2, exe.to_str().unwrap());
    let cwd = std::env::current_dir().unwrap();

    let issuer = ResourceLeaseIssuer::new().unwrap();
    let report = ReportedResourceLease::checked(
        1,
        lc631_substrate::Digest32::from_bytes(*source.as_bytes()),
        &[
            request(ResourceKind::Ram, 32 * 1024 * 1024),
            request(ResourceKind::Child, 1),
            request(ResourceKind::Cleanup, 1024 * 1024),
        ],
        Deadline::after(Duration::from_secs(30)).unwrap(),
        Cancellation::default(),
    )
    .unwrap();
    let lease = issuer.issue_admitted(1, report).unwrap();
    let view = lease.view();
    let mut store = OwnedPayloadStore::with_resources(1, &view).unwrap();
    owned_test_hooks::reset();
    let denied = execute_supervised_owned_payload_into_store(
        &registry,
        &q,
        &fixture::budget(0),
        &view,
        &cwd,
        &mut store,
    );
    assert!(matches!(
        denied,
        Err(OwnedPayloadError::Resource(ResourceError::Unreserved(
            ResourceKind::Output
        )))
    ));
    assert_eq!(owned_test_hooks::counters(), (0, 0));
    assert_eq!(store.len(), 0);
    drop(store);
    lease.close_confirmed().unwrap();

    let (_issuer, lease) = fixture::resource_owner(source);
    let view = lease.view();
    let output_limit = view.amount(ResourceKind::Output).unwrap();
    let mut store = OwnedPayloadStore::with_resources(1, &view).unwrap();
    let held = view
        .claim_resources(&[request(ResourceKind::Output, output_limit)])
        .unwrap();
    owned_test_hooks::reset();
    let denied = execute_supervised_owned_payload_into_store(
        &registry,
        &q,
        &fixture::budget(0),
        &view,
        &cwd,
        &mut store,
    );
    assert!(matches!(
        denied,
        Err(OwnedPayloadError::Resource(ResourceError::Exhausted(
            ResourceKind::Output
        )))
    ));
    assert_eq!(owned_test_hooks::counters(), (0, 0));
    assert_eq!(store.len(), 0);
    drop(held);
    drop(store);
    lease.close_confirmed().unwrap();
}

#[test]
fn tlcx10_r2_owned_copy_rejects_exhausted_ram_without_store_insert() {
    let source = fixture::source();
    let (_issuer, lease) = fixture::resource_owner(source);
    let view = lease.view();
    let limit = view.amount(ResourceKind::Ram).unwrap();
    let metadata = u64::try_from(std::mem::size_of::<StoredSolverResult>()).unwrap();
    let mut store = OwnedPayloadStore::with_resources(1, &view).unwrap();
    // The currently registered store charges exactly one metadata record.
    let held = view
        .claim_resources(&[request(ResourceKind::Ram, limit - metadata)])
        .unwrap();
    let exe = std::env::current_exe().unwrap().join("must-not-dispatch");
    let registry = registry(SolverProtocol::SmtLibPayloadWireV2, exe.to_str().unwrap());
    let q = query(QueryResultKind::ModelPayload);
    owned_test_hooks::reset();
    let result = execute_supervised_owned_payload_into_store(
        &registry,
        &q,
        &fixture::budget(0),
        &view,
        &std::env::current_dir().unwrap(),
        &mut store,
    );
    assert!(matches!(
        result,
        Err(OwnedPayloadError::Resource(ResourceError::Exhausted(
            ResourceKind::Ram
        )))
    ));
    assert_eq!(owned_test_hooks::counters(), (0, 0));
    assert_eq!(store.len(), 0);
    drop(held);
    drop(store);
    lease.close_confirmed().unwrap();
}

#[test]
fn tlcx10_r2_v1_counterexample_mapping_and_generic_syntax_remain_unchanged() {
    let kind = QueryResultKind::CounterexamplePayload;
    assert_eq!(
        query_protocol_compatibility(SolverProtocol::SmtLibPayloadV1, kind),
        QueryProtocolCompatibility::Supported
    );
    assert_eq!(
        query_protocol_compatibility(SolverProtocol::SmtLibPayloadWireV2, kind),
        QueryProtocolCompatibility::Unsupported
    );
    let raw = b"(counterexample (x 2))";
    let parsed = parse_payload_syntax(
        SolverProtocol::SmtLibPayloadV1,
        kind,
        raw,
        SExprLimits::default(),
    )
    .unwrap();
    assert_eq!(parsed.requested_shape(), PayloadShape::Counterexample);
    assert_eq!(parsed.document().source_bytes(), raw);
    assert!(!parsed.semantically_validated());
    let q = query(kind);
    let exe = std::env::current_exe().unwrap();
    let legacy = registry(SolverProtocol::SmtLibPayloadV1, exe.to_str().unwrap());
    let (_issuer, lease) = fixture::resource_owner(q.source());
    let view = lease.view();
    let mut runner = SyntheticRunner {
        calls: 0,
        stdout: raw,
    };
    let report = execute_registered(&legacy, &q, &fixture::budget(0), &view, &mut runner);
    assert_eq!(runner.calls, 1);
    assert_eq!(report.state(), SolverExecutionState::PayloadSyntaxObserved);
    assert_eq!(
        report.payload_syntax_summary().unwrap().requested_shape(),
        PayloadShape::Counterexample
    );
    assert_eq!(report.logical_result(), None);
    assert_eq!(report.validation_basis(), None);
    assert!(!report.proof_checked());
    assert!(!report.authority_created());
    let wire = registry(SolverProtocol::SmtLibPayloadWireV2, exe.to_str().unwrap());
    let rejected = execute_registered(&wire, &q, &fixture::budget(0), &view, &mut MustNotDispatch);
    assert_eq!(rejected.state(), SolverExecutionState::UnsupportedMapping);
    assert_eq!(rejected.logical_result(), None);
    lease.close_confirmed().unwrap();
}

#[test]
fn tlcx10_r2_generic_nominal_reservation_cannot_dispatch_v2_payloads() {
    let exe = std::env::current_exe().unwrap();
    let registry = registry(SolverProtocol::SmtLibPayloadWireV2, exe.to_str().unwrap());
    for kind in [
        QueryResultKind::ModelPayload,
        QueryResultKind::ProofPayload,
        QueryResultKind::CounterexamplePayload,
    ] {
        let q = query(kind);
        let (_issuer, lease) = fixture::resource_owner(q.source());
        let view = lease.view();
        let report = execute_registered(
            &registry,
            &q,
            &fixture::budget(0),
            &view,
            &mut MustNotDispatch,
        );
        assert_eq!(report.state(), SolverExecutionState::UnsupportedMapping);
        assert_eq!(report.logical_result(), None);
        assert_eq!(report.validation_basis(), None);
        assert_eq!(report.payload_wire_summary(), None);
        assert!(!report.proof_checked());
        lease.close_confirmed().unwrap();
    }
}

#[test]
fn tlcx10_r2_legacy_v1_model_report_fingerprint_is_unchanged() {
    // Deterministic fictitious paths are data for the fake runner, never run.
    let executable = if cfg!(windows) {
        r"C:\tools\fixture.exe"
    } else {
        "/usr/bin/fixture"
    };
    let registry = registry(SolverProtocol::SmtLibPayloadV1, executable);
    let q = query(QueryResultKind::ModelPayload);
    let (_issuer, lease) = fixture::resource_owner(q.source());
    let view = lease.view();
    let mut runner = SyntheticRunner {
        calls: 0,
        stdout: b"(model)",
    };
    let report = execute_registered(&registry, &q, &fixture::budget(0), &view, &mut runner);
    assert_eq!(report.state(), SolverExecutionState::PayloadSyntaxObserved);
    let mut native_runner = SyntheticRunner {
        calls: 0,
        stdout: b"(model)",
    };
    let native_report = execute_registered_native_for_test(
        &registry,
        &q,
        &fixture::budget(0),
        &view,
        &mut native_runner,
    );
    assert_eq!(native_runner.calls, 1);
    assert_eq!(native_report, report);
    let bytes = u64::try_from(std::mem::size_of::<StoredSolverResult>()).unwrap();
    let metadata = view
        .claim_resources(&[request(ResourceKind::Ram, bytes)])
        .unwrap();
    let mut store = ResultStore::with_capacity(1).unwrap();
    store.try_reserve_backing().unwrap();
    let insertion = store.append_for_query(&q, report).unwrap();
    let actual = *store
        .lookup_by_id(insertion.id())
        .unwrap()
        .report_fingerprint();
    // Independent V1 byte-preimage golden, not produced by current code.
    let expected = if cfg!(windows) {
        [
            0x45, 0xbf, 0xe3, 0xa7, 0x40, 0xc1, 0x8c, 0xa3, 0x8c, 0x63, 0xf2, 0x02, 0x2c, 0xb1,
            0x46, 0xb4, 0xcb, 0x33, 0x17, 0xab, 0xc6, 0xae, 0x15, 0x72, 0x6d, 0x5d, 0xfb, 0xad,
            0x5d, 0x5d, 0xa2, 0xd7,
        ]
    } else {
        [
            0x3d, 0xcf, 0x72, 0x70, 0x0e, 0x99, 0x3c, 0x1e, 0x95, 0x32, 0x73, 0xcb, 0x4d, 0xf9,
            0x9b, 0xbe, 0xa6, 0x19, 0x78, 0xe4, 0x02, 0xf1, 0xd9, 0xa8, 0x8e, 0x27, 0xac, 0x44,
            0x59, 0x34, 0xdd, 0x56,
        ]
    };
    assert_eq!(actual, expected);
    drop(store);
    drop(metadata);
    let limit = view.amount(ResourceKind::Ram).unwrap();
    let held = view
        .claim_resources(&[request(ResourceKind::Ram, limit)])
        .unwrap();
    let denied = execute_registered_native_for_test(
        &registry,
        &q,
        &fixture::budget(0),
        &view,
        &mut MustNotDispatch,
    );
    assert_eq!(denied.state(), SolverExecutionState::ResourceRejected);
    assert_eq!(denied.logical_result(), None);
    drop(held);
    lease.close_confirmed().unwrap();
}

#[test]
fn tlcx10_native_parser_scratch_is_capped_by_exact_byte_bound() {
    use crate::protocol::sexpr::{self, SExprLimits};
    use crate::runner::native_payload_limits;
    for bytes in [1_usize, 2, 4096, 65_535, 65_536, 65_537] {
        let bounded = native_payload_limits(bytes).unwrap();
        assert_eq!(bounded.max_input_bytes(), bytes);
        assert_eq!(bounded.max_nodes(), bytes.min(65_536));
        assert_eq!(bounded.max_depth(), 128);
        let prior = SExprLimits::checked(bytes, 65_536, 128).unwrap();
        assert!(sexpr::scratch_bytes(bounded).unwrap() <= sexpr::scratch_bytes(prior).unwrap());
    }
    assert!(native_payload_limits(0).is_err());
    let raw = b"a b c";
    let document = sexpr::parse(raw, native_payload_limits(raw.len()).unwrap()).unwrap();
    assert_eq!(document.node_count(), 3);
    assert!(document.node_count() <= raw.len());
    assert_eq!(document.roots().len(), 3);
    let tiny = native_payload_limits(4096).unwrap();
    let prior = SExprLimits::checked(4096, 65_536, 128).unwrap();
    assert!(sexpr::scratch_bytes(tiny).unwrap() < sexpr::scratch_bytes(prior).unwrap());
}

// Append inside the existing crate-private owned_payload_tests module.
#[test]
fn tlcx10_owned_finalizer_rejects_same_length_mutation_preserving_report() {
    let q = query(QueryResultKind::ModelPayload);
    let (_issuer, lease) = fixture::resource_owner(q.source());
    let view = lease.view();
    let mut store = OwnedPayloadStore::with_resources(1, &view).unwrap();
    let exe = std::env::current_exe().unwrap();
    let registry = registry(SolverProtocol::SmtLibPayloadWireV2, exe.to_str().unwrap());
    let budget = fixture::budget(0);
    let original = b"sat\n((define-fun x () Int 2))\n";
    let changed = b"sat\n((define-fun x () Int 3))\n";
    assert_eq!(original.len(), changed.len());
    let mut runner = SyntheticRunner {
        calls: 0,
        stdout: original,
    };
    let report = execute_registered_native_for_test(&registry, &q, &budget, &view, &mut runner);
    assert_eq!(report.state(), SolverExecutionState::PayloadWireObserved);
    match finish_owned_for_test(&q, &budget, &view, &mut store, report, changed) {
        Err(OwnedPayloadError::Report(preserved)) => assert_eq!(preserved, report),
        _ => panic!("same-length altered bytes must not attach"),
    }
    assert_eq!(store.len(), 0);
    drop(store);
    lease.close_confirmed().unwrap();
}

#[test]
fn tlcx10_owned_finalizer_rejects_foreign_query_preserving_report() {
    let q = query(QueryResultKind::ModelPayload);
    let other = SolverQuery::checked(
        BackendId::Z3,
        q.source(),
        LogicProfile::SmtBooleanQfLia,
        QueryResultKind::ModelPayload,
        b"different query",
    )
    .unwrap();
    assert_ne!(q.digest(), other.digest());
    let (_issuer, lease) = fixture::resource_owner(q.source());
    let view = lease.view();
    let mut store = OwnedPayloadStore::with_resources(1, &view).unwrap();
    let exe = std::env::current_exe().unwrap();
    let registry = registry(SolverProtocol::SmtLibPayloadWireV2, exe.to_str().unwrap());
    let budget = fixture::budget(0);
    let raw = b"sat\n()\n";
    let mut runner = SyntheticRunner {
        calls: 0,
        stdout: raw,
    };
    let report = execute_registered_native_for_test(&registry, &q, &budget, &view, &mut runner);
    assert_eq!(report.state(), SolverExecutionState::PayloadWireObserved);
    match finish_owned_for_test(&other, &budget, &view, &mut store, report, raw) {
        Err(OwnedPayloadError::Report(preserved)) => assert_eq!(preserved, report),
        _ => panic!("foreign query must not attach"),
    }
    assert_eq!(store.len(), 0);
    drop(store);
    lease.close_confirmed().unwrap();
}

#[test]
fn tlcx10_metadata_only_supervisor_rejects_v2_before_launch() {
    let q = query(QueryResultKind::ModelPayload);
    let (_issuer, lease) = fixture::resource_owner(q.source());
    let view = lease.view();
    // Deliberately impossible executable path: no child can successfully run it.
    let impossible = std::env::current_exe().unwrap().join("must-not-dispatch");
    let registry = registry(
        SolverProtocol::SmtLibPayloadWireV2,
        impossible.to_str().unwrap(),
    );
    let report = crate::supervisor_adapter::execute_supervised(
        &registry,
        &q,
        &fixture::budget(0),
        &view,
        &std::env::current_dir().unwrap(),
    )
    .unwrap();
    assert_eq!(report.state(), SolverExecutionState::UnsupportedMapping);
    assert_eq!(report.process_state(), None);
    assert_eq!(report.logical_result(), None);
    assert_eq!(report.validation_basis(), None);
    lease.close_confirmed().unwrap();
}
