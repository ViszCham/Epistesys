#![allow(dead_code)]

use lc631_solvers::protocol::{QueryResultKind, SolverProtocol};
use lc631_solvers::registry::{
    BackendId, LogicProfile, SolverRegistration, SolverRegistry, SolverResourceCaps,
};
use lc631_solvers::runner::{
    execute_registered, ExecutionBudget, LiveResourceReservation, ProcessFailure, ProcessRequest,
    ProcessRunState, QuerySourceIdentity, RunnerReportedProcessObservation, SolverExecutionReport,
    SolverProcessRunner, SolverQuery,
};
use lc631_substrate::resource::{
    Cancellation, Deadline, ReportedResourceLease, ResourceAmount, ResourceKind, ResourceLease,
    ResourceLeaseIssuer, ResourceRequest,
};
use std::path::Path;
use std::time::Duration;

pub(crate) fn source() -> QuerySourceIdentity {
    QuerySourceIdentity::from_bytes([0x2a; 32])
}

pub(crate) fn caps() -> SolverResourceCaps {
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

pub(crate) fn budget(disk_bytes: u64) -> ExecutionBudget {
    ExecutionBudget::checked(
        1000,
        4096,
        4096,
        1024,
        16,
        4096,
        8 * 1024 * 1024,
        disk_bytes,
        65_536,
    )
    .unwrap()
}

pub(crate) fn query(source: QuerySourceIdentity) -> SolverQuery<'static> {
    query_with_payload(source, b"(check-sat)\n")
}

pub(crate) fn query_with_payload(
    source: QuerySourceIdentity,
    payload: &'static [u8],
) -> SolverQuery<'static> {
    SolverQuery::checked(
        BackendId::Z3,
        source,
        LogicProfile::SmtBooleanQfLia,
        QueryResultKind::Satisfiability,
        payload,
    )
    .unwrap()
}

pub(crate) fn registration(executable: &Path, digest: [u8; 32]) -> SolverRegistration {
    SolverRegistration::checked(
        BackendId::Z3,
        LogicProfile::SmtBooleanQfLia,
        SolverProtocol::SmtLibStatusV1,
        executable.to_str().expect("test executable is UTF-8"),
        "fixture-only-not-a-real-solver",
        digest,
        Vec::new(),
        caps(),
    )
    .unwrap()
}

pub(crate) fn resource_owner(source: QuerySourceIdentity) -> (ResourceLeaseIssuer, ResourceLease) {
    // This is reported lease metadata only: the issuer has no runtime quota
    // broker and does not establish process admission or OS enforcement.
    let issuer = ResourceLeaseIssuer::new().unwrap();
    let report = ReportedResourceLease::checked(
        1,
        lc631_substrate::Digest32::from_bytes(*source.as_bytes()),
        &[
            ResourceRequest::new(
                ResourceKind::Ram,
                ResourceAmount::checked(32 * 1024 * 1024).unwrap(),
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
        ],
        Deadline::after(Duration::from_secs(30)).unwrap(),
        Cancellation::default(),
    )
    .unwrap();
    let lease = issuer.issue_admitted(1, report).unwrap();
    (issuer, lease)
}

pub(crate) struct CancelledRunner;

impl SolverProcessRunner for CancelledRunner {
    fn run<'a>(
        &'a mut self,
        _request: ProcessRequest<'a>,
        _reservation: &'a dyn LiveResourceReservation,
    ) -> Result<RunnerReportedProcessObservation<'a>, ProcessFailure> {
        Err(ProcessFailure::Cancelled)
    }
}

pub(crate) struct ReportedRunner {
    pub(crate) state: ProcessRunState,
    pub(crate) stdout: &'static [u8],
    pub(crate) stdout_truncated: bool,
    pub(crate) cleanup_verified: bool,
}

impl SolverProcessRunner for ReportedRunner {
    fn run<'a>(
        &'a mut self,
        request: ProcessRequest<'a>,
        _reservation: &'a dyn LiveResourceReservation,
    ) -> Result<RunnerReportedProcessObservation<'a>, ProcessFailure> {
        Ok(RunnerReportedProcessObservation {
            state: self.state,
            source: request.source,
            query: request.query,
            invocation: request.invocation,
            observed_arguments: request.fixed_arguments,
            executable_digest: *request.expected_executable_digest,
            exit_code: (self.state == ProcessRunState::Completed).then_some(0),
            stdout: self.stdout,
            stderr: b"",
            stdout_truncated: self.stdout_truncated,
            stderr_truncated: false,
            cleanup_verified: self.cleanup_verified,
        })
    }
}

pub(crate) fn report_with(
    query: &SolverQuery<'_>,
    state: ProcessRunState,
    stdout: &'static [u8],
    stdout_truncated: bool,
    cleanup_verified: bool,
) -> SolverExecutionReport {
    let executable = std::env::current_exe().unwrap();
    let registry = SolverRegistry::checked(vec![registration(&executable, [0x51; 32])]).unwrap();
    let (_issuer, lease) = resource_owner(query.source());
    let view = lease.view();
    let mut runner = ReportedRunner {
        state,
        stdout,
        stdout_truncated,
        cleanup_verified,
    };
    let report = execute_registered(&registry, query, &budget(0), &view, &mut runner);
    lease.close_confirmed().unwrap();
    report
}
