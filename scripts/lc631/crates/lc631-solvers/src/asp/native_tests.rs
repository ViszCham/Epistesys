//! Synthetic finalizer cuts only. These tests are not native process evidence.
use super::*;
use crate::asp::{AspBinding, AspCompleteness, AspLimits, AspSemantics, AtomId, GroundRule};
use crate::registry::{BackendId, LogicProfile, SolverRegistration, SolverResourceCaps};
use crate::runner::{
    execute_registered, LiveResourceReservation, ProcessFailure, ProcessRequest, ProcessRunState,
    RunnerReportedProcessObservation, SolverProcessRunner,
};
use lc631_substrate::context_identity::{ContextRevision, SourceRevision};
use lc631_substrate::resource::{
    Cancellation, Deadline, ReportedResourceLease, ResourceLeaseIssuer,
};
use lc631_substrate::Digest32;
use std::time::Duration;

struct Injected {
    state: ProcessRunState,
    truncated: bool,
    cleanup: bool,
    stdout: &'static [u8],
}

impl SolverProcessRunner for Injected {
    fn run<'a>(
        &'a mut self,
        request: ProcessRequest<'a>,
        _lease: &'a dyn LiveResourceReservation,
    ) -> Result<RunnerReportedProcessObservation<'a>, ProcessFailure> {
        Ok(RunnerReportedProcessObservation {
            state: self.state,
            source: request.source,
            query: request.query,
            invocation: request.invocation,
            observed_arguments: request.fixed_arguments,
            executable_digest: *request.expected_executable_digest,
            exit_code: Some(0),
            stdout: self.stdout,
            stderr: b"",
            stdout_truncated: self.truncated,
            stderr_truncated: false,
            cleanup_verified: self.cleanup,
        })
    }
}

fn with_case(mut injected: Injected, inspect: impl FnOnce(&NativeAspOutcome<'_>)) {
    let source = SourceRevision::of("synthetic finalizer cuts");
    let p = AspProgram::checked(
        AspBinding::declared(
            ContextRevision::original(source),
            Digest32::from_bytes([2; 32]),
            Digest32::from_bytes([3; 32]),
        ),
        AspSemantics::StableModelsClosedWorld,
        vec![AtomId::new(0)],
        vec![GroundRule::new(Some(AtomId::new(0)), vec![], vec![])],
        vec![],
        AspLimits::default(),
    )
    .unwrap();
    let issuer = ResourceLeaseIssuer::new().unwrap();
    let requests = [
        ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(32 * 1024 * 1024).unwrap(),
        ),
        ResourceRequest::new(
            ResourceKind::Output,
            ResourceAmount::checked(1024 * 1024).unwrap(),
        ),
        ResourceRequest::new(ResourceKind::Child, ResourceAmount::checked(1).unwrap()),
        ResourceRequest::new(
            ResourceKind::Cleanup,
            ResourceAmount::checked(1024 * 1024).unwrap(),
        ),
    ];
    let lease = issuer
        .issue_admitted(
            1,
            ReportedResourceLease::checked(
                1,
                source.digest(),
                &requests,
                Deadline::after(Duration::from_secs(30)).unwrap(),
                Cancellation::default(),
            )
            .unwrap(),
        )
        .unwrap();
    {
        let view = lease.view();
        let mut store = NativeAspStore::with_resources(1, &view).unwrap();
        let budget =
            ExecutionBudget::checked(1000, 4096, 4096, 1024, 16, 4096, 16 * 1024 * 1024, 0, 65536)
                .unwrap();
        let exe = std::env::current_exe().unwrap();
        let caps = SolverResourceCaps {
            max_argv_count: 16,
            max_argv_bytes: 4096,
            max_stdin_bytes: 4096,
            max_stdout_bytes: 4096,
            max_stderr_bytes: 1024,
            max_timeout_ms: 1000,
            max_ram_bytes: 16 * 1024 * 1024,
            max_disk_bytes: 0,
            max_cleanup_bytes: 65536,
            max_child_count: 1,
        };
        let registry = SolverRegistry::checked(vec![SolverRegistration::checked(
            BackendId::Clingo,
            LogicProfile::AspFiniteGroundNormal,
            SolverProtocol::ClingoStatusV1,
            exe.to_str().unwrap(),
            "synthetic-only",
            [7; 32],
            vec![],
            caps,
        )
        .unwrap()])
        .unwrap();
        let prepared = p.prepare().unwrap();
        let query = prepared.solver_query().unwrap();
        let buffers = prepare_buffers(&p, &budget, &view).unwrap();
        let report = execute_registered(&registry, &query, &budget, &view, &mut injected);
        let outcome = finish_native_asp(&p, &query, report, injected.stdout, buffers, &mut store);
        assert_eq!(outcome.report(), report);
        assert_eq!(outcome.stdout(), injected.stdout);
        assert_eq!(store.len(), 1);
        assert!(
            !report.authority_created()
                && !report.proof_checked()
                && !report.output_commit_allowed()
        );
        inspect(&outcome);
    }
    lease.close_confirmed().unwrap();
}

#[test]
fn tlcx12_native_finalizer_timeout_truncation_and_unknown_cleanup_remain_process_rejections() {
    for (state, truncated, cleanup) in [
        (ProcessRunState::TimedOut, false, true),
        (ProcessRunState::Completed, true, true),
        (ProcessRunState::Completed, false, false),
    ] {
        with_case(
            Injected {
                state,
                truncated,
                cleanup,
                stdout: b"Answer: 1\na0\nSATISFIABLE\n",
            },
            |outcome| {
                assert!(matches!(
                    outcome.evaluation(),
                    Err(NativeAspRejection::Process)
                ));
                assert!(outcome.insertion().is_ok());
            },
        );
    }
}

#[test]
fn tlcx12_native_finalizer_duplicate_and_unknown_payloads_preserve_actual_metadata() {
    with_case(
        Injected {
            state: ProcessRunState::Completed,
            truncated: false,
            cleanup: true,
            stdout: b"Answer: 1\na0\nAnswer: 2\na0\nSATISFIABLE\n",
        },
        |outcome| {
            assert!(matches!(
                outcome.evaluation(),
                Err(NativeAspRejection::Asp(AspError::DuplicateModel))
            ));
        },
    );
    with_case(
        Injected {
            state: ProcessRunState::Completed,
            truncated: false,
            cleanup: true,
            stdout: b"Answer: 1\na0\nUNKNOWN\n",
        },
        |outcome| {
            let evaluation = outcome.evaluation().as_ref().unwrap();
            assert_eq!(evaluation.completeness(), AspCompleteness::SolverUnknown);
            assert_eq!(evaluation.models().len(), 1);
            assert_eq!(evaluation.minimum_cost(), None);
        },
    );
}
