use lc631_substrate::Digest32;
use sha2::{Digest, Sha256};
/// Stable, selected actual report identity fields. Explicit v1 tags below are
/// independent of Rust discriminants/Debug. Detailed syntax summaries, logical
/// validation and registry-completeness metadata remain UNBOUND (explicit U).
/// The full actual typed report is still retained by its evidence owner.
pub(in crate::canonical_solve) fn report_digest(
    report: lc631_solvers::runner::SolverExecutionReport,
) -> Digest32 {
    use lc631_solvers::protocol::payload_wire::PayloadWireStatus as W;
    use lc631_solvers::runner::{
        ProcessCleanupState as C, ProcessRunState as P, SolverExecutionState as E,
    };
    fn optional(hash: &mut Sha256, bytes: Option<&[u8]>) {
        match bytes {
            None => hash.update([0]),
            Some(b) => {
                hash.update([1]);
                hash.update((b.len() as u64).to_le_bytes());
                hash.update(b);
            }
        }
    }
    let mut hash = Sha256::new();
    hash.update(b"canonical.actual-runner-selected-fields.v1\0");
    hash.update(report.source().as_bytes());
    hash.update(report.query().as_bytes());
    let invocation = report.invocation_digest().map(|d| *d.as_bytes());
    optional(&mut hash, invocation.as_ref().map(|v| v.as_slice()));
    hash.update([match report.state() {
        E::SolverReported => 1,
        E::UnsupportedProfile => 2,
        E::ResourceRejected => 3,
        E::NotStarted => 4,
        E::StartedUnknown => 5,
        E::TimedOut => 6,
        E::Cancelled => 7,
        E::Failed => 8,
        E::OutputTruncated => 9,
        E::CleanupUnknown => 10,
        E::BindingMismatch => 11,
        E::UnsupportedMapping => 12,
        E::ProtocolFailure => 13,
        E::PayloadSyntaxObserved => 14,
        E::PayloadSyntaxRejected => 15,
        E::PayloadWireObserved => 16,
        E::PayloadWireRejected => 17,
        E::AmbiguousRegistration => 18,
    }]);
    hash.update([match report.process_state() {
        None => 0,
        Some(P::Completed) => 1,
        Some(P::NotStarted) => 2,
        Some(P::TimedOut) => 3,
        Some(P::ResourceExhaustedBeforeStart) => 4,
        Some(P::ResourceExhaustedAfterStartUnknown) => 5,
        Some(P::Unsupported) => 6,
        Some(P::Failed) => 7,
    }]);
    hash.update([match report.cleanup_state() {
        C::RunnerReportedNotStarted => 1,
        C::RunnerReportedVerified => 2,
        C::RunnerReportedUnknown => 3,
    }]);
    hash.update([match report.payload_wire_status() {
        None => 0,
        Some(W::Sat) => 1,
        Some(W::Unsat) => 2,
        Some(W::Unknown) => 3,
    }]);
    let exit = report.exit_code().map(i32::to_le_bytes);
    optional(&mut hash, exit.as_ref().map(|v| v.as_slice()));
    let stdout = report.stdout_digest();
    let stderr = report.stderr_digest();
    optional(&mut hash, stdout.as_ref().map(|v| v.as_slice()));
    optional(&mut hash, stderr.as_ref().map(|v| v.as_slice()));
    hash.update((report.stdout_bytes() as u64).to_le_bytes());
    hash.update((report.stderr_bytes() as u64).to_le_bytes());
    hash.update([
        u8::from(report.stdout_truncated()),
        u8::from(report.stderr_truncated()),
        u8::from(report.stdout_fully_observed()),
        u8::from(report.stderr_fully_observed()),
    ]);
    Digest32::from_bytes(hash.finalize().into())
}
