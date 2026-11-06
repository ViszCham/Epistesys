//! Read-only recapture through the concrete owner's original configuration.
use super::DgclFileRepairDriver;
use crate::declared_build_capture::{
    capture_declared_build, declared_capture_digest, CurrentSnapshotAuthority,
    SnapshotCaptureAttempt, SnapshotCaptureError, SourceReadBinding,
};
use lc631_substrate::{resource::ResourceReservationView, Digest32};

impl DgclFileRepairDriver<'_> {
    pub fn declared_program_digest(&self) -> Result<Digest32, SnapshotCaptureError> {
        declared_capture_digest(&self.config.pipeline.program_ir.program_digest)
    }

    /// No apply, journal append, validation receipt, refund or cached snapshot.
    /// Expected identities must come from accepted application/history evidence.
    pub fn recapture_current_build<'res>(
        &mut self,
        expected_root: Digest32,
        expected_program: Digest32,
        expected_plan: Digest32,
        authority: &mut dyn CurrentSnapshotAuthority,
        resources: &'res ResourceReservationView<'_>,
    ) -> SnapshotCaptureAttempt<'res> {
        let binding = match SourceReadBinding::checked(
            self.config.root,
            self.config.source,
            self.config.pipeline,
            expected_root,
            expected_program,
            expected_plan,
        ) {
            Ok(binding) => binding,
            Err(error) => return SnapshotCaptureAttempt::failure(error),
        };
        match self.runner.as_deref_mut() {
            Some(runner) => capture_declared_build(&binding, runner, authority, resources),
            None => SnapshotCaptureAttempt::failure(SnapshotCaptureError::MissingRunner),
        }
    }
}
