use crate::{
    Obligation, ObligationKind, ObligationStrength, PreservationState, ProjectionGateDecision,
    TargetAnchor, VerificationReceipt, VerificationStatus, VerifierKind, WitnessError,
};
use lc631_core::{stable_sha256, ObligationId, SourceSpan};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

pub const TLDG_PROJECTION_SCHEMA: &str = "lc631-projection-defect-graph.v2";

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SourceAnchor {
    pub revision: String,
    pub span: SourceSpan,
    pub digest: String,
}

impl SourceAnchor {
    pub fn checked(
        revision: impl Into<String>,
        source: &str,
        start: usize,
        end: usize,
    ) -> Result<Self, ProjectionV2Error> {
        let revision = revision.into();
        if revision.trim().is_empty() {
            return Err(ProjectionV2Error::EmptyRevision);
        }
        let span = SourceSpan::checked(source, start, end)
            .map_err(|_| ProjectionV2Error::InvalidSourceSpan)?;
        let surface = span
            .slice(source)
            .map_err(|_| ProjectionV2Error::InvalidSourceSpan)?;
        Ok(Self {
            digest: stable_sha256(&format!("{revision}\0{start}\0{end}\0{surface}")),
            revision,
            span,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct VerifierCapability {
    pub verifier: VerifierKind,
    pub obligation_kind: ObligationKind,
    pub exact_allowed: bool,
    pub revision: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CorrespondenceWitnessV2 {
    pub obligation_id: ObligationId,
    pub source: Vec<SourceAnchor>,
    pub target: Vec<TargetAnchor>,
    pub state: PreservationState,
    pub rule_id: String,
    pub verifier: VerificationReceipt,
    pub counterexample: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum ProjectionV2Error {
    DuplicateCapability,
    DuplicateCorrespondence,
    DuplicateObligation(ObligationId),
    EmptyRevision,
    InvalidIntroducedCorrespondence,
    InvalidSourceSpan,
    InvalidDroppedCorrespondence,
    MissingCounterexample,
    MissingSource,
    MissingTarget,
    UnknownObligation(ObligationId),
    WeakVerifier(ObligationId),
    Witness(WitnessError),
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct ProjectionDefectGraph {
    obligations: BTreeMap<ObligationId, Obligation>,
    correspondences: Vec<CorrespondenceWitnessV2>,
    capabilities: BTreeMap<(VerifierKind, ObligationKind), VerifierCapability>,
}

impl ProjectionDefectGraph {
    pub fn add_obligation(
        &mut self,
        source: &str,
        obligation: Obligation,
    ) -> Result<(), ProjectionV2Error> {
        obligation
            .validate(source)
            .map_err(ProjectionV2Error::Witness)?;
        if self.obligations.contains_key(&obligation.id) {
            return Err(ProjectionV2Error::DuplicateObligation(obligation.id));
        }
        self.obligations.insert(obligation.id, obligation);
        Ok(())
    }

    pub fn register_capability(
        &mut self,
        capability: VerifierCapability,
    ) -> Result<(), ProjectionV2Error> {
        if capability.revision.trim().is_empty() {
            return Err(ProjectionV2Error::EmptyRevision);
        }
        let key = (capability.verifier, capability.obligation_kind);
        if self.capabilities.contains_key(&key) {
            return Err(ProjectionV2Error::DuplicateCapability);
        }
        self.capabilities.insert(key, capability);
        Ok(())
    }

    pub fn add_correspondence(
        &mut self,
        correspondence: CorrespondenceWitnessV2,
    ) -> Result<(), ProjectionV2Error> {
        let obligation = self.obligations.get(&correspondence.obligation_id).ok_or(
            ProjectionV2Error::UnknownObligation(correspondence.obligation_id),
        )?;
        if correspondence.rule_id.trim().is_empty()
            || correspondence.verifier.revision.trim().is_empty()
        {
            return Err(ProjectionV2Error::EmptyRevision);
        }
        match correspondence.state {
            PreservationState::Exact => {
                if correspondence.source.is_empty() {
                    return Err(ProjectionV2Error::MissingSource);
                }
                if correspondence.target.is_empty() {
                    return Err(ProjectionV2Error::MissingTarget);
                }
                let capable = self
                    .capabilities
                    .get(&(correspondence.verifier.verifier, obligation.kind))
                    .is_some_and(|capability| {
                        capability.exact_allowed
                            && capability.revision == correspondence.verifier.revision
                    });
                if !capable || !correspondence.verifier.authoritative_for_exact() {
                    return Err(ProjectionV2Error::WeakVerifier(
                        correspondence.obligation_id,
                    ));
                }
            }
            PreservationState::Refinement => {
                if correspondence.source.is_empty() {
                    return Err(ProjectionV2Error::MissingSource);
                }
                if correspondence.target.is_empty() {
                    return Err(ProjectionV2Error::MissingTarget);
                }
                if correspondence.verifier.status != VerificationStatus::Passed
                    || matches!(
                        correspondence.verifier.verifier,
                        VerifierKind::ModelAdvisory
                    )
                {
                    return Err(ProjectionV2Error::WeakVerifier(
                        correspondence.obligation_id,
                    ));
                }
            }
            PreservationState::IntroducedWithoutSource => {
                if !correspondence.source.is_empty() || correspondence.target.is_empty() {
                    return Err(ProjectionV2Error::InvalidIntroducedCorrespondence);
                }
            }
            PreservationState::Dropped => {
                if correspondence.source.is_empty() || !correspondence.target.is_empty() {
                    return Err(ProjectionV2Error::InvalidDroppedCorrespondence);
                }
            }
            PreservationState::Contradicted if correspondence.counterexample.is_none() => {
                return Err(ProjectionV2Error::MissingCounterexample);
            }
            _ => {}
        }
        if self.correspondences.contains(&correspondence) {
            return Err(ProjectionV2Error::DuplicateCorrespondence);
        }
        self.correspondences.push(correspondence);
        Ok(())
    }

    pub fn obligations(&self) -> impl Iterator<Item = &Obligation> {
        self.obligations.values()
    }

    pub fn correspondences(&self) -> impl Iterator<Item = &CorrespondenceWitnessV2> {
        self.correspondences.iter()
    }

    pub fn gate(&self) -> ProjectionGateDecision {
        let must = self
            .obligations
            .values()
            .filter(|obligation| obligation.strength == ObligationStrength::Must)
            .map(|obligation| obligation.id)
            .collect::<BTreeSet<_>>();
        let mut review = false;
        let mut clarify = false;
        for witness in &self.correspondences {
            match witness.state {
                PreservationState::Contradicted
                | PreservationState::Dropped
                | PreservationState::Strengthened
                | PreservationState::IntroducedWithoutSource
                    if must.contains(&witness.obligation_id) =>
                {
                    return ProjectionGateDecision::HardHold;
                }
                PreservationState::Unresolved | PreservationState::Incomparable => {
                    clarify = true;
                }
                PreservationState::Weakened => review = true,
                _ => {}
            }
            if matches!(
                witness.verifier.status,
                VerificationStatus::NeedsEvidence
                    | VerificationStatus::Unavailable
                    | VerificationStatus::AdvisoryOnly
            ) {
                review = true;
            }
        }
        let observed = self
            .correspondences
            .iter()
            .map(|witness| witness.obligation_id)
            .collect::<BTreeSet<_>>();
        if must.iter().any(|id| !observed.contains(id)) {
            clarify = true;
        }
        if clarify {
            ProjectionGateDecision::Clarify
        } else if review {
            ProjectionGateDecision::Review
        } else {
            ProjectionGateDecision::CandidateCommit
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ObligationPolarity, ObligationStrength};

    fn must(source: &str) -> Obligation {
        Obligation {
            id: ObligationId(1),
            kind: ObligationKind::Semantic,
            strength: ObligationStrength::Must,
            polarity: ObligationPolarity::Positive,
            scope: "answer".into(),
            source_span: SourceSpan::checked(source, 0, source.len()).unwrap(),
            source_text: source.into(),
        }
    }

    #[test]
    fn introduced_without_source_is_retained_and_blocks_must() {
        let source = "answer";
        let mut graph = ProjectionDefectGraph::default();
        graph.add_obligation(source, must(source)).unwrap();
        graph
            .add_correspondence(CorrespondenceWitnessV2 {
                obligation_id: ObligationId(1),
                source: Vec::new(),
                target: vec![TargetAnchor {
                    artifact: "candidate".into(),
                    path: "extra".into(),
                    span: None,
                }],
                state: PreservationState::IntroducedWithoutSource,
                rule_id: "extra.v1".into(),
                verifier: VerificationReceipt {
                    verifier: VerifierKind::Schema,
                    status: VerificationStatus::Failed,
                    revision: "schema-r1".into(),
                    evidence_digest: Some("sha256:x".into()),
                },
                counterexample: None,
            })
            .unwrap();
        assert_eq!(graph.gate(), ProjectionGateDecision::HardHold);
    }
}
