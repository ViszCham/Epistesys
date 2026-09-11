use crate::{AnchorState, RelationKind, StructuralKernel, SyntaxNodeId};
use serde::Serialize;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GeometrySignature {
    pub hyperbolic_dims: u16,
    pub euclidean_dims: u16,
    pub spherical_dims: u16,
    pub directed_dims: u16,
    pub revision: String,
}

impl GeometrySignature {
    pub fn mixed_reference() -> Self {
        Self {
            hyperbolic_dims: 2,
            euclidean_dims: 2,
            spherical_dims: 1,
            directed_dims: 1,
            revision: "lc631-mixed-curvature-reference.v1".into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GeometryPoint {
    pub node_id: SyntaxNodeId,
    pub order: u32,
    pub hyperbolic_radius_x1000: i32,
    pub euclidean_x1000: i32,
    pub spherical_phase_x1024: i32,
    pub directed_bias_x1000: i32,
    pub state: AnchorState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalAuthority {
    AdvisoryOnly,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GeometryProposal {
    pub id: u32,
    pub source: SyntaxNodeId,
    pub target: SyntaxNodeId,
    pub source_state: AnchorState,
    pub target_state: AnchorState,
    pub relation: RelationKind,
    pub forward_cost_x1000: u32,
    pub reverse_cost_x1000: u32,
    pub authority: ProposalAuthority,
    pub geometry_revision: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GeometryCoupling {
    pub source: SyntaxNodeId,
    pub target: SyntaxNodeId,
    pub mass_x1000: u16,
    pub relational_cost_x1000: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GeometryShadow {
    pub signature: GeometrySignature,
    pub source_kernel_digest: String,
    pub points: Vec<GeometryPoint>,
    pub proposals: Vec<GeometryProposal>,
    pub couplings: Vec<GeometryCoupling>,
    pub unanchored_mass_x1000: u16,
    pub canonical_edge_count: usize,
    pub claim_boundary: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GeometryError {
    EmptyKernel,
    InvalidSignature,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub struct CpuGeometryBackend;

impl CpuGeometryBackend {
    pub fn finsler_cost(source: &GeometryPoint, target: &GeometryPoint) -> u32 {
        let base = source
            .euclidean_x1000
            .abs_diff(target.euclidean_x1000)
            .saturating_add(
                source
                    .hyperbolic_radius_x1000
                    .abs_diff(target.hyperbolic_radius_x1000),
            )
            .saturating_add(cyclic_delta(
                source.spherical_phase_x1024,
                target.spherical_phase_x1024,
            ));
        let direction = if target.order >= source.order { 7 } else { 31 };
        base.saturating_add(direction)
            .saturating_add(source.directed_bias_x1000.unsigned_abs() / 16)
    }
}

pub fn build_geometry_shadow(
    kernel: &StructuralKernel,
    signature: &GeometrySignature,
) -> Result<GeometryShadow, GeometryError> {
    if kernel.anchors.is_empty() {
        return Err(GeometryError::EmptyKernel);
    }
    if signature.hyperbolic_dims == 0
        || signature.euclidean_dims == 0
        || signature.spherical_dims == 0
        || signature.directed_dims == 0
    {
        return Err(GeometryError::InvalidSignature);
    }
    let points = kernel
        .anchors
        .iter()
        .map(|anchor| GeometryPoint {
            node_id: anchor.node_id,
            order: anchor.order,
            hyperbolic_radius_x1000: 1_000
                + i32::try_from(anchor.relation_degree).unwrap_or(i32::MAX) * 97,
            euclidean_x1000: i32::try_from(anchor.order).unwrap_or(i32::MAX) * 101,
            spherical_phase_x1024: i32::try_from(anchor.node_id.0 % 1_024).unwrap_or_default(),
            directed_bias_x1000: i32::from(anchor.profile_count) * 37,
            state: anchor.state,
        })
        .collect::<Vec<_>>();
    let proposals = points
        .windows(2)
        .enumerate()
        .map(|(index, pair)| GeometryProposal {
            id: index as u32 + 1,
            source: pair[0].node_id,
            target: pair[1].node_id,
            source_state: pair[0].state,
            target_state: pair[1].state,
            relation: RelationKind::DependencyCandidate,
            forward_cost_x1000: CpuGeometryBackend::finsler_cost(&pair[0], &pair[1]),
            reverse_cost_x1000: CpuGeometryBackend::finsler_cost(&pair[1], &pair[0]),
            authority: ProposalAuthority::AdvisoryOnly,
            geometry_revision: signature.revision.clone(),
        })
        .collect::<Vec<_>>();
    let couplings = points
        .iter()
        .enumerate()
        .map(|(index, point)| {
            let target = &points[(index + 1) % points.len()];
            GeometryCoupling {
                source: point.node_id,
                target: target.node_id,
                mass_x1000: 1_000,
                relational_cost_x1000: relational_cost(point, target),
            }
        })
        .collect();
    Ok(GeometryShadow {
        signature: signature.clone(),
        source_kernel_digest: kernel.kernel_digest.clone(),
        points,
        proposals,
        couplings,
        unanchored_mass_x1000: 0,
        canonical_edge_count: 0,
        claim_boundary:
            "mixed-curvature CPU proposal shadow only; no parse truth, evidence, or authority"
                .into(),
    })
}

fn cyclic_delta(left: i32, right: i32) -> u32 {
    let direct = left.abs_diff(right);
    direct.min(1_024_u32.saturating_sub(direct))
}

fn relational_cost(left: &GeometryPoint, right: &GeometryPoint) -> u32 {
    left.order
        .abs_diff(right.order)
        .saturating_mul(100)
        .saturating_add(
            left.node_id
                .0
                .abs_diff(right.node_id.0)
                .min(u64::from(u32::MAX)) as u32,
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{analyze, build_structural_kernel};

    #[test]
    fn geometry_is_advisory_and_directional() {
        let artifact = analyze("a -> b").unwrap();
        let kernel = build_structural_kernel(&artifact).unwrap();
        let shadow = build_geometry_shadow(&kernel, &GeometrySignature::mixed_reference()).unwrap();
        assert_eq!(shadow.canonical_edge_count, 0);
        assert!(shadow.proposals.iter().all(|proposal| {
            proposal.authority == ProposalAuthority::AdvisoryOnly
                && proposal.forward_cost_x1000 != proposal.reverse_cost_x1000
        }));
    }
}
