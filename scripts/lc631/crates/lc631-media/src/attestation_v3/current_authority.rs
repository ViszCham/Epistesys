//! Current source-receipt and bounded local snapshot checks, not semantic trust.
use super::{media_backend_payload_digest_v3, MediaAttestationV3Error};
use crate::{MediaBackendExecutionReceipt, MediaSourceAuthority};
use lc631_receipt_kernel::{ReceiptVerifier, ReplayGuard};
use std::time::Instant;

/// Revalidate the private authenticated source binding and its receipt before
/// accepting the exact v3 report. The file is rehashed through the existing
/// bounded reader; no report flag, hash, or signature creates source authority.
///
/// `now_epoch` must be trusted current time. Expiry is checked again after I/O.
/// This observes snapshot bytes during the read, not an atomic lease preventing
/// later file changes. Host revocation, rights provenance, independent model
/// validation and durable/cross-process replay remain separate caller contracts.
pub fn verify_media_backend_with_current_authority_v3(
    report: &MediaBackendExecutionReceipt,
    authority: &MediaSourceAuthority,
    verifier: &ReceiptVerifier,
    replay: &mut ReplayGuard,
    now_epoch: u64,
) -> Result<(), MediaAttestationV3Error> {
    let (base_epoch, started) = authority
        .elapsed_anchor(now_epoch)
        .map_err(|_| MediaAttestationV3Error::InvalidBinding)?;
    verify_from_execution(
        report, authority, verifier, replay, base_epoch, started, None,
    )
}

/// Funded counterpart: keep snapshot revalidation under the same admitted
/// source-bound RAM, deadline and cancellation view as process execution.
pub fn verify_media_backend_with_current_authority_v3_with_resources<'a>(
    report: &MediaBackendExecutionReceipt,
    authority: &MediaSourceAuthority,
    verifier: &ReceiptVerifier,
    replay: &mut ReplayGuard,
    now_epoch: u64,
    resources: &'a lc631_process_supervisor::ResourceReservationView<'a>,
) -> Result<(), MediaAttestationV3Error> {
    let (base_epoch, started) = authority
        .elapsed_anchor(now_epoch)
        .map_err(|_| MediaAttestationV3Error::InvalidBinding)?;
    verify_from_execution(
        report,
        authority,
        verifier,
        replay,
        base_epoch,
        started,
        Some(resources),
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn verify_from_execution<'a>(
    report: &MediaBackendExecutionReceipt,
    authority: &MediaSourceAuthority,
    verifier: &ReceiptVerifier,
    replay: &mut ReplayGuard,
    base_epoch: u64,
    started: Instant,
    resources: Option<&'a lc631_process_supervisor::ResourceReservationView<'a>>,
) -> Result<(), MediaAttestationV3Error> {
    let now_epoch = authority
        .current_epoch(super::clock::epoch_after_elapsed(
            base_epoch,
            started.elapsed(),
        )?)
        .map_err(|_| MediaAttestationV3Error::InvalidBinding)?;
    crate::validate_current_media_authority(authority, verifier, now_epoch)
        .map_err(|_| MediaAttestationV3Error::InvalidBinding)?;
    if authority.source_snapshot_sha256.as_deref() != Some(report.source_sha256.as_str())
        || report.execution_plan.source_snapshot_sha256 != report.source_sha256
        || authority.source_bytes != Some(report.execution_plan.source_bytes)
    {
        return Err(MediaAttestationV3Error::InvalidBinding);
    }
    let path = authority
        .local_path
        .as_deref()
        .ok_or(MediaAttestationV3Error::InvalidBinding)?;
    let observed = super::source_hash::hash_source(
        path,
        report.execution_plan.source_bytes,
        &report.source_sha256,
        resources,
    )?;
    let source_binding = authority
        .authenticated_source
        .as_ref()
        .ok_or(MediaAttestationV3Error::InvalidBinding)?;
    if observed.sha256 != report.source_sha256
        || observed.bytes != source_binding.bytes
        || observed.identity != source_binding.file_identity
        || observed.canonical_path != source_binding.path
    {
        return Err(MediaAttestationV3Error::InvalidBinding);
    }
    // Include bounded report hashing time in the last current-source/cancel
    // check. Rehashing after that check would reopen a stale-time window.
    let payload = media_backend_payload_digest_v3(report)?;
    let observed_epoch = authority
        .current_epoch(super::clock::epoch_after_elapsed(
            base_epoch,
            started.elapsed(),
        )?)
        .map_err(|_| MediaAttestationV3Error::InvalidBinding)?;
    crate::validate_current_media_authority(authority, verifier, observed_epoch)
        .map_err(|_| MediaAttestationV3Error::InvalidBinding)?;
    if let Some(view) = resources {
        view.check()
            .map_err(|_| MediaAttestationV3Error::InvalidBinding)?;
    }
    let parent = authority
        .attestation_receipt_digest
        .as_deref()
        .ok_or(MediaAttestationV3Error::InvalidBinding)?;
    super::authentication::verify_prehashed(
        report,
        verifier,
        replay,
        observed_epoch,
        parent,
        payload,
    )
}
