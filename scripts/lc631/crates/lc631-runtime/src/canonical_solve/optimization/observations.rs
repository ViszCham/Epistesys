//! Bind actual third-call observations even when the model is NOT selected.
//! Full typed errors remain owned. Only selected report fields have the stable
//! report codec; detailed report metadata stays explicitly unbound/U.
use super::*;
use lc631_solvers::supervisor_adapter::OwnedPayloadError;
use sha2::{Digest, Sha256};

pub(super) fn report(step: &OwnedObjectiveStep<'_>) -> Option<SolverExecutionReport> {
    step.improvement
        .report
        .or_else(|| match step.native_error.as_ref() {
            Some(NativeModelError::Rejected { observation, .. }) => Some(observation.report()),
            Some(NativeModelError::Execution(
                OwnedPayloadError::Report(report)
                | OwnedPayloadError::StoreAfterExecution { report, .. },
            )) => Some(*report),
            _ => None,
        })
}
pub(super) fn stdout<'s>(step: &'s OwnedObjectiveStep<'_>) -> Option<&'s [u8]> {
    if step.improvement.report.is_some() {
        return Some(&step.improvement.stdout);
    }
    match step.native_error.as_ref() {
        Some(NativeModelError::Rejected { observation, .. }) => Some(observation.stdout()),
        // Report-only failure never fabricates a raw buffer, including empty.
        _ => None,
    }
}
pub(super) fn binding(step: &OwnedObjectiveStep<'_>) -> Option<Digest32> {
    let actual = report(step)?;
    let mut hash = Sha256::new();
    hash.update(b"canonical.actual-third-model-observation.v1\0");
    hash.update(
        crate::canonical_solve::decision::report_identity::report_digest(actual).as_bytes(),
    );
    hash.update((step.improved_stored_results() as u64).to_le_bytes());
    match stdout(step) {
        None => hash.update([0]),
        Some(bytes) => {
            hash.update([1]);
            hash.update((bytes.len() as u64).to_le_bytes());
            hash.update(bytes);
        }
    }
    Some(Digest32::from_bytes(hash.finalize().into()))
}
