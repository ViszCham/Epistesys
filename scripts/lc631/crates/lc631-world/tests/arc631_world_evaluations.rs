use lc631_world::{materialize_evaluations, WorldArena, MAX_EVALUATIONS};
use std::collections::BTreeSet;

#[test]
fn arc631_15_materializes_all_projection_effects_as_distinct_evaluation_rows() {
    let arena = WorldArena::new("premise").unwrap();
    let evaluations = materialize_evaluations(arena.worlds()).unwrap();
    assert_eq!(evaluations.len(), MAX_EVALUATIONS);
    assert_eq!(
        evaluations
            .iter()
            .map(|evaluation| evaluation.evaluation_digest.clone())
            .collect::<BTreeSet<_>>()
            .len(),
        MAX_EVALUATIONS
    );
    assert_eq!(
        evaluations
            .iter()
            .map(|evaluation| evaluation.axes_x100)
            .collect::<BTreeSet<_>>()
            .len(),
        MAX_EVALUATIONS
    );
    assert!(evaluations
        .iter()
        .all(|evaluation| !evaluation.effect_digest.is_empty()));
}
