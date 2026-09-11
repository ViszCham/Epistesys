#![forbid(unsafe_code)]

use lc631_tldg::{
    analyze, build_geometry_shadow, build_structural_kernel, CpuGeometryBackend, DistillationEpoch,
    GeometrySignature, ParseBudget, ProposalAuthority, Validated,
};

#[test]
fn tldg_16_structural_kernel_uses_verified_relations_not_legacy_tl() {
    let artifact = analyze("fn main() { return x; }").unwrap();
    let kernel = build_structural_kernel(&artifact).unwrap();
    assert!(!kernel.anchors.is_empty());
    assert!(!kernel.legacy_translation_loss_consumed);
    assert!(kernel
        .anchors
        .iter()
        .all(|anchor| !anchor.source_spans.is_empty()));
}

#[test]
fn tldg_17_geometry_is_mixed_curvature_and_advisory() {
    let artifact = analyze("if x { return y }").unwrap();
    let kernel = build_structural_kernel(&artifact).unwrap();
    let signature = GeometrySignature::mixed_reference();
    let shadow = build_geometry_shadow(&kernel, &signature).unwrap();
    assert!(signature.hyperbolic_dims > 0);
    assert!(signature.euclidean_dims > 0);
    assert!(signature.spherical_dims > 0);
    assert!(shadow
        .proposals
        .iter()
        .all(|proposal| proposal.authority == ProposalAuthority::AdvisoryOnly));
}

#[test]
fn tldg_18_finsler_cost_is_directed_and_gw_coupling_is_bounded() {
    let artifact = analyze("a -> b -> c").unwrap();
    let kernel = build_structural_kernel(&artifact).unwrap();
    let shadow = build_geometry_shadow(&kernel, &GeometrySignature::mixed_reference()).unwrap();
    let first = &shadow.points[0];
    let last = shadow.points.last().unwrap();
    assert_ne!(
        CpuGeometryBackend::finsler_cost(first, last),
        CpuGeometryBackend::finsler_cost(last, first)
    );
    assert!(shadow
        .couplings
        .iter()
        .all(|coupling| coupling.mass_x1000 <= 1_000));
    assert_eq!(shadow.unanchored_mass_x1000, 0);
}

#[test]
fn tldg_19_mutual_distillation_requires_discrete_validation() {
    let artifact = analyze("if user says yes, return value").unwrap();
    let kernel = build_structural_kernel(&artifact).unwrap();
    let epoch = DistillationEpoch::anchored(kernel, ParseBudget::reference());
    let validated: DistillationEpoch<Validated> = epoch
        .relax(&CpuGeometryBackend)
        .unwrap()
        .decode()
        .unwrap()
        .validate(&artifact);
    assert!(validated.payload.geometry_direct_promotions == 0);
    assert!(validated
        .payload
        .accepted
        .iter()
        .all(|accepted| accepted.receipt.verifier == "lc631-discrete-structure.v1"));
}

#[test]
fn tldg_20_backflow_never_mutates_grammar_or_promotes_unknowns() {
    let artifact = analyze("call(x) or call x").unwrap();
    let kernel = build_structural_kernel(&artifact).unwrap();
    let validated = DistillationEpoch::anchored(kernel, ParseBudget::reference())
        .relax(&CpuGeometryBackend)
        .unwrap()
        .decode()
        .unwrap()
        .validate(&artifact);
    let backflow = validated.backflow();
    assert!(!backflow.grammar_mutated);
    assert!(!backflow.thresholds_mutated);
    assert_eq!(
        backflow.retained_unknowns,
        validated.payload.needs_evidence.len()
    );
}
