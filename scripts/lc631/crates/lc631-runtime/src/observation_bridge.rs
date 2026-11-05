//! Reported NLP fragments become Proposed premises, never verified propositions.
//! Registration pins and source checks establish structural correspondence only.
#![forbid(unsafe_code)]
pub mod worker;
pub mod media;
use lc631_analysis::{DependencySentence, LanguageObservation};
use lc631_core::context::{
    ContextAdmission, ContextConsumer, ContextDelta, ContextError, ContextPremise, ContextRevision,
    ContextState, PremiseBasis, PremiseEdit,
};
use lc631_core::AuthorityRevision;
use lc631_substrate::source::{SourceSpan, SourceStore};
use lc631_substrate::Digest32;
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug)]
pub struct LanguageRegistration<'a> {
    pub language: &'a str,
    pub backend: &'a str,
    pub model: Digest32,
    pub runtime: Digest32,
    pub requirements: Digest32,
}

/// All fields are caller reports. This is deliberately not an execution receipt.
#[derive(Clone, Copy, Debug)]
pub struct ReportedLanguageInput<'a> {
    pub source: Digest32,
    pub registration: LanguageRegistration<'a>,
    pub payload: Digest32,
    pub sentences: &'a [DependencySentence],
}

impl<'a> ReportedLanguageInput<'a> {
    /// Borrows the actual worker API without asserting that public report fields
    /// are unforgeable, or recomputing its legacy JSON payload hash.
    pub fn from_observation(
        value: &'a LanguageObservation,
    ) -> Result<Self, ObservationBridgeError> {
        if value.schema_version != "epistesys-dgcl-nlp.v1" || value.semantic_truth_claim {
            return Err(ObservationBridgeError::UnsupportedReport);
        }
        Ok(Self {
            source: parse_digest(&value.source_revision)?,
            registration: LanguageRegistration {
                language: &value.language,
                backend: &value.backend_revision,
                model: parse_digest(&value.model_manifest_digest)?,
                runtime: parse_digest(&value.runtime_manifest_digest)?,
                requirements: parse_digest(&value.requirements_lock_digest)?,
            },
            payload: parse_digest(&value.payload_digest)?,
            sentences: &value.sentences,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ObservationBridgeLimits {
    pub max_sentences: usize,
    pub max_tokens: usize,
    pub max_fragment_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObservationBridgeError {
    InvalidLimits,
    UnsupportedReport,
    MalformedDigest,
    SourceMismatch,
    RegistrationMismatch,
    EmptyObservation,
    InvalidToken,
    BudgetExceeded,
    AllocationFailed,
    Context(ContextError),
}

/// Keeps source/payload/registration reports attached to the prepared Delta.
/// It contains no authority receipt, semantic-validation token or completion flag.
pub struct LanguagePremiseBatch<'a> {
    delta: ContextDelta,
    report: ReportedLanguageInput<'a>,
    count: usize,
}

pub struct AdmittedLanguageCandidates<'a> {
    pub admission: ContextAdmission,
    pub report: ReportedLanguageInput<'a>,
    pub candidate_count: usize,
}

impl<'a> LanguagePremiseBatch<'a> {
    pub fn delta(&self) -> &ContextDelta {
        &self.delta
    }

    /// Actual existing core consumer checks parent/scope/authority identity,
    /// hard invariants and budgets. Success admits proposals, not their truth.
    pub fn admit(
        self,
        consumer: &mut ContextConsumer,
    ) -> Result<AdmittedLanguageCandidates<'a>, ObservationBridgeError> {
        let admission = consumer
            .admit(self.delta)
            .map_err(ObservationBridgeError::Context)?;
        Ok(AdmittedLanguageCandidates {
            admission,
            report: self.report,
            candidate_count: self.count,
        })
    }
}

pub fn prepare_language_candidates<'a>(
    source: &SourceStore,
    parent: &ContextState,
    authority: AuthorityRevision,
    registration: LanguageRegistration<'_>,
    report: ReportedLanguageInput<'a>,
    limits: ObservationBridgeLimits,
) -> Result<LanguagePremiseBatch<'a>, ObservationBridgeError> {
    validate_input(source, parent.revision(), registration, report, limits)?;
    let mut edits = Vec::new();
    edits
        .try_reserve_exact(report.sentences.len())
        .map_err(|_| ObservationBridgeError::AllocationFailed)?;
    let mut token_count = 0usize;
    let mut byte_count = 0usize;
    let mut previous_end = 0usize;
    for sentence in report.sentences {
        let first = sentence
            .tokens
            .first()
            .ok_or(ObservationBridgeError::EmptyObservation)?;
        let mut end = first.start_byte;
        for token in &sentence.tokens {
            token_count = token_count
                .checked_add(1)
                .ok_or(ObservationBridgeError::BudgetExceeded)?;
            if token_count > limits.max_tokens {
                return Err(ObservationBridgeError::BudgetExceeded);
            }
            if token.start_byte < previous_end || token.start_byte >= token.end_byte {
                return Err(ObservationBridgeError::InvalidToken);
            }
            let fragment_bytes = token
                .end_byte
                .checked_sub(first.start_byte)
                .and_then(|length| byte_count.checked_add(length))
                .ok_or(ObservationBridgeError::BudgetExceeded)?;
            if fragment_bytes > limits.max_fragment_bytes {
                return Err(ObservationBridgeError::BudgetExceeded);
            }
            let span = SourceSpan::checked(source, token.start_byte, token.end_byte)
                .map_err(|_| ObservationBridgeError::InvalidToken)?;
            if source
                .slice(span)
                .map_err(|_| ObservationBridgeError::InvalidToken)?
                != token.text
            {
                return Err(ObservationBridgeError::InvalidToken);
            }
            end = token.end_byte;
            previous_end = end;
        }
        byte_count = byte_count
            .checked_add(end - first.start_byte)
            .ok_or(ObservationBridgeError::BudgetExceeded)?;
        if byte_count > limits.max_fragment_bytes {
            return Err(ObservationBridgeError::BudgetExceeded);
        }
        let span = SourceSpan::checked(source, first.start_byte, end)
            .map_err(|_| ObservationBridgeError::InvalidToken)?;
        let id = fragment_id(report, span);
        let premise = ContextPremise::from_source(id, source, span, PremiseBasis::Proposed, true)
            .map_err(|_| ObservationBridgeError::InvalidToken)?;
        edits.push(PremiseEdit::Add(premise));
    }
    Ok(LanguagePremiseBatch {
        count: edits.len(),
        delta: ContextDelta::proposed(
            parent.revision(),
            parent.scope(),
            authority,
            parent.applicability(),
            edits,
        ),
        report,
    })
}

fn validate_input(
    source: &SourceStore,
    parent: ContextRevision,
    expected: LanguageRegistration<'_>,
    report: ReportedLanguageInput<'_>,
    limits: ObservationBridgeLimits,
) -> Result<(), ObservationBridgeError> {
    if limits.max_sentences == 0
        || limits.max_sentences > 1_024
        || limits.max_tokens == 0
        || limits.max_tokens > 4_096
        || limits.max_fragment_bytes == 0
        || limits.max_fragment_bytes > 65_536
    {
        return Err(ObservationBridgeError::InvalidLimits);
    }
    if source.id().digest() != report.source || parent.source().digest() != report.source {
        return Err(ObservationBridgeError::SourceMismatch);
    }
    let actual = report.registration;
    if !matches!(expected.language, "ja" | "en")
        || expected.backend.is_empty()
        || expected.backend.len() > 256
        || report.registration.backend.len() > 256
        || expected.language != actual.language
        || expected.backend != actual.backend
        || expected.model != actual.model
        || expected.runtime != actual.runtime
        || expected.requirements != actual.requirements
    {
        return Err(ObservationBridgeError::RegistrationMismatch);
    }
    if report.sentences.is_empty() {
        return Err(ObservationBridgeError::EmptyObservation);
    }
    if report.sentences.len() > limits.max_sentences {
        return Err(ObservationBridgeError::BudgetExceeded);
    }
    Ok(())
}

fn fragment_id(report: ReportedLanguageInput<'_>, span: SourceSpan) -> Digest32 {
    let mut hash = Sha256::new();
    hash.update(b"epistesys.reported-language-fragment.v1\0");
    for digest in [
        report.source,
        report.payload,
        report.registration.model,
        report.registration.runtime,
        report.registration.requirements,
    ] {
        hash.update(digest.as_bytes());
    }
    for text in [report.registration.language, report.registration.backend] {
        hash.update((text.len() as u64).to_le_bytes());
        hash.update(text.as_bytes());
    }
    // SourceSpan has checked u32 offsets; conversion here is lossless.
    hash.update((span.start() as u64).to_le_bytes());
    hash.update((span.end() as u64).to_le_bytes());
    Digest32::from_bytes(hash.finalize().into())
}

fn parse_digest(text: &str) -> Result<Digest32, ObservationBridgeError> {
    let hex = text
        .strip_prefix("sha256:")
        .ok_or(ObservationBridgeError::MalformedDigest)?;
    if hex.len() != 64 {
        return Err(ObservationBridgeError::MalformedDigest);
    }
    let mut bytes = [0u8; 32];
    for (index, pair) in hex.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        let high = nibble(pair[0])?;
        let low = nibble(pair[1])?;
        bytes[index] = (high << 4) | low;
    }
    Ok(Digest32::from_bytes(bytes))
}

fn nibble(byte: u8) -> Result<u8, ObservationBridgeError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(ObservationBridgeError::MalformedDigest),
    }
}
