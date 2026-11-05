//! Strict reported DTO boundary. Decoding creates no verified token or authority.
#![forbid(unsafe_code)]
use crate::{DgclEvidenceLease, DgclExecutionSnapshot, DgclRepairApplication, DgclRepairRequest};
use lc631_receipt_kernel::UntrustedReceipt;
use serde::Deserialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodingResumeWireError {
    Limit,
    Shape,
    Schema,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestEnvelope {
    schema_version: String,
    request: DgclRepairRequest,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReceiptEnvelope {
    schema_version: String,
    receipt: UntrustedReceipt,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ApplicationEnvelope {
    schema_version: String,
    application: ApplicationWire,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ApplicationWire {
    schema_version: String,
    attempt: u8,
    gap_id: String,
    request_digest: String,
    source_revision: String,
    plan_digest: String,
    target_ref: String,
    target_digest_before: String,
    target_digest_after: String,
    candidate_digest: String,
    delta_bytes: u64,
    run_digest: String,
    permit_digest: String,
    applied: bool,
    after_snapshot: DgclExecutionSnapshot,
    after_lease: DgclEvidenceLease,
    attestation: Option<UntrustedReceipt>,
}

fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, CodingResumeWireError> {
    // Caller must fund its bounded DTO allocation before entering this parser.
    if bytes.len() > 65536 {
        return Err(CodingResumeWireError::Limit);
    }
    serde_json::from_slice(bytes).map_err(|_| CodingResumeWireError::Shape)
}
fn labels(values: &[&str]) -> Result<(), CodingResumeWireError> {
    if values.iter().any(|text| text.len() > 16384) {
        Err(CodingResumeWireError::Limit)
    } else {
        Ok(())
    }
}
pub fn decode_coding_resume_request(
    bytes: &[u8],
) -> Result<DgclRepairRequest, CodingResumeWireError> {
    let e: RequestEnvelope = decode(bytes)?;
    if e.schema_version != "epistesys-coding-resume-request.v1" {
        return Err(CodingResumeWireError::Schema);
    }
    let r = &e.request;
    labels(&[
        &r.gap_id,
        &r.source_revision,
        &r.plan_digest,
        &r.target_ref,
        &r.target_digest_before,
        &r.candidate_digest,
        &r.authorization_scope,
        r.repository_root_digest.as_deref().unwrap_or(""),
    ])?;
    Ok(e.request)
}
pub fn decode_coding_resume_receipt(
    bytes: &[u8],
) -> Result<UntrustedReceipt, CodingResumeWireError> {
    let e: ReceiptEnvelope = decode(bytes)?;
    if e.schema_version != "epistesys-coding-resume-attestation.v1" {
        return Err(CodingResumeWireError::Schema);
    }
    Ok(e.receipt)
}
pub fn decode_coding_resume_application(
    bytes: &[u8],
) -> Result<DgclRepairApplication, CodingResumeWireError> {
    let e: ApplicationEnvelope = decode(bytes)?;
    if e.schema_version != "epistesys-coding-resume-application.v1"
        || e.application.schema_version != "epistesys-dgcl-repair-application.v1"
    {
        return Err(CodingResumeWireError::Schema);
    }
    let a = e.application;
    labels(&[
        &a.gap_id,
        &a.request_digest,
        &a.source_revision,
        &a.plan_digest,
        &a.target_ref,
        &a.target_digest_before,
        &a.target_digest_after,
        &a.candidate_digest,
        &a.run_digest,
        &a.permit_digest,
        &a.after_snapshot.observed_boundary,
    ])?;
    if a.after_snapshot.targets.len() > 64 {
        return Err(CodingResumeWireError::Limit);
    }
    for t in &a.after_snapshot.targets {
        labels(&[&t.target_ref, &t.target_digest])?;
    }
    Ok(DgclRepairApplication {
        schema_version: "epistesys-dgcl-repair-application.v1",
        attempt: a.attempt,
        gap_id: a.gap_id,
        request_digest: a.request_digest,
        source_revision: a.source_revision,
        plan_digest: a.plan_digest,
        target_ref: a.target_ref,
        target_digest_before: a.target_digest_before,
        target_digest_after: a.target_digest_after,
        candidate_digest: a.candidate_digest,
        delta_bytes: a.delta_bytes,
        run_digest: a.run_digest,
        permit_digest: a.permit_digest,
        applied: a.applied,
        after_snapshot: a.after_snapshot,
        after_lease: a.after_lease,
        attestation: a.attestation,
    })
}
