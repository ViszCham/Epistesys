use lc631_core::stable_sha256;
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptPolicy, ReceiptScope, ReceiptVerifier, ReplayGuard, SubjectRevision,
    UntrustedReceipt,
};
use lc631_tldg::{analyze_dg1, Dg1Budget, Dg1ConditionKind, Dg1RequirementPolarity};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

const GOLD_SCHEMA: &str = "epistesys-dgcl-gold.v1";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GoldCorpus {
    pub schema_version: String,
    pub cases: Vec<GoldCase>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GoldCase {
    pub id: String,
    pub family_id: String,
    pub partition: GoldPartition,
    pub language: GoldLanguage,
    pub source: String,
    pub source_revision: String,
    pub annotators: Vec<String>,
    pub adjudicator: String,
    pub gold_requirements: Vec<GoldRequirement>,
    pub critical: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GoldPartition {
    Dev,
    Holdout,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GoldLanguage {
    Japanese,
    English,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GoldRequirement {
    pub span: Range<usize>,
    pub polarity: Dg1GoldPolarity,
    pub condition: Option<Dg1GoldCondition>,
    pub scope: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Dg1GoldPolarity {
    Positive,
    Negative,
    Unknown,
    Conflict,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Dg1GoldCondition {
    Necessary,
    Conditional,
    Temporal,
    Exception,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GoldGateState {
    CorpusInsufficient,
    ThresholdUnmet,
    PendingIndependentReview,
}

#[derive(Clone, Debug, Serialize)]
pub struct LanguageGoldMetrics {
    pub language: GoldLanguage,
    pub holdout_documents: usize,
    pub holdout_gold_obligations: usize,
    pub true_positive: usize,
    pub false_positive: usize,
    pub false_negative: usize,
    pub document_exact: usize,
    pub critical_false_acceptance: usize,
    pub critical_missed_obligations: usize,
    pub precision: Option<f64>,
    pub recall: Option<f64>,
    pub document_exact_rate: Option<f64>,
    pub non_hold_coverage: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct GoldEvaluationReport {
    pub schema_version: &'static str,
    pub corpus_digest: String,
    pub metrics: Vec<LanguageGoldMetrics>,
    pub gate: GoldGateState,
    pub independent_review_verified: bool,
    pub claim_boundary: &'static str,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DgclObservedDecision {
    ImplementationClosed,
    Hold,
    Clarify,
    Conflict,
    NoAction,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DgclDecisionObservation {
    pub case_id: String,
    pub source_revision: String,
    pub decision: DgclObservedDecision,
    pub candidate_digest: Option<String>,
    pub candidate_requirements: Vec<GoldRequirement>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RiskCoveragePoint {
    pub coverage: Option<f64>,
    pub risk: Option<f64>,
    pub accepted_documents: usize,
    pub total_documents: usize,
    pub false_acceptance_documents: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct LanguageDecisionMetrics {
    pub language: GoldLanguage,
    pub holdout_documents: usize,
    pub holdout_gold_obligations: usize,
    pub candidate_nonempty_documents: usize,
    pub accepted_documents: usize,
    pub held_documents: usize,
    pub no_action_documents: usize,
    pub committed_true_positive: usize,
    pub committed_false_positive: usize,
    pub committed_false_negative: usize,
    pub committed_precision: Option<f64>,
    pub committed_recall: Option<f64>,
    pub false_acceptance_documents: usize,
    pub exact_candidate_documents: usize,
    pub false_hold_documents: usize,
    pub decision_coverage: Option<f64>,
    pub selective_risk: Option<f64>,
    pub over_hold_rate: Option<f64>,
    pub risk_coverage: RiskCoveragePoint,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GoldDecisionEvaluationReport {
    pub schema_version: &'static str,
    pub corpus_digest: String,
    pub metrics: Vec<LanguageDecisionMetrics>,
    pub observation_count: usize,
    pub decision_observations_authenticated: bool,
    pub gold_annotation_signatures_verified: bool,
    pub human_independence_verified: bool,
    pub gate: GoldGateState,
    pub legacy_non_hold_coverage_semantics: &'static str,
    pub claim_boundary: &'static str,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GoldSignerAttestation {
    pub signer_id: String,
    pub key_fingerprint: String,
    pub attestation: UntrustedReceipt,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GoldCaseProvenance {
    pub case_id: String,
    pub annotator_attestations: Vec<GoldSignerAttestation>,
    pub adjudicator_attestation: GoldSignerAttestation,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GoldProvenanceBundle {
    pub schema_version: String,
    pub cases: Vec<GoldCaseProvenance>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GoldCaseProvenanceObservation {
    pub case_id: String,
    pub source_revision: String,
    pub distinct_registered_signers: bool,
    pub annotation_receipt_count: usize,
    pub adjudication_receipt_digest: String,
    pub annotation_receipt_digests: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GoldProvenanceReport {
    pub schema_version: &'static str,
    pub corpus_digest: String,
    pub cases: Vec<GoldCaseProvenanceObservation>,
    pub authenticated_case_count: usize,
    pub signer_separation_verified: bool,
    pub human_independence_verified: bool,
    pub claim_boundary: &'static str,
    #[serde(skip)]
    authenticated_content_digest: String,
}

impl GoldProvenanceReport {
    fn content_digest(&self) -> String {
        stable_sha256(
            &serde_json::to_string(&(
                self.schema_version,
                &self.corpus_digest,
                &self.cases,
                self.authenticated_case_count,
                self.signer_separation_verified,
                self.human_independence_verified,
                self.claim_boundary,
            ))
            .expect("provenance report serializes"),
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum GoldError {
    CorpusShape,
    DuplicateCase,
    FamilyLeakage,
    ContentLeakage,
    AnnotationProvenance,
    InvalidGoldSpan,
    DuplicateRequirement,
    Parse(String),
    Encoding,
    DecisionObservationMissing,
    DecisionObservationUnknown,
    DecisionObservationDuplicate,
    DecisionSourceRevisionMismatch,
    DecisionCandidateInvalid,
    ProvenanceBundleShape,
    ProvenanceCaseMissing,
    ProvenanceSignerUnregistered,
    ProvenanceSignerMismatch,
    ProvenanceReceiptRejected,
}

pub fn evaluate_gold(corpus: &GoldCorpus) -> Result<GoldEvaluationReport, GoldError> {
    if corpus.schema_version != GOLD_SCHEMA || corpus.cases.is_empty() {
        return Err(GoldError::CorpusShape);
    }
    let mut case_ids = BTreeSet::new();
    let mut family_partition = BTreeMap::new();
    let mut content_partition = BTreeMap::new();
    let mut metrics =
        [GoldLanguage::Japanese, GoldLanguage::English].map(|language| LanguageGoldMetrics {
            language,
            holdout_documents: 0,
            holdout_gold_obligations: 0,
            true_positive: 0,
            false_positive: 0,
            false_negative: 0,
            document_exact: 0,
            critical_false_acceptance: 0,
            critical_missed_obligations: 0,
            precision: None,
            recall: None,
            document_exact_rate: None,
            non_hold_coverage: None,
        });
    let mut covered = [0_usize; 2];
    for case in &corpus.cases {
        if case.id.is_empty()
            || case.family_id.is_empty()
            || !case_ids.insert(case.id.clone())
            || case.source_revision != stable_sha256(&case.source)
        {
            return Err(GoldError::DuplicateCase);
        }
        if family_partition
            .insert(case.family_id.clone(), case.partition)
            .is_some_and(|prior| prior != case.partition)
        {
            return Err(GoldError::FamilyLeakage);
        }
        if content_partition
            .insert(case.source_revision.clone(), case.partition)
            .is_some_and(|prior| prior != case.partition)
        {
            return Err(GoldError::ContentLeakage);
        }
        let annotators = case.annotators.iter().collect::<BTreeSet<_>>();
        if annotators.len() < 2
            || case.adjudicator.trim().is_empty()
            || case.annotators.iter().any(|name| name.trim().is_empty())
        {
            return Err(GoldError::AnnotationProvenance);
        }
        for requirement in &case.gold_requirements {
            if requirement.span.start >= requirement.span.end
                || case.source.get(requirement.span.clone()).is_none()
            {
                return Err(GoldError::InvalidGoldSpan);
            }
        }
        if case
            .gold_requirements
            .iter()
            .map(gold_key)
            .collect::<BTreeSet<_>>()
            .len()
            != case.gold_requirements.len()
        {
            return Err(GoldError::DuplicateRequirement);
        }
        if case.partition != GoldPartition::Holdout {
            continue;
        }
        let index = if case.language == GoldLanguage::Japanese {
            0
        } else {
            1
        };
        let entry = &mut metrics[index];
        entry.holdout_documents += 1;
        entry.holdout_gold_obligations += case.gold_requirements.len();
        let parsed = analyze_dg1(&case.source, Dg1Budget::default())
            .map_err(|error| GoldError::Parse(format!("{error:?}")))?;
        let predicted = parsed
            .requirement_candidates
            .iter()
            .map(|candidate| GoldRequirement {
                span: candidate.source_span.clone(),
                polarity: match candidate.polarity {
                    Dg1RequirementPolarity::Positive => Dg1GoldPolarity::Positive,
                    Dg1RequirementPolarity::Negative => Dg1GoldPolarity::Negative,
                    Dg1RequirementPolarity::Unknown => Dg1GoldPolarity::Unknown,
                    Dg1RequirementPolarity::Conflict => Dg1GoldPolarity::Conflict,
                },
                condition: candidate.condition_kind.map(|kind| match kind {
                    Dg1ConditionKind::Necessary => Dg1GoldCondition::Necessary,
                    Dg1ConditionKind::Conditional => Dg1GoldCondition::Conditional,
                    Dg1ConditionKind::Temporal => Dg1GoldCondition::Temporal,
                    Dg1ConditionKind::Exception => Dg1GoldCondition::Exception,
                }),
                scope: candidate.scope.clone(),
            })
            .map(|item| gold_key(&item))
            .collect::<BTreeSet<_>>();
        let gold = case
            .gold_requirements
            .iter()
            .map(gold_key)
            .collect::<BTreeSet<_>>();
        let tp = predicted.intersection(&gold).count();
        let fp = predicted.difference(&gold).count();
        let missed = gold.difference(&predicted).count();
        entry.true_positive += tp;
        entry.false_positive += fp;
        entry.false_negative += missed;
        if predicted == gold {
            entry.document_exact += 1
        }
        if case.critical && fp > 0 {
            entry.critical_false_acceptance += 1;
        }
        if case.critical && missed > 0 {
            entry.critical_missed_obligations += 1;
        }
        if !predicted.is_empty() {
            covered[index] += 1
        }
    }
    for (index, entry) in metrics.iter_mut().enumerate() {
        entry.precision = ratio(
            entry.true_positive,
            entry.true_positive + entry.false_positive,
        );
        entry.recall = ratio(
            entry.true_positive,
            entry.true_positive + entry.false_negative,
        );
        entry.document_exact_rate = ratio(entry.document_exact, entry.holdout_documents);
        entry.non_hold_coverage = ratio(covered[index], entry.holdout_documents);
    }
    let sufficient = metrics
        .iter()
        .all(|metric| metric.holdout_documents >= 200 && metric.holdout_gold_obligations >= 1_000);
    let thresholds = metrics.iter().all(|metric| {
        metric.precision.is_some_and(|value| value >= 0.99)
            && metric.recall.is_some_and(|value| value >= 0.98)
            && metric
                .document_exact_rate
                .is_some_and(|value| value >= 0.95)
            && metric.non_hold_coverage.is_some_and(|value| value >= 0.95)
            && metric.critical_false_acceptance == 0
            && metric.critical_missed_obligations == 0
    });
    let canonical = serde_json::to_string(corpus).map_err(|_| GoldError::Encoding)?;
    Ok(GoldEvaluationReport {
        schema_version: "epistesys-dgcl-gold-evaluation.v1",
        corpus_digest: stable_sha256(&canonical),
        metrics: metrics.to_vec(),
        gate: if !sufficient { GoldGateState::CorpusInsufficient }
            else if !thresholds { GoldGateState::ThresholdUnmet }
            else { GoldGateState::PendingIndependentReview },
        independent_review_verified: false,
        claim_boundary: "metrics cover the supplied corpus only; annotator identity and independent adjudication metadata are not authenticated",
    })
}

pub fn gold_annotation_payload_digest(
    case: &GoldCase,
    annotator_id: &str,
) -> Result<String, GoldError> {
    let case_digest = gold_case_content_digest(case)?;
    Ok(stable_sha256(&format!(
        "epistesys-dgcl-gold-annotation.v1\0{case_digest}\0{annotator_id}"
    )))
}

pub fn gold_adjudication_payload_digest(case: &GoldCase) -> Result<String, GoldError> {
    Ok(stable_sha256(&format!(
        "epistesys-dgcl-gold-adjudication.v1\0{}",
        gold_case_content_digest(case)?
    )))
}

pub fn validate_gold_provenance(
    corpus: &GoldCorpus,
    bundle: &GoldProvenanceBundle,
    trusted_signers: &BTreeMap<String, ReceiptVerifier>,
    replay: &mut ReplayGuard,
    now_epoch: u64,
) -> Result<GoldProvenanceReport, GoldError> {
    if bundle.schema_version != "epistesys-dgcl-gold-provenance.v1" {
        return Err(GoldError::ProvenanceBundleShape);
    }
    let _ = evaluate_gold(corpus)?;
    let corpus_canonical = serde_json::to_string(corpus).map_err(|_| GoldError::Encoding)?;
    let corpus_digest = stable_sha256(&corpus_canonical);
    let holdout = corpus
        .cases
        .iter()
        .filter(|case| case.partition == GoldPartition::Holdout)
        .collect::<Vec<_>>();
    let mut provenance_by_case = BTreeMap::new();
    for case_provenance in &bundle.cases {
        if provenance_by_case
            .insert(case_provenance.case_id.as_str(), case_provenance)
            .is_some()
        {
            return Err(GoldError::ProvenanceBundleShape);
        }
    }
    if provenance_by_case.len() != holdout.len()
        || provenance_by_case
            .keys()
            .any(|case_id| !holdout.iter().any(|case| case.id == **case_id))
    {
        return Err(GoldError::ProvenanceBundleShape);
    }
    let mut observations = Vec::new();
    for case in holdout {
        let provenance = provenance_by_case
            .get(case.id.as_str())
            .copied()
            .ok_or(GoldError::ProvenanceCaseMissing)?;
        let expected_annotators = case.annotators.iter().cloned().collect::<BTreeSet<_>>();
        let observed_annotators = provenance
            .annotator_attestations
            .iter()
            .map(|attestation| attestation.signer_id.clone())
            .collect::<BTreeSet<_>>();
        if expected_annotators.len() != case.annotators.len()
            || observed_annotators != expected_annotators
            || provenance.annotator_attestations.len() != case.annotators.len()
            || provenance.adjudicator_attestation.signer_id != case.adjudicator
            || expected_annotators.contains(&case.adjudicator)
        {
            return Err(GoldError::AnnotationProvenance);
        }
        let mut signer_fingerprints = BTreeSet::new();
        let mut annotation_receipt_digests = Vec::new();
        for annotation in &provenance.annotator_attestations {
            let verifier = trusted_signers
                .get(&annotation.signer_id)
                .ok_or(GoldError::ProvenanceSignerUnregistered)?;
            if annotation.key_fingerprint != verifier.key_fingerprint()
                || !signer_fingerprints.insert(annotation.key_fingerprint.clone())
            {
                return Err(GoldError::ProvenanceSignerMismatch);
            }
            let scope = ReceiptScope::checked(format!(
                "dgcl/gold/{}/annotation/{}",
                case.id, annotation.signer_id
            ))
            .map_err(|_| GoldError::ProvenanceBundleShape)?;
            let subject = SubjectRevision::checked(&case.source_revision)
                .map_err(|_| GoldError::ProvenanceBundleShape)?;
            let payload = gold_annotation_payload_digest(case, &annotation.signer_id)?;
            let verified = verifier
                .verify(
                    annotation.attestation.clone(),
                    &ReceiptPolicy::exact(
                        ReceiptClass::ReleaseEvaluation,
                        subject,
                        scope,
                        payload,
                        now_epoch,
                    ),
                    replay,
                )
                .map_err(|_| GoldError::ProvenanceReceiptRejected)?;
            annotation_receipt_digests.push(verified.receipt_digest());
        }
        let adjudicator = &provenance.adjudicator_attestation;
        let verifier = trusted_signers
            .get(&adjudicator.signer_id)
            .ok_or(GoldError::ProvenanceSignerUnregistered)?;
        if adjudicator.key_fingerprint != verifier.key_fingerprint()
            || !signer_fingerprints.insert(adjudicator.key_fingerprint.clone())
        {
            return Err(GoldError::ProvenanceSignerMismatch);
        }
        let subject = SubjectRevision::checked(&case.source_revision)
            .map_err(|_| GoldError::ProvenanceBundleShape)?;
        let scope = ReceiptScope::checked(format!(
            "dgcl/gold/{}/adjudication/{}",
            case.id, adjudicator.signer_id
        ))
        .map_err(|_| GoldError::ProvenanceBundleShape)?;
        let verified = verifier
            .verify(
                adjudicator.attestation.clone(),
                &ReceiptPolicy::exact(
                    ReceiptClass::ReleaseEvaluation,
                    subject,
                    scope,
                    gold_adjudication_payload_digest(case)?,
                    now_epoch,
                ),
                replay,
            )
            .map_err(|_| GoldError::ProvenanceReceiptRejected)?;
        annotation_receipt_digests.sort();
        observations.push(GoldCaseProvenanceObservation {
            case_id: case.id.clone(),
            source_revision: case.source_revision.clone(),
            distinct_registered_signers: true,
            annotation_receipt_count: annotation_receipt_digests.len(),
            adjudication_receipt_digest: verified.receipt_digest(),
            annotation_receipt_digests,
        });
    }
    observations.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    let signer_separation_verified = !observations.is_empty();
    let mut report = GoldProvenanceReport {
        schema_version: "epistesys-dgcl-gold-provenance-report.v1",
        corpus_digest,
        authenticated_case_count: observations.len(),
        cases: observations,
        signer_separation_verified,
        human_independence_verified: false,
        claim_boundary: "receipt signatures bind the listed case labels to trusted keys and distinct signer identifiers; they do not prove real-human identity, independent thought, or unbiased adjudication",
        authenticated_content_digest: String::new(),
    };
    report.authenticated_content_digest = report.content_digest();
    Ok(report)
}

pub fn evaluate_gold_decisions(
    corpus: &GoldCorpus,
    observations: &[DgclDecisionObservation],
    provenance: Option<&GoldProvenanceReport>,
) -> Result<GoldDecisionEvaluationReport, GoldError> {
    let _ = evaluate_gold(corpus)?;
    let canonical = serde_json::to_string(corpus).map_err(|_| GoldError::Encoding)?;
    let corpus_digest = stable_sha256(&canonical);
    if provenance.is_some_and(|report| {
        report.corpus_digest != corpus_digest
            || report.authenticated_content_digest != report.content_digest()
    }) {
        return Err(GoldError::ProvenanceBundleShape);
    }
    let holdout = corpus
        .cases
        .iter()
        .filter(|case| case.partition == GoldPartition::Holdout)
        .map(|case| (case.id.as_str(), case))
        .collect::<BTreeMap<_, _>>();
    let mut observations_by_id = BTreeMap::new();
    for observation in observations {
        if observations_by_id
            .insert(observation.case_id.as_str(), observation)
            .is_some()
        {
            return Err(GoldError::DecisionObservationDuplicate);
        }
        if !holdout.contains_key(observation.case_id.as_str()) {
            return Err(GoldError::DecisionObservationUnknown);
        }
    }
    if observations_by_id.len() != holdout.len() {
        return Err(GoldError::DecisionObservationMissing);
    }
    let mut metrics =
        [GoldLanguage::Japanese, GoldLanguage::English].map(|language| LanguageDecisionMetrics {
            language,
            holdout_documents: 0,
            holdout_gold_obligations: 0,
            candidate_nonempty_documents: 0,
            accepted_documents: 0,
            held_documents: 0,
            no_action_documents: 0,
            committed_true_positive: 0,
            committed_false_positive: 0,
            committed_false_negative: 0,
            committed_precision: None,
            committed_recall: None,
            false_acceptance_documents: 0,
            exact_candidate_documents: 0,
            false_hold_documents: 0,
            decision_coverage: None,
            selective_risk: None,
            over_hold_rate: None,
            risk_coverage: RiskCoveragePoint {
                coverage: None,
                risk: None,
                accepted_documents: 0,
                total_documents: 0,
                false_acceptance_documents: 0,
            },
        });
    for (case_id, case) in &holdout {
        let observation = observations_by_id[case_id];
        if observation.source_revision != case.source_revision {
            return Err(GoldError::DecisionSourceRevisionMismatch);
        }
        if observation.decision == DgclObservedDecision::ImplementationClosed
            && observation
                .candidate_digest
                .as_deref()
                .is_none_or(|digest| !valid_digest(digest))
        {
            return Err(GoldError::DecisionCandidateInvalid);
        }
        if observation
            .candidate_digest
            .as_deref()
            .is_some_and(|digest| !valid_digest(digest))
            || (observation.decision == DgclObservedDecision::NoAction
                && !observation.candidate_requirements.is_empty())
        {
            return Err(GoldError::DecisionCandidateInvalid);
        }
        for requirement in &observation.candidate_requirements {
            if requirement.span.start >= requirement.span.end
                || case.source.get(requirement.span.clone()).is_none()
            {
                return Err(GoldError::InvalidGoldSpan);
            }
        }
        if observation
            .candidate_requirements
            .iter()
            .map(gold_key)
            .collect::<BTreeSet<_>>()
            .len()
            != observation.candidate_requirements.len()
        {
            return Err(GoldError::DuplicateRequirement);
        }
        let index = if case.language == GoldLanguage::Japanese {
            0
        } else {
            1
        };
        let metric = &mut metrics[index];
        metric.holdout_documents += 1;
        metric.holdout_gold_obligations += case.gold_requirements.len();
        let gold = case
            .gold_requirements
            .iter()
            .map(gold_key)
            .collect::<BTreeSet<_>>();
        let candidates = observation
            .candidate_requirements
            .iter()
            .map(gold_key)
            .collect::<BTreeSet<_>>();
        let exact_candidate = candidates == gold;
        if exact_candidate {
            metric.exact_candidate_documents += 1;
        }
        if !candidates.is_empty() {
            metric.candidate_nonempty_documents += 1;
        }
        if observation.decision == DgclObservedDecision::ImplementationClosed {
            metric.accepted_documents += 1;
            let true_positive = candidates.intersection(&gold).count();
            let false_positive = candidates.difference(&gold).count();
            let false_negative = gold.difference(&candidates).count();
            metric.committed_true_positive += true_positive;
            metric.committed_false_positive += false_positive;
            metric.committed_false_negative += false_negative;
            if !exact_candidate {
                metric.false_acceptance_documents += 1;
            }
        } else if observation.decision == DgclObservedDecision::NoAction {
            metric.no_action_documents += 1;
            metric.committed_false_negative += gold.len();
        } else {
            metric.held_documents += 1;
            metric.committed_false_negative += gold.len();
            if !gold.is_empty() && exact_candidate {
                metric.false_hold_documents += 1;
            }
        }
    }
    for metric in &mut metrics {
        metric.committed_precision = ratio(
            metric.committed_true_positive,
            metric.committed_true_positive + metric.committed_false_positive,
        );
        metric.committed_recall = ratio(
            metric.committed_true_positive,
            metric.holdout_gold_obligations,
        );
        metric.decision_coverage = ratio(metric.accepted_documents, metric.holdout_documents);
        metric.selective_risk = ratio(metric.false_acceptance_documents, metric.accepted_documents);
        metric.over_hold_rate = ratio(
            metric.false_hold_documents,
            metric.exact_candidate_documents,
        );
        metric.risk_coverage = RiskCoveragePoint {
            coverage: metric.decision_coverage,
            risk: metric.selective_risk,
            accepted_documents: metric.accepted_documents,
            total_documents: metric.holdout_documents,
            false_acceptance_documents: metric.false_acceptance_documents,
        };
    }
    let sufficient = metrics
        .iter()
        .all(|metric| metric.holdout_documents >= 200 && metric.holdout_gold_obligations >= 1_000);
    let threshold_unmet = metrics.iter().any(|metric| {
        metric.committed_precision.is_some_and(|value| value < 0.99)
            || metric.committed_recall.is_some_and(|value| value < 0.98)
            || metric.decision_coverage.is_some_and(|value| value < 0.95)
            || metric.selective_risk.is_some_and(|value| value > 0.01)
            || metric.over_hold_rate.is_some_and(|value| value > 0.05)
            || metric.false_acceptance_documents > 0
    });
    let gold_annotation_signatures_verified = provenance.is_some_and(|report| {
        !holdout.is_empty()
            && report.corpus_digest == corpus_digest
            && report.authenticated_case_count == holdout.len()
    });
    Ok(GoldDecisionEvaluationReport {
        schema_version: "epistesys-dgcl-decision-evaluation.v1",
        corpus_digest,
        observation_count: observations.len(),
        metrics: metrics.to_vec(),
        decision_observations_authenticated: false,
        gold_annotation_signatures_verified,
        human_independence_verified: false,
        gate: if !sufficient {
            GoldGateState::CorpusInsufficient
        } else if threshold_unmet {
            GoldGateState::ThresholdUnmet
        } else {
            GoldGateState::PendingIndependentReview
        },
        legacy_non_hold_coverage_semantics: "legacy metric is nonempty parser-candidate documents divided by holdout documents; it is not decision coverage",
        claim_boundary: "decision metrics are computed over supplied per-document observations and the supplied corpus; decision observations are not authenticated here, human independence remains unverified, and no general reliability or operating-frontier claim follows",
    })
}

pub fn gold_case_content_digest(case: &GoldCase) -> Result<String, GoldError> {
    let canonical = serde_json::to_string(&(
        case.id.as_str(),
        case.family_id.as_str(),
        case.partition,
        case.language,
        case.source_revision.as_str(),
        &case.gold_requirements,
        case.critical,
    ))
    .map_err(|_| GoldError::Encoding)?;
    Ok(stable_sha256(&canonical))
}

fn ratio(numerator: usize, denominator: usize) -> Option<f64> {
    (denominator != 0).then(|| numerator as f64 / denominator as f64)
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn gold_key(
    item: &GoldRequirement,
) -> (
    usize,
    usize,
    Dg1GoldPolarity,
    Option<Dg1GoldCondition>,
    Option<String>,
) {
    (
        item.span.start,
        item.span.end,
        item.polarity,
        item.condition,
        item.scope.clone(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use lc631_receipt_kernel::ReceiptIssuer;

    fn case(
        id: &str,
        family: &str,
        partition: GoldPartition,
        language: GoldLanguage,
        source: &str,
        gold_requirements: Vec<GoldRequirement>,
    ) -> GoldCase {
        GoldCase {
            id: id.into(),
            family_id: family.into(),
            partition,
            language,
            source: source.into(),
            source_revision: stable_sha256(source),
            annotators: vec!["reviewer-a".into(), "reviewer-b".into()],
            adjudicator: "adjudicator".into(),
            gold_requirements,
            critical: true,
        }
    }

    #[test]
    fn metrics_keep_missing_predictions_in_the_denominator() {
        let source = "Please verify tests.";
        let corpus = GoldCorpus {
            schema_version: GOLD_SCHEMA.into(),
            cases: vec![
                case(
                    "en-1",
                    "family-1",
                    GoldPartition::Holdout,
                    GoldLanguage::English,
                    source,
                    vec![GoldRequirement {
                        span: 0..source.len(),
                        polarity: Dg1GoldPolarity::Positive,
                        condition: None,
                        scope: None,
                    }],
                ),
                case(
                    "ja-1",
                    "family-2",
                    GoldPartition::Holdout,
                    GoldLanguage::Japanese,
                    "説明文のみ。",
                    vec![],
                ),
            ],
        };
        let report = evaluate_gold(&corpus).unwrap();
        assert_eq!(report.gate, GoldGateState::CorpusInsufficient);
        assert_eq!(report.metrics[1].true_positive, 1);
        assert_eq!(report.metrics[0].precision, None);
        assert!(!report.independent_review_verified);
    }

    #[test]
    fn split_by_name_does_not_hide_family_leakage() {
        let corpus = GoldCorpus {
            schema_version: GOLD_SCHEMA.into(),
            cases: vec![
                case(
                    "dev",
                    "same-family",
                    GoldPartition::Dev,
                    GoldLanguage::English,
                    "one",
                    vec![],
                ),
                case(
                    "hold",
                    "same-family",
                    GoldPartition::Holdout,
                    GoldLanguage::English,
                    "two",
                    vec![],
                ),
            ],
        };
        assert!(matches!(
            evaluate_gold(&corpus),
            Err(GoldError::FamilyLeakage)
        ));
    }

    #[test]
    fn decision_metrics_use_committed_outputs_and_report_over_hold_separately() {
        let first_source = "Please test the package.";
        let held_source = "Please run the tests.";
        let false_accept_source = "Please publish the package.";
        let first_gold = GoldRequirement {
            span: 0..first_source.len(),
            polarity: Dg1GoldPolarity::Positive,
            condition: None,
            scope: None,
        };
        let held_gold = GoldRequirement {
            span: 0..held_source.len(),
            polarity: Dg1GoldPolarity::Positive,
            condition: None,
            scope: None,
        };
        let false_accept = GoldRequirement {
            span: 0..false_accept_source.len(),
            polarity: Dg1GoldPolarity::Positive,
            condition: None,
            scope: None,
        };
        let corpus = GoldCorpus {
            schema_version: GOLD_SCHEMA.into(),
            cases: vec![
                case(
                    "accepted-correct",
                    "f-1",
                    GoldPartition::Holdout,
                    GoldLanguage::English,
                    first_source,
                    vec![first_gold.clone()],
                ),
                case(
                    "held-correct-candidate",
                    "f-2",
                    GoldPartition::Holdout,
                    GoldLanguage::English,
                    held_source,
                    vec![held_gold.clone()],
                ),
                case(
                    "accepted-false",
                    "f-3",
                    GoldPartition::Holdout,
                    GoldLanguage::English,
                    false_accept_source,
                    vec![],
                ),
            ],
        };
        let observations = vec![
            DgclDecisionObservation {
                case_id: "accepted-correct".into(),
                source_revision: stable_sha256(first_source),
                decision: DgclObservedDecision::ImplementationClosed,
                candidate_digest: Some(stable_sha256("accepted output 1")),
                candidate_requirements: vec![first_gold],
            },
            DgclDecisionObservation {
                case_id: "held-correct-candidate".into(),
                source_revision: stable_sha256(held_source),
                decision: DgclObservedDecision::Hold,
                candidate_digest: None,
                candidate_requirements: vec![held_gold],
            },
            DgclDecisionObservation {
                case_id: "accepted-false".into(),
                source_revision: stable_sha256(false_accept_source),
                decision: DgclObservedDecision::ImplementationClosed,
                candidate_digest: Some(stable_sha256("false output")),
                candidate_requirements: vec![false_accept],
            },
        ];
        let report = evaluate_gold_decisions(&corpus, &observations, None).unwrap();
        let english = &report.metrics[1];
        assert_eq!(english.holdout_documents, 3);
        assert_eq!(english.accepted_documents, 2);
        assert_eq!(english.committed_true_positive, 1);
        assert_eq!(english.committed_false_positive, 1);
        assert_eq!(english.committed_false_negative, 1);
        assert_eq!(english.false_acceptance_documents, 1);
        assert_eq!(english.false_hold_documents, 1);
        assert_eq!(english.decision_coverage, Some(2.0 / 3.0));
        assert_eq!(english.selective_risk, Some(0.5));
        assert_eq!(english.over_hold_rate, Some(0.5));
        assert!(!report.decision_observations_authenticated);
        assert!(!report.human_independence_verified);
        assert_eq!(report.gate, GoldGateState::CorpusInsufficient);
    }

    #[test]
    fn decision_evaluation_rejects_missing_duplicates_and_stale_observations() {
        let source = "Quoted-only content.";
        let corpus = GoldCorpus {
            schema_version: GOLD_SCHEMA.into(),
            cases: vec![case(
                "context-only",
                "context-family",
                GoldPartition::Holdout,
                GoldLanguage::English,
                source,
                vec![],
            )],
        };
        let observation = DgclDecisionObservation {
            case_id: "context-only".into(),
            source_revision: stable_sha256(source),
            decision: DgclObservedDecision::NoAction,
            candidate_digest: None,
            candidate_requirements: vec![],
        };
        assert!(matches!(
            evaluate_gold_decisions(&corpus, &[], None),
            Err(GoldError::DecisionObservationMissing)
        ));
        assert!(matches!(
            evaluate_gold_decisions(&corpus, &[observation.clone(), observation.clone()], None),
            Err(GoldError::DecisionObservationDuplicate)
        ));
        let mut stale = observation;
        stale.source_revision = stable_sha256("different source");
        assert!(matches!(
            evaluate_gold_decisions(&corpus, &[stale], None),
            Err(GoldError::DecisionSourceRevisionMismatch)
        ));

        let dev_only = GoldCorpus {
            schema_version: GOLD_SCHEMA.into(),
            cases: vec![case(
                "dev-only",
                "dev-family",
                GoldPartition::Dev,
                GoldLanguage::English,
                "Please test.",
                vec![],
            )],
        };
        let empty_metrics = evaluate_gold_decisions(&dev_only, &[], None).unwrap();
        assert_eq!(empty_metrics.metrics[1].decision_coverage, None);
        assert_eq!(empty_metrics.metrics[1].committed_precision, None);
        assert_eq!(empty_metrics.metrics[1].selective_risk, None);
        assert_eq!(empty_metrics.gate, GoldGateState::CorpusInsufficient);
    }

    #[test]
    fn signer_receipts_are_verified_but_do_not_claim_human_independence() {
        let source = "Quoted-only content.";
        let gold_case = case(
            "context-only",
            "context-family",
            GoldPartition::Holdout,
            GoldLanguage::English,
            source,
            vec![],
        );
        let case_digest = gold_case_content_digest(&gold_case).unwrap();
        let signers = [
            ("reviewer-a", [201_u8; 32]),
            ("reviewer-b", [202_u8; 32]),
            ("adjudicator", [203_u8; 32]),
        ];
        let mut trusted = BTreeMap::new();
        let mut attestations = BTreeMap::new();
        for (signer_id, key) in signers {
            let issuer = ReceiptIssuer::from_key_bytes(signer_id, &key).unwrap();
            let verifier = ReceiptVerifier::from_key_bytes(signer_id, &key).unwrap();
            let scope_name = if signer_id == "adjudicator" {
                format!("dgcl/gold/{}/adjudication/{}", gold_case.id, signer_id)
            } else {
                format!("dgcl/gold/{}/annotation/{}", gold_case.id, signer_id)
            };
            let payload = if signer_id == "adjudicator" {
                gold_adjudication_payload_digest(&gold_case).unwrap()
            } else {
                gold_annotation_payload_digest(&gold_case, signer_id).unwrap()
            };
            let wire = issuer
                .issue(
                    ReceiptClass::ReleaseEvaluation,
                    SubjectRevision::checked(&gold_case.source_revision).unwrap(),
                    ReceiptScope::checked(scope_name).unwrap(),
                    payload,
                    100,
                    Some(200),
                    None,
                )
                .unwrap();
            attestations.insert(
                signer_id.to_string(),
                GoldSignerAttestation {
                    signer_id: signer_id.to_string(),
                    key_fingerprint: verifier.key_fingerprint().to_string(),
                    attestation: wire,
                },
            );
            trusted.insert(signer_id.to_string(), verifier);
        }
        let provenance = GoldProvenanceBundle {
            schema_version: "epistesys-dgcl-gold-provenance.v1".into(),
            cases: vec![GoldCaseProvenance {
                case_id: gold_case.id.clone(),
                annotator_attestations: vec![
                    attestations.remove("reviewer-a").unwrap(),
                    attestations.remove("reviewer-b").unwrap(),
                ],
                adjudicator_attestation: attestations.remove("adjudicator").unwrap(),
            }],
        };
        let corpus = GoldCorpus {
            schema_version: GOLD_SCHEMA.into(),
            cases: vec![gold_case],
        };
        let verified = validate_gold_provenance(
            &corpus,
            &provenance,
            &trusted,
            &mut ReplayGuard::default(),
            110,
        )
        .unwrap();
        assert_eq!(verified.authenticated_case_count, 1);
        assert!(verified.signer_separation_verified);
        assert!(!verified.human_independence_verified);
        assert_eq!(verified.cases[0].annotation_receipt_count, 2);
        let mut forged = verified.clone();
        forged.authenticated_case_count = 200;
        assert!(matches!(
            evaluate_gold_decisions(&corpus, &[], Some(&forged)),
            Err(GoldError::ProvenanceBundleShape)
        ));
        assert_eq!(
            verified.corpus_digest,
            stable_sha256(&serde_json::to_string(&corpus).unwrap())
        );
        let _ = case_digest;
    }
}
