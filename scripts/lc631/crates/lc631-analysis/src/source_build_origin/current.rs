//! Fresh bounded current views; retained reports never renew authority or spawn.
#![forbid(unsafe_code)]

use super::{
    check_claim, producer, BuildError, BuildPlan, BuildServices, BuildStage, BuildView,
    OwnedSourceBuildOrigin,
};
use crate::declared_build_capture::{OwnedDeclaredBuildCapture, SnapshotCaptureError};
use lc631_substrate::resource::ResourceReservationView;

impl OwnedSourceBuildOrigin<'_, '_, '_> {
    pub fn with_current<R>(
        &self, plan: &BuildPlan<'_, '_>, services: &mut BuildServices<'_, '_>,
        view: &ResourceReservationView<'_>,
        consume: impl for<'current> FnOnce(BuildView<'current>) -> R,
    ) -> Result<R, BuildError> {
        check_claim(&self.retained, view)?;
        if !std::ptr::eq(self.plan, plan) { return Err(BuildError::FrameMismatch); }
        // One request consumes one read, including a later failed check. There
        // is no public scalar/current setter and no refund on a rejected read.
        self.plan.debit_current(view)?;
        self.check_current(services, view)?;
        let result = consume(BuildView {
            plan: self.plan,
            before: &self.before,
            after: &self.after,
            children: &self.children,
            artifact_digest: self.images.artifact_digest()?,
        });
        self.check_current(services, view)?;
        Ok(result)
    }

    fn check_current(
        &self, services: &mut BuildServices<'_, '_>, view: &ResourceReservationView<'_>,
    ) -> Result<(), BuildError> {
        check_claim(&self.retained, view)?;
        self.plan.check(view)?;
        // This is the live provider, including registration/revocation/expiry
        // and the plan's monotone epoch state, never an old child witness.
        producer::check_authority(self.plan, services, BuildStage::CurrentOrigin, view)?;
        self.check_witnesses()?;
        self.before.check_current(view)?;
        self.after.check_current(view)?;
        compare_capture(&self.before, &self.after)?;

        // Reacquire the actual declared Rust/Cargo inventory without compiling.
        // Preserve any producer diagnostic instead of blessing an Option owner.
        let attempt = producer::capture_source(self.plan, services, view);
        let fresh = attempt.current()?;
        fresh.check_current(view)?;
        compare_capture(&self.after, fresh)?;
        self.images.check_current(self.plan, view)?;
        producer::check_authority(self.plan, services, BuildStage::CurrentOrigin, view)?;
        check_claim(&self.retained, view)?;
        self.plan.check(view)
    }

    fn check_witnesses(&self) -> Result<(), BuildError> {
        let stages = [BuildStage::CompilerVersion, BuildStage::CompilerSysroot, BuildStage::Link];
        if self.children.len() != stages.len() { return Err(BuildError::BuildFailed); }
        for (child, stage) in self.children.iter().zip(stages) {
            if child.stage != stage || child.exit_code != Some(0) || !child.cleanup_verified {
                return Err(BuildError::BuildFailed);
            }
            if child.frame_digest != self.plan.frame_digest()
                || child.invocation_digest != self.plan.invocation_digest(stage)
                || child.options_digest != self.plan.options_digest(stage)
                || child.source_revision != self.plan.input.binding.source()
                || child.compiler_digest != self.plan.input.compiler.expected_image
                || child.cwd_digest != self.plan.cwd_digest()
                || child.environment_digest != self.plan.environment_digest()
                || child.test_scope != self.plan.execution_scope(stage)
            { return Err(BuildError::FrameMismatch); }
        }
        Ok(())
    }
}

fn compare_capture(
    previous: &OwnedDeclaredBuildCapture<'_>, current: &OwnedDeclaredBuildCapture<'_>,
) -> Result<(), BuildError> {
    if previous.root() != current.root() || previous.program() != current.program()
        || previous.plan() != current.plan() || previous.content_pins() != current.content_pins()
    { return Err(BuildError::SourceDrift); }
    let before = previous.authority_after().ok_or(BuildError::Capture(SnapshotCaptureError::InvalidSnapshot))?;
    let current_before = current.authority_before();
    let current_after = current.authority_after().ok_or(BuildError::Capture(SnapshotCaptureError::InvalidSnapshot))?;
    for stamp in [current_before, current_after] {
        if stamp.principal != before.principal || stamp.host != before.host {
            return Err(BuildError::Capture(SnapshotCaptureError::PrincipalChanged));
        }
        if stamp.checked_epoch < before.checked_epoch { return Err(BuildError::ClockRollback); }
        if stamp.revocation_revision < before.revocation_revision {
            return Err(BuildError::RevocationRollback);
        }
    }
    Ok(())
}
