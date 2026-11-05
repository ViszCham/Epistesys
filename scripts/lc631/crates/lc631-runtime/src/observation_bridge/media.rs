//! 実v3実行をProposed前提へ接続する。意味・権限・完成を発行しない。
//! Actual funded v3 execution to Proposed premises; no semantic or action grant.
#![forbid(unsafe_code)]
mod temporal;
pub use temporal::*;
use lc631_core::{context::{ContextAdmission, ContextConsumer, ContextDelta, ContextError,
    ContextPremise, ContextRevision, ContextScope, PremiseBasis, PremiseEdit}, AuthorityRevision};
use lc631_media::{execute_local_media_backend_attested_v3_with_resources, validate_current_media_authority,
    BackendDescriptor, MediaBackendConfig, MediaBackendExecutionReceipt, MediaError,
    MediaResourceBudget, MediaSourceAuthority};
use lc631_receipt_kernel::{ReceiptIssuer, ReceiptVerifier, ReplayGuard};
use lc631_substrate::{resource::{ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
    ResourceReservationView}, Digest32};
use sha2::{Digest, Sha256};
use std::{sync::Arc, time::Instant};

/// Declared task-to-media association. Task text and binary media are distinct sources.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MediaContextBinding {
    pub task_source: Digest32,
    pub parent: ContextRevision,
    pub authority: AuthorityRevision,
    pub media_source: Digest32,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaRegistration {
    pub backend: String,
    /// Registered model-cache manifest, not a model-accuracy certificate.
    pub model: String,
    pub configuration: String,
}
#[derive(Clone, Copy, Debug)]
pub struct MediaAdmissionLimits {
    pub max_observations: usize,
    pub max_label_bytes: usize,
}
pub struct MediaExecutionRequest<'a> {
    pub binding: MediaContextBinding,
    pub registration: &'a MediaRegistration,
    pub source_authority: &'a MediaSourceAuthority,
    pub descriptor: &'a BackendDescriptor,
    pub config: &'a MediaBackendConfig,
    pub budget: &'a MediaResourceBudget,
    pub limits: MediaAdmissionLimits,
}
#[derive(Debug)]
pub enum MediaConnectionError {
    InvalidLimits,
    BindingMismatch,
    RegistrationMismatch,
    UnknownParent,
    EmptyObservation,
    InvalidObservation,
    AllocationFailed,
    Resource(ResourceError),
    Backend(MediaError),
    Context(ContextError),
    CurrentVerification,
    InvalidProbePlan,
    Measurement(lc631_analysis::creativity_measurement::MeasurementError),
    TemporalInput(lc631_analysis::spatiotemporal_loss::InputError),
}
impl From<ResourceError> for MediaConnectionError {
    fn from(value: ResourceError) -> Self { Self::Resource(value) }
}
/// Constructed only after real funded execution and actual ContextConsumer admission.
/// The report stays immutable; its signature is provenance, not semantic validation.
pub struct ExecutedMediaAdmission<'view> {
    report: MediaBackendExecutionReceipt,
    binding: MediaContextBinding,
    scope: ContextScope,
    admission: ContextAdmission,
    receipt: Digest32,
    logical_payload_bytes: u64,
    // Last field: drop report storage before releasing its logical RAM claim.
    report_claim: ResourceClaim<'view>,
}
impl ExecutedMediaAdmission<'_> {
    pub fn report(&self) -> &MediaBackendExecutionReceipt { &self.report }
    pub const fn admission(&self) -> ContextAdmission { self.admission }
    pub const fn binding(&self) -> MediaContextBinding { self.binding }
    pub const fn receipt_digest(&self) -> Digest32 { self.receipt }
    pub const fn logical_payload_bytes(&self) -> u64 { self.logical_payload_bytes }
    pub fn check_live(&self) -> Result<(), MediaConnectionError> { self.report_claim.check().map_err(Into::into) }
    pub const fn spatial_status(&self) -> &'static str { "Unsupported: no typed region/frame/calibration payload" }
    pub const fn claim_boundary(&self) -> &'static str {
        "actual funded v3 provenance and Proposed admission only; no semantic truth, independent validation, authority, completion or host delivery"
    }
}

/// Caller supplies trusted current epoch and genuine authority/issuer/verifier.
/// The returned report retains a live logical-payload RAM claim. Context storage
/// remains bounded by its caller-owned ContextConsumer; neither cap is total RSS.
#[allow(clippy::too_many_arguments)]
pub fn observe_registered_media<'view>(
    request: MediaExecutionRequest<'_>,
    issuer: &ReceiptIssuer,
    verifier: &ReceiptVerifier,
    replay: &mut ReplayGuard,
    now_epoch: u64,
    resources: &'view ResourceReservationView<'view>,
    consumer: &mut ContextConsumer,
) -> Result<ExecutedMediaAdmission<'view>, MediaConnectionError> {
    preflight(&request, resources, consumer)?;
    // Reserve before capture. This bounded capacity covers encoded report data,
    // generated envelope/label copies and fixed receipt metadata, not allocator overhead.
    let capacity = (request.budget.max_stdout_bytes as u64).checked_mul(8)
        .and_then(|n| n.checked_add((request.limits.max_label_bytes as u64) * 4))
        .and_then(|n| n.checked_add(128 * 1024))
        .ok_or(MediaConnectionError::InvalidLimits)?;
    let report_claim = resources.claim_resources(&[ResourceRequest::new(ResourceKind::Ram,
        ResourceAmount::checked(capacity)?)])?;
    let started = Instant::now();
    let report = execute_local_media_backend_attested_v3_with_resources(
        request.source_authority, request.descriptor, request.config, request.budget,
        issuer, verifier, replay, now_epoch, resources,
    ).map_err(MediaConnectionError::Backend)?;
    check_report(&report, &request)?;
    let logical_payload_bytes = count_report(&report, capacity)?;
    let receipt = digest(report.authenticity_receipt_digest.as_deref()
        .ok_or(MediaConnectionError::CurrentVerification)?)?;
    let parent = consumer.get(request.binding.parent).ok_or(MediaConnectionError::UnknownParent)?;
    let scope = parent.scope();
    let bytes = request.limits.max_label_bytes.checked_add(
        request.limits.max_observations.checked_mul(std::mem::size_of::<PremiseEdit>())
            .ok_or(MediaConnectionError::InvalidLimits)?,
    ).ok_or(MediaConnectionError::InvalidLimits)?;
    let _claim = resources.claim_resources(&[ResourceRequest::new(ResourceKind::Ram,
        ResourceAmount::checked(bytes as u64)?)])?;
    let mut edits = Vec::new();
    edits.try_reserve_exact(report.graph.observations().len())
        .map_err(|_| MediaConnectionError::AllocationFailed)?;
    for observation in report.graph.observations() {
        let mut hash = Sha256::new();
        hash.update(b"epistesys.actual-media-proposed-premise.v1\0");
        hash.update(request.binding.task_source.as_bytes());
        hash.update(receipt.as_bytes());
        hash_text(&mut hash, &observation.observation_id);
        hash_text(&mut hash, &observation.provenance_digest);
        let id = Digest32::from_bytes(hash.finalize().into());
        edits.push(PremiseEdit::Add(ContextPremise::reported(id,
            Arc::from(observation.label.as_str()), PremiseBasis::Proposed, true)));
    }
    let delta = ContextDelta::proposed(parent.revision(), scope, request.binding.authority,
        parent.applicability(), edits);
    resources.check()?;
    let current = now_epoch.checked_add(started.elapsed().as_secs())
        .ok_or(MediaConnectionError::CurrentVerification)?;
    validate_current_media_authority(request.source_authority, verifier, current)
        .map_err(MediaConnectionError::Backend)?;
    let admission = consumer.admit(delta).map_err(MediaConnectionError::Context)?;
    Ok(ExecutedMediaAdmission { report, binding: request.binding, scope, admission, receipt,
        logical_payload_bytes, report_claim })
}

fn preflight(request: &MediaExecutionRequest<'_>, resources: &ResourceReservationView<'_>,
    consumer: &ContextConsumer) -> Result<(), MediaConnectionError> {
    resources.check()?;
    let b = request.binding;
    if resources.source_revision() != b.media_source || b.parent.source().digest() != b.task_source
        || consumer.original().state().revision().source().digest() != b.task_source
        || consumer.original().authority() != b.authority
        || request.source_authority.source_snapshot_sha256.as_deref()
            .map(digest).transpose()? != Some(b.media_source) {
        return Err(MediaConnectionError::BindingMismatch);
    }
    if consumer.get(b.parent).is_none() { return Err(MediaConnectionError::UnknownParent); }
    let limits = request.limits;
    if limits.max_observations == 0 || limits.max_observations > 128
        || limits.max_label_bytes == 0 || limits.max_label_bytes > 65536
        || request.budget.max_observations > limits.max_observations
        || request.budget.max_stdout_bytes > 1024 * 1024
        || request.budget.max_stderr_bytes > 65536 {
        return Err(MediaConnectionError::InvalidLimits);
    }
    if request.registration.backend.is_empty() || request.registration.backend.len() > 256
        || request.registration.backend != request.descriptor.backend_id
        || request.descriptor.model_sha256.as_deref() != Some(request.registration.model.as_str()) {
        return Err(MediaConnectionError::RegistrationMismatch);
    }
    digest(&request.registration.model)?;
    digest(&request.registration.configuration)?;
    Ok(())
}

fn check_report(report: &MediaBackendExecutionReceipt, request: &MediaExecutionRequest<'_>)
    -> Result<(), MediaConnectionError> {
    if report.backend_id != request.registration.backend
        || report.model_cache_manifest_sha256 != request.registration.model
        || report.execution_plan.configuration_digest != request.registration.configuration
        || digest(&report.source_sha256)? != request.binding.media_source {
        return Err(MediaConnectionError::RegistrationMismatch);
    }
    let observations = report.graph.observations();
    if observations.is_empty() { return Err(MediaConnectionError::EmptyObservation); }
    let mut bytes = 0usize;
    for observation in observations {
        bytes = bytes.checked_add(observation.label.len()).ok_or(MediaConnectionError::InvalidLimits)?;
        if !observation.candidate_only || observation.label.trim().is_empty()
            || observation.backend_id != report.backend_id
            || observation.configuration_digest != report.execution_plan.configuration_digest
            || observation.validate().is_err() {
            return Err(MediaConnectionError::InvalidObservation);
        }
    }
    if observations.len() > request.limits.max_observations || bytes > request.limits.max_label_bytes {
        return Err(MediaConnectionError::InvalidLimits);
    }
    Ok(())
}
fn digest(value: &str) -> Result<Digest32, MediaConnectionError> {
    super::parse_digest(value).map_err(|_| MediaConnectionError::RegistrationMismatch)
}
fn hash_text(hash: &mut Sha256, value: &str) {
    hash.update((value.len() as u64).to_le_bytes());
    hash.update(value.as_bytes());
}

fn count_report(report: &MediaBackendExecutionReceipt, capacity: u64)
    -> Result<u64, MediaConnectionError> {
    struct Counter(u64);
    impl std::io::Write for Counter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 = self.0.checked_add(bytes.len() as u64)
                .ok_or_else(|| std::io::Error::other("report size overflow"))?;
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
    }
    let mut counter = Counter(0);
    lc631_media::attestation_v3::write_media_report_v3(
        &mut lc631_media::attestation_v3::CanonicalWriter::new(&mut counter, capacity), report,
    ).map_err(|_| MediaConnectionError::InvalidLimits)?;
    // The encoder excludes the authenticated wire, which is bounded separately
    // by the receipt kernel. Retain explicit fixed metadata headroom as capacity.
    if counter.0.checked_add(65536).is_none_or(|n| n > capacity) {
        return Err(MediaConnectionError::InvalidLimits);
    }
    Ok(counter.0)
}
