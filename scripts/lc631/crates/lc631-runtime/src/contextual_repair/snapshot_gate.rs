//! Accepted content identities and an actual read port; neither grants Test.
#![forbid(unsafe_code)]
use lc631_analysis::{DgclPipelineReport, DgclVerifiedRepairValidation, ToolProcessRunner};
use lc631_analysis::declared_build_capture::{capture_declared_build, declared_capture_digest,
    CurrentSnapshotAuthority, DeclaredBuildContentPins, SnapshotCaptureAttempt,
    SnapshotCaptureError, SourceReadBinding};
use lc631_substrate::{Digest32, resource::ResourceReservationView};
use std::path::Path;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SnapshotGateError {
    Capture(SnapshotCaptureError), MissingOriginalRoot, BindingMismatch,
    ContentMismatch, MissingBuildAuthority,
}
impl From<SnapshotCaptureError> for SnapshotGateError {
    fn from(error: SnapshotCaptureError) -> Self { Self::Capture(error) }
}

/// Borrow an immutable concrete read configuration and its existing funded
/// runner. The live file driver uses its own runner via the narrow forwarding
/// method instead; no concurrent alias or deep pipeline clone is required.
pub struct CodingBuildPort<'ctx, 'runner> {
    binding: SourceReadBinding<'ctx>,
    runner: &'runner mut dyn ToolProcessRunner,
}
impl<'ctx, 'runner> CodingBuildPort<'ctx, 'runner> {
    #[allow(clippy::too_many_arguments)]
    pub fn checked(root: &'ctx Path, source: &'ctx str, pipeline: &'ctx DgclPipelineReport,
        expected_root: Digest32, expected_program: Digest32, expected_plan: Digest32,
        runner: &'runner mut dyn ToolProcessRunner) -> Result<Self, SnapshotGateError> {
        Ok(Self { binding: SourceReadBinding::checked(root, source, pipeline,
            expected_root, expected_program, expected_plan)?, runner })
    }
    pub fn binding(&self) -> &SourceReadBinding<'ctx> { &self.binding }
    /// Admit only the bounded Goal input profile before legacy serialization.
    /// This issues no build, semantic, authority or source-origin evidence.
    pub(crate) fn check_goal_profile_inputs(
        &self,
        resources: &ResourceReservationView<'_>,
    ) -> Result<(), SnapshotGateError> {
        self.binding.check_goal_profile_inputs(resources)?;
        Ok(())
    }
    pub fn capture_current<'res>(&mut self, authority: &mut dyn CurrentSnapshotAuthority,
        resources: &'res ResourceReservationView<'_>) -> SnapshotCaptureAttempt<'res> {
        capture_declared_build(&self.binding, self.runner, authority, resources)
    }
}

/// Private origin: only the accepted validation callback or checked signed
/// transition supplies expected pins. A drift capture never replaces them.
#[derive(Clone, Copy)]
pub struct ExpectedCurrentBuild {
    root: Digest32, program: Digest32, plan: Digest32, content: DeclaredBuildContentPins,
}
impl ExpectedCurrentBuild {
    pub fn from_verified_validation(token: &DgclVerifiedRepairValidation<'_>, program: Digest32)
        -> Result<Self, SnapshotGateError> {
        let request = token.request();
        let root = request.repository_root_digest.as_deref()
            .ok_or(SnapshotGateError::MissingOriginalRoot)?;
        let content = DeclaredBuildContentPins::from_snapshot(&token.application().after_snapshot)?;
        if content.source != declared_capture_digest(&request.source_revision)? {
            return Err(SnapshotGateError::BindingMismatch);
        }
        Ok(Self { root: declared_capture_digest(root)?, program,
            plan: declared_capture_digest(&request.plan_digest)?, content })
    }
    pub fn from_verified_history(history: &crate::contextual_resume::transition::HistoricalRepairTransition,
        port: &CodingBuildPort<'_, '_>) -> Result<Self, SnapshotGateError> {
        let root = history.original_request_root().ok_or(SnapshotGateError::MissingOriginalRoot)?;
        let binding = port.binding();
        if binding.root() != root || binding.source() != history.task_source()
            || binding.plan() != history.original_plan_digest() {
            return Err(SnapshotGateError::BindingMismatch);
        }
        Ok(Self { root, program: binding.program(), plan: history.original_plan_digest(),
            content: DeclaredBuildContentPins { source: history.task_source(), build: history.after_build(),
                profile: history.after_profile(), validator: history.after_validator(), targets: history.after_targets() } })
    }
    pub const fn root(&self) -> Digest32 { self.root }
    pub const fn program(&self) -> Digest32 { self.program }
    pub const fn plan(&self) -> Digest32 { self.plan }
    pub fn check_attempt(&self, attempt: &SnapshotCaptureAttempt<'_>,
        resources: &ResourceReservationView<'_>) -> Result<(), SnapshotGateError> {
        // A retained actual owner with a late authority error is historical,
        // even when its numeric content still equals the accepted content.
        let observed = attempt.current()?;
        observed.check_current(resources)?;
        if observed.root() != self.root || observed.program() != self.program || observed.plan() != self.plan {
            return Err(SnapshotGateError::BindingMismatch);
        }
        observed.matches_expected(self.content).map_err(|error| match error {
            SnapshotCaptureError::ContentMismatch => SnapshotGateError::ContentMismatch,
            error => SnapshotGateError::Capture(error),
        })
    }
}
