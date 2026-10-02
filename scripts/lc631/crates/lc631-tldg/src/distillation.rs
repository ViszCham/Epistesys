use crate::{
    build_geometry_shadow, AnchorState, CpuGeometryBackend, DeepGrammarArtifact, GeometryProposal,
    GeometryShadow, GeometrySignature, MaterializedRelationEvidence, RelationState,
    StructuralKernel, SyntaxNodeId,
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
    StateEncodingUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DistillationStopReason {
    ResidualsCleared,
    NoProgress,
    EpochBudget,
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
    pub materialized_relations_added: usize,
    pub rejected: usize,
    pub needs_evidence: usize,
    pub incomparable: usize,
    pub grammar_mutated: bool,
    pub thresholds_mutated: bool,
    pub state_digest: String,
    pub state_changed: bool,
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
    pub stop_reason: DistillationStopReason,
}

pub fn run_mutual_distillation(
    mut kernel: StructuralKernel,
    artifact: &DeepGrammarArtifact,
    budget: ParseBudget,
    backend: &CpuGeometryBackend,
) -> Result<MutualDistillationRun, DistillationError> {
    if budget.max_epochs == 0 || budget.max_proposals == 0 {
        return Err(DistillationError::BudgetExhausted);
    }
    let mut input_revision = kernel.kernel_digest.clone();
    let mut stop_reason = DistillationStopReason::EpochBudget;
    let mut records = Vec::new();
    let mut requests = Vec::new();
    let mut final_payload = None;
    let mut final_backflow = None;
    for epoch in 0..budget.max_epochs {
        let input_kernel_digest = kernel.kernel_digest.clone();
        let mut anchored = DistillationEpoch::anchored(kernel.clone(), budget);
        anchored.epoch = epoch;
        let validated = anchored.relax(backend)?.decode()?.validate(artifact);
        let backflow = validated.backflow();
        let (next_kernel, materialized_relations_added) =
            apply_accepted_materializations(&kernel, artifact, &validated.payload.accepted, epoch)?;
        let state_changed = next_kernel.kernel_digest != input_kernel_digest;
        let output_revision = next_kernel.kernel_digest.clone();
        let mut output_payload = validated.payload.clone();
        output_payload.kernel = next_kernel.clone();
        let state_digest = epoch_state_digest(&output_payload)?;
        records.push(DistillationEpochRecord {
            epoch,
            input_revision: input_revision.clone(),
            output_revision: output_revision.clone(),
            accepted: validated.payload.accepted.len(),
            materialized_relations_added,
            rejected: validated.payload.rejected.len(),
            needs_evidence: validated.payload.needs_evidence.len(),
            incomparable: validated.payload.incomparable.len(),
            grammar_mutated: false,
            thresholds_mutated: false,
            state_digest: state_digest.clone(),
            state_changed,
        });
        let persistent_unknown = next_kernel
            .anchors
            .iter()
            .any(|anchor| anchor.state != AnchorState::Verified);
        let has_residual = persistent_unknown
            || !validated.payload.rejected.is_empty()
            || !validated.payload.needs_evidence.is_empty()
            || !validated.payload.incomparable.is_empty();
        if has_residual && state_changed {
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
                next_input_revision: next_kernel.kernel_digest.clone(),
                residual_classes,
                authority_created: false,
            });
        }
        input_revision = output_revision;
        final_payload = Some(output_payload);
        final_backflow = Some(backflow);
        if !has_residual {
            stop_reason = DistillationStopReason::ResidualsCleared;
            break;
        }
        if !state_changed {
            stop_reason = DistillationStopReason::NoProgress;
            break;
        }
        kernel = next_kernel;
        if usize::from(epoch) + 1 == usize::from(budget.max_epochs) {
            stop_reason = DistillationStopReason::EpochBudget;
        }
    }
    Ok(MutualDistillationRun {
        max_epochs_respected: records.len() <= usize::from(budget.max_epochs),
        epochs: records,
        reprojection_requests: requests,
        final_payload: final_payload.ok_or(DistillationError::BudgetExhausted)?,
        final_backflow: final_backflow.ok_or(DistillationError::BudgetExhausted)?,
        stop_reason,
    })
}

fn apply_accepted_materializations(
    kernel: &StructuralKernel,
    artifact: &DeepGrammarArtifact,
    accepted: &[AcceptedProposal],
    epoch: u16,
) -> Result<(StructuralKernel, usize), DistillationError> {
    let mut next = kernel.clone();
    let before_count = next.materialized_relations.len();
    for accepted in accepted {
        let proposal = &accepted.proposal;
        let Some(edge) = artifact.syntax.relations.iter().find(|edge| {
            edge.from == proposal.source
                && edge.to == proposal.target
                && edge.kind == proposal.relation
                && edge.state == RelationState::Verified
                && !edge.evidence.trim().is_empty()
        }) else {
            continue;
        };
        let source_relation_evidence_digest = stable_sha256(&format!(
            "{}\0{:?}\0{}",
            edge.evidence, edge.kind, artifact.source.revision
        ));
        let materialization_id = stable_sha256(&format!(
            "epistesys-distillation-materialization.v1\0{}\0{}\0{}\0{:?}\0{}",
            artifact.source.revision,
            proposal.source.0,
            proposal.target.0,
            proposal.relation,
            source_relation_evidence_digest
        ));
        if !next
            .materialized_relations
            .iter()
            .any(|materialized| materialized.materialization_id == materialization_id)
        {
            next.materialized_relations
                .push(MaterializedRelationEvidence {
                    materialization_id,
                    source: proposal.source,
                    target: proposal.target,
                    kind: proposal.relation,
                    source_relation_evidence_digest,
                    first_materialized_epoch: epoch,
                    claim_boundary: "materialized from a non-empty source relation already marked Verified; no new semantic truth, grammar mutation, or authority is created",
                });
        }
    }
    next.materialized_relations
        .sort_by(|left, right| left.materialization_id.cmp(&right.materialization_id));
    let added = next
        .materialized_relations
        .len()
        .saturating_sub(before_count);
    next.kernel_digest = crate::kernel::distillation_kernel_digest(
        &next.structural_revision,
        &next.materialized_relations,
    )
    .map_err(|_| DistillationError::StateEncodingUnavailable)?;
    Ok((next, added))
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
            } else if artifact.syntax.relations.iter().any(|edge| {
                edge.from == proposal.source
                    && edge.to == proposal.target
                    && edge.kind == proposal.relation
                    && edge.state == RelationState::Verified
                    && !edge.evidence.trim().is_empty()
            }) {
                ValidationState::Passed
            } else {
                ValidationState::NeedsEvidence
            };
            let receipt = ValidationReceipt {
                proposal_id: proposal.id,
                verifier: "lc631-syntax-edge-membership.v1".into(),
                status,
                canonical_edge_authorized: false,
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

fn epoch_state_digest(payload: &EpochPayload) -> Result<String, DistillationError> {
    let canonical = serde_json::to_string(&(
        &payload.kernel.source_revision,
        &payload.kernel.kernel_digest,
        &payload.kernel.materialized_relations,
        &payload.decoded,
        &payload.accepted,
        &payload.rejected,
        &payload.needs_evidence,
        &payload.incomparable,
    ))
    .map_err(|_| DistillationError::StateEncodingUnavailable)?;
    Ok(stable_sha256(&canonical))
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

    #[test]
    fn geometry_candidate_requires_matching_verified_edge_and_never_authorizes_canonical_state() {
        let artifact = analyze("alpha beta").unwrap();
        let kernel = build_structural_kernel(&artifact).unwrap();
        let validated = DistillationEpoch::anchored(kernel, ParseBudget::reference())
            .relax(&CpuGeometryBackend)
            .unwrap()
            .decode()
            .unwrap()
            .validate(&artifact);
        assert!(validated.payload.accepted.is_empty());
        assert!(!validated.payload.needs_evidence.is_empty());
        assert!(validated
            .payload
            .needs_evidence
            .iter()
            .all(|receipt| !receipt.canonical_edge_authorized));
    }
}
