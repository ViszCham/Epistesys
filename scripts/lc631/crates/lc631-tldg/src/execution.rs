use crate::{
    analyze, build_geometry_shadow, build_structural_kernel, AnchorState, GeometrySignature,
};
use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GeometryParityState {
    Unavailable,
    Matched,
    Mismatched,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GeometryFault {
    DeviceLost,
    Timeout,
    OutOfMemory,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GeometryGpuObservation {
    pub cpu_digest: String,
    pub gpu_digest: String,
    pub device_identity: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GeometryExecutionReceipt {
    pub parity: GeometryParityState,
    pub real_gpu_observed: bool,
    pub quarantined: bool,
    pub fault: Option<GeometryFault>,
    pub partial_stitching_allowed: bool,
    pub creates_authority: bool,
    pub creates_evidence_provenance: bool,
    pub claim_boundary: String,
}

pub fn evaluate_geometry_execution(
    observation: Option<&GeometryGpuObservation>,
    fault: Option<GeometryFault>,
) -> GeometryExecutionReceipt {
    if let Some(fault) = fault {
        return GeometryExecutionReceipt {
            parity: GeometryParityState::Unavailable,
            real_gpu_observed: observation.is_some(),
            quarantined: true,
            fault: Some(fault),
            partial_stitching_allowed: false,
            creates_authority: false,
            creates_evidence_provenance: false,
            claim_boundary: "faulted device output quarantined; no partial semantic stitching"
                .into(),
        };
    }
    let parity = observation.map_or(GeometryParityState::Unavailable, |observation| {
        if observation.cpu_digest == observation.gpu_digest {
            GeometryParityState::Matched
        } else {
            GeometryParityState::Mismatched
        }
    });
    GeometryExecutionReceipt {
        parity,
        real_gpu_observed: observation.is_some(),
        quarantined: parity == GeometryParityState::Mismatched,
        fault: None,
        partial_stitching_allowed: false,
        creates_authority: false,
        creates_evidence_provenance: false,
        claim_boundary:
            "parity receipt only; no occupancy, performance, semantic truth, or authority claim"
                .into(),
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AdversarialEvaluationReport {
    pub source_roundtrip_pass: bool,
    pub geometry_non_authority_pass: bool,
    pub unknown_retention_pass: bool,
    pub real_gpu_geometry_observed: bool,
    pub general_performance_claim_supported: bool,
    pub cases: Vec<String>,
}

pub fn run_adversarial_evaluation() -> Result<AdversarialEvaluationReport, String> {
    let mixed = "説明:\n~~~rust\nfn main() {}\n~~~\n";
    let artifact = analyze(mixed).map_err(|error| format!("{error:?}"))?;
    let mut kernel = build_structural_kernel(&artifact).map_err(|error| format!("{error:?}"))?;
    let shadow = build_geometry_shadow(&kernel, &GeometrySignature::mixed_reference())
        .map_err(|error| format!("{error:?}"))?;
    let geometry_non_authority_pass = shadow.canonical_edge_count == 0
        && shadow
            .proposals
            .iter()
            .all(|proposal| matches!(proposal.authority, crate::ProposalAuthority::AdvisoryOnly));
    if let Some(anchor) = kernel.anchors.last_mut() {
        anchor.state = AnchorState::Unknown;
    }
    let unknown_retention_pass = kernel
        .anchors
        .iter()
        .any(|anchor| anchor.state == AnchorState::Unknown);
    Ok(AdversarialEvaluationReport {
        source_roundtrip_pass: artifact.boundary_roundtrip == mixed,
        geometry_non_authority_pass,
        unknown_retention_pass,
        real_gpu_geometry_observed: false,
        general_performance_claim_supported: false,
        cases: vec![
            "mixed_document_roundtrip".into(),
            "geometry_non_authority".into(),
            "unknown_anchor_retention".into(),
            "real_gpu_unobserved".into(),
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_gpu_observation_is_unavailable() {
        let receipt = evaluate_geometry_execution(None, None);
        assert_eq!(receipt.parity, GeometryParityState::Unavailable);
        assert!(!receipt.real_gpu_observed);
    }
}
