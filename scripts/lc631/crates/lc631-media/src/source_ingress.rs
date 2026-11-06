//! Source-only regular-file observations; model and executable policies are separate.
use crate::{format_digest, MediaError, MAX_MEDIA_BYTES};
use lc631_process_supervisor::{
    filesystem::{DirectoryAnchor, FileIdentity},
    ResourceAmount, ResourceKind, ResourceRequest, ResourceReservationView,
};
use sha2::{Digest, Sha256};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const BUFFER_BYTES: usize = 65_536;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RegularSourceObservation {
    pub(crate) canonical_path: PathBuf,
    pub(crate) sha256: String,
    pub(crate) bytes: u64,
    pub(crate) identity: FileIdentity,
}

fn check(
    resources: Option<&ResourceReservationView<'_>>,
    deadline: Instant,
) -> Result<(), MediaError> {
    if Instant::now() >= deadline {
        return Err(MediaError::BackendTimeout);
    }
    if let Some(view) = resources {
        view.check()
            .map_err(|error| MediaError::Io(format!("media_source_resource:{error:?}")))?;
    }
    Ok(())
}

/// Resolve the original locator through a no-follow anchor before obtaining a
/// canonical display/binding path. Digest, length and identity come from one
/// regular read-only handle, and the named occurrence is checked while reading.
///
/// The caller compares identity with its private authenticated-source binding.
/// This is a bounded observation, not an atomic lease over later execution.
/// Deadline/cancellation checks surround OS calls; arbitrary blocking I/O is
/// not universally bounded. The Linux capsule uses O_NONBLOCK and rejects a
/// FIFO/device before any read. No model/executable path policy is changed.
pub(crate) fn observe_regular_source<'a>(
    path: &Path,
    expected: Option<(&str, u64)>,
    resources: Option<&'a ResourceReservationView<'a>>,
) -> Result<RegularSourceObservation, MediaError> {
    let deadline = Instant::now()
        .checked_add(Duration::from_secs(1_800))
        .ok_or(MediaError::InvalidTimeout)?;
    check(resources, deadline)?;
    if expected.is_some_and(|(_, bytes)| bytes > MAX_MEDIA_BYTES) {
        return Err(MediaError::MediaBudgetExceeded);
    }
    let _claim = resources
        .map(|view| {
            if let Some((digest, _)) = expected {
                if format_digest(view.source_revision().as_bytes()) != digest {
                    return Err(lc631_process_supervisor::ResourceError::SourceMismatch);
                }
            }
            view.claim_resources(&[ResourceRequest::new(
                ResourceKind::Ram,
                ResourceAmount::checked(BUFFER_BYTES as u64)?,
            )])
        })
        .transpose()
        .map_err(|error| MediaError::Io(format!("media_source_resource:{error:?}")))?;
    let unavailable = || {
        if expected.is_some() {
            MediaError::SourceSnapshotChanged
        } else {
            MediaError::SourceSnapshotUnavailable
        }
    };
    // Capture cwd once. Canonicalizing first would erase evidence of a link in
    // the supplied path before the no-follow capsule could reject it.
    let absolute = std::path::absolute(path).map_err(|_| unavailable())?;
    let parent = absolute.parent().ok_or_else(unavailable)?;
    let name = absolute.file_name().ok_or_else(unavailable)?;
    let directory = DirectoryAnchor::open_existing(parent).map_err(|_| unavailable())?;
    let mut source = directory
        .open_read_only(Path::new(name))
        .map_err(|_| unavailable())?;
    check(resources, deadline)?;
    source.validate_current().map_err(|_| unavailable())?;
    let identity = source.identity();
    let metadata = source.metadata().map_err(|_| unavailable())?;
    if !metadata.is_file() {
        return Err(unavailable());
    }
    let bytes = metadata.len();
    if bytes > MAX_MEDIA_BYTES {
        return Err(MediaError::MediaBudgetExceeded);
    }
    if expected.is_some_and(|(_, expected_bytes)| expected_bytes != bytes) {
        return Err(MediaError::SourceSnapshotChanged);
    }
    let canonical_path = source.path().canonicalize().map_err(|_| unavailable())?;
    source.validate_current().map_err(|_| unavailable())?;
    check(resources, deadline)?;

    let mut hash = Sha256::new();
    let mut buffer = [0; BUFFER_BYTES];
    let mut observed = 0_u64;
    loop {
        check(resources, deadline)?;
        // One sentinel byte detects growth without reading beyond the declared
        // metadata length plus one, even when the length is an exact chunk.
        let remaining = bytes
            .checked_sub(observed)
            .ok_or(MediaError::SourceSnapshotChanged)?;
        let length = usize::try_from(remaining.min((BUFFER_BYTES - 1) as u64) + 1)
            .map_err(|_| MediaError::MediaBudgetExceeded)?;
        let count = match source.read(&mut buffer[..length]) {
            Ok(count) => count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(unavailable()),
        };
        check(resources, deadline)?;
        if count == 0 {
            break;
        }
        observed = observed
            .checked_add(count as u64)
            .filter(|value| *value <= bytes)
            .ok_or(MediaError::SourceSnapshotChanged)?;
        hash.update(&buffer[..count]);
    }
    // validate_current checks the held identity against the same named leaf and
    // all held ancestors; metadata below is from that same read-only handle.
    source.validate_current().map_err(|_| unavailable())?;
    let final_metadata = source.metadata().map_err(|_| unavailable())?;
    check(resources, deadline)?;
    if observed != bytes || !final_metadata.is_file() || final_metadata.len() != bytes {
        return Err(MediaError::SourceSnapshotChanged);
    }
    let sha256 = format_digest(hash.finalize());
    if expected.is_some_and(|(digest, _)| digest != sha256) {
        return Err(MediaError::SourceSnapshotChanged);
    }
    check(resources, deadline)?;
    Ok(RegularSourceObservation {
        canonical_path,
        sha256,
        bytes,
        identity,
    })
}
