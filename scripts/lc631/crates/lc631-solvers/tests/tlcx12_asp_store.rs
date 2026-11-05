//! Reported-runner contract tests only; not actual Clingo process evidence.
#[path = "common/tlcx10_result_fixture.rs"]
mod fixture;

use lc631_solvers::asp::*;
use lc631_solvers::protocol::SolverProtocol;
use lc631_solvers::registry::{BackendId, LogicProfile, SolverRegistration, SolverRegistry};
use lc631_solvers::result_store::{ResultStore, SolverResultBinding};
use lc631_solvers::runner::{execute_registered, ProcessRunState};
use lc631_substrate::context_identity::{ContextRevision, SourceRevision};
use lc631_substrate::Digest32;

fn program(revision: u8) -> AspProgram {
    let context = ContextRevision::original(SourceRevision::of("context binding"));
    AspProgram::checked(
        AspBinding::declared(
            context.reconstructed([revision; 32]),
            Digest32::from_bytes([2; 32]),
            Digest32::from_bytes([3; 32]),
        ),
        AspSemantics::StableModelsClosedWorld,
        vec![AtomId::new(0)],
        vec![GroundRule::new(Some(AtomId::new(0)), vec![], vec![])],
        vec![],
        AspLimits::default(),
    )
    .unwrap()
}

#[test]
fn tlcx12_stored_result_consumes_exact_output_and_rejects_swapped_context_or_bytes() {
    let p = program(1);
    let prepared = p.prepare().unwrap();
    let query = prepared.solver_query().unwrap();
    let executable = std::env::current_exe().unwrap();
    let registration = SolverRegistration::checked(
        BackendId::Clingo,
        LogicProfile::AspFiniteGroundNormal,
        SolverProtocol::ClingoStatusV1,
        executable.to_str().unwrap(),
        "reported-fixture-only",
        [7; 32],
        vec!["--models=0".into()],
        fixture::caps(),
    )
    .unwrap();
    let registry = SolverRegistry::checked(vec![registration]).unwrap();
    let (_issuer, lease) = fixture::resource_owner(query.source());
    let stdout: &'static [u8] = b"Answer: 1\na0\nSATISFIABLE\n";
    let mut runner = fixture::ReportedRunner {
        state: ProcessRunState::Completed,
        stdout,
        stdout_truncated: false,
        cleanup_verified: true,
    };
    let report = execute_registered(
        &registry,
        &query,
        &fixture::budget(0),
        &lease.view(),
        &mut runner,
    );
    let mut store = ResultStore::with_capacity(1).unwrap();
    let insertion = store.append_for_query(&query, report).unwrap();
    let expected = SolverResultBinding::for_query(&query, report.invocation_digest());
    let stored = store.lookup(expected).unwrap();
    assert_eq!(stored.id(), insertion.id());
    let result = p.reconstruct_stored(stored, stdout).unwrap();
    assert_eq!(result.models().len(), 1);
    assert_eq!(result.binding(), p.binding());
    assert_eq!(
        p.reconstruct_stored(stored, b"Answer: 1\n\nSATISFIABLE\n")
            .unwrap_err(),
        AspError::OutputMismatch
    );
    assert_eq!(
        program(9).reconstruct_stored(stored, stdout).unwrap_err(),
        AspError::BindingMismatch
    );
    lease.close_confirmed().unwrap();
}
