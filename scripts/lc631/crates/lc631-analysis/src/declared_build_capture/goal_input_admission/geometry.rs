//! Explicit borrowed geometry fields; no Serialize and no wildcard field omission.
use super::{number, Field, Node, Object, Sequence};

pub(super) fn geometry(value: &lc631_tldg::GeometryShadow, index: usize) -> Option<Field<'_>> {
    let lc631_tldg::GeometryShadow {
        signature,
        source_kernel_digest,
        points,
        proposals,
        couplings,
        unanchored_mass_x1000,
        canonical_edge_count,
        claim_boundary,
    } = value;
    match index {
        0 => Some(Field {
            key: "signature",
            value: Node::Object(Object::Signature(signature)),
        }),
        1 => Some(Field {
            key: "source_kernel_digest",
            value: Node::Str(source_kernel_digest),
        }),
        2 => Some(Field {
            key: "points",
            value: Node::Seq(Sequence::Point(points)),
        }),
        3 => Some(Field {
            key: "proposals",
            value: Node::Seq(Sequence::Proposal(proposals)),
        }),
        4 => Some(Field {
            key: "couplings",
            value: Node::Seq(Sequence::Coupling(couplings)),
        }),
        5 => Some(Field {
            key: "unanchored_mass_x1000",
            value: number(unanchored_mass_x1000),
        }),
        6 => Some(Field {
            key: "canonical_edge_count",
            value: number(canonical_edge_count),
        }),
        7 => Some(Field {
            key: "claim_boundary",
            value: Node::Str(claim_boundary),
        }),
        _ => None,
    }
}

pub(super) fn signature(value: &lc631_tldg::GeometrySignature, index: usize) -> Option<Field<'_>> {
    let lc631_tldg::GeometrySignature {
        hyperbolic_dims,
        euclidean_dims,
        spherical_dims,
        directed_dims,
        revision,
    } = value;
    match index {
        0 => Some(Field {
            key: "hyperbolic_dims",
            value: number(hyperbolic_dims),
        }),
        1 => Some(Field {
            key: "euclidean_dims",
            value: number(euclidean_dims),
        }),
        2 => Some(Field {
            key: "spherical_dims",
            value: number(spherical_dims),
        }),
        3 => Some(Field {
            key: "directed_dims",
            value: number(directed_dims),
        }),
        4 => Some(Field {
            key: "revision",
            value: Node::Str(revision),
        }),
        _ => None,
    }
}

pub(super) fn point(value: &lc631_tldg::GeometryPoint, index: usize) -> Option<Field<'_>> {
    let lc631_tldg::GeometryPoint {
        node_id,
        order,
        hyperbolic_radius_x1000,
        euclidean_x1000,
        spherical_phase_x1024,
        directed_bias_x1000,
        state,
    } = value;
    match index {
        0 => Some(Field {
            key: "node_id",
            value: number(node_id),
        }),
        1 => Some(Field {
            key: "order",
            value: number(order),
        }),
        2 => Some(Field {
            key: "hyperbolic_radius_x1000",
            value: number(hyperbolic_radius_x1000),
        }),
        3 => Some(Field {
            key: "euclidean_x1000",
            value: number(euclidean_x1000),
        }),
        4 => Some(Field {
            key: "spherical_phase_x1024",
            value: number(spherical_phase_x1024),
        }),
        5 => Some(Field {
            key: "directed_bias_x1000",
            value: number(directed_bias_x1000),
        }),
        6 => Some(Field {
            key: "state",
            value: Node::Str(super::tags_state::anchorstate(state)),
        }),
        _ => None,
    }
}

pub(super) fn proposal(value: &lc631_tldg::GeometryProposal, index: usize) -> Option<Field<'_>> {
    let lc631_tldg::GeometryProposal {
        id,
        source,
        target,
        source_state,
        target_state,
        relation,
        forward_cost_x1000,
        reverse_cost_x1000,
        authority,
        geometry_revision,
    } = value;
    match index {
        0 => Some(Field {
            key: "id",
            value: number(id),
        }),
        1 => Some(Field {
            key: "source",
            value: number(source),
        }),
        2 => Some(Field {
            key: "target",
            value: number(target),
        }),
        3 => Some(Field {
            key: "source_state",
            value: Node::Str(super::tags_state::anchorstate(source_state)),
        }),
        4 => Some(Field {
            key: "target_state",
            value: Node::Str(super::tags_state::anchorstate(target_state)),
        }),
        5 => Some(Field {
            key: "relation",
            value: Node::Str(super::tags_state::relationkind(relation)),
        }),
        6 => Some(Field {
            key: "forward_cost_x1000",
            value: number(forward_cost_x1000),
        }),
        7 => Some(Field {
            key: "reverse_cost_x1000",
            value: number(reverse_cost_x1000),
        }),
        8 => Some(Field {
            key: "authority",
            value: Node::Str(super::tags_state::proposalauthority(authority)),
        }),
        9 => Some(Field {
            key: "geometry_revision",
            value: Node::Str(geometry_revision),
        }),
        _ => None,
    }
}

pub(super) fn coupling(value: &lc631_tldg::GeometryCoupling, index: usize) -> Option<Field<'_>> {
    let lc631_tldg::GeometryCoupling {
        source,
        target,
        mass_x1000,
        relational_cost_x1000,
    } = value;
    match index {
        0 => Some(Field {
            key: "source",
            value: number(source),
        }),
        1 => Some(Field {
            key: "target",
            value: number(target),
        }),
        2 => Some(Field {
            key: "mass_x1000",
            value: number(mass_x1000),
        }),
        3 => Some(Field {
            key: "relational_cost_x1000",
            value: number(relational_cost_x1000),
        }),
        _ => None,
    }
}
