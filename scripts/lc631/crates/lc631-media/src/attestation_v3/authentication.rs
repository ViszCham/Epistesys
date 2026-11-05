//! In-memory hashing and explicit receipt verification for v3 report bytes.
use super::{write_media_report_v3, CanonicalWriter};
use crate::MediaBackendExecutionReceipt;
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptPolicy, ReceiptScope, ReceiptVerifier, ReplayGuard, SubjectRevision,
};
use sha2::{Digest, Sha256};
use std::io::{self, Write};

pub const MAX_MEDIA_ATTESTATION_V3_BYTES: u64 = 16 * 1024 * 1024;
const SCOPE_PREFIX: &str = "media/backend/v3/";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MediaAttestationV3Error {
    EncodingOrSizeLimit,
    InvalidBinding,
    MissingAttestation,
    VerificationRejected,
}

struct HashSink(Sha256);
impl Write for HashSink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Hash the complete v3 binary preimage with no report-sized buffer.
/// A hash by itself is not authentication or proof of backend execution.
pub fn media_backend_payload_digest_v3(
    report: &MediaBackendExecutionReceipt,
) -> Result<String, MediaAttestationV3Error> {
    let mut sink = HashSink(Sha256::new());
    let mut writer = CanonicalWriter::new(&mut sink, MAX_MEDIA_ATTESTATION_V3_BYTES);
    write_media_report_v3(&mut writer, report)
        .map_err(|_| MediaAttestationV3Error::EncodingOrSizeLimit)?;
    let digest = sink.0.finalize();
    let mut text = String::with_capacity(71);
    text.push_str("sha256:");
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in digest {
        text.push(char::from(HEX[usize::from(byte >> 4)]));
        text.push(char::from(HEX[usize::from(byte & 15)]));
    }
    Ok(text)
}

pub fn media_backend_scope_v3(backend_id: &str) -> String {
    format!("{SCOPE_PREFIX}{backend_id}")
}

/// Verify only the exact report bytes and local receipt bindings. The parent
/// digest must be separately validated as current by the calling authority.
pub fn verify_media_backend_attestation_v3(
    report: &MediaBackendExecutionReceipt,
    verifier: &ReceiptVerifier,
    replay: &mut ReplayGuard,
    now_epoch: u64,
    parent_digest: &str,
) -> Result<(), MediaAttestationV3Error> {
    check_binding_sizes(report, parent_digest)?;
    let payload = media_backend_payload_digest_v3(report)?;
    verify_prehashed(report, verifier, replay, now_epoch, parent_digest, payload)
}

fn check_binding_sizes(
    report: &MediaBackendExecutionReceipt,
    parent_digest: &str,
) -> Result<(), MediaAttestationV3Error> {
    if parent_digest.len() != 71
        || !parent_digest.starts_with("sha256:")
        || !parent_digest[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || report.source_sha256.len() > 256
        || report.backend_id.len() > 512 - SCOPE_PREFIX.len()
    {
        return Err(MediaAttestationV3Error::InvalidBinding);
    }
    Ok(())
}

// Only sibling trusted code can supply this preimage hash. Public callers must
// use a verifier that computes the exact report bytes itself.
pub(super) fn verify_prehashed(
    report: &MediaBackendExecutionReceipt,
    verifier: &ReceiptVerifier,
    replay: &mut ReplayGuard,
    now_epoch: u64,
    parent_digest: &str,
    payload: String,
) -> Result<(), MediaAttestationV3Error> {
    check_binding_sizes(report, parent_digest)?;
    let subject = SubjectRevision::checked(&report.source_sha256)
        .map_err(|_| MediaAttestationV3Error::InvalidBinding)?;
    let scope = ReceiptScope::checked(media_backend_scope_v3(&report.backend_id))
        .map_err(|_| MediaAttestationV3Error::InvalidBinding)?;
    let wire = report
        .authenticity_attestation
        .as_ref()
        .ok_or(MediaAttestationV3Error::MissingAttestation)?;
    wire.check_size_bounds()
        .map_err(|_| MediaAttestationV3Error::EncodingOrSizeLimit)?;
    if report.authenticity_receipt_digest.as_deref() != Some(wire.receipt_digest().as_str()) {
        return Err(MediaAttestationV3Error::InvalidBinding);
    }
    verifier
        .verify(
            wire.clone(),
            &ReceiptPolicy::exact(
                ReceiptClass::MediaBackend,
                subject,
                scope,
                payload,
                now_epoch,
            )
            .with_parent(parent_digest.to_owned()),
            replay,
        )
        .map_err(|_| MediaAttestationV3Error::VerificationRejected)?;
    Ok(())
}
