use lc631_accelerator::{
    decision_summary_u32, input_from_evaluations, AcceleratorBroker, CpuBackend, LaneKind,
    NumericMask, RelationCode,
};
use lc631_core::{stable_sha256, ArtifactId, AuthorityRevision, EpochId, RevisionBinding};
use lc631_world::{materialize_evaluations, WorldArena, MAX_EVALUATIONS};

fn binding() -> RevisionBinding {
    RevisionBinding {
        artifact: ArtifactId(1),
        source_sha256: stable_sha256("source"),
        program_sha256: stable_sha256("program"),
        authority_revision: AuthorityRevision(1),
        engine_version: "6.3.1".into(),
        kernel_revision: "arc631".into(),
        device_identity: "cpu".into(),
    }
}

#[test]
fn arc631_14_known_summary_collision_is_separated() {
    assert_ne!(
        decision_summary_u32(&[0, 0, 2, 1]),
        decision_summary_u32(&[0, 1, 0, 2])
    );
}

#[test]
fn arc631_13_mask_and_relation_codes_are_distinct() {
    assert_eq!(
        [
            NumericMask::Known.code(),
            NumericMask::Unknown.code(),
            NumericMask::NotApplicable.code(),
            NumericMask::Incomparable.code(),
        ]
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>()
        .len(),
        4
    );
    assert_ne!(
        RelationCode::Unknown.code(),
        RelationCode::Incomparable.code()
    );
    assert_ne!(
        RelationCode::NotApplicable.code(),
        RelationCode::Incomparable.code()
    );
}

#[test]
fn arc631_15_accelerator_consumes_all_2048_materialized_evaluations() {
    let arena = WorldArena::new("premise").unwrap();
    let evaluations = materialize_evaluations(arena.worlds()).unwrap();
    let input = input_from_evaluations(&evaluations).unwrap();
    assert_eq!(input.rows(), MAX_EVALUATIONS);
    assert_eq!(input.row_identity_digests.len(), MAX_EVALUATIONS);
    let mut broker = AcceleratorBroker::new(CpuBackend);
    let receipt = broker
        .execute_epoch(EpochId(1), &binding(), &input)
        .unwrap();
    assert_eq!(
        receipt
            .lanes
            .iter()
            .find(|lane| lane.lane == LaneKind::ProjectionLoss)
            .unwrap()
            .rows,
        MAX_EVALUATIONS
    );
}
