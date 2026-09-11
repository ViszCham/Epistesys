use crate::{
    build_geometry_shadow, AnchorState, CpuGeometryBackend, DeepGrammarArtifact, GeometryProposal,
    GeometryShadow, GeometrySignature, StructuralKernel, SyntaxNodeId,
};
use lc631_core::stable_sha256;
use serde::Serialize;
use std::collections::BTreeSet;
use std::marker::PhantomData;

pub enum Anchored {}
pub enum Proposed {}
pub enum Decoded {}
pub enum Validated {}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ParseBudget {
    pub max_epochs: u16,
    pub max_proposals: usize,
}

impl ParseBudget {
    pub const fn reference() -> Self {
        Self {
            max_epochs: 4,
            max_proposals: 128,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DecodedProposal {
    pub proposal: GeometryProposal,
    pub decoded_from_geometry: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationState {
    Passed,
    Failed,
    NeedsEvidence,
    Incomparable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ValidationReceipt {
    pub proposal_id: u32,
    pub verifier: String,
    pub status: ValidationState,
    pub canonical_edge_authorized: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AcceptedProposal {
    pub proposal: GeometryProposal,
    pub receipt: ValidationReceipt,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RejectedProposal {
    pub proposal_id: u32,
    pub reason: String,
    pub receipt: ValidationReceipt,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EpochPayload {
    pub kernel: StructuralKernel,
    pub geometry: Option<GeometryShadow>,
    pub decoded: Vec<DecodedProposal>,
    pub accepted: Vec<AcceptedProposal>,
    pub rejected: Vec<RejectedProposal>,
    pub needs_evidence: Vec<ValidationReceipt>,
    pub incomparable: Vec<ValidationReceipt>,
    pub geometry_direct_promotions: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DistillationEpoch<State> {
    pub epoch: u16,
    pub budget: ParseBudget,
    pub payload: EpochPayload,
    #[serde(skip)]
    state: PhantomData<State>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DistillationError {
    BudgetExhausted,
    GeometryUnavailable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BackflowReport {
    pub grammar_mutated: bool,
    pub thresholds_mutated: bool,
    pub accepted_candidates: usize,
    pub rejected_residuals: usize,
    pub retained_unknowns: usize,
    pub retained_incomparable: usize,
    pub claim_boundary: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DistillationEpochRecord {
    pub epoch: u16,
    pub input_revision: String,
    pub output_revision: String,
    pub accepted: usize,
    pub rejected: usize,
    pub needs_evidence: usize,
    pub incomparable: usize,
    pub grammar_mutated: bool,
    pub thresholds_mutated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReprojectionRequest {
    pub after_epoch: u16,
    pub from_revision: String,
    pub next_input_revision: String,
    pub residual_classes: Vec<String>,
    pub authority_created: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MutualDistillationRun {
    pub epochs: Vec<DistillationEpochRecord>,
    pub reprojection_requests: Vec<ReprojectionRequest>,
    pub final_payload: EpochPayload,
    pub final_backflow: BackflowReport,
    pub max_epochs_respected: bool,
}

pub fn run_mutual_distillation(
    kernel: StructuralKernel,
    artifact: &DeepGrammarArtifact,
    budget: ParseBudget,
    backend: &CpuGeometryBackend,
) -> Result<MutualDistillationRun, DistillationError> {
    if budget.max_epochs == 0 || budget.max_proposals == 0 {
        return Err(DistillationError::BudgetExhausted);
    }
    let persistent_unknown = kernel
        .anchors
        .iter()
        .any(|anchor| anchor.state != AnchorState::Verified);
    let mut input_revision = kernel.kernel_digest.clone();
    let mut records = Vec::new();
    let mut requests = Vec::new();
    let mut final_payload = None;
    let mut final_backflow = None;
    for epoch in 0..budget.max_epochs {
        let mut anchored = DistillationEpoch::anchored(kernel.clone(), budget);
        anchored.epoch = epoch;
        let validated = anchored.relax(backend)?.decode()?.validate(artifact);
        let backflow = validated.backflow();
        let output_revision = stable_sha256(&format!(
            "{}:{}:{}:{}:{}:{}",
            input_revision,
            epoch,
            validated.payload.accepted.len(),
            validated.payload.rejected.len(),
            validated.payload.needs_evidence.len(),
            validated.payload.incomparable.len()
        ));
        records.push(DistillationEpochRecord {
            epoch,
            input_revision: input_revision.clone(),
            output_revision: output_revision.clone(),
            accepted: validated.payload.accepted.len(),
            rejected: validated.payload.rejected.len(),
            needs_evidence: validated.payload.needs_evidence.len(),
            incomparable: validated.payload.incomparable.len(),
            grammar_mutated: false,
            thresholds_mutated: false,
        });
        let has_residual = persistent_unknown
            || !validated.payload.rejected.is_empty()
            || !validated.payload.needs_evidence.is_empty()
            || !validated.payload.incomparable.is_empty();
        if has_residual {
            let mut residual_classes = Vec::new();
            if persistent_unknown || !validated.payload.needs_evidence.is_empty() {
                residual_classes.push("needs_evidence".into());
            }
            if !validated.payload.rejected.is_empty() {
                residual_classes.push("rejected".into());
            }
            if !validated.payload.incomparable.is_empty() {
                residual_classes.push("incomparable".into());
            }
            requests.push(ReprojectionRequest {
                after_epoch: epoch,
                from_revision: input_revision.clone(),
                next_input_revision: output_revision.clone(),
                residual_classes,
                authority_created: false,
            });
        }
        input_revision = output_revision;
        final_payload = Some(validated.payload.clone());
        final_backflow = Some(backflow);
        if !has_residual {
            break;
        }
    }
    Ok(MutualDistillationRun {
        max_epochs_respected: records.len() <= usize::from(budget.max_epochs),
        epochs: records,
        reprojection_requests: requests,
        final_payload: final_payload.ok_or(DistillationError::BudgetExhausted)?,
        final_backflow: final_backflow.ok_or(DistillationError::BudgetExhausted)?,
    })
}

impl DistillationEpoch<Anchored> {
    pub fn anchored(kernel: StructuralKernel, budget: ParseBudget) -> Self {
        Self {
            epoch: 0,
            budget,
            payload: EpochPayload {
                kernel,
                geometry: None,
                decoded: Vec::new(),
                accepted: Vec::new(),
                rejected: Vec::new(),
                needs_evidence: Vec::new(),
                incomparable: Vec::new(),
                geometry_direct_promotions: 0,
            },
            state: PhantomData,
        }
    }

    pub fn relax(
        self,
        _backend: &CpuGeometryBackend,
    ) -> Result<DistillationEpoch<Proposed>, DistillationError> {
        if self.budget.max_epochs == 0 || self.budget.max_proposals == 0 {
            return Err(DistillationError::BudgetExhausted);
        }
        let geometry =
            build_geometry_shadow(&self.payload.kernel, &GeometrySignature::mixed_reference())
                .map_err(|_| DistillationError::GeometryUnavailable)?;
        Ok(DistillationEpoch {
            epoch: self.epoch,
            budget: self.budget,
            payload: EpochPayload {
                geometry: Some(geometry),
                ..self.payload
            },
            state: PhantomData,
        })
    }
}

impl DistillationEpoch<Proposed> {
    pub fn decode(self) -> Result<DistillationEpoch<Decoded>, DistillationError> {
        let geometry = self
            .payload
            .geometry
            .as_ref()
            .ok_or(DistillationError::GeometryUnavailable)?;
        let decoded = geometry
            .proposals
            .iter()
            .take(self.budget.max_proposals)
            .cloned()
            .map(|proposal| DecodedProposal {
                proposal,
                decoded_from_geometry: true,
            })
            .collect();
        Ok(DistillationEpoch {
            epoch: self.epoch,
            budget: self.budget,
            payload: EpochPayload {
                decoded,
                ..self.payload
            },
            state: PhantomData,
        })
    }
}

impl DistillationEpoch<Decoded> {
    pub fn validate(self, artifact: &DeepGrammarArtifact) -> DistillationEpoch<Validated> {
        let nodes = artifact
            .syntax
            .nodes
            .iter()
            .map(|node| node.id)
            .collect::<BTreeSet<SyntaxNodeId>>();
        let mut accepted = Vec::new();
        let mut rejected = Vec::new();
        let mut needs_evidence = Vec::new();
        let mut incomparable = Vec::new();
        for decoded in &self.payload.decoded {
            let proposal = &decoded.proposal;
            let status = if proposal.source_state == AnchorState::Incomparable
                || proposal.target_state == AnchorState::Incomparable
            {
                ValidationState::Incomparable
            } else if proposal.source_state == AnchorState::Unknown
                || proposal.target_state == AnchorState::Unknown
            {
                ValidationState::NeedsEvidence
            } else if proposal.source == proposal.target
                || !nodes.contains(&proposal.source)
                || !nodes.contains(&proposal.target)
            {
                ValidationState::Failed
            } else {
                ValidationState::Passed
            };
            let receipt = ValidationReceipt {
                proposal_id: proposal.id,
                verifier: "lc631-discrete-structure.v1".into(),
                status,
                canonical_edge_authorized: status == ValidationState::Passed,
            };
            match status {
                ValidationState::Passed => accepted.push(AcceptedProposal {
                    proposal: proposal.clone(),
                    receipt,
                }),
                ValidationState::Failed => rejected.push(RejectedProposal {
                    proposal_id: proposal.id,
                    reason: "decoded_relation_failed_discrete_validation".into(),
                    receipt,
                }),
                ValidationState::NeedsEvidence => needs_evidence.push(receipt),
                ValidationState::Incomparable => incomparable.push(receipt),
            }
        }
        DistillationEpoch {
            epoch: self.epoch,
            budget: self.budget,
            payload: EpochPayload {
                accepted,
                rejected,
                needs_evidence,
                incomparable,
                geometry_direct_promotions: 0,
                ..self.payload
            },
            state: PhantomData,
        }
    }
}

impl DistillationEpoch<Validated> {
    pub fn backflow(&self) -> BackflowReport {
        BackflowReport {
            grammar_mutated: false,
            thresholds_mutated: false,
            accepted_candidates: self.payload.accepted.len(),
            rejected_residuals: self.payload.rejected.len(),
            retained_unknowns: self.payload.needs_evidence.len(),
            retained_incomparable: self.payload.incomparable.len(),
            claim_boundary:
                "residual suggestions only; no in-session grammar, threshold, or authority mutation"
                    .into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{analyze, build_structural_kernel};

    #[test]
    fn zero_budget_is_fail_visible() {
        let artifact = analyze("a").unwrap();
        let kernel = build_structural_kernel(&artifact).unwrap();
        let result = DistillationEpoch::anchored(
            kernel,
            ParseBudget {
                max_epochs: 0,
                max_proposals: 0,
            },
        )
        .relax(&CpuGeometryBackend);
        assert!(matches!(result, Err(DistillationError::BudgetExhausted)));
    }
}
