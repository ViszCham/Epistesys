//! Actual bounded-process/store admission, not an actual SMT backend or truth oracle.
#[path = "common/tlcx10_result_fixture.rs"]
mod fixture;

use fixture::{budget, caps, query, resource_owner, source};
use lc631_solvers::protocol::SolverProtocol;
use lc631_solvers::registry::{BackendId, LogicProfile, SolverRegistration, SolverRegistry};
use lc631_solvers::result_store::{ResultStore, ResultStoreError};
use lc631_solvers::runner::SolverExecutionState;
use lc631_solvers::supervisor_adapter::{
    execute_supervised_into_store, hash_registered_executable, SupervisorAdapterError,
};
use lc631_solvers::{LogicalResult, Satisfiability};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const MARKER: &str = "tlcx10-started.txt";
static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "epistesys-tlcx10-store-preflight-{}-{stamp}-{sequence}",
            std::process::id()
        ));
        assert!(path.is_absolute());
        std::fs::create_dir(&path).unwrap();
        // cmd.exe rejects extended-length/UNC current-directory syntax even
        // though CreateProcess accepts it. Keep the already absolute temp path.
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        // Only this fixture's exact ordinary marker and empty directory may go.
        let marker = self.0.join(MARKER);
        match std::fs::symlink_metadata(&marker) {
            Ok(metadata) if metadata.file_type().is_file() => {
                if let Err(error) = std::fs::remove_file(&marker) {
                    eprintln!("fixture marker retained: {error}");
                    return;
                }
            }
            Ok(_) => {
                eprintln!("unexpected fixture marker retained");
                return;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                eprintln!("fixture marker inspection failed: {error}");
                return;
            }
        }
        if let Err(error) = std::fs::remove_dir(&self.0) {
            eprintln!("nonempty or unavailable fixture directory retained: {error}");
        }
    }
}

fn native_fixture() -> (PathBuf, Vec<String>) {
    #[cfg(windows)]
    {
        let executable = PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("System32")
            .join("cmd.exe");
        (
            executable,
            vec![
                // The supervisor deliberately CRT-quotes every argv entry.
                // cmd.exe is not a CRT argv consumer: quoted separate /c and
                // script entries leave a quote in its command name. A single
                // fixed command tail uses cmd's own parser; trailing `rem `
                // consumes the closing quote. No user text is interpolated.
                "/d /c echo started>tlcx10-started.txt&echo sat&rem ".into(),
            ],
        )
    }
    #[cfg(not(windows))]
    {
        (
            std::fs::canonicalize("/bin/sh").unwrap(),
            vec![
                "-c".into(),
                "printf 'started\n' > tlcx10-started.txt; printf 'sat\n'".into(),
            ],
        )
    }
}

#[test]
fn tlcx10_full_store_refuses_before_actual_native_dispatch_and_preserves_prior_result() {
    let scratch = Scratch::new();
    let marker = scratch.0.join(MARKER);
    let source = source();
    let (_issuer, lease) = resource_owner(source);
    {
        let view = lease.view();
        let (executable, arguments) = native_fixture();
        let binary_digest = hash_registered_executable(&view, &executable).unwrap();
        let registry = SolverRegistry::checked(vec![SolverRegistration::checked(
            BackendId::Z3,
            LogicProfile::SmtBooleanQfLia,
            SolverProtocol::SmtLibStatusV1,
            executable.to_str().unwrap(),
            "native-process-fixture-not-a-solver",
            binary_digest,
            arguments,
            caps(),
        )
        .unwrap()])
        .unwrap();
        let query = query(source);
        let mut store = ResultStore::with_capacity(1).unwrap();
        let first = execute_supervised_into_store(
            &registry,
            &query,
            &budget(0),
            &view,
            &scratch.0,
            &mut store,
        )
        .unwrap();
        eprintln!("initial fixture report: {:?}", first.report());
        if first.report().state() != SolverExecutionState::SolverReported {
            // Diagnostic replay only; never count a setup failure as the
            // full-store negative. Same limits and concrete supervisor.
            let arguments: Vec<std::ffi::OsString> = registry.entries()[0]
                .fixed_arguments()
                .iter()
                .map(std::ffi::OsString::from)
                .collect();
            let run_budget = budget(0);
            let request = lc631_process_supervisor::RunRequest {
                executable: executable.as_os_str(),
                arguments: &arguments,
                current_dir: &scratch.0,
                environment: &[],
                stdin: Some(query.payload()),
                timeout: std::time::Duration::from_millis(run_budget.timeout_ms()),
                stdout_limit: run_budget.max_stdout_bytes(),
                stderr_limit: run_budget.max_stderr_bytes(),
            };
            match lc631_process_supervisor::run_bounded_with_resources_observed(request, &view) {
                Ok(observed) => {
                    let outcome = observed.outcome();
                    eprintln!(
                        "fixture diagnostic: exit={:?}, diagnostic={:?}, stdout={:?}, stderr={:?}, cleanup={}",
                        outcome.exit_code,
                        outcome.diagnostic,
                        String::from_utf8_lossy(&outcome.stdout),
                        String::from_utf8_lossy(&outcome.stderr),
                        observed.cleanup_complete(),
                    );
                }
                Err(error) => eprintln!("fixture diagnostic replay: {error:?}"),
            }
        }
        assert_eq!(first.report().state(), SolverExecutionState::SolverReported);
        assert_eq!(
            first.report().logical_result(),
            Some(LogicalResult::Satisfiability(Satisfiability::Sat))
        );
        assert!(first.report().stdout_fully_observed());
        assert!(!first.report().authority_created());
        assert!(!first.report().proof_checked());
        assert!(!first.report().output_commit_allowed());
        let prior = first.insertion().unwrap();
        assert_eq!(
            store.lookup_by_id(prior.id()).unwrap().report(),
            first.report()
        );
        assert_eq!(store.len(), 1);
        assert!(std::fs::read(&marker).unwrap().starts_with(b"started"));
        std::fs::remove_file(&marker).unwrap();
        assert_eq!(
            store.try_reserve_append(),
            Err(ResultStoreError::CapacityReached)
        );

        let second = execute_supervised_into_store(
            &registry,
            &query,
            &budget(0),
            &view,
            &scratch.0,
            &mut store,
        );
        let started_again = marker.try_exists().unwrap();
        let actual = second.as_ref().map(|receipt| {
            (
                receipt.report().state(),
                receipt.insertion(),
                receipt.report().stdout_bytes(),
            )
        });
        assert!(
            second.is_err() && !started_again,
            "full-store dispatch must be NoRun; actual={actual:?}, native_marker={started_again}"
        );
        assert!(matches!(
            second,
            Err(SupervisorAdapterError::StoreBeforeExecution(
                ResultStoreError::CapacityReached
            ))
        ));
        assert_eq!(store.len(), 1);
        assert_eq!(
            store.lookup_by_id(prior.id()).unwrap().report(),
            first.report()
        );
    }
    lease.close_confirmed().unwrap();
}
