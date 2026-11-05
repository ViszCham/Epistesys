//! Common-owner wrapper around the unchanged snapshot identity/seal capsule.
use super::{AuthenticatedSourceSnapshot, CaptureFailure, MAX_SOURCE_BYTES};
use lc631_substrate::resource::{
    ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
    ResourceReservationView,
};
use std::path::Path;

impl AuthenticatedSourceSnapshot {
    /// Common-owner path: reserve before filesystem mutation, stream using the
    /// admitted deadline/cancel state, and bypass the legacy static quota owner.
    pub fn capture_with_resources<'a>(
        source: &Path,
        expected_sha256: &str,
        max_bytes: u64,
        temporary_budget: u64,
        view: &'a ResourceReservationView<'a>,
    ) -> Result<ReservedSourceSnapshot<'a>, ResourceError> {
        view.check()?;
        if view.source_revision() != expected_source(expected_sha256)? {
            return Err(ResourceError::SourceMismatch);
        }
        let disk = view.amount(ResourceKind::Disk)?;
        let limit = max_bytes
            .min(temporary_budget)
            .min(MAX_SOURCE_BYTES)
            .min(disk);
        let claim = view.claim_resources(&[
            request(ResourceKind::Ram, 65_536)?,
            request(ResourceKind::Disk, limit)?,
            request(ResourceKind::Cleanup, 1)?,
        ])?;
        let snapshot =
            Self::capture_inner(source, expected_sha256, limit, limit, false, Some(view)).map_err(
                |error| {
                    if error
                        .get_ref()
                        .and_then(|error| error.downcast_ref::<CaptureFailure>())
                        .is_some_and(|failure| failure.cleanup.is_some())
                    {
                        view.record_cleanup_unknown();
                        return ResourceError::CleanupUnverified;
                    }
                    view.check()
                        .err()
                        .unwrap_or(ResourceError::Io(error.kind()))
                },
            )?;
        Ok(ReservedSourceSnapshot {
            snapshot,
            claim,
            confirmed: false,
        })
    }
}

/// Keeps common Disk/RAM/Cleanup charges live through observed physical close.
/// Drop is not a cleanup receipt and never refunds the runtime reservation.
pub struct ReservedSourceSnapshot<'a> {
    snapshot: AuthenticatedSourceSnapshot,
    claim: ResourceClaim<'a>,
    confirmed: bool,
}
impl ReservedSourceSnapshot<'_> {
    pub fn path(&self) -> &Path {
        self.snapshot.path()
    }
    pub fn bytes(&self) -> u64 {
        self.snapshot.bytes()
    }
    pub fn close(mut self) -> Result<(), ResourceError> {
        self.snapshot.cleanup().map_err(|_| {
            self.claim.record_cleanup_unknown();
            ResourceError::CleanupUnverified
        })?;
        self.confirmed = true;
        Ok(())
    }
}
impl Drop for ReservedSourceSnapshot<'_> {
    fn drop(&mut self) {
        if !self.confirmed {
            self.claim.record_cleanup_unknown();
        }
    }
}

fn request(kind: ResourceKind, amount: u64) -> Result<ResourceRequest, ResourceError> {
    Ok(ResourceRequest::new(kind, ResourceAmount::checked(amount)?))
}

fn expected_source(value: &str) -> Result<lc631_substrate::Digest32, ResourceError> {
    let hexadecimal = value
        .strip_prefix("sha256:")
        .ok_or(ResourceError::InvalidConfig)?;
    if hexadecimal.len() != 64 {
        return Err(ResourceError::InvalidConfig);
    }
    let mut digest = [0; 32];
    for (byte, pair) in digest
        .iter_mut()
        .zip(hexadecimal.as_bytes().as_chunks::<2>().0)
    {
        let high = nibble(pair[0])?;
        let low = nibble(pair[1])?;
        *byte = high
            .checked_mul(16)
            .and_then(|value| value.checked_add(low))
            .ok_or(ResourceError::InvalidConfig)?;
    }
    Ok(lc631_substrate::Digest32::from_bytes(digest))
}

fn nibble(value: u8) -> Result<u8, ResourceError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err(ResourceError::InvalidConfig),
    }
}
