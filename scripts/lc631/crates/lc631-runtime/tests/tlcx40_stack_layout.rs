//! Layout diagnostics only: no process-stack, RSS or memory-safety proof.
use lc631_runtime::canonical_solve::{
    CanonicalError, NativeEvidence, OwnedCanonicalDecision, OwnedObjectiveStep,
};
use lc631_solvers::runner::SolverExecutionReport;
use std::mem::size_of;

#[test]
fn tlcx40_stack_layout_diagnostic_only() {
    type Evidence = NativeEvidence<'static, 'static>;
    type Decision = OwnedCanonicalDecision<'static, 'static>;
    type Error = CanonicalError<'static, 'static>;
    type Step = OwnedObjectiveStep<'static>;
    for (name, bytes) in [
        ("SolverExecutionReport", size_of::<SolverExecutionReport>()),
        ("OwnedObjectiveStep", size_of::<Step>()),
        ("Option<OwnedObjectiveStep>", size_of::<Option<Step>>()),
        ("Vec<OwnedObjectiveStep>", size_of::<Vec<Step>>()),
        ("NativeEvidence", size_of::<Evidence>()),
        ("OwnedCanonicalDecision", size_of::<Decision>()),
        ("CanonicalError", size_of::<Error>()),
        (
            "Result<OwnedCanonicalDecision,CanonicalError>",
            size_of::<Result<Decision, Error>>(),
        ),
        ("Result<(),CanonicalError>", size_of::<Result<(), Error>>()),
    ] {
        eprintln!("tlcx40 stack-layout {name}: {bytes} bytes");
    }
    // There is deliberately no guessed stack limit or vacuous safety assertion.
    // Only real normal-process regression tests can corroborate the stack fix.
}
