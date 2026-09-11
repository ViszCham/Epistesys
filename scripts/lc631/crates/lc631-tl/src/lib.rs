#![forbid(unsafe_code)]

mod projection_v2;
mod projection_v3;
mod regime;

use lc631_core::{ObligationId, SourceSpan};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

pub use projection_v2::{
    CorrespondenceWitnessV2, ProjectionDefectGraph, ProjectionV2Error, SourceAnchor,
    VerifierCapability, TLDG_PROJECTION_SCHEMA,
};
pub use projection_v3::*;
pub use regime::{MeasurementRegime, MeasurementRegimeSet};

pub const TL_WITNESS_SCHEMA: &str = "lc631-projection-witness.v1";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ObligationStrength {
    May,
    Should,
    Must,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ObligationKind {
    AnswerKind,
    Authority,
    CausalFrame,
    Evidence,
    Modality,
    OutputContract,
    Polarity,
    Scope,
    Semantic,
    Structural,
    TemporalFrame,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ObligationPolarity {
    Positive,
    Negative,
    Unknown,
    Conflict,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Obligation {
    pub id: ObligationId,
    pub kind: ObligationKind,
    pub strength: ObligationStrength,
    pub polarity: ObligationPolarity,
    pub scope: String,
    pub source_span: SourceSpan,
    pub source_text: String,
}

impl Obligation {
    pub fn validate(&self, source: &str) -> Result<(), WitnessError> {
        let anchored = self
            .source_span
            .slice(source)
            .map_err(|_| WitnessError::InvalidSourceSpan(self.id))?;
        if anchored != self.source_text {
            return Err(WitnessError::SourceAnchorMismatch(self.id));
        }
        if self.strength == ObligationStrength::Must && anchored.trim().is_empty() {
            return Err(WitnessError::EmptyMustObligation(self.id));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionStage {
    SeedToContract,
    ContractToProgram,
    ProgramToCandidate,
    CandidateToOutput,
    AudioToTranscript,
    FrameToObservation,
    ObservationToEvent,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreservationState {
    Exact,
    Refinement,
    Weakened,
    Strengthened,
    Contradicted,
    Dropped,
    IntroducedWithoutSource,
    Unresolved,
    Incomparable,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VerifierKind {
    SourceSpan,
    Schema,
    TypeChecker,
    Compiler,
    Smt,
    Roundtrip,
    RuntimeObservation,
    HumanReview,
    ModelAdvisory,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct VerifierDescriptor {
    pub kind: VerifierKind,
    pub revision: String,
    pub authoritative_for_exact: bool,
    pub available: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct VerifierRegistry {
    entries: BTreeMap<VerifierKind, VerifierDescriptor>,
}

impl VerifierRegistry {
    pub fn register(&mut self, descriptor: VerifierDescriptor) -> Result<(), WitnessError> {
        if self.entries.contains_key(&descriptor.kind) {
            return Err(WitnessError::DuplicateVerifier(descriptor.kind));
        }
        self.entries.insert(descriptor.kind, descriptor);
        Ok(())
    }

    pub fn can_verify_exact(&self, kind: VerifierKind) -> bool {
        self.entries
            .get(&kind)
            .is_some_and(|descriptor| descriptor.available && descriptor.authoritative_for_exact)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    Passed,
    Failed,
    NeedsEvidence,
    Unavailable,
    AdvisoryOnly,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct VerificationReceipt {
    pub verifier: VerifierKind,
    pub status: VerificationStatus,
    pub revision: String,
    pub evidence_digest: Option<String>,
}

impl VerificationReceipt {
    pub fn authoritative_for_exact(&self) -> bool {
        self.status == VerificationStatus::Passed
            && !matches!(self.verifier, VerifierKind::ModelAdvisory)
            && self.evidence_digest.is_some()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TargetAnchor {
    pub artifact: String,
    pub path: String,
    pub span: Option<SourceSpan>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProjectionWitness {
    pub schema_version: &'static str,
    pub obligation_id: ObligationId,
    pub stage: ProjectionStage,
    pub state: PreservationState,
    pub target: Option<TargetAnchor>,
    pub rule_id: String,
    pub verifier: VerificationReceipt,
    pub counterexample: Option<String>,
}

impl ProjectionWitness {
    pub fn validate(&self) -> Result<(), WitnessError> {
        match self.state {
            PreservationState::Exact | PreservationState::Refinement => {
                if self.target.is_none() {
                    return Err(WitnessError::MissingTarget(self.obligation_id));
                }
                if !self.verifier.authoritative_for_exact() {
                    return Err(WitnessError::WeakExactVerifier(self.obligation_id));
                }
            }
            PreservationState::IntroducedWithoutSource => {
                return Err(WitnessError::IntroducedStateCannotClaimObligation(
                    self.obligation_id,
                ));
            }
            PreservationState::Contradicted if self.counterexample.is_none() => {
                return Err(WitnessError::MissingCounterexample(self.obligation_id));
            }
            _ => {}
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum WitnessError {
    DuplicateObligation(ObligationId, ProjectionStage),
    DuplicateVerifier(VerifierKind),
    EmptyMustObligation(ObligationId),
    IntroducedStateCannotClaimObligation(ObligationId),
    InvalidSourceSpan(ObligationId),
    MissingCounterexample(ObligationId),
    MissingStage(ObligationId, ProjectionStage),
    MissingTarget(ObligationId),
    SourceAnchorMismatch(ObligationId),
    UnknownObligation(ObligationId),
    WeakExactVerifier(ObligationId),
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct ProjectionWitnessLedger {
    obligations: BTreeMap<ObligationId, Obligation>,
    witnesses: BTreeMap<(ObligationId, ProjectionStage), ProjectionWitness>,
}

impl ProjectionWitnessLedger {
    pub fn add_obligation(
        &mut self,
        source: &str,
        obligation: Obligation,
    ) -> Result<(), WitnessError> {
        obligation.validate(source)?;
        if self.obligations.contains_key(&obligation.id) {
            return Err(WitnessError::DuplicateObligation(
                obligation.id,
                ProjectionStage::SeedToContract,
            ));
        }
        self.obligations.insert(obligation.id, obligation);
        Ok(())
    }

    pub fn add_witness(&mut self, witness: ProjectionWitness) -> Result<(), WitnessError> {
        if !self.obligations.contains_key(&witness.obligation_id) {
            return Err(WitnessError::UnknownObligation(witness.obligation_id));
        }
        witness.validate()?;
        let key = (witness.obligation_id, witness.stage);
        if self.witnesses.contains_key(&key) {
            return Err(WitnessError::DuplicateObligation(key.0, key.1));
        }
        self.witnesses.insert(key, witness);
        Ok(())
    }

    pub fn require_chain(&self, stages: &[ProjectionStage]) -> Result<(), WitnessError> {
        for obligation in self.obligations.values() {
            if obligation.strength != ObligationStrength::Must {
                continue;
            }
            for stage in stages {
                if !self.witnesses.contains_key(&(obligation.id, *stage)) {
                    return Err(WitnessError::MissingStage(obligation.id, *stage));
                }
            }
        }
        Ok(())
    }

    pub fn obligations(&self) -> impl Iterator<Item = &Obligation> {
        self.obligations.values()
    }

    pub fn witnesses(&self) -> impl Iterator<Item = &ProjectionWitness> {
        self.witnesses.values()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct ProjectionDefectSet {
    pub weakened: BTreeSet<ObligationId>,
    pub strengthened: BTreeSet<ObligationId>,
    pub contradicted: BTreeSet<ObligationId>,
    pub dropped: BTreeSet<ObligationId>,
    pub unresolved: BTreeSet<ObligationId>,
    pub incomparable: BTreeSet<ObligationId>,
    pub verifier_gaps: BTreeSet<ObligationId>,
}

impl ProjectionDefectSet {
    pub fn from_ledger(ledger: &ProjectionWitnessLedger) -> Self {
        let mut set = Self::default();
        for witness in ledger.witnesses() {
            let target = match witness.state {
                PreservationState::Weakened => Some(&mut set.weakened),
                PreservationState::Strengthened => Some(&mut set.strengthened),
                PreservationState::Contradicted => Some(&mut set.contradicted),
                PreservationState::Dropped => Some(&mut set.dropped),
                PreservationState::Unresolved => Some(&mut set.unresolved),
                PreservationState::Incomparable => Some(&mut set.incomparable),
                _ => None,
            };
            if let Some(target) = target {
                target.insert(witness.obligation_id);
            }
            if matches!(
                witness.verifier.status,
                VerificationStatus::NeedsEvidence
                    | VerificationStatus::Unavailable
                    | VerificationStatus::AdvisoryOnly
            ) {
                set.verifier_gaps.insert(witness.obligation_id);
            }
        }
        set
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionGateDecision {
    CandidateCommit,
    Review,
    Clarify,
    HardHold,
}

pub fn reduce_projection_gate(
    ledger: &ProjectionWitnessLedger,
    defects: &ProjectionDefectSet,
) -> ProjectionGateDecision {
    let must = ledger
        .obligations()
        .filter(|obligation| obligation.strength == ObligationStrength::Must)
        .map(|obligation| obligation.id)
        .collect::<BTreeSet<_>>();
    let hard = defects
        .contradicted
        .iter()
        .chain(&defects.dropped)
        .chain(&defects.strengthened)
        .any(|id| must.contains(id));
    if hard {
        ProjectionGateDecision::HardHold
    } else if !defects.unresolved.is_empty() || !defects.incomparable.is_empty() {
        ProjectionGateDecision::Clarify
    } else if !defects.verifier_gaps.is_empty() || !defects.weakened.is_empty() {
        ProjectionGateDecision::Review
    } else {
        ProjectionGateDecision::CandidateCommit
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EmpiricalRiskEstimate {
    pub model_revision: String,
    pub calibration_revision: String,
    pub prediction_set: BTreeSet<String>,
    pub coverage_assumption: String,
    pub abstained: bool,
    pub advisory_only: bool,
}

impl EmpiricalRiskEstimate {
    pub fn can_authorize(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct LegacyShadowComparison {
    pub legacy_route: String,
    pub witness_route: ProjectionGateDecision,
    pub divergence: bool,
    pub taxonomy: Vec<String>,
}

pub fn compare_legacy_route(
    legacy_route: &str,
    witness_route: ProjectionGateDecision,
) -> LegacyShadowComparison {
    let normalized = match legacy_route {
        "commit" => ProjectionGateDecision::CandidateCommit,
        "clarify" => ProjectionGateDecision::Clarify,
        "hold" | "block" => ProjectionGateDecision::HardHold,
        _ => ProjectionGateDecision::Review,
    };
    let divergence = normalized != witness_route;
    LegacyShadowComparison {
        legacy_route: legacy_route.to_string(),
        witness_route,
        divergence,
        taxonomy: if divergence {
            vec!["legacy_scalar_vs_witness_defect_divergence".to_string()]
        } else {
            Vec::new()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obligation(source: &str, strength: ObligationStrength) -> Obligation {
        Obligation {
            id: ObligationId(1),
            kind: ObligationKind::Authority,
            strength,
            polarity: ObligationPolarity::Negative,
            scope: "delete".into(),
            source_span: SourceSpan::checked(source, 0, source.len()).unwrap(),
            source_text: source.into(),
        }
    }

    #[test]
    fn exact_requires_target_and_non_model_evidence() {
        let witness = ProjectionWitness {
            schema_version: TL_WITNESS_SCHEMA,
            obligation_id: ObligationId(1),
            stage: ProjectionStage::SeedToContract,
            state: PreservationState::Exact,
            target: None,
            rule_id: "r".into(),
            verifier: VerificationReceipt {
                verifier: VerifierKind::ModelAdvisory,
                status: VerificationStatus::AdvisoryOnly,
                revision: "m".into(),
                evidence_digest: None,
            },
            counterexample: None,
        };
        assert!(matches!(
            witness.validate(),
            Err(WitnessError::MissingTarget(_))
        ));
    }

    #[test]
    fn dropped_must_is_hard_hold_without_numeric_threshold() {
        let source = "do not delete";
        let mut ledger = ProjectionWitnessLedger::default();
        ledger
            .add_obligation(source, obligation(source, ObligationStrength::Must))
            .unwrap();
        ledger
            .add_witness(ProjectionWitness {
                schema_version: TL_WITNESS_SCHEMA,
                obligation_id: ObligationId(1),
                stage: ProjectionStage::CandidateToOutput,
                state: PreservationState::Dropped,
                target: None,
                rule_id: "output".into(),
                verifier: VerificationReceipt {
                    verifier: VerifierKind::SourceSpan,
                    status: VerificationStatus::Failed,
                    revision: "v1".into(),
                    evidence_digest: Some("sha256:x".into()),
                },
                counterexample: None,
            })
            .unwrap();
        let defects = ProjectionDefectSet::from_ledger(&ledger);
        assert_eq!(
            reduce_projection_gate(&ledger, &defects),
            ProjectionGateDecision::HardHold
        );
    }

    #[test]
    fn empirical_risk_never_grants_authority() {
        let estimate = EmpiricalRiskEstimate {
            model_revision: "model".into(),
            calibration_revision: "cal".into(),
            prediction_set: BTreeSet::from(["safe".into()]),
            coverage_assumption: "exchangeable_fixture_only".into(),
            abstained: false,
            advisory_only: true,
        };
        assert!(!estimate.can_authorize());
    }

    #[test]
    fn verifier_registry_separates_model_advice_from_exact_validation() {
        let mut registry = VerifierRegistry::default();
        registry
            .register(VerifierDescriptor {
                kind: VerifierKind::SourceSpan,
                revision: "span-v1".into(),
                authoritative_for_exact: true,
                available: true,
            })
            .unwrap();
        registry
            .register(VerifierDescriptor {
                kind: VerifierKind::ModelAdvisory,
                revision: "model-v1".into(),
                authoritative_for_exact: false,
                available: true,
            })
            .unwrap();
        assert!(registry.can_verify_exact(VerifierKind::SourceSpan));
        assert!(!registry.can_verify_exact(VerifierKind::ModelAdvisory));
    }
}
