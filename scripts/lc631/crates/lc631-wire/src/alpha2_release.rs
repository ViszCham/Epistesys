use lc631_core::stable_sha256;
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptPolicy, ReceiptScope, ReceiptVerifier, ReplayGuard, SubjectRevision,
    UntrustedReceipt,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const PROFILE_SCHEMA: &str = "epistesys-alpha2-release-profile.v1";
const REPORT_SCHEMA: &str = "epistesys-alpha2-release-report.v1";
const GATE_RECEIPT_PREFIX: &str = "epistesys/alpha2/implementation";
const TARGET_VERSION: &str = "6.3.2-alpha.2";
const MAX_ALPHA2_RECEIPTS: usize = 256;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Alpha2GateFeature {
    ArtifactIdentity,
    SourceRegionAccounting,
    RustCst,
    LanguageWorker,
    InstructionGrammar,
    RequirementIr,
    AuthorityProjection,
    TranslationLossChain,
    EvidenceIssuance,
    StaticConnectivity,
    RuntimeConnectivity,
    CodingClosure,
    EvidenceLifecycle,
    Resume,
    RepairLoop,
    Distillation,
    StandaloneFinalizer,
    SchemaCompatibility,
    ProductionCli,
    ResourceGovernance,
    ReproduciblePackage,
    AdversarialMatrix,
}

impl Alpha2GateFeature {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ArtifactIdentity => "artifact_identity",
            Self::SourceRegionAccounting => "source_region_accounting",
            Self::RustCst => "rust_cst",
            Self::LanguageWorker => "language_worker",
            Self::InstructionGrammar => "instruction_grammar",
            Self::RequirementIr => "requirement_ir",
            Self::AuthorityProjection => "authority_projection",
            Self::TranslationLossChain => "translation_loss_chain",
            Self::EvidenceIssuance => "evidence_issuance",
            Self::StaticConnectivity => "static_connectivity",
            Self::RuntimeConnectivity => "runtime_connectivity",
            Self::CodingClosure => "coding_closure",
            Self::EvidenceLifecycle => "evidence_lifecycle",
            Self::Resume => "resume",
            Self::RepairLoop => "repair_loop",
            Self::Distillation => "distillation",
            Self::StandaloneFinalizer => "standalone_finalizer",
            Self::SchemaCompatibility => "schema_compatibility",
            Self::ProductionCli => "production_cli",
            Self::ResourceGovernance => "resource_governance",
            Self::ReproduciblePackage => "reproducible_package",
            Self::AdversarialMatrix => "adversarial_matrix",
        }
    }
}

const REQUIRED_FEATURES: [Alpha2GateFeature; 22] = [
    Alpha2GateFeature::ArtifactIdentity,
    Alpha2GateFeature::SourceRegionAccounting,
    Alpha2GateFeature::RustCst,
    Alpha2GateFeature::LanguageWorker,
    Alpha2GateFeature::InstructionGrammar,
    Alpha2GateFeature::RequirementIr,
    Alpha2GateFeature::AuthorityProjection,
    Alpha2GateFeature::TranslationLossChain,
    Alpha2GateFeature::EvidenceIssuance,
    Alpha2GateFeature::StaticConnectivity,
    Alpha2GateFeature::RuntimeConnectivity,
    Alpha2GateFeature::CodingClosure,
    Alpha2GateFeature::EvidenceLifecycle,
    Alpha2GateFeature::Resume,
    Alpha2GateFeature::RepairLoop,
    Alpha2GateFeature::Distillation,
    Alpha2GateFeature::StandaloneFinalizer,
    Alpha2GateFeature::SchemaCompatibility,
    Alpha2GateFeature::ProductionCli,
    Alpha2GateFeature::ResourceGovernance,
    Alpha2GateFeature::ReproduciblePackage,
    Alpha2GateFeature::AdversarialMatrix,
];

pub const fn alpha2_required_features() -> &'static [Alpha2GateFeature; 22] {
    &REQUIRED_FEATURES
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Alpha2ProfileKind {
    StandaloneSource,
    HostIntegrated,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Alpha2ReleaseProfile {
    schema_version: &'static str,
    target_version: &'static str,
    kind: Alpha2ProfileKind,
    identity: String,
    required_features: Vec<Alpha2GateFeature>,
}

impl Alpha2ReleaseProfile {
    pub fn standalone() -> Self {
        Self::new(Alpha2ProfileKind::StandaloneSource)
    }

    pub fn host_integrated() -> Self {
        Self::new(Alpha2ProfileKind::HostIntegrated)
    }

    pub fn identity(&self) -> String {
        self.identity.clone()
    }

    pub fn schema_version(&self) -> &'static str {
        self.schema_version
    }

    pub fn target_version(&self) -> &'static str {
        self.target_version
    }

    pub fn kind(&self) -> Alpha2ProfileKind {
        self.kind
    }

    pub fn required_features(&self) -> &[Alpha2GateFeature] {
        &self.required_features
    }

    fn new(kind: Alpha2ProfileKind) -> Self {
        let required_features = REQUIRED_FEATURES.to_vec();
        let feature_basis = required_features
            .iter()
            .map(|feature| feature.as_str())
            .collect::<Vec<_>>()
            .join("\0");
        let kind_name = match kind {
            Alpha2ProfileKind::StandaloneSource => "standalone_source",
            Alpha2ProfileKind::HostIntegrated => "host_integrated",
        };
        let identity = stable_sha256(&format!(
            "{PROFILE_SCHEMA}\0{TARGET_VERSION}\0{kind_name}\0{feature_basis}"
        ));
        Self {
            schema_version: PROFILE_SCHEMA,
            target_version: TARGET_VERSION,
            kind,
            identity,
            required_features,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Alpha2GateAttestation {
    pub feature: Alpha2GateFeature,
    pub evidence_digest: String,
    pub receipt: UntrustedReceipt,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Alpha2SourceReadiness {
    Held,
    SourceReleaseReady,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum Alpha2ResearchStatus {
    PendingNoCorpus,
    PendingIndependentReview,
    ObservedUnverified {
        corpus_digest: String,
        result_digest: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Alpha2HostObservation {
    NotRequested,
    PendingHostObservation,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Alpha2ReleaseReport {
    pub schema_version: &'static str,
    pub profile: Alpha2ReleaseProfile,
    pub source_revision: String,
    pub source_readiness: Alpha2SourceReadiness,
    pub accepted_features: Vec<Alpha2GateFeature>,
    pub blocked_features: Vec<Alpha2GateFeature>,
    pub blocked_external_requirements: Vec<String>,
    pub rejected_receipts: Vec<String>,
    pub research_status: Alpha2ResearchStatus,
    pub host_observation: Alpha2HostObservation,
    pub automatic_promotion: bool,
    pub claim_boundary: &'static str,
}

pub fn alpha2_gate_scope(profile: &Alpha2ReleaseProfile, feature: Alpha2GateFeature) -> String {
    format!(
        "{GATE_RECEIPT_PREFIX}/{}/{}",
        profile.identity(),
        feature.as_str()
    )
}

pub fn alpha2_gate_payload_digest(
    profile_identity: &str,
    source_revision: &str,
    feature: Alpha2GateFeature,
    evidence_digest: &str,
) -> String {
    stable_sha256(&format!(
        "{PROFILE_SCHEMA}\0{profile_identity}\0{source_revision}\0{}\0{evidence_digest}",
        feature.as_str()
    ))
}

pub fn assess_alpha2_release(
    profile: &Alpha2ReleaseProfile,
    source_revision: &str,
    attestations: &[Alpha2GateAttestation],
    verifier: Option<&ReceiptVerifier>,
    replay: &mut ReplayGuard,
    now_epoch: u64,
    research_status: Alpha2ResearchStatus,
) -> Alpha2ReleaseReport {
    let mut grouped = BTreeMap::<Alpha2GateFeature, Vec<&Alpha2GateAttestation>>::new();
    let mut rejected_receipts = Vec::new();
    let over_budget = attestations.len() > MAX_ALPHA2_RECEIPTS;
    if over_budget {
        rejected_receipts.push("receipt_budget_exceeded".into());
    }
    for attestation in attestations
        .iter()
        .take(if over_budget { 0 } else { attestations.len() })
    {
        if !profile.required_features().contains(&attestation.feature) {
            rejected_receipts.push(format!(
                "unexpected_feature:{}",
                attestation.feature.as_str()
            ));
            continue;
        }
        grouped
            .entry(attestation.feature)
            .or_default()
            .push(attestation);
    }

    let mut accepted_features = Vec::new();
    let mut blocked_features = Vec::new();
    for feature in profile.required_features() {
        if over_budget {
            blocked_features.push(*feature);
            continue;
        }
        let Some(candidates) = grouped.get(feature) else {
            blocked_features.push(*feature);
            continue;
        };
        if candidates.len() != 1 {
            blocked_features.push(*feature);
            rejected_receipts.push(format!("duplicate_feature:{}", feature.as_str()));
            continue;
        }
        let attestation = candidates[0];
        let result = validate_gate_attestation(
            profile,
            source_revision,
            *feature,
            attestation,
            verifier,
            replay,
            now_epoch,
        );
        match result {
            Ok(()) => accepted_features.push(*feature),
            Err(reason) => {
                blocked_features.push(*feature);
                rejected_receipts.push(format!("{}:{reason}", feature.as_str()));
            }
        }
    }

    let host_observation = match profile.kind() {
        Alpha2ProfileKind::StandaloneSource => Alpha2HostObservation::NotRequested,
        Alpha2ProfileKind::HostIntegrated => Alpha2HostObservation::PendingHostObservation,
    };
    let mut blocked_external_requirements = Vec::new();
    if profile.kind() == Alpha2ProfileKind::HostIntegrated {
        blocked_external_requirements.push("host_output".to_string());
    }
    let ready = blocked_features.is_empty() && blocked_external_requirements.is_empty();
    Alpha2ReleaseReport {
        schema_version: REPORT_SCHEMA,
        profile: profile.clone(),
        source_revision: source_revision.to_owned(),
        source_readiness: if ready {
            Alpha2SourceReadiness::SourceReleaseReady
        } else {
            Alpha2SourceReadiness::Held
        },
        accepted_features,
        blocked_features,
        blocked_external_requirements,
        rejected_receipts,
        research_status,
        host_observation,
        automatic_promotion: false,
        claim_boundary: "verified feature receipts establish only the declared source profile; they do not prove research performance, host delivery, truth, or promotion authority",
    }
}

fn validate_gate_attestation(
    profile: &Alpha2ReleaseProfile,
    source_revision: &str,
    feature: Alpha2GateFeature,
    attestation: &Alpha2GateAttestation,
    verifier: Option<&ReceiptVerifier>,
    replay: &mut ReplayGuard,
    now_epoch: u64,
) -> Result<(), String> {
    if !valid_digest(source_revision) || !valid_digest(&attestation.evidence_digest) {
        return Err("invalid_digest".into());
    }
    let verifier = verifier.ok_or_else(|| "verifier_unavailable".to_string())?;
    let policy = ReceiptPolicy::exact(
        ReceiptClass::StageCompletion,
        SubjectRevision::checked(source_revision).map_err(|error| format!("subject:{error:?}"))?,
        ReceiptScope::checked(alpha2_gate_scope(profile, feature))
            .map_err(|error| format!("scope:{error:?}"))?,
        alpha2_gate_payload_digest(
            &profile.identity(),
            source_revision,
            feature,
            &attestation.evidence_digest,
        ),
        now_epoch,
    );
    verifier
        .verify(attestation.receipt.clone(), &policy, replay)
        .map_err(|error| format!("receipt:{error:?}"))?;
    Ok(())
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn required_feature_set_is_nonempty_and_unique() {
        let set = REQUIRED_FEATURES.into_iter().collect::<BTreeSet<_>>();
        assert_eq!(set.len(), REQUIRED_FEATURES.len());
        assert_eq!(REQUIRED_FEATURES.len(), 22);
    }

    #[test]
    fn source_and_host_profiles_have_distinct_stable_identities() {
        let standalone = Alpha2ReleaseProfile::standalone();
        let host = Alpha2ReleaseProfile::host_integrated();
        assert_ne!(standalone.identity, host.identity);
        assert_eq!(
            standalone.identity,
            Alpha2ReleaseProfile::standalone().identity
        );
        assert_eq!(standalone.required_features(), host.required_features());
    }
}
