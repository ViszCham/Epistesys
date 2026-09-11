use crate::{
    Obligation, ObligationKind, ObligationStrength, PreservationState, ProjectionGateDecision,
    ProjectionStage, VerificationReceipt, VerificationStatus, VerifierKind,
};
use lc631_core::{stable_sha256, ObligationId, SourceSpan};
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptPolicy, ReceiptScope, ReceiptVerifier, ReplayGuard, SubjectRevision,
    UntrustedReceipt,
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

pub const PROJECTION_DEFECT_GRAPH_V3_SCHEMA: &str = "lc631-projection-defect-graph.v3";
pub const TRANSLATION_ENVELOPE_V3_SCHEMA: &str = "lc631-translation-envelope.v3";

macro_rules! id_type {
    ($name:ident) => {
        #[repr(transparent)]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
        pub struct $name(pub u64);
    };
}

id_type!(SourceLedgerId);
id_type!(TargetClaimId);
id_type!(ProjectionEdgeId);

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SourceAnchorV3 {
    pub ledger_id: SourceLedgerId,
    pub revision: String,
    pub span: SourceSpan,
    pub surface_digest: String,
}

impl SourceAnchorV3 {
    pub fn checked(
        ledger_id: SourceLedgerId,
        revision: impl Into<String>,
        source: &str,
        start: usize,
        end: usize,
    ) -> Result<Self, ProjectionV3Error> {
        let revision = revision.into();
        if revision.trim().is_empty() {
            return Err(ProjectionV3Error::InvalidRevision);
        }
        let span = SourceSpan::checked(source, start, end)
            .map_err(|_| ProjectionV3Error::InvalidSourceSpan)?;
        let surface = span
            .slice(source)
            .map_err(|_| ProjectionV3Error::InvalidSourceSpan)?;
        Ok(Self {
            ledger_id,
            revision: revision.clone(),
            span,
            surface_digest: stable_sha256(&format!(
                "{}\0{}\0{}\0{}\0{}",
                ledger_id.0, revision, start, end, surface
            )),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TargetClaim {
    pub id: TargetClaimId,
    pub artifact: String,
    pub path: String,
    pub revision: String,
    pub content_digest: String,
    pub span: Option<SourceSpan>,
}

impl TargetClaim {
    pub fn checked(
        id: TargetClaimId,
        artifact: impl Into<String>,
        path: impl Into<String>,
        revision: impl Into<String>,
        content: &str,
    ) -> Result<Self, ProjectionV3Error> {
        let artifact = artifact.into();
        let path = path.into();
        let revision = revision.into();
        if artifact.trim().is_empty() || path.trim().is_empty() || revision.trim().is_empty() {
            return Err(ProjectionV3Error::InvalidTarget);
        }
        Ok(Self {
            id,
            artifact,
            path,
            revision: revision.clone(),
            content_digest: stable_sha256(&format!("{revision}\0{content}")),
            span: None,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CapabilityDescriptorV3 {
    pub verifier: VerifierKind,
    pub obligation_kind: ObligationKind,
    pub revision: String,
    pub stages: BTreeSet<ProjectionStage>,
    pub exact_allowed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct VerifiedCapabilityV3 {
    descriptor: CapabilityDescriptorV3,
    receipt_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProjectionEdgeV3 {
    pub id: ProjectionEdgeId,
    pub obligation_id: Option<ObligationId>,
    pub source: Option<SourceAnchorV3>,
    pub targets: Vec<TargetClaimId>,
    pub stage: ProjectionStage,
    pub state: PreservationState,
    pub rule_id: String,
    pub verifier: VerificationReceipt,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DomainTranslationPayload {
    pub domain: String,
    pub schema_version: String,
    pub payload_digest: String,
    pub unknowns: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CanonicalTranslationEnvelope {
    pub schema_version: &'static str,
    pub source_ledger_id: SourceLedgerId,
    pub source_revision: String,
    pub source_digest: String,
    pub target_claims: Vec<TargetClaim>,
    pub edges: Vec<ProjectionEdgeV3>,
    pub domain_payloads: Vec<DomainTranslationPayload>,
    pub gate: ProjectionGateDecision,
    pub scalar_aggregate_used: bool,
}

impl CanonicalTranslationEnvelope {
    pub fn from_graph(
        graph: &ProjectionDefectGraphV3,
        mut domain_payloads: Vec<DomainTranslationPayload>,
    ) -> Self {
        domain_payloads.sort_by(|left, right| {
            left.domain
                .cmp(&right.domain)
                .then_with(|| left.schema_version.cmp(&right.schema_version))
        });
        Self {
            schema_version: TRANSLATION_ENVELOPE_V3_SCHEMA,
            source_ledger_id: graph.source_ledger_id,
            source_revision: graph.source_revision.clone(),
            source_digest: graph.source_digest.clone(),
            target_claims: graph.target_claims.values().cloned().collect(),
            edges: graph.edges.values().cloned().collect(),
            domain_payloads,
            gate: graph.gate(),
            scalar_aggregate_used: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProjectionDefectGraphV3 {
    pub schema_version: &'static str,
    source_ledger_id: SourceLedgerId,
    source_revision: String,
    source_digest: String,
    #[serde(skip)]
    source_text: String,
    obligations: BTreeMap<ObligationId, Obligation>,
    anchors: BTreeMap<ObligationId, SourceAnchorV3>,
    target_claims: BTreeMap<TargetClaimId, TargetClaim>,
    edges: BTreeMap<ProjectionEdgeId, ProjectionEdgeV3>,
    capabilities: BTreeMap<(VerifierKind, ObligationKind), VerifiedCapabilityV3>,
    not_applicable: BTreeSet<(ObligationId, ProjectionStage)>,
    not_applicable_receipts: BTreeMap<(ObligationId, ProjectionStage), String>,
}

impl ProjectionDefectGraphV3 {
    pub fn new(
        source_ledger_id: SourceLedgerId,
        source_revision: impl Into<String>,
        source: &str,
    ) -> Result<Self, ProjectionV3Error> {
        let source_revision = source_revision.into();
        if source_revision.trim().is_empty() {
            return Err(ProjectionV3Error::InvalidRevision);
        }
        Ok(Self {
            schema_version: PROJECTION_DEFECT_GRAPH_V3_SCHEMA,
            source_ledger_id,
            source_revision: source_revision.clone(),
            source_digest: stable_sha256(&format!("{source_revision}\0{source}")),
            source_text: source.to_string(),
            obligations: BTreeMap::new(),
            anchors: BTreeMap::new(),
            target_claims: BTreeMap::new(),
            edges: BTreeMap::new(),
            capabilities: BTreeMap::new(),
            not_applicable: BTreeSet::new(),
            not_applicable_receipts: BTreeMap::new(),
        })
    }

    pub fn required_output_stages() -> BTreeSet<ProjectionStage> {
        BTreeSet::from([
            ProjectionStage::SeedToContract,
            ProjectionStage::ContractToProgram,
            ProjectionStage::ProgramToCandidate,
            ProjectionStage::CandidateToOutput,
        ])
    }

    pub fn add_obligation(&mut self, obligation: Obligation) -> Result<(), ProjectionV3Error> {
        obligation
            .validate(&self.source_text)
            .map_err(|_| ProjectionV3Error::ObligationSourceMismatch)?;
        if self.obligations.contains_key(&obligation.id) {
            return Err(ProjectionV3Error::DuplicateObligation(obligation.id));
        }
        let anchor = SourceAnchorV3::checked(
            self.source_ledger_id,
            self.source_revision.clone(),
            &self.source_text,
            obligation.source_span.start,
            obligation.source_span.end,
        )?;
        self.anchors.insert(obligation.id, anchor);
        self.obligations.insert(obligation.id, obligation);
        Ok(())
    }

    pub fn obligation_anchor(&self, id: ObligationId) -> Option<&SourceAnchorV3> {
        self.anchors.get(&id)
    }

    pub fn add_target_claim(&mut self, claim: TargetClaim) -> Result<(), ProjectionV3Error> {
        if self.target_claims.insert(claim.id, claim).is_some() {
            return Err(ProjectionV3Error::DuplicateTargetClaim);
        }
        Ok(())
    }

    pub fn register_capability(
        &mut self,
        descriptor: CapabilityDescriptorV3,
        attestation: UntrustedReceipt,
        verifier: &ReceiptVerifier,
        replay: &mut ReplayGuard,
        now_epoch: u64,
    ) -> Result<(), ProjectionV3Error> {
        if descriptor.revision.trim().is_empty() || descriptor.stages.is_empty() {
            return Err(ProjectionV3Error::InvalidCapability);
        }
        let key = (descriptor.verifier, descriptor.obligation_kind);
        if self.capabilities.contains_key(&key) {
            return Err(ProjectionV3Error::DuplicateCapability);
        }
        let subject = SubjectRevision::checked(self.source_revision.clone())
            .map_err(|_| ProjectionV3Error::InvalidCapability)?;
        let scope = ReceiptScope::checked(verifier_capability_scope(&descriptor))
            .map_err(|_| ProjectionV3Error::InvalidCapability)?;
        let verified = verifier
            .verify(
                attestation,
                &ReceiptPolicy::exact(
                    ReceiptClass::VerifierCapability,
                    subject,
                    scope,
                    verifier_capability_payload_digest(&descriptor),
                    now_epoch,
                ),
                replay,
            )
            .map_err(|_| ProjectionV3Error::UnauthenticatedCapability)?;
        self.capabilities.insert(
            key,
            VerifiedCapabilityV3 {
                descriptor,
                receipt_digest: verified.receipt_digest(),
            },
        );
        Ok(())
    }

    pub fn add_edge(&mut self, edge: ProjectionEdgeV3) -> Result<(), ProjectionV3Error> {
        if edge.rule_id.trim().is_empty() || self.edges.contains_key(&edge.id) {
            return Err(ProjectionV3Error::DuplicateOrInvalidEdge);
        }
        if edge
            .targets
            .iter()
            .any(|target| !self.target_claims.contains_key(target))
        {
            return Err(ProjectionV3Error::UnknownTargetClaim);
        }
        if edge.state == PreservationState::IntroducedWithoutSource {
            if edge.obligation_id.is_some() || edge.source.is_some() || edge.targets.is_empty() {
                return Err(ProjectionV3Error::InvalidIntroducedEdge);
            }
        } else {
            let obligation_id = edge
                .obligation_id
                .ok_or(ProjectionV3Error::MissingObligation)?;
            let obligation = self
                .obligations
                .get(&obligation_id)
                .ok_or(ProjectionV3Error::UnknownObligation(obligation_id))?;
            let expected_anchor = self
                .anchors
                .get(&obligation_id)
                .ok_or(ProjectionV3Error::ObligationSourceMismatch)?;
            if edge.source.as_ref() != Some(expected_anchor) {
                return Err(ProjectionV3Error::ObligationSourceMismatch);
            }
            if edge.state == PreservationState::Dropped {
                if !edge.targets.is_empty() {
                    return Err(ProjectionV3Error::InvalidDroppedEdge);
                }
            } else if edge.targets.is_empty()
                && matches!(
                    edge.state,
                    PreservationState::Exact | PreservationState::Refinement
                )
            {
                return Err(ProjectionV3Error::MissingTarget);
            }
            if matches!(
                edge.state,
                PreservationState::Exact | PreservationState::Refinement
            ) {
                let capability = self
                    .capabilities
                    .get(&(edge.verifier.verifier, obligation.kind))
                    .ok_or(ProjectionV3Error::UnauthenticatedCapability)?;
                if !capability.descriptor.stages.contains(&edge.stage)
                    || capability.descriptor.revision != edge.verifier.revision
                    || edge.verifier.status != VerificationStatus::Passed
                    || edge.verifier.evidence_digest.is_none()
                    || (edge.state == PreservationState::Exact
                        && !capability.descriptor.exact_allowed)
                {
                    return Err(ProjectionV3Error::WeakVerifier);
                }
            }
        }
        self.edges.insert(edge.id, edge);
        Ok(())
    }

    pub fn mark_stage_not_applicable(
        &mut self,
        obligation_id: ObligationId,
        stage: ProjectionStage,
        attestation: UntrustedReceipt,
        verifier: &ReceiptVerifier,
        replay: &mut ReplayGuard,
        now_epoch: u64,
    ) -> Result<(), ProjectionV3Error> {
        if !self.obligations.contains_key(&obligation_id) {
            return Err(ProjectionV3Error::UnknownObligation(obligation_id));
        }
        let payload = stage_not_applicable_payload_digest(obligation_id, stage);
        let scope =
            ReceiptScope::checked(format!("tl/not-applicable/{}/{:?}", obligation_id.0, stage))
                .map_err(|_| ProjectionV3Error::InvalidCapability)?;
        let subject = SubjectRevision::checked(self.source_revision.clone())
            .map_err(|_| ProjectionV3Error::InvalidCapability)?;
        let verified = verifier
            .verify(
                attestation,
                &ReceiptPolicy::exact(
                    ReceiptClass::VerifierCapability,
                    subject,
                    scope,
                    payload,
                    now_epoch,
                ),
                replay,
            )
            .map_err(|_| ProjectionV3Error::UnauthenticatedCapability)?;
        let key = (obligation_id, stage);
        self.not_applicable.insert(key);
        self.not_applicable_receipts
            .insert(key, verified.receipt_digest());
        Ok(())
    }

    pub fn introduced_target_claims(&self) -> Vec<TargetClaimId> {
        self.edges
            .values()
            .filter(|edge| edge.state == PreservationState::IntroducedWithoutSource)
            .flat_map(|edge| edge.targets.iter().copied())
            .collect()
    }

    pub fn gate(&self) -> ProjectionGateDecision {
        if self
            .edges
            .values()
            .any(|edge| edge.state == PreservationState::IntroducedWithoutSource)
        {
            return ProjectionGateDecision::HardHold;
        }
        let required = Self::required_output_stages();
        let mut review = false;
        let mut clarify = false;
        for obligation in self
            .obligations
            .values()
            .filter(|obligation| obligation.strength == ObligationStrength::Must)
        {
            for stage in &required {
                let edges = self
                    .edges
                    .values()
                    .filter(|edge| {
                        edge.obligation_id == Some(obligation.id) && edge.stage == *stage
                    })
                    .collect::<Vec<_>>();
                if edges.is_empty() && !self.not_applicable.contains(&(obligation.id, *stage)) {
                    clarify = true;
                    continue;
                }
                for edge in edges {
                    match edge.state {
                        PreservationState::Contradicted
                        | PreservationState::Dropped
                        | PreservationState::Strengthened => {
                            return ProjectionGateDecision::HardHold;
                        }
                        PreservationState::Unresolved | PreservationState::Incomparable => {
                            clarify = true;
                        }
                        PreservationState::Weakened => review = true,
                        PreservationState::Exact | PreservationState::Refinement => {}
                        PreservationState::IntroducedWithoutSource => {
                            return ProjectionGateDecision::HardHold;
                        }
                    }
                }
            }
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

pub fn verifier_capability_payload_digest(descriptor: &CapabilityDescriptorV3) -> String {
    stable_sha256(&format!(
        "{:?}\0{:?}\0{}\0{:?}\0{}",
        descriptor.verifier,
        descriptor.obligation_kind,
        descriptor.revision,
        descriptor.stages,
        descriptor.exact_allowed
    ))
}

pub fn verifier_capability_scope(descriptor: &CapabilityDescriptorV3) -> String {
    format!(
        "tl/verifier/{}/{}",
        verifier_name(descriptor.verifier),
        obligation_name(descriptor.obligation_kind)
    )
}

pub fn stage_not_applicable_payload_digest(
    obligation_id: ObligationId,
    stage: ProjectionStage,
) -> String {
    stable_sha256(&format!("not-applicable\0{}\0{:?}", obligation_id.0, stage))
}

fn verifier_name(verifier: VerifierKind) -> &'static str {
    match verifier {
        VerifierKind::SourceSpan => "source_span",
        VerifierKind::Schema => "schema",
        VerifierKind::TypeChecker => "type_checker",
        VerifierKind::Compiler => "compiler",
        VerifierKind::Smt => "smt",
        VerifierKind::Roundtrip => "roundtrip",
        VerifierKind::RuntimeObservation => "runtime_observation",
        VerifierKind::HumanReview => "human_review",
        VerifierKind::ModelAdvisory => "model_advisory",
    }
}

fn obligation_name(kind: ObligationKind) -> &'static str {
    match kind {
        ObligationKind::AnswerKind => "answer_kind",
        ObligationKind::Authority => "authority",
        ObligationKind::CausalFrame => "causal_frame",
        ObligationKind::Evidence => "evidence",
        ObligationKind::Modality => "modality",
        ObligationKind::OutputContract => "output_contract",
        ObligationKind::Polarity => "polarity",
        ObligationKind::Scope => "scope",
        ObligationKind::Semantic => "semantic",
        ObligationKind::Structural => "structural",
        ObligationKind::TemporalFrame => "temporal_frame",
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum ProjectionV3Error {
    DuplicateCapability,
    DuplicateObligation(ObligationId),
    DuplicateOrInvalidEdge,
    DuplicateTargetClaim,
    InvalidCapability,
    InvalidDroppedEdge,
    InvalidIntroducedEdge,
    InvalidRevision,
    InvalidSourceSpan,
    InvalidTarget,
    MissingObligation,
    MissingTarget,
    ObligationSourceMismatch,
    UnauthenticatedCapability,
    UnknownObligation(ObligationId),
    UnknownTargetClaim,
    WeakVerifier,
}
