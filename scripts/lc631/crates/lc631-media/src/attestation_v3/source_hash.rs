//! Source-only anchored revalidation; model/executable path policies are separate.
use super::MediaAttestationV3Error;
use lc631_process_supervisor::{
    filesystem::DirectoryAnchor, ResourceAmount, ResourceKind, ResourceRequest,
    ResourceReservationView,
};
use sha2::{Digest, Sha256};
use std::io::{self, Read};
use std::path::Path;
use std::time::{Duration, Instant};

const BUFFER_BYTES: usize = 65_536;

fn check(
    resources: Option<&ResourceReservationView<'_>>,
    deadline: Instant,
) -> Result<(), MediaAttestationV3Error> {
    if Instant::now() >= deadline {
        return Err(MediaAttestationV3Error::InvalidBinding);
    }
    if let Some(view) = resources {
        view.check()
            .map_err(|_| MediaAttestationV3Error::InvalidBinding)?;
    }
    Ok(())
}

/// Hash only the expected regular source through a read-only anchored handle.
/// Linux's existing capsule rejects FIFO/device/symlink inputs before reading.
/// Checks are cooperative around OS calls, not a universal I/O wall-clock bound.
pub(super) fn hash_source<'a>(
    path: &Path,
    expected_bytes: u64,
    expected_digest: &str,
    resources: Option<&'a ResourceReservationView<'a>>,
) -> Result<String, MediaAttestationV3Error> {
    let deadline = Instant::now()
        .checked_add(Duration::from_secs(1_800))
        .ok_or(MediaAttestationV3Error::InvalidBinding)?;
    check(resources, deadline)?;
    if expected_bytes > crate::MAX_MEDIA_BYTES {
        return Err(MediaAttestationV3Error::InvalidBinding);
    }
    let _claim = resources
        .map(|view| {
            if crate::format_digest(view.source_revision().as_bytes()) != expected_digest {
                return Err(lc631_process_supervisor::ResourceError::SourceMismatch);
            }
            view.claim_resources(&[ResourceRequest::new(
                ResourceKind::Ram,
                ResourceAmount::checked(BUFFER_BYTES as u64)?,
            )])
        })
        .transpose()
        .map_err(|_| MediaAttestationV3Error::InvalidBinding)?;
    let parent = path
        .parent()
        .ok_or(MediaAttestationV3Error::InvalidBinding)?;
    let name = path
        .file_name()
        .ok_or(MediaAttestationV3Error::InvalidBinding)?;
    let directory = DirectoryAnchor::open_existing(parent)
        .map_err(|_| MediaAttestationV3Error::InvalidBinding)?;
    let mut source = directory
        .open_read_only(Path::new(name))
        .map_err(|_| MediaAttestationV3Error::InvalidBinding)?;
    check(resources, deadline)?;
    let metadata = source
        .metadata()
        .map_err(|_| MediaAttestationV3Error::InvalidBinding)?;
    if !metadata.is_file() || metadata.len() != expected_bytes {
        return Err(MediaAttestationV3Error::InvalidBinding);
    }
    source
        .validate_current()
        .map_err(|_| MediaAttestationV3Error::InvalidBinding)?;
    let mut hash = Sha256::new();
    let mut buffer = [0; BUFFER_BYTES];
    let mut observed = 0_u64;
    loop {
        check(resources, deadline)?;
        let remaining = expected_bytes
            .checked_sub(observed)
            .ok_or(MediaAttestationV3Error::InvalidBinding)?;
        let length = usize::try_from(remaining.min((BUFFER_BYTES - 1) as u64) + 1)
            .map_err(|_| MediaAttestationV3Error::InvalidBinding)?;
        let count = match source.read(&mut buffer[..length]) {
            Ok(count) => count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(MediaAttestationV3Error::InvalidBinding),
        };
        check(resources, deadline)?;
        if count == 0 {
            break;
        }
        observed = observed
            .checked_add(count as u64)
            .filter(|bytes| *bytes <= expected_bytes)
            .ok_or(MediaAttestationV3Error::InvalidBinding)?;
        hash.update(&buffer[..count]);
    }
    source
        .validate_current()
        .map_err(|_| MediaAttestationV3Error::InvalidBinding)?;
    let metadata = source
        .metadata()
        .map_err(|_| MediaAttestationV3Error::InvalidBinding)?;
    check(resources, deadline)?;
    if observed != expected_bytes || metadata.len() != expected_bytes {
        return Err(MediaAttestationV3Error::InvalidBinding);
    }
    Ok(crate::format_digest(hash.finalize()))
}
