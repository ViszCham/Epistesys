//! Actual anchored images for one declared build; no compiler is run by these reads.
#![forbid(unsafe_code)]

use super::{BuildError, BuildPlan};
use crate::declared_build_capture::declared_capture_digest;
use lc631_process_supervisor::filesystem::{DirectoryAnchor, FileIdentity};
use lc631_substrate::{
    resource::{ResourceAmount, ResourceKind, ResourceRequest, ResourceReservationView},
    Digest32,
};
use sha2::{Digest, Sha256};
use std::{
    fs::Metadata,
    io::Read,
    path::{Path, PathBuf},
};

const READ_BYTES: usize = 64 * 1024;
const HEADER_BYTES: usize = 4096;

#[derive(Clone, Copy, Eq, PartialEq)]
struct Fingerprint {
    identity: FileIdentity,
    length: u64,
    digest: Digest32,
}

#[derive(Clone, Copy)]
enum ImageKind {
    Source,
    Compiler,
    Artifact,
}
impl ImageKind {
    fn mismatch(self) -> BuildError {
        match self {
            Self::Source => BuildError::SourceDrift,
            Self::Compiler => BuildError::CompilerRegistrationMismatch,
            Self::Artifact => BuildError::ArtifactDrift,
        }
    }
}

struct InputImage {
    parent: DirectoryAnchor,
    leaf: PathBuf,
    observed: Fingerprint,
    cap: usize,
    kind: ImageKind,
}
impl InputImage {
    fn capture(
        path: &Path,
        expected: Digest32,
        cap: usize,
        kind: ImageKind,
        view: &ResourceReservationView<'_>,
    ) -> Result<Self, BuildError> {
        let parent =
            DirectoryAnchor::open_existing(path.parent().ok_or(BuildError::FrameMismatch)?)?;
        let leaf = PathBuf::from(path.file_name().ok_or(BuildError::FrameMismatch)?);
        let observed = fingerprint(&parent, &leaf, cap, kind, view)?;
        if observed.digest != expected {
            return Err(kind.mismatch());
        }
        Ok(Self {
            parent,
            leaf,
            observed,
            cap,
            kind,
        })
    }

    fn check(&self, view: &ResourceReservationView<'_>) -> Result<(), BuildError> {
        if fingerprint(&self.parent, &self.leaf, self.cap, self.kind, view)? != self.observed {
            return Err(self.kind.mismatch());
        }
        Ok(())
    }
}

/// Directory handles retain path ancestry; file handles are fresh and read-only.
/// Closing leaf handles after each read allows the real append-drift cuts while
/// keeping the original identity, length and digest for subsequent comparison.
pub(super) struct BuildImages {
    root: DirectoryAnchor,
    cwd: DirectoryAnchor,
    sysroot: DirectoryAnchor,
    subject: InputImage,
    driver: InputImage,
    frame: InputImage,
    compiler: InputImage,
    output_parent: DirectoryAnchor,
    output_leaf: PathBuf,
    artifact: Option<Fingerprint>,
    artifact_cap: usize,
    cwd_digest: Digest32,
    environment_digest: Digest32,
}
impl BuildImages {
    pub(super) fn capture_inputs(
        plan: &BuildPlan<'_, '_>,
        view: &ResourceReservationView<'_>,
    ) -> Result<Self, BuildError> {
        plan.check(view)?;
        // The plan's retained claim funds bounded descriptor/handle bookkeeping.
        // A separate actual-view claim precedes all streaming buffers below.
        let root = DirectoryAnchor::open_existing(plan.input.root)?;
        let cwd = DirectoryAnchor::open_existing(plan.input.frame.cwd)?;
        let sysroot = DirectoryAnchor::open_existing(plan.input.compiler.declared_sysroot)?;
        let subject = InputImage::capture(
            &plan.subject_path,
            plan.subject_digest,
            plan.input.limits.max_subject_bytes,
            ImageKind::Source,
            view,
        )?;
        let driver = InputImage::capture(
            &plan.driver_path,
            plan.driver_digest,
            plan.input.limits.max_frame_bytes,
            ImageKind::Source,
            view,
        )?;
        let frame = InputImage::capture(
            &plan.frame_path,
            plan.frame_digest,
            plan.input.limits.max_frame_bytes,
            ImageKind::Source,
            view,
        )?;
        let compiler = InputImage::capture(
            plan.input.compiler.executable,
            plan.input.compiler.expected_image,
            plan.input.limits.max_compiler_bytes,
            ImageKind::Compiler,
            view,
        )?;
        let output_parent = DirectoryAnchor::open_existing(
            plan.output_path.parent().ok_or(BuildError::FrameMismatch)?,
        )?;
        let output_leaf = PathBuf::from(
            plan.output_path
                .file_name()
                .ok_or(BuildError::FrameMismatch)?,
        );
        let images = Self {
            root,
            cwd,
            sysroot,
            subject,
            driver,
            frame,
            compiler,
            output_parent,
            output_leaf,
            artifact: None,
            artifact_cap: plan.input.limits.max_artifact_bytes,
            cwd_digest: plan.cwd_digest,
            environment_digest: plan.environment_digest,
        };
        images.check_context(plan, view)?;
        Ok(images)
    }

    fn check_context(
        &self,
        plan: &BuildPlan<'_, '_>,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), BuildError> {
        plan.check(view)?;
        // Match the lexical absolute/component walk used by DirectoryAnchor;
        // this is not a canonicalized-path substitute for the handle checks.
        if self.root.path() != absolute_components(plan.input.root)?.as_path()
            || self.cwd.path() != absolute_components(plan.input.frame.cwd)?.as_path()
            || self.sysroot.path()
                != absolute_components(plan.input.compiler.declared_sysroot)?.as_path()
            || self.cwd_digest != plan.cwd_digest
            || self.environment_digest != plan.environment_digest
        {
            return Err(BuildError::FrameMismatch);
        }
        self.root.validate_current()?;
        let root_digest = crate::dgcl_repository_root_digest(self.root.path())
            .map_err(|_| BuildError::FrameMismatch)?;
        if declared_capture_digest(&root_digest)? != plan.root_digest() {
            return Err(BuildError::FrameMismatch);
        }
        self.cwd.validate_current()?;
        // This checks the declared sysroot's directory identity, not the full
        // external standard-library/linker closure or compiler honesty.
        self.sysroot.validate_current()?;
        self.output_parent.validate_current()?;
        view.check()?;
        Ok(())
    }

    pub(super) fn check_inputs(
        &self,
        plan: &BuildPlan<'_, '_>,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), BuildError> {
        self.check_context(plan, view)?;
        self.subject.check(view)?;
        self.driver.check(view)?;
        self.frame.check(view)?;
        self.compiler.check(view)?;
        self.check_context(plan, view)
    }

    pub(super) fn check_output_absent(
        &self,
        plan: &BuildPlan<'_, '_>,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), BuildError> {
        self.check_context(plan, view)?;
        // The parent is already a live no-follow anchor. A dangling symlink,
        // alias or non-regular entry is never treated as observed absence.
        match self.output_parent.open_read_only(&self.output_leaf) {
            Ok(_) => Err(BuildError::OutputAlreadyExists),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                self.output_parent.validate_current()?;
                view.check()?;
                Ok(())
            }
            Err(error) => Err(error.into()),
        }
    }

    pub(super) fn capture_artifact(
        &mut self,
        plan: &BuildPlan<'_, '_>,
        view: &ResourceReservationView<'_>,
    ) -> Result<Digest32, BuildError> {
        self.check_context(plan, view)?;
        if self.artifact.is_some() {
            return Err(BuildError::BuildAlreadyAttempted);
        }
        let actual = fingerprint(
            &self.output_parent,
            &self.output_leaf,
            self.artifact_cap,
            ImageKind::Artifact,
            view,
        )
        .map_err(|error| match error {
            BuildError::Io(std::io::ErrorKind::NotFound) => BuildError::MissingArtifact,
            other => other,
        })?;
        self.artifact = Some(actual);
        Ok(actual.digest)
    }

    pub(super) fn artifact_digest(&self) -> Result<Digest32, BuildError> {
        self.artifact
            .map(|image| image.digest)
            .ok_or(BuildError::MissingArtifact)
    }

    pub(super) fn check_current(
        &self,
        plan: &BuildPlan<'_, '_>,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), BuildError> {
        self.check_inputs(plan, view)?;
        let expected = self.artifact.ok_or(BuildError::MissingArtifact)?;
        if fingerprint(
            &self.output_parent,
            &self.output_leaf,
            self.artifact_cap,
            ImageKind::Artifact,
            view,
        )? != expected
        {
            return Err(BuildError::ArtifactDrift);
        }
        self.check_context(plan, view)
    }
}

fn absolute_components(path: &Path) -> Result<PathBuf, BuildError> {
    Ok(std::path::absolute(path)?.components().collect())
}

fn fingerprint(
    parent: &DirectoryAnchor,
    leaf: &Path,
    cap: usize,
    kind: ImageKind,
    view: &ResourceReservationView<'_>,
) -> Result<Fingerprint, BuildError> {
    view.check()?;
    let sentinel = cap.checked_add(1).ok_or(BuildError::InputLimit)?;
    let claim = view.claim_resources(&[ResourceRequest::new(
        ResourceKind::Ram,
        ResourceAmount::checked((READ_BYTES + HEADER_BYTES) as u64)?,
    )])?;
    parent.validate_current()?;
    let mut file = parent.open_read_only(leaf)?;
    let before = file.metadata()?;
    if before.len() > u64::try_from(cap).map_err(|_| BuildError::InputLimit)? {
        return Err(BuildError::InputLimit);
    }
    let identity = file.identity();
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; READ_BYTES];
    let mut header = [0_u8; HEADER_BYTES];
    let mut header_len = 0usize;
    let mut total = 0usize;
    loop {
        view.check()?;
        let remaining = sentinel.checked_sub(total).ok_or(BuildError::InputLimit)?;
        if remaining == 0 {
            return Err(BuildError::InputLimit);
        }
        let read_limit = buffer.len().min(remaining);
        let n = file.read(&mut buffer[..read_limit])?;
        if n == 0 {
            break;
        }
        total = total.checked_add(n).ok_or(BuildError::InputLimit)?;
        if total > cap {
            return Err(BuildError::InputLimit);
        }
        let copied = n.min(HEADER_BYTES - header_len);
        header[header_len..header_len + copied].copy_from_slice(&buffer[..copied]);
        header_len += copied;
        hash.update(&buffer[..n]);
    }
    file.validate_current()?;
    let after = file.metadata()?;
    if before.len() != after.len() || total as u64 != after.len() || file.identity() != identity {
        return Err(kind.mismatch());
    }
    if !matches!(kind, ImageKind::Source) {
        native_header(&header[..header_len], &after)?;
    }
    claim.check()?;
    view.check()?;
    Ok(Fingerprint {
        identity,
        length: after.len(),
        digest: Digest32::from_bytes(hash.finalize().into()),
    })
}

fn native_header(header: &[u8], metadata: &Metadata) -> Result<(), BuildError> {
    #[cfg(windows)]
    {
        let _ = metadata;
        let machine = if cfg!(target_arch = "x86_64") {
            0x8664
        } else if cfg!(target_arch = "aarch64") {
            0xaa64
        } else if cfg!(target_arch = "x86") {
            0x014c
        } else {
            return Err(BuildError::Unsupported("native image architecture"));
        };
        if header.len() < 64 || &header[..2] != b"MZ" {
            return Err(BuildError::NotExecutable);
        }
        let offset = u32::from_le_bytes(
            header[60..64]
                .try_into()
                .map_err(|_| BuildError::NotExecutable)?,
        ) as usize;
        let end = offset.checked_add(44).ok_or(BuildError::NotExecutable)?;
        if offset < 64 || end > header.len() || &header[offset..offset + 4] != b"PE\0\0" {
            return Err(BuildError::NotExecutable);
        }
        let field = |at: usize| u16::from_le_bytes([header[offset + at], header[offset + at + 1]]);
        let magic = if cfg!(target_pointer_width = "64") {
            0x20b
        } else {
            0x10b
        };
        if field(4) != machine
            || field(6) == 0
            || field(20) < 20
            || field(22) & 2 == 0
            || field(24) != magic
        {
            return Err(BuildError::NotExecutable);
        }
        Ok(())
    }
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::PermissionsExt;
        let machine = if cfg!(target_arch = "x86_64") {
            62
        } else if cfg!(target_arch = "aarch64") {
            183
        } else if cfg!(target_arch = "x86") {
            3
        } else {
            return Err(BuildError::Unsupported("native image architecture"));
        };
        let class = if cfg!(target_pointer_width = "64") {
            2
        } else {
            1
        };
        let length = if class == 2 { 64 } else { 52 };
        if header.len() < length
            || &header[..4] != b"\x7fELF"
            || header[4] != class
            || header[5] != 1
            || header[6] != 1
            || metadata.permissions().mode() & 0o111 == 0
        {
            return Err(BuildError::NotExecutable);
        }
        let kind = u16::from_le_bytes([header[16], header[17]]);
        if !matches!(kind, 2 | 3) || u16::from_le_bytes([header[18], header[19]]) != machine {
            return Err(BuildError::NotExecutable);
        }
        Ok(())
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = (header, metadata);
        Err(BuildError::Unsupported("native executable format"))
    }
}
