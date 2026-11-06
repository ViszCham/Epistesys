//! 日本語: 不信descriptorと、実producerだけが構築するborrowed ownerを分離する。
//! English: Untrusted descriptors are separate from producer-only borrowed owners.
use super::{compiler_image::BuildImages, BuildError, BuildPlan};
use crate::{
    declared_build_capture::{
        CurrentSnapshotAuthority, OwnedDeclaredBuildCapture, SourceReadBinding,
    },
    ToolProcessRunner,
};
use lc631_core::{ExecutionPermit, ExecutionPermitContext};
use lc631_receipt_kernel::{ReceiptVerifier, UntrustedReceipt};
use lc631_substrate::{
    resource::{ResourceClaim, ResourceReservationView},
    source::SourceStore,
    Digest32,
};
use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StandaloneProfile {
    CheckedU32AddFourV1,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Emission {
    LinkExecutable,
    MetadataOnly,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuildStage {
    CompilerVersion,
    CompilerSysroot,
    Link,
    CurrentOrigin,
}
impl BuildStage {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::CompilerVersion => "compiler-version",
            Self::CompilerSysroot => "compiler-sysroot",
            Self::Link => "link",
            Self::CurrentOrigin => "current-origin",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OriginKind {
    CompilerObservedDeclaredBuild,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputState {
    AbsentBeforeObservedAfterLink,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticAcceptance {
    NotAssessed,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Independence {
    SharedDependency,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HermeticClosure {
    UnobservedExternalToolchain,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceC1 {
    BlockedNeedsLaterGates,
}

#[derive(Clone, Copy)]
pub struct CompilerSpec<'a> {
    pub executable: &'a Path,
    pub expected_image: Digest32,
    pub expected_verbose: Digest32,
    pub declared_sysroot: &'a Path,
    pub toolchain_profile: Digest32,
}
#[derive(Clone, Copy)]
pub struct EnvironmentEntry<'a> {
    pub name: &'a str,
    pub value: &'a str,
}
#[derive(Clone, Copy)]
pub struct ProcessFrame<'a> {
    pub cwd: &'a Path,
    pub environment: &'a [EnvironmentEntry<'a>],
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BuildLimits {
    pub max_builds: u32,
    pub max_children: u32,
    pub max_current_reads: u32,
    pub max_subject_bytes: usize,
    pub max_frame_bytes: usize,
    pub max_artifact_bytes: usize,
    pub max_compiler_bytes: usize,
    pub max_metadata_bytes: usize,
    pub max_environment_bytes: usize,
    pub max_retained_bytes: usize,
    pub child_timeout_ms: u64,
    pub total_wall_ms: u64,
}
pub struct BuildPlanInput<'s> {
    pub binding: &'s SourceReadBinding<'s>,
    pub root: &'s Path,
    pub subject: &'s SourceStore,
    pub subject_relative: &'s Path,
    pub driver_relative: &'s Path,
    pub frame_relative: &'s Path,
    pub output_relative: &'s Path,
    pub entrypoint: &'s str,
    pub profile: StandaloneProfile,
    pub emission: Emission,
    pub compiler: CompilerSpec<'s>,
    pub compiler_registration: &'s UntrustedReceipt,
    pub frame: ProcessFrame<'s>,
    pub limits: BuildLimits,
}
pub struct BuildServices<'local, 'observer> {
    pub source_reader: &'local mut dyn ToolProcessRunner,
    pub capture_authority: &'local mut dyn CurrentSnapshotAuthority,
    pub authority: &'local mut dyn CurrentBuildAuthority,
    pub observer: Option<&'observer mut dyn BuildEventObserver>,
}
pub struct BuildAuthorityView<'a> {
    pub permit: &'a ExecutionPermit,
    pub context: ExecutionPermitContext<'a>,
    pub tool_verifier: &'a ReceiptVerifier,
}
pub trait CurrentBuildAuthority {
    fn current(
        &mut self,
        plan: &BuildPlan<'_, '_>,
        stage: BuildStage,
    ) -> Result<BuildAuthorityView<'_>, BuildError>;
}
pub trait BuildEventObserver {
    fn observed(&mut self, event: BuildEvent<'_>) -> Result<(), BuildError>;
}
pub struct BuildEvent<'a> {
    pub(super) stage: BuildStage,
    pub(super) artifact_fingerprint: Option<Digest32>,
    pub(super) children: &'a [CompilerChildWitness],
}
impl BuildEvent<'_> {
    pub fn children(&self) -> &[CompilerChildWitness] {
        self.children
    }
    pub fn stage(&self) -> BuildStage {
        self.stage
    }
    pub fn artifact_fingerprint(&self) -> Option<Digest32> {
        self.artifact_fingerprint
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct AuthorityStamp {
    pub(super) epoch: u64,
    pub(super) expires: u64,
    pub(super) principal: Digest32,
    pub(super) host: Digest32,
    pub(super) revocation: u64,
}

/// Only the private producer fills these fields from one actual supervised child.
pub struct CompilerChildWitness {
    pub(super) stage: BuildStage,
    pub(super) exit_code: Option<i32>,
    pub(super) cleanup_verified: bool,
    pub(super) frame_digest: Digest32,
    pub(super) invocation_digest: Digest32,
    pub(super) options_digest: Digest32,
    pub(super) source_revision: Digest32,
    pub(super) compiler_digest: Digest32,
    pub(super) cwd_digest: Digest32,
    pub(super) environment_digest: Digest32,
    pub(super) test_scope: String,
    pub(super) test_checked_epoch: u64,
    pub(super) stdout: Vec<u8>,
    pub(super) stderr: Vec<u8>,
}
impl CompilerChildWitness {
    /// Borrow retained captures; never detach them from the funded owner.
    pub fn stdout(&self) -> &[u8] {
        &self.stdout
    }
    pub fn stderr(&self) -> &[u8] {
        &self.stderr
    }
    pub fn stage(&self) -> BuildStage {
        self.stage
    }
    pub fn exit_code(&self) -> Option<i32> {
        self.exit_code
    }
    pub fn cleanup_verified(&self) -> bool {
        self.cleanup_verified
    }
    pub fn frame_digest(&self) -> Digest32 {
        self.frame_digest
    }
    pub fn invocation_digest(&self) -> Digest32 {
        self.invocation_digest
    }
    pub fn options_digest(&self) -> Digest32 {
        self.options_digest
    }
    pub fn source_revision(&self) -> Digest32 {
        self.source_revision
    }
    pub fn compiler_digest(&self) -> Digest32 {
        self.compiler_digest
    }
    pub fn cwd_digest(&self) -> Digest32 {
        self.cwd_digest
    }
    pub fn environment_digest(&self) -> Digest32 {
        self.environment_digest
    }
    pub fn test_scope(&self) -> &str {
        &self.test_scope
    }
    pub fn test_checked_epoch(&self) -> u64 {
        self.test_checked_epoch
    }
}

pub struct OwnedSourceBuildOrigin<'p, 's, 'v> {
    pub(super) plan: &'p BuildPlan<'s, 'v>,
    pub(super) before: OwnedDeclaredBuildCapture<'v>,
    pub(super) after: OwnedDeclaredBuildCapture<'v>,
    pub(super) children: Vec<CompilerChildWitness>,
    pub(super) images: BuildImages,
    pub(super) retained: ResourceClaim<'v>,
}
pub struct OwnedBuildFailure<'p, 's, 'v> {
    // Keep the failed observation tied to its plan lifetime, even without a view.
    pub(super) _plan: &'p BuildPlan<'s, 'v>,
    pub(super) cause: BuildError,
    pub(super) observed_children: usize,
    pub(super) children: Vec<CompilerChildWitness>,
    pub(super) before: Option<OwnedDeclaredBuildCapture<'v>>,
    pub(super) after: Option<OwnedDeclaredBuildCapture<'v>>,
    pub(super) images: Option<BuildImages>,
    pub(super) partial: Option<lc631_process_supervisor::ResourceRunAttempt<'v, BuildError>>,
    pub(super) retained: Option<ResourceClaim<'v>>,
}
impl OwnedBuildFailure<'_, '_, '_> {
    pub fn cause(&self) -> &BuildError {
        &self.cause
    }
    pub fn observed_child_count(&self) -> usize {
        self.observed_children
    }
    pub fn has_origin(&self) -> bool {
        false
    }
}
impl std::fmt::Debug for OwnedBuildFailure<'_, '_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OwnedBuildFailure")
            .field("cause", &self.cause)
            .field("observed_child_count", &self.observed_children)
            .field("has_origin", &false)
            .finish_non_exhaustive()
    }
}
pub struct BuildView<'current> {
    pub(super) plan: &'current BuildPlan<'current, 'current>,
    pub(super) before: &'current OwnedDeclaredBuildCapture<'current>,
    pub(super) after: &'current OwnedDeclaredBuildCapture<'current>,
    pub(super) children: &'current [CompilerChildWitness],
    pub(super) artifact_digest: Digest32,
}
impl<'c> BuildView<'c> {
    pub fn kind(&self) -> OriginKind {
        OriginKind::CompilerObservedDeclaredBuild
    }
    pub fn output_state(&self) -> OutputState {
        OutputState::AbsentBeforeObservedAfterLink
    }
    pub fn subject_digest(&self) -> Digest32 {
        self.plan.subject_digest
    }
    pub fn driver_digest(&self) -> Digest32 {
        self.plan.driver_digest
    }
    pub fn entrypoint(&self) -> &'c str {
        self.plan.input.entrypoint
    }
    pub fn frame_digest(&self) -> Digest32 {
        self.plan.frame_digest()
    }
    pub fn before_source(&self) -> &'c OwnedDeclaredBuildCapture<'c> {
        self.before
    }
    pub fn after_source(&self) -> &'c OwnedDeclaredBuildCapture<'c> {
        self.after
    }
    pub fn root_digest(&self) -> Digest32 {
        self.plan.root_digest()
    }
    pub fn program_digest(&self) -> Digest32 {
        self.plan.program_digest()
    }
    pub fn plan_digest(&self) -> Digest32 {
        self.plan.plan_digest()
    }
    pub fn compiler_digest(&self) -> Digest32 {
        self.plan.input.compiler.expected_image
    }
    pub fn verbose_digest(&self) -> Digest32 {
        self.plan.input.compiler.expected_verbose
    }
    pub fn sysroot(&self) -> &'c Path {
        self.plan.input.compiler.declared_sysroot
    }
    pub fn cwd(&self) -> &'c Path {
        self.plan.input.frame.cwd
    }
    pub fn environment_digest(&self) -> Digest32 {
        self.plan.environment_digest()
    }
    pub fn artifact_path(&self) -> &'c Path {
        &self.plan.output_path
    }
    pub fn artifact_digest(&self) -> Digest32 {
        self.artifact_digest
    }
    pub fn children(&self) -> &'c [CompilerChildWitness] {
        self.children
    }
    pub fn semantic_acceptance(&self) -> SemanticAcceptance {
        SemanticAcceptance::NotAssessed
    }
    pub fn independence(&self) -> Independence {
        Independence::SharedDependency
    }
    pub fn hermetic_closure(&self) -> HermeticClosure {
        HermeticClosure::UnobservedExternalToolchain
    }
    pub fn source_goal(&self) -> Option<Digest32> {
        None
    }
    pub fn source_c1(&self) -> SourceC1 {
        SourceC1::BlockedNeedsLaterGates
    }
}

pub(super) fn check_claim(
    claim: &ResourceClaim<'_>,
    view: &ResourceReservationView<'_>,
) -> Result<(), BuildError> {
    view.check()?;
    claim.check()?;
    if !claim.same_reservation(view) {
        return Err(lc631_substrate::resource::ResourceError::ForeignOwner.into());
    }
    Ok(())
}
