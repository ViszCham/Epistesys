//! Opt-in actual Clingo tests. Missing pins fail when selected; ignored is not evidence.
//! The admitted test lease accounts local claims, not host-wide quota enforcement.
use lc631_solvers::asp::native::{
    execute_native_asp, NativeAspRejection, NativeAspSetupError, NativeAspStore,
};
use lc631_solvers::asp::*;
use lc631_solvers::protocol::SolverProtocol;
use lc631_solvers::registry::{
    BackendId, LogicProfile, SolverRegistration, SolverRegistry, SolverResourceCaps,
};
use lc631_solvers::runner::{ExecutionBudget, SolverExecutionState};
use lc631_solvers::supervisor_adapter::hash_registered_executable;
use lc631_substrate::context_identity::{ContextRevision, SourceRevision};
use lc631_substrate::resource::{
    Cancellation, Deadline, ReportedResourceLease, ResourceAmount, ResourceKind,
    ResourceLeaseIssuer, ResourceRequest, ResourceReservationView,
};
use lc631_substrate::Digest32;
use std::path::{Path, PathBuf};
use std::time::Duration;

fn path(name: &str) -> PathBuf {
    let path = PathBuf::from(
        std::env::var_os(name)
            .unwrap_or_else(|| panic!("{name} required for selected native profile")),
    );
    assert!(path.is_absolute());
    path
}

fn digest(hex: &str) -> [u8; 32] {
    assert_eq!(hex.len(), 64);
    let mut bytes = [0; 32];
    for (i, b) in bytes.iter_mut().enumerate() {
        *b = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap();
    }
    bytes
}

fn pins() -> [[u8; 32]; 3] {
    if cfg!(windows) {
        [
            digest("0b471133e110cfb53a061cad528ce8e517d7b9ac41a0a396c39ad795a487fc14"),
            digest("dec53ced54fd123dc538df12ffda3c031a6906ffd7cfc76160e323b123076fd7"),
            digest("91270e42b401878a414e947d4a16f5a7f63db5b9b2cdacfcdf19da91114f1441"),
        ]
    } else {
        [
            digest("e50d468e8b0adfb05733f5b87b3cff34829c4a8c1aea50c865aa8bdfe4bb150f"),
            digest("39ab82bdaf2d382a594bbd707552acfa201651e8d818a1741f770baa2c4187dd"),
            digest("ce9135fa69df02f822d9e7ff4079a5e27acfdb774ab6e943be60ff2eb59a20c5"),
        ]
    }
}

fn program(context: ContextRevision, exception: bool) -> AspProgram {
    let a = AtomId::new(0);
    let b = AtomId::new(1);
    let mut rules = vec![
        GroundRule::new(Some(a), vec![], vec![b]),
        GroundRule::new(Some(b), vec![], vec![a]),
    ];
    if exception {
        rules.push(GroundRule::new(Some(b), vec![], vec![]));
    }
    AspProgram::checked(
        AspBinding::declared(
            context,
            Digest32::from_bytes([2; 32]),
            Digest32::from_bytes([3; 32]),
        ),
        AspSemantics::StableModelsClosedWorld,
        vec![a, b],
        rules,
        vec![
            AtomCost { atom: a, weight: 3 },
            AtomCost { atom: b, weight: 3 },
        ],
        AspLimits::default(),
    )
    .unwrap()
}

fn registry(view: &ResourceReservationView<'_>, limit: usize) -> SolverRegistry {
    let python = path("EPISTESYS_TLCX10_CLINGO_PYTHON");
    let extension = path("EPISTESYS_TLCX10_CLINGO_PYD");
    let main = path("EPISTESYS_TLCX10_CLINGO_MAIN");
    let pins = pins();
    for (path, pin) in [(&python, pins[0]), (&extension, pins[1]), (&main, pins[2])] {
        assert_eq!(hash_registered_executable(view, path).unwrap(), pin);
    }
    let caps = SolverResourceCaps {
        max_argv_count: 16,
        max_argv_bytes: 4096,
        max_stdin_bytes: 65536,
        max_stdout_bytes: 65536,
        max_stderr_bytes: 16384,
        max_timeout_ms: 10000,
        max_ram_bytes: 64 * 1024 * 1024,
        max_disk_bytes: 0,
        max_cleanup_bytes: 65536,
        max_child_count: 1,
    };
    SolverRegistry::checked(vec![SolverRegistration::checked(
        BackendId::Clingo,
        LogicProfile::AspFiniteGroundNormal,
        SolverProtocol::ClingoStatusV1,
        python.to_str().unwrap(),
        "5.8.2",
        pins[0],
        vec![
            "-I".into(),
            "-B".into(),
            "-m".into(),
            "clingo".into(),
            format!("--models={limit}"),
            "-".into(),
        ],
        caps,
    )
    .unwrap()])
    .unwrap()
}

fn native_case(
    limit: usize,
    exception: bool,
    inspect: impl FnOnce(&AspProgram, &lc631_solvers::asp::native::NativeAspOutcome<'_>),
) {
    match std::env::consts::OS {
        "windows" | "linux" => (),
        platform => panic!("unsupported native profile: {platform}"),
    }
    let source = SourceRevision::of("mutual defaults; optional exception");
    let context = ContextRevision::original(source);
    let context = if exception {
        context.reconstructed([9; 32])
    } else {
        context
    };
    let program = program(context, exception);
    let issuer = ResourceLeaseIssuer::new().unwrap();
    let requests = [
        ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(128 * 1024 * 1024).unwrap(),
        ),
        ResourceRequest::new(
            ResourceKind::Output,
            ResourceAmount::checked(4 * 1024 * 1024).unwrap(),
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
                Deadline::after(Duration::from_secs(60)).unwrap(),
                Cancellation::default(),
            )
            .unwrap(),
        )
        .unwrap();
    {
        let view = lease.view();
        let registry = registry(&view, limit);
        let mut store = NativeAspStore::with_resources(1, &view).unwrap();
        let budget = ExecutionBudget::checked(
            10000,
            65536,
            65536,
            16384,
            16,
            4096,
            64 * 1024 * 1024,
            0,
            65536,
        )
        .unwrap();
        let cwd = std::env::current_dir().unwrap();
        let outcome = execute_native_asp(
            &program,
            &registry,
            &budget,
            &view,
            Path::new(&cwd),
            &mut store,
        )
        .unwrap();
        assert_eq!(
            outcome.report().state(),
            SolverExecutionState::SolverReported
        );
        assert!(!outcome.report().authority_created());
        assert!(!outcome.report().proof_checked());
        assert!(!outcome.stdout().is_empty());
        assert_eq!(store.len(), 1);
        let id = outcome.insertion().unwrap().id();
        assert_eq!(store.lookup_by_id(id).unwrap().report(), outcome.report());
        inspect(&program, &outcome);
    }
    lease.close_confirmed().unwrap();
}

#[test]
#[ignore = "explicit actual Clingo pins required"]
fn tlcx12_native_missing_answer_rejected_with_actual_report_and_stdout_retained() {
    native_case(1, false, |_, outcome| {
        assert!(matches!(
            outcome.evaluation(),
            Err(NativeAspRejection::Asp(AspError::IncompleteAnswerSet))
        ));
        assert!(std::str::from_utf8(outcome.stdout())
            .unwrap()
            .contains("Answer: 1"));
    });
}

#[test]
#[ignore = "explicit actual Clingo pins required"]
fn tlcx12_native_two_models_and_brave_cautious_are_consumed() {
    native_case(0, false, |program, outcome| {
        let evaluation = outcome.evaluation().as_ref().unwrap();
        assert_eq!(evaluation.binding(), program.binding());
        assert_eq!(evaluation.models().len(), 2);
        assert_eq!(evaluation.minimum_cost(), Some(3));
        assert_eq!(evaluation.optimal_model_indices().len(), 2);
        assert!(!evaluation.clingo_native_optimization());
        assert_eq!(
            evaluation
                .consequence(AtomId::new(0), ConsequenceMode::Brave)
                .unwrap(),
            Consequence::Holds
        );
        assert_eq!(
            evaluation
                .consequence(AtomId::new(0), ConsequenceMode::Cautious)
                .unwrap(),
            Consequence::DoesNotHold
        );
    });
}

#[test]
#[ignore = "explicit actual Clingo pins required"]
fn tlcx12_native_exception_changes_conclusion_in_reconstructed_context() {
    native_case(0, true, |program, outcome| {
        let evaluation = outcome.evaluation().as_ref().unwrap();
        assert_eq!(evaluation.binding(), program.binding());
        assert_eq!(evaluation.models().len(), 1);
        assert_eq!(
            evaluation
                .consequence(AtomId::new(0), ConsequenceMode::Brave)
                .unwrap(),
            Consequence::DoesNotHold
        );
        assert_eq!(
            evaluation
                .consequence(AtomId::new(1), ConsequenceMode::Cautious)
                .unwrap(),
            Consequence::Holds
        );
        let original = ContextRevision::original(program.binding().context().source());
        assert_ne!(program.binding().context(), original);
    });
}

#[test]
fn tlcx12_native_resource_refusal_happens_before_dispatch_and_store_append() {
    let source = SourceRevision::of("preflight refusal");
    let p = program(ContextRevision::original(source), false);
    let issuer = ResourceLeaseIssuer::new().unwrap();
    let requests = [
        ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(16 * 1024 * 1024).unwrap(),
        ),
        ResourceRequest::new(ResourceKind::Output, ResourceAmount::checked(1).unwrap()),
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
        let caps = SolverResourceCaps {
            max_argv_count: 16,
            max_argv_bytes: 4096,
            max_stdin_bytes: 65536,
            max_stdout_bytes: 65536,
            max_stderr_bytes: 16384,
            max_timeout_ms: 10000,
            max_ram_bytes: 64 * 1024 * 1024,
            max_disk_bytes: 0,
            max_cleanup_bytes: 65536,
            max_child_count: 1,
        };
        let exe = std::env::current_exe().unwrap();
        let registry = SolverRegistry::checked(vec![SolverRegistration::checked(
            BackendId::Clingo,
            LogicProfile::AspFiniteGroundNormal,
            SolverProtocol::ClingoStatusV1,
            exe.to_str().unwrap(),
            "not-dispatched-fixture",
            [7; 32],
            vec![],
            caps,
        )
        .unwrap()])
        .unwrap();
        let budget = ExecutionBudget::checked(
            1000,
            65536,
            65536,
            16384,
            16,
            4096,
            64 * 1024 * 1024,
            0,
            65536,
        )
        .unwrap();
        let cwd = std::env::current_dir().unwrap();
        let error = match execute_native_asp(&p, &registry, &budget, &view, &cwd, &mut store) {
            Err(error) => error,
            Ok(_) => panic!("insufficient output reservation dispatched"),
        };
        assert!(matches!(error, NativeAspSetupError::Resource(_)));
        assert!(store.is_empty());
    }
    lease.close_confirmed().unwrap();
}
