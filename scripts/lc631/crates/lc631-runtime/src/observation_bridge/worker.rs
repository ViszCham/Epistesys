//! 実pin読取りと実workerを接続する。成功もProposedに限定する。
//! Actual pinned worker connection; successful fragments remain Proposed.
use super::{
    prepare_language_candidates, LanguageRegistration, ObservationBridgeError,
    ObservationBridgeLimits, ReportedLanguageInput,
};
use crate::resource::{ResourceError, ResourceReservationView};
use lc631_analysis::{
    load_dgcl_runtime_pins_with_resources, run_language_worker_with_resources, LanguageObservation,
    LanguageWorkerError,
};
use lc631_core::{
    context::{ContextAdmission, ContextConsumer, ContextRevision},
    AuthorityRevision,
};
use lc631_substrate::source::SourceStore;
use std::path::Path;

pub struct LanguageExecutionRequest<'a> {
    pub source: &'a SourceStore,
    pub parent: ContextRevision,
    /// Identity equality only; not an Edit/Test or media-source authority receipt.
    pub authority: AuthorityRevision,
    pub registration: LanguageRegistration<'a>,
    pub limits: ObservationBridgeLimits,
    pub timeout_ms: u64,
}

#[derive(Debug)]
pub enum LanguageExecutionError {
    Resource(ResourceError),
    Bridge(ObservationBridgeError),
    Worker(LanguageWorkerError),
    UnknownParent,
    AuthorityRevisionMismatch,
    InvalidTimeout,
}

/// Only this actual execution path constructs this owner. Its observation
/// remains provenance data, never an independent semantic-validation token.
pub struct ExecutedLanguageAdmission {
    observation: LanguageObservation,
    admission: ContextAdmission,
    candidate_count: usize,
}
impl ExecutedLanguageAdmission {
    pub fn observation(&self) -> &LanguageObservation {
        &self.observation
    }
    pub const fn admission(&self) -> ContextAdmission {
        self.admission
    }
    pub const fn candidate_count(&self) -> usize {
        self.candidate_count
    }
    pub const fn claim_boundary(&self) -> &'static str {
        "actual configured-worker execution and source correspondence only; Proposed fragments, no semantic truth, independent validation, authority, completion or host delivery"
    }
}

/// Uses the existing bounded repository-pin loader and actual resource-backed
/// worker. Missing/unsupported pins and backend failures remain typed errors.
/// No model acquisition, surrogate worker or reported-success input exists.
pub fn observe_registered_language(
    repository: &Path,
    request: LanguageExecutionRequest<'_>,
    resources: &ResourceReservationView<'_>,
    consumer: &mut ContextConsumer,
) -> Result<ExecutedLanguageAdmission, LanguageExecutionError> {
    preflight(&request, resources, consumer)?;
    let pins = load_dgcl_runtime_pins_with_resources(repository, resources)
        .map_err(LanguageExecutionError::Worker)?;
    let config = pins
        .bind_pinned_worker(request.registration.language, request.timeout_ms)
        .map_err(LanguageExecutionError::Worker)?;
    let observation = run_language_worker_with_resources(
        request.source.text(),
        request.registration.language,
        &config,
        resources,
    )
    .map_err(LanguageExecutionError::Worker)?;
    resources
        .check()
        .map_err(LanguageExecutionError::Resource)?;
    let report = ReportedLanguageInput::from_observation(&observation)
        .map_err(LanguageExecutionError::Bridge)?;
    let parent = consumer
        .get(request.parent)
        .ok_or(LanguageExecutionError::UnknownParent)?;
    let batch = prepare_language_candidates(
        request.source,
        parent,
        request.authority,
        request.registration,
        report,
        request.limits,
    )
    .map_err(LanguageExecutionError::Bridge)?;
    resources
        .check()
        .map_err(LanguageExecutionError::Resource)?;
    let admitted = batch
        .admit(consumer)
        .map_err(LanguageExecutionError::Bridge)?;
    let admission = admitted.admission;
    let candidate_count = admitted.candidate_count;
    // End the borrowed report before moving its actual observation owner.
    Ok(ExecutedLanguageAdmission {
        observation,
        admission,
        candidate_count,
    })
}

fn preflight(
    request: &LanguageExecutionRequest<'_>,
    resources: &ResourceReservationView<'_>,
    consumer: &ContextConsumer,
) -> Result<(), LanguageExecutionError> {
    resources
        .check()
        .map_err(LanguageExecutionError::Resource)?;
    let source = request.source.id().digest();
    if resources.source_revision() != source || request.parent.source().digest() != source {
        return Err(LanguageExecutionError::Bridge(
            ObservationBridgeError::SourceMismatch,
        ));
    }
    if consumer.get(request.parent).is_none() {
        return Err(LanguageExecutionError::UnknownParent);
    }
    if consumer.original().authority() != request.authority {
        return Err(LanguageExecutionError::AuthorityRevisionMismatch);
    }
    let r = request.registration;
    if !matches!(r.language, "ja" | "en") || r.backend.is_empty() || r.backend.len() > 256 {
        return Err(LanguageExecutionError::Bridge(
            ObservationBridgeError::RegistrationMismatch,
        ));
    }
    let l = request.limits;
    if l.max_sentences == 0
        || l.max_sentences > 1_024
        || l.max_tokens == 0
        || l.max_tokens > 4_096
        || l.max_fragment_bytes == 0
        || l.max_fragment_bytes > 65_536
    {
        return Err(LanguageExecutionError::Bridge(
            ObservationBridgeError::InvalidLimits,
        ));
    }
    if !(1..=60_000).contains(&request.timeout_ms) {
        return Err(LanguageExecutionError::InvalidTimeout);
    }
    Ok(())
}
