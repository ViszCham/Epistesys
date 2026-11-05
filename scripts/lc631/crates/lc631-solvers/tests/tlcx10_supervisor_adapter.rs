#[path = "common/tlcx10_result_fixture.rs"]
mod fixture;

use fixture::{budget, query, registration, resource_owner, source};
use lc631_solvers::registry::SolverRegistry;
use lc631_solvers::runner::SolverExecutionState;
use lc631_solvers::supervisor_adapter::{
    execute_supervised, hash_registered_executable, SupervisorAdapterError,
};
use std::path::Path;

#[test]
fn tlcx10_path_digest_mismatch_is_held_before_any_child_start() {
    // A current executable is used only as local file input. The deliberately
    // mismatched registration must stop before process launch; this test does
    // not pretend to run or validate a real Z3 backend.
    let executable = std::env::current_exe().unwrap();
    let source = source();
    let (_issuer, lease) = resource_owner(source);
    let report = {
        let view = lease.view();
        let observed_digest = hash_registered_executable(&view, &executable).unwrap();
        let mut wrong_digest = observed_digest;
        wrong_digest[0] ^= 0x80;
        let registry =
            SolverRegistry::checked(vec![registration(&executable, wrong_digest)]).unwrap();

        execute_supervised(
            &registry,
            &query(source),
            &budget(0),
            &view,
            &std::env::current_dir().unwrap(),
        )
        .unwrap()
    };

    assert_eq!(report.state(), SolverExecutionState::BindingMismatch);
    assert_eq!(
        report.process_state(),
        Some(lc631_solvers::runner::ProcessRunState::NotStarted)
    );
    assert_eq!(report.binary_binding_state(), None);
    assert_eq!(report.logical_result(), None);
    assert_eq!(report.stdout_bytes(), 0);
    assert!(!report.stdout_fully_observed());
    assert!(!report.stderr_fully_observed());
    assert!(!report.authority_created());
    assert!(!report.output_commit_allowed());
    lease.close_confirmed().unwrap();
}

#[test]
fn tlcx10_disk_quota_is_rejected_instead_of_ignored_by_this_adapter() {
    let executable = std::env::current_exe().unwrap();
    let source = source();
    let (_issuer, lease) = resource_owner(source);
    {
        let view = lease.view();
        let observed_digest = hash_registered_executable(&view, &executable).unwrap();
        let registry =
            SolverRegistry::checked(vec![registration(&executable, observed_digest)]).unwrap();
        assert_eq!(
            execute_supervised(
                &registry,
                &query(source),
                &budget(1),
                &view,
                &std::env::current_dir().unwrap(),
            ),
            Err(SupervisorAdapterError::DiskQuotaUnsupported)
        );
    }
    lease.close_confirmed().unwrap();
}

#[test]
fn tlcx10_relative_working_directory_is_rejected_before_dispatch() {
    let executable = std::env::current_exe().unwrap();
    let source = source();
    let (_issuer, lease) = resource_owner(source);
    {
        let view = lease.view();
        let digest = hash_registered_executable(&view, &executable).unwrap();
        let registry = SolverRegistry::checked(vec![registration(&executable, digest)]).unwrap();
        assert_eq!(
            execute_supervised(&registry, &query(source), &budget(0), &view, Path::new(".")),
            Err(SupervisorAdapterError::RelativeWorkingDirectory)
        );
    }
    lease.close_confirmed().unwrap();
}
