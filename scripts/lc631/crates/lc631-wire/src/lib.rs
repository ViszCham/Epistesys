#![forbid(unsafe_code)]

use lc631_core::LC631_VERSION;
use serde::Serialize;
use std::collections::BTreeMap;

pub const WIRE_SCHEMA: &str = "lc631-wire.v1";
pub const PROMOTION_PREDICATES: [&str; 13] = [
    "v630_frozen",
    "namespace_collision_free",
    "translation_witness_end_to_end",
    "world_budget_exact_2048",
    "world_distinctness",
    "gpu_mandatory_lane_coverage",
    "gpu_fault_contained",
    "media_source_authority",
    "media_backends_observed",
    "host_output_bound",
    "paired_v630_v631_regression",
    "full_validation",
    "package_installed_host_pickup",
];

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WireEnvelope<T> {
    pub schema_version: &'static str,
    pub command: String,
    pub version: &'static str,
    pub shadow_only: bool,
    pub claim_boundary: &'static str,
    pub payload: T,
}

impl<T: Serialize> WireEnvelope<T> {
    pub fn shadow(command: impl Into<String>, payload: T) -> Self {
        Self {
            schema_version: WIRE_SCHEMA,
            command: command.into(),
            version: LC631_VERSION,
            shadow_only: true,
            claim_boundary:
                "local shadow evidence only; not truth, proof, mutation authority, host interception, performance, or v6.3.0 replacement readiness",
            payload,
        }
    }

    pub fn to_pretty_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct ResidualRiskLedger {
    pub risks: BTreeMap<String, String>,
}

impl ResidualRiskLedger {
    pub fn record(&mut self, code: impl Into<String>, note: impl Into<String>) {
        self.risks.insert(code.into(), note.into());
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PromotionGate {
    pub predicates: BTreeMap<String, bool>,
    pub evidence_complete: bool,
    pub promotion_allowed: bool,
    pub blocked_reasons: Vec<String>,
    pub automatic_promotion: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence_binding_digest: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub accepted_receipt_digests: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub rejected_evidence: Vec<String>,
}

impl PromotionGate {
    pub fn evaluate(candidate_predicates: BTreeMap<String, bool>) -> Self {
        let exact_predicate_universe = candidate_predicates.len() == PROMOTION_PREDICATES.len()
            && candidate_predicates
                .keys()
                .all(|name| PROMOTION_PREDICATES.contains(&name.as_str()));
        let predicates = PROMOTION_PREDICATES
            .into_iter()
            .map(|name| {
                (
                    name.to_string(),
                    candidate_predicates.get(name).copied().unwrap_or(false),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut blocked_reasons = predicates
            .iter()
            .filter(|(_, passed)| !**passed)
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();
        let evidence_complete = exact_predicate_universe && blocked_reasons.is_empty();
        if evidence_complete {
            blocked_reasons.push("independent_trust_principals".to_string());
        }
        Self {
            evidence_complete,
            promotion_allowed: false,
            predicates,
            blocked_reasons,
            automatic_promotion: false,
            evidence_binding_digest: None,
            accepted_receipt_digests: Vec::new(),
            rejected_evidence: Vec::new(),
        }
    }

    pub fn with_evidence(
        mut self,
        binding_digest: String,
        accepted_receipt_digests: Vec<String>,
        rejected_evidence: Vec<String>,
    ) -> Self {
        self.evidence_binding_digest = Some(binding_digest);
        self.accepted_receipt_digests = accepted_receipt_digests;
        self.rejected_evidence = rejected_evidence;
        self
    }

    pub fn alpha_default() -> Self {
        Self::evaluate(BTreeMap::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_is_shadow_and_promotion_is_fail_closed() {
        let wire = WireEnvelope::shadow("lc631-doctor", PromotionGate::alpha_default());
        let json = wire.to_pretty_json().unwrap();
        assert!(json.contains("\"shadow_only\": true"));
        assert!(json.contains("\"promotion_allowed\": false"));
        assert!(json.contains("host_output_bound"));
    }

    #[test]
    fn all_predicates_still_require_explicit_promotion() {
        let gate =
            PromotionGate::evaluate(BTreeMap::from([("a".into(), true), ("b".into(), true)]));
        assert_eq!(gate.predicates.len(), 13);
        assert!(gate.predicates.values().all(|passed| !passed));
        assert!(!gate.promotion_allowed);
        assert!(!gate.automatic_promotion);
    }

    #[test]
    fn unexpected_predicate_keeps_an_all_true_gate_closed() {
        let mut predicates = PROMOTION_PREDICATES
            .into_iter()
            .map(|name| (name.to_string(), true))
            .collect::<BTreeMap<_, _>>();
        predicates.insert("unexpected".into(), true);
        let gate = PromotionGate::evaluate(predicates);
        assert!(gate.predicates.values().all(|passed| *passed));
        assert!(!gate.evidence_complete);
        assert!(!gate.promotion_allowed);
        assert!(!gate.automatic_promotion);
    }

    #[test]
    fn a_complete_single_issuer_bundle_is_evidence_only() {
        let predicates = PROMOTION_PREDICATES
            .into_iter()
            .map(|name| (name.to_string(), true))
            .collect::<BTreeMap<_, _>>();
        let gate = PromotionGate::evaluate(predicates);
        assert!(gate.evidence_complete);
        assert!(!gate.promotion_allowed);
        assert_eq!(gate.blocked_reasons, ["independent_trust_principals"]);
        assert!(!gate.automatic_promotion);
    }
}
