//! 宣言Rust/Cargo入力の実再取得。fresh Test権限と内容identityを分離する。
//! Actual declared-input recapture; fresh Test authority is separate from content.
#![forbid(unsafe_code)]
mod inventory;
use crate::dgcl_pipeline::goal_input_admission;
use crate::{DgclExecutionSnapshot, DgclPipelineReport, ToolProcessRunner};
use lc631_core::{
    Action, AuthorityRevision, ExecutionPermit, ExecutionPermitContext, PermitValidation,
    PrincipalBinding,
};
use lc631_process_supervisor::filesystem::DirectoryAnchor;
use lc631_substrate::{
    resource::{
        ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
        ResourceReservationView,
    },
    Digest32,
};
use sha2::{Digest, Sha256};
use std::path::Path;

pub const DECLARED_CAPTURE_SCRATCH_RAM: u64 = 8 * 1024 * 1024;
pub const DECLARED_CAPTURE_RETAINED_RAM: u64 = 4096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SnapshotCaptureError {
    BindingMismatch,
    MissingOriginalRoot,
    PipelineIdentity,
    RootMismatch,
    UnsafePath,
    Unauthorized,
    AuthorityUnavailable,
    ClockRollback,
    RevocationRollback,
    PrincipalChanged,
    InventoryLimit,
    InvalidSnapshot,
    MissingRunner,
    ContentMismatch,
    Resource(ResourceError),
}
impl From<ResourceError> for SnapshotCaptureError {
    fn from(value: ResourceError) -> Self {
        Self::Resource(value)
    }
}

/// Immutable read descriptor, not an execution permit or historical receipt.
/// Bridges must derive expected pins from accepted31 validation/checked32 history.
/// `checked` only admits bounded metadata without I/O/allocation. Full pipeline
/// seal/root checks happen inside the funded capture, before inventory traversal.
pub struct SourceReadBinding<'a> {
    root_path: &'a Path,
    source_text: &'a str,
    pipeline: &'a DgclPipelineReport,
    root: Digest32,
    source: Digest32,
    program: Digest32,
    plan: Digest32,
}
impl<'a> SourceReadBinding<'a> {
    pub fn checked(
        root: &'a Path,
        source: &'a str,
        pipeline: &'a DgclPipelineReport,
        expected_root: Digest32,
        expected_program: Digest32,
        expected_plan: Digest32,
    ) -> Result<Self, SnapshotCaptureError> {
        if !root.is_absolute() || root.as_os_str().len() > 4096 || source.len() > 262144 {
            return Err(SnapshotCaptureError::BindingMismatch);
        }
        let source_digest = Digest32::from_bytes(Sha256::digest(source.as_bytes()).into());
        if declared_capture_digest(&pipeline.source_revision)? != source_digest
            || declared_capture_digest(&pipeline.program_ir.program_digest)? != expected_program
            || declared_capture_digest(&pipeline.completion_plan.plan_digest)? != expected_plan
        {
            return Err(SnapshotCaptureError::BindingMismatch);
        }
        Ok(Self {
            root_path: root,
            source_text: source,
            pipeline,
            root: expected_root,
            source: source_digest,
            program: expected_program,
            plan: expected_plan,
        })
    }
    /// Bounds admission for the narrow Goal profile only, not identity,
    /// provenance, meaning, authority or a declared-build capture.
    pub fn check_goal_profile_inputs(
        &self,
        resources: &ResourceReservationView<'_>,
    ) -> Result<(), SnapshotCaptureError> {
        resources.check()?;
        if self.source_text.len() > 262_144 {
            return Err(SnapshotCaptureError::InventoryLimit);
        }
        if resources.source_revision() != self.source {
            return Err(SnapshotCaptureError::BindingMismatch);
        }
        let scratch = claim(resources, 64 * 1024)?;
        goal_input_admission::check(self.pipeline, resources)?;
        scratch.check()?;
        Ok(())
    }

    pub const fn root(&self) -> Digest32 {
        self.root
    }
    pub const fn source(&self) -> Digest32 {
        self.source
    }
    pub const fn program(&self) -> Digest32 {
        self.program
    }
    pub const fn plan(&self) -> Digest32 {
        self.plan
    }
    /// Bounded grant-registration metadata only; no permission is issued here.
    pub fn execution_scope(&self) -> String {
        let mut hash = Sha256::new();
        hash.update(b"Epistesys/DeclaredBuildRead/Descriptor/v1\0");
        for pin in [self.root, self.source, self.program, self.plan] {
            hash.update(pin.as_bytes());
        }
        let mut scope = String::with_capacity(100);
        scope.push_str("tlcx/declared-build/read/v1/");
        for byte in hash.finalize() {
            use std::fmt::Write as _;
            let _ = write!(scope, "{byte:02x}");
        }
        scope
    }
}

/// A trusted host/caller must supply a genuinely fresh context on EACH call,
/// including current clock and revocation data. Known-empty revocations are
/// valid input; this module never substitutes an empty/default/historical view.
pub trait CurrentSnapshotAuthority {
    fn current(&mut self) -> Result<SnapshotAuthorityView<'_>, SnapshotCaptureError>;
}
pub struct SnapshotAuthorityView<'a> {
    pub permit: &'a ExecutionPermit,
    pub context: ExecutionPermitContext<'a>,
    /// Explicit fresh caller pin, never inherited from historical Edit authority.
    pub expected_principal: &'a PrincipalBinding,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SnapshotAuthorityStamp {
    pub principal: Digest32,
    pub host: Digest32,
    pub authority_revision: AuthorityRevision,
    pub revocation_revision: u64,
    pub checked_epoch: u64,
}

/// Caller-reported expected values may be parsed here, but only real inventory
/// execution constructs an OwnedDeclaredBuildCapture. No authority digest or
/// whole snapshot digest participates in this content-only equality relation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeclaredBuildContentPins {
    pub source: Digest32,
    pub build: Digest32,
    pub profile: Digest32,
    pub validator: Digest32,
    pub targets: Digest32,
}
impl DeclaredBuildContentPins {
    pub fn from_snapshot(snapshot: &DgclExecutionSnapshot) -> Result<Self, SnapshotCaptureError> {
        if snapshot.schema_version != crate::DGCL_EVIDENCE_LIFECYCLE_SCHEMA {
            return Err(SnapshotCaptureError::InvalidSnapshot);
        }
        Ok(Self {
            source: declared_capture_digest(&snapshot.source_revision)?,
            build: declared_capture_digest(&snapshot.build_input_digest)?,
            profile: declared_capture_digest(&snapshot.profile_digest)?,
            validator: declared_capture_digest(&snapshot.validator_digest)?,
            targets: declared_capture_digest(&snapshot.targets_digest)?,
        })
    }
}

/// Private construction, no Clone/Deserialize/raw report admission. Fixed
/// content pins survive late failures; historical access never restores a grant.
pub struct OwnedDeclaredBuildCapture<'res> {
    pins: DeclaredBuildContentPins,
    root: Digest32,
    program: Digest32,
    plan: Digest32,
    snapshot: Digest32,
    before: SnapshotAuthorityStamp,
    after: Option<SnapshotAuthorityStamp>,
    current_error: Option<SnapshotCaptureError>,
    // Last: drop all owned metadata before releasing retained accounting.
    retained: ResourceClaim<'res>,
}
impl OwnedDeclaredBuildCapture<'_> {
    /// Historical numeric identities only, including when a post-check failed.
    pub const fn content_pins(&self) -> DeclaredBuildContentPins {
        self.pins
    }
    pub const fn root(&self) -> Digest32 {
        self.root
    }
    pub const fn program(&self) -> Digest32 {
        self.program
    }
    pub const fn plan(&self) -> Digest32 {
        self.plan
    }
    pub const fn snapshot_digest(&self) -> Digest32 {
        self.snapshot
    }
    pub const fn authority_before(&self) -> SnapshotAuthorityStamp {
        self.before
    }
    pub const fn authority_after(&self) -> Option<SnapshotAuthorityStamp> {
        self.after
    }
    fn live(&self) -> Result<(), SnapshotCaptureError> {
        if let Some(error) = &self.current_error {
            return Err(error.clone());
        }
        self.retained.check()?;
        Ok(())
    }
    pub fn check_current(
        &self,
        resources: &ResourceReservationView<'_>,
    ) -> Result<(), SnapshotCaptureError> {
        self.live()?;
        resources.check()?;
        if !self.retained.same_reservation(resources)
            || resources.source_revision() != self.pins.source
        {
            return Err(SnapshotCaptureError::BindingMismatch);
        }
        Ok(())
    }
    pub fn matches_expected(
        &self,
        expected: DeclaredBuildContentPins,
    ) -> Result<(), SnapshotCaptureError> {
        self.live()?;
        if self.pins != expected {
            return Err(SnapshotCaptureError::ContentMismatch);
        }
        Ok(())
    }
    pub const fn claim_boundary(&self) -> &'static str {
        "current-at-capture declared Rust/Cargo content and explicit fresh Test checks only; SourceTarget profile, non-hermetic dependencies/build scripts/includes and mixed-read/subpath TOCTOU remain; no action/completion/replay authority or atomic filesystem lease"
    }
}

/// Completed inventory observations remain owned if a later check fails. A
/// failed precheck/collector has no fabricated actual capture. No caller setter
/// can inject a snapshot, completion bit, current owner or success report.
pub struct SnapshotCaptureAttempt<'res> {
    capture: Option<OwnedDeclaredBuildCapture<'res>>,
    diagnostic: Option<SnapshotCaptureError>,
}
impl<'res> SnapshotCaptureAttempt<'res> {
    pub fn failure(error: SnapshotCaptureError) -> Self {
        Self {
            capture: None,
            diagnostic: Some(error),
        }
    }
    pub fn capture(&self) -> Option<&OwnedDeclaredBuildCapture<'res>> {
        self.capture.as_ref()
    }
    pub fn diagnostic(&self) -> Option<&SnapshotCaptureError> {
        self.diagnostic.as_ref()
    }
    /// Liveness plus the completed capture's pre/post checks; not a fresh reread.
    pub fn current(&self) -> Result<&OwnedDeclaredBuildCapture<'res>, SnapshotCaptureError> {
        if let Some(error) = &self.diagnostic {
            return Err(error.clone());
        }
        let capture = self
            .capture
            .as_ref()
            .ok_or(SnapshotCaptureError::InvalidSnapshot)?;
        capture.live()?;
        Ok(capture)
    }
    pub fn into_parts(
        self,
    ) -> (
        Option<OwnedDeclaredBuildCapture<'res>>,
        Option<SnapshotCaptureError>,
    ) {
        (self.capture, self.diagnostic)
    }
}

/// The only actual owner producer. Both claims precede pipeline verification,
/// root I/O and collector map/JSON/target allocation. These are logical payload
/// reservations, not a total allocator/RSS or hermetic-build guarantee.
pub fn capture_declared_build<'res>(
    binding: &SourceReadBinding<'_>,
    runner: &mut dyn ToolProcessRunner,
    authority: &mut dyn CurrentSnapshotAuthority,
    resources: &'res ResourceReservationView<'_>,
) -> SnapshotCaptureAttempt<'res> {
    let retained = match claim(resources, DECLARED_CAPTURE_RETAINED_RAM) {
        Ok(value) => value,
        Err(error) => return SnapshotCaptureAttempt::failure(error),
    };
    let scratch = match claim(resources, DECLARED_CAPTURE_SCRATCH_RAM) {
        Ok(value) => value,
        Err(error) => return SnapshotCaptureAttempt::failure(error),
    };
    let mut attempt = SnapshotCaptureAttempt {
        capture: None,
        diagnostic: None,
    };
    let mut anchor = None;
    let result = (|| {
        resources.check()?;
        if resources.source_revision() != binding.source {
            return Err(SnapshotCaptureError::BindingMismatch);
        }
        if !crate::verify_dgcl_pipeline_identity(binding.source_text, binding.pipeline) {
            return Err(SnapshotCaptureError::PipelineIdentity);
        }
        // Do not hold a provider view across its next mutable current() call.
        {
            let view = authority.current()?;
            let before = check_authority(binding, &view, None)?;
            let root_anchor = DirectoryAnchor::open_existing(binding.root_path)
                .map_err(|_| SnapshotCaptureError::UnsafePath)?;
            check_root(binding, &root_anchor)?;
            let snapshot = inventory::collect(
                binding.root_path,
                binding.source_text,
                binding.pipeline,
                &view.context,
                runner,
                resources,
            )
            .map_err(map_inventory)?;
            let pins = DeclaredBuildContentPins::from_snapshot(&snapshot)?;
            let snapshot_digest = declared_capture_digest(&snapshot.snapshot_digest)?;
            attempt.capture = Some(OwnedDeclaredBuildCapture {
                pins,
                root: binding.root,
                program: binding.program,
                plan: binding.plan,
                snapshot: snapshot_digest,
                before,
                after: None,
                current_error: Some(SnapshotCaptureError::AuthorityUnavailable),
                retained,
            });
            anchor = Some(root_anchor);
            // Drop the full map-derived snapshot while scratch is still held.
        }
        let captured = attempt
            .capture
            .as_mut()
            .ok_or(SnapshotCaptureError::InvalidSnapshot)?;
        check_root(
            binding,
            anchor.as_ref().ok_or(SnapshotCaptureError::UnsafePath)?,
        )?;
        let view = authority.current()?;
        captured.after = Some(check_authority(binding, &view, Some(captured.before))?);
        resources.check()?;
        scratch.check()?;
        captured.current_error = None;
        Ok(())
    })();
    if let Err(error) = result {
        if let Some(captured) = attempt.capture.as_mut() {
            captured.current_error = Some(error.clone());
        }
        attempt.diagnostic = Some(error);
    }
    attempt
}

fn check_authority(
    binding: &SourceReadBinding<'_>,
    view: &SnapshotAuthorityView<'_>,
    previous: Option<SnapshotAuthorityStamp>,
) -> Result<SnapshotAuthorityStamp, SnapshotCaptureError> {
    let context = &view.context;
    if context.principal != view.expected_principal
        || context.action != Action::Test
        || context.scope != binding.execution_scope()
        || declared_capture_digest(context.source_revision)? != binding.source
        || view.permit.action() != Action::Test
        || view.permit.validate(context) != PermitValidation::ValidGrant
    {
        return Err(SnapshotCaptureError::Unauthorized);
    }
    let stamp = SnapshotAuthorityStamp {
        principal: declared_capture_digest(view.expected_principal.as_str())?,
        host: declared_capture_digest(context.trusted_host_fingerprint)?,
        authority_revision: context.authority_revision,
        revocation_revision: context.revocation_revision,
        checked_epoch: context.now_epoch,
    };
    if let Some(prior) = previous {
        if stamp.principal != prior.principal || stamp.host != prior.host {
            return Err(SnapshotCaptureError::PrincipalChanged);
        }
        if stamp.checked_epoch < prior.checked_epoch {
            return Err(SnapshotCaptureError::ClockRollback);
        }
        if stamp.revocation_revision < prior.revocation_revision {
            return Err(SnapshotCaptureError::RevocationRollback);
        }
    }
    Ok(stamp)
}
fn check_root(
    binding: &SourceReadBinding<'_>,
    anchor: &DirectoryAnchor,
) -> Result<(), SnapshotCaptureError> {
    anchor
        .validate_current()
        .map_err(|_| SnapshotCaptureError::UnsafePath)?;
    let actual = crate::dgcl_repository_root_digest(binding.root_path)
        .map_err(|_| SnapshotCaptureError::UnsafePath)?;
    if declared_capture_digest(&actual)? != binding.root {
        return Err(SnapshotCaptureError::RootMismatch);
    }
    anchor
        .validate_current()
        .map_err(|_| SnapshotCaptureError::UnsafePath)
}
fn claim<'res>(
    resources: &'res ResourceReservationView<'_>,
    bytes: u64,
) -> Result<ResourceClaim<'res>, SnapshotCaptureError> {
    Ok(resources.claim_resources(&[ResourceRequest::new(
        ResourceKind::Ram,
        ResourceAmount::checked(bytes)?,
    )])?)
}
fn map_inventory(error: inventory::InventoryError) -> SnapshotCaptureError {
    match error {
        inventory::InventoryError::Resource(error) => SnapshotCaptureError::Resource(error),
        inventory::InventoryError::UnsafePath => SnapshotCaptureError::UnsafePath,
        inventory::InventoryError::Limit => SnapshotCaptureError::InventoryLimit,
        inventory::InventoryError::Snapshot => SnapshotCaptureError::InvalidSnapshot,
    }
}
pub fn declared_capture_digest(value: &str) -> Result<Digest32, SnapshotCaptureError> {
    let hex = value
        .strip_prefix("sha256:")
        .ok_or(SnapshotCaptureError::BindingMismatch)?;
    if hex.len() != 64 {
        return Err(SnapshotCaptureError::BindingMismatch);
    }
    let mut bytes = [0u8; 32];
    for (index, pair) in hex.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        let digit = |b: u8| match b {
            b'0'..=b'9' => Ok(b - b'0'),
            b'a'..=b'f' => Ok(b - b'a' + 10),
            _ => Err(SnapshotCaptureError::BindingMismatch),
        };
        bytes[index] = digit(pair[0])? * 16 + digit(pair[1])?;
    }
    Ok(Digest32::from_bytes(bytes))
}
