use lc631_tldg::{
    analyze, build_structural_kernel, run_mutual_distillation, AnchorState, CpuGeometryBackend,
    ParseBudget,
};
use std::collections::BTreeSet;

#[test]
fn arc631_16_max_epochs_controls_real_revisioned_backflow_iterations() {
    let artifact = analyze("ambiguous input").unwrap();
    let mut kernel = build_structural_kernel(&artifact).unwrap();
    kernel.anchors.last_mut().unwrap().state = AnchorState::Unknown;
    let run = run_mutual_distillation(
        kernel,
        &artifact,
        ParseBudget {
            max_epochs: 3,
            max_proposals: 128,
        },
        &CpuGeometryBackend,
    )
    .unwrap();
    assert_eq!(run.epochs.len(), 3);
    assert_eq!(
        run.epochs
            .iter()
            .map(|epoch| epoch.output_revision.clone())
            .collect::<BTreeSet<_>>()
            .len(),
        3
    );
    assert!(!run.reprojection_requests.is_empty());
    assert!(run
        .epochs
        .iter()
        .all(|epoch| !epoch.grammar_mutated && !epoch.thresholds_mutated));
}
