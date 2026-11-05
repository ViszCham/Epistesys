//! Explicit v2 integrity binding for a returned media report.
//!
//! This verifies a local receipt for the exact serialized report fields; it
//! does not prove backend semantics, media truth, rights, or host authorization.
//! Legacy v1 digests and receipts are not interpreted as v2.

use crate::{
    MediaBackendExecutionReceipt, MediaExecutionPlan, MediaObservation, MediaProcessReceipt,
};
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptPolicy, ReceiptScope, ReceiptVerifier, ReplayGuard, SubjectRevision,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::{self, Write};

/// Maximum serialized input hashed by this v2 profile, excluding the domain.
pub const MAX_MEDIA_ATTESTATION_V2_BYTES: usize = 16 * 1024 * 1024;
const DOMAIN: &[u8] = b"lc631/media-backend-report-attestation/v2\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MediaAttestationV2Error {
    EncodingOrSizeLimit,
    InvalidBinding,
    MissingAttestation,
    VerificationRejected,
}

// The field order, names, serde JSON encoding, and this explicit set of fields
// form the v2 preimage profile. The graph's ordered observations are used, not
// its redundant implementation-only lookup index. Self-attestation fields are
// excluded to avoid a circular digest. A future encoding change needs v3.
#[derive(Serialize)]
struct SignedReport<'a> {
    schema_version: &'a str,
    source_sha256: &'a str,
    backend_id: &'a str,
    backend_version: &'a str,
    model_revision: &'a str,
    model_cache_manifest_sha256: &'a str,
    model_card_license_hint: &'a str,
    backend_receipt_sha256: &'a str,
    execution_plan: &'a MediaExecutionPlan,
    process: &'a MediaProcessReceipt,
    executed_family_states:
        &'a std::collections::BTreeMap<crate::BackendFamily, crate::BackendState>,
    observations: &'a [MediaObservation],
    diagnostics: &'a [String],
    source_authority_preserved: bool,
    identity_inference_committed: bool,
    internal_state_committed: bool,
    family_validation_count: usize,
    translation_envelope: &'a lc631_tl::CanonicalTranslationEnvelope,
    claim_boundary: &'a str,
}

struct BoundedHashWriter {
    hasher: Sha256,
    bytes: usize,
}

impl Write for BoundedHashWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let next = self.bytes.checked_add(bytes.len()).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "attestation input too large")
        })?;
        if next > MAX_MEDIA_ATTESTATION_V2_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "attestation input too large",
            ));
        }
        self.hasher.update(bytes);
        self.bytes = next;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Stream the versioned v2 representation into SHA-256; no report-sized
/// intermediate buffer is built. The hash is not authentication by itself.
pub fn media_backend_payload_digest_v2(
    report: &MediaBackendExecutionReceipt,
) -> Result<String, MediaAttestationV2Error> {
    let signed = SignedReport {
        schema_version: report.schema_version,
        source_sha256: &report.source_sha256,
        backend_id: &report.backend_id,
        backend_version: &report.backend_version,
        model_revision: &report.model_revision,
        model_cache_manifest_sha256: &report.model_cache_manifest_sha256,
        model_card_license_hint: &report.model_card_license_hint,
        backend_receipt_sha256: &report.backend_receipt_sha256,
        execution_plan: &report.execution_plan,
        process: &report.process,
        executed_family_states: &report.executed_family_states,
        observations: report.graph.observations(),
        diagnostics: &report.diagnostics,
        source_authority_preserved: report.source_authority_preserved,
        identity_inference_committed: report.identity_inference_committed,
        internal_state_committed: report.internal_state_committed,
        family_validation_count: report.family_validation_count,
        translation_envelope: &report.translation_envelope,
        claim_boundary: report.claim_boundary,
    };
    let mut writer = BoundedHashWriter {
        hasher: Sha256::new(),
        bytes: 0,
    };
    writer.hasher.update(DOMAIN);
    serde_json::to_writer(&mut writer, &signed)
        .map_err(|_| MediaAttestationV2Error::EncodingOrSizeLimit)?;
    let digest = writer.hasher.finalize();
    let mut hex = String::with_capacity(71);
    hex.push_str("sha256:");
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in digest {
        hex.push(char::from(HEX[usize::from(byte >> 4)]));
        hex.push(char::from(HEX[usize::from(byte & 15)]));
    }
    Ok(hex)
}

pub fn media_backend_scope_v2(backend_id: &str) -> String {
    format!("media/backend/v2/{backend_id}")
}

/// Recompute the current report, subject and v2 scope before verification.
/// `parent_digest` must be the caller's separately validated current source
/// receipt digest; this function does not establish its authority or freshness.
pub fn verify_media_backend_attestation_v2(
    report: &MediaBackendExecutionReceipt,
    verifier: &ReceiptVerifier,
    replay: &mut ReplayGuard,
    now_epoch: u64,
    parent_digest: &str,
) -> Result<(), MediaAttestationV2Error> {
    if parent_digest.len() != 71
        || !parent_digest.starts_with("sha256:")
        || !parent_digest[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(MediaAttestationV2Error::InvalidBinding);
    }
    let payload = media_backend_payload_digest_v2(report)?;
    let subject = SubjectRevision::checked(&report.source_sha256)
        .map_err(|_| MediaAttestationV2Error::InvalidBinding)?;
    let scope = ReceiptScope::checked(media_backend_scope_v2(&report.backend_id))
        .map_err(|_| MediaAttestationV2Error::InvalidBinding)?;
    let attestation = report
        .authenticity_attestation
        .as_ref()
        .ok_or(MediaAttestationV2Error::MissingAttestation)?;
    attestation
        .check_size_bounds()
        .map_err(|_| MediaAttestationV2Error::EncodingOrSizeLimit)?;
    if report.authenticity_receipt_digest.as_deref() != Some(attestation.receipt_digest().as_str())
    {
        return Err(MediaAttestationV2Error::InvalidBinding);
    }
    verifier
        .verify(
            attestation.clone(),
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
        .map_err(|_| MediaAttestationV2Error::VerificationRejected)?;
    Ok(())
}
