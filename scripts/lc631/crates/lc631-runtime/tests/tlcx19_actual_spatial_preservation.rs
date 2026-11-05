use lc631_core::context::{ContextRevision, ContextScope, SourceRevision};
use lc631_runtime::causal_spatial_observation::CausalSpatialObservations;
use lc631_runtime::resource::*;
use lc631_runtime::spatial_preservation::*;
use lc631_solvers::causal_spatial::{FrameId, Rectangle, SpatialRelation};
use lc631_substrate::{measurement_numeric::RationalInterval, Digest32};
use std::time::Duration;

fn id(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}
fn frame(n: u8) -> FrameId {
    FrameId([n; 32])
}
fn scope() -> ContextScope {
    ContextScope::declared(id(1), id(2), id(3), id(4))
}
fn rect(frame: FrameId, min: [i64; 2], max: [i64; 2]) -> Rectangle {
    Rectangle::new(frame, min, max).unwrap()
}
fn model(
    source: &str,
    frame: FrameId,
    left: Option<Rectangle>,
    right: Option<Rectangle>,
) -> SpatialModelInput {
    SpatialModelInput {
        context: ContextRevision::original(SourceRevision::of(source)),
        scope: scope(),
        frame,
        unit: SpatialUnit::Pixel,
        left,
        right,
    }
}
fn map(
    a: SpatialModelInput,
    b: SpatialModelInput,
    x_sign: AxisSign,
    dx: i64,
) -> DeclaredSpatialMap {
    DeclaredSpatialMap {
        source_frame: a.frame,
        target_frame: b.frame,
        source_unit: a.unit,
        target_unit: b.unit,
        signs: (x_sign, AxisSign::Positive),
        translation: (dx, 0),
    }
}
fn probes() -> Vec<ProtectedSpatialProbe> {
    [
        SpatialRelation::Contains,
        SpatialRelation::Connected,
        SpatialRelation::Touches,
        SpatialRelation::StrictlyLeftOf,
    ]
    .into_iter()
    .enumerate()
    .map(|(i, relation)| ProtectedSpatialProbe {
        id: id(10 + i as u8),
        weight: 1,
        relation: ProtectedSpatialRelation::Kernel(relation),
    })
    .chain(std::iter::once(ProtectedSpatialProbe {
        id: id(14),
        weight: 1,
        relation: ProtectedSpatialRelation::MappedPlacement,
    }))
    .collect()
}
fn run(
    policy: &SpatialPreservationPolicy,
    a: SpatialModelInput,
    b: SpatialModelInput,
) -> Result<SpatialPreservationReport, SpatialPreservationError> {
    let cap = ResourceLimits::uniform(8 * 1024 * 1024).unwrap();
    let owner = ResourceBroker::new(cap).unwrap();
    let run = owner
        .begin_run(
            19,
            a.context.source().digest(),
            cap,
            Deadline::after(Duration::from_secs(30)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let requests = [ResourceRequest::new(
        ResourceKind::Ram,
        ResourceAmount::checked(2 * 1024 * 1024).unwrap(),
    )];
    let mut ar = run
        .reserve_for_source(a.context.source().digest(), &requests)
        .unwrap();
    let mut br = run
        .reserve_for_source(b.context.source().digest(), &requests)
        .unwrap();
    let report = {
        let av = ar.lease();
        let bv = br.lease();
        let mut ast = CausalSpatialObservations::with_resources(4, &av).unwrap();
        let mut bst = CausalSpatialObservations::with_resources(4, &bv).unwrap();
        evaluate_spatial_preservation(policy, a, b, &mut ast, &mut bst, &av, &bv)
    };
    ar.release_confirmed().unwrap();
    br.release_confirmed().unwrap();
    report
}

#[test]
fn tlcx19_spatial_negative_unregistered_map_and_continuous_quantity_are_no_scalar() {
    let f = frame(1);
    let a = model(
        "input",
        f,
        Some(rect(f, [0, 0], [1, 1])),
        Some(rect(f, [2, 0], [3, 1])),
    );
    let policy = SpatialPreservationPolicy::declared(a, a, probes(), None).unwrap();
    let report = run(&policy, a, a).unwrap();
    assert!(report.loss().is_none());
    assert!(matches!(
        report.state(),
        SpatialPreservationState::NoScalar {
            reason: Gap::MissingCalibration,
            ..
        }
    ));
    assert_eq!(report.source_observations().len(), 4);
    assert_eq!(report.target_observations().len(), 4);
    let mut plan = probes();
    plan.push(ProtectedSpatialProbe {
        id: id(15),
        weight: 3,
        relation: ProtectedSpatialRelation::ContinuousDistance,
    });
    let unsupported =
        SpatialPreservationPolicy::declared(a, a, plan, Some(map(a, a, AxisSign::Positive, 0)))
            .unwrap();
    let result = run(&unsupported, a, a).unwrap();
    assert!(matches!(
        result.state(),
        SpatialPreservationState::NoScalar {
            reason: Gap::UnsupportedQuantity,
            ..
        }
    ));
    assert!(result.loss().is_none());
    assert_eq!(result.denominator(), 8);
}

#[test]
fn tlcx19_spatial_actual_x_reflection_swaps_strict_left_predicates_not_truth_bits() {
    let f = frame(1);
    let g = frame(2);
    let a = model(
        "source",
        f,
        Some(rect(f, [0, 0], [1, 1])),
        Some(rect(f, [2, 0], [3, 1])),
    );
    let b = model(
        "target",
        g,
        Some(rect(g, [9, 0], [10, 1])),
        Some(rect(g, [7, 0], [8, 1])),
    );
    let mapping = map(a, b, AxisSign::Negative, 10);
    let policy = SpatialPreservationPolicy::declared(a, b, probes(), Some(mapping)).unwrap();
    let report = run(&policy, a, b).unwrap();
    assert_eq!(
        report.loss().unwrap(),
        &RationalInterval::exact_ratio(0, 1).unwrap()
    );
    assert_eq!(report.denominator(), 5);
    assert_eq!(report.source_observations().len(), 4);
    let bad = SpatialModelInput {
        left: b.right,
        right: b.left,
        ..b
    };
    let changed = SpatialPreservationPolicy::declared(a, bad, probes(), Some(mapping)).unwrap();
    assert_eq!(policy.digest(), changed.digest());
    assert_ne!(policy.binding_digest(), changed.binding_digest());
    assert_eq!(
        run(&changed, a, bad).unwrap().loss().unwrap(),
        &RationalInterval::exact_ratio(2, 5).unwrap()
    );
}

#[test]
fn tlcx19_spatial_actual_boundary_contact_change_and_missing_objects_keep_fixed_denominator() {
    let f = frame(1);
    let g = frame(2);
    let a = model(
        "source",
        f,
        Some(rect(f, [0, 0], [2, 2])),
        Some(rect(f, [2, 0], [4, 2])),
    );
    let b = model(
        "target",
        g,
        Some(rect(g, [100, 0], [102, 2])),
        Some(rect(g, [102, 0], [104, 2])),
    );
    let mapping = map(a, b, AxisSign::Positive, 100);
    let preserved = SpatialPreservationPolicy::declared(a, b, probes(), Some(mapping)).unwrap();
    assert_eq!(
        run(&preserved, a, b).unwrap().loss().unwrap(),
        &RationalInterval::exact_ratio(0, 1).unwrap()
    );
    let changed = SpatialModelInput {
        right: Some(rect(g, [103, 0], [105, 2])),
        ..b
    };
    let policy = SpatialPreservationPolicy::declared(a, changed, probes(), Some(mapping)).unwrap();
    assert_eq!(
        run(&policy, a, changed).unwrap().loss().unwrap(),
        &RationalInterval::exact_ratio(4, 5).unwrap()
    );
    let missing = SpatialModelInput { right: None, ..b };
    let policy = SpatialPreservationPolicy::declared(a, missing, probes(), Some(mapping)).unwrap();
    let report = run(&policy, a, missing).unwrap();
    assert_eq!(report.denominator(), 5);
    assert_eq!(
        report.loss().unwrap(),
        &RationalInterval::from_ratio(0, 1, 1, 1).unwrap()
    );
    assert_eq!(report.validation_gap(), Gap::MissingEvidence);
}

#[test]
fn tlcx19_spatial_negative_frame_unit_and_exact_model_scope_swaps_are_incomparable() {
    let f = frame(1);
    let a = model(
        "input",
        f,
        Some(rect(f, [0, 0], [1, 1])),
        Some(rect(f, [2, 0], [3, 1])),
    );
    let mut wrong_map = map(a, a, AxisSign::Positive, 0);
    wrong_map.source_frame = frame(99);
    let policy = SpatialPreservationPolicy::declared(a, a, probes(), Some(wrong_map)).unwrap();
    assert!(matches!(
        run(&policy, a, a).unwrap().state(),
        SpatialPreservationState::Incomparable(IncomparableReason::FrameMismatch)
    ));
    let b = SpatialModelInput {
        unit: SpatialUnit::Millimeter,
        ..a
    };
    let policy =
        SpatialPreservationPolicy::declared(a, b, probes(), Some(map(a, b, AxisSign::Positive, 0)))
            .unwrap();
    assert!(matches!(
        run(&policy, a, b).unwrap().state(),
        SpatialPreservationState::Incomparable(IncomparableReason::UnitMismatch)
    ));
    let policy =
        SpatialPreservationPolicy::declared(a, a, probes(), Some(map(a, a, AxisSign::Positive, 0)))
            .unwrap();
    let swapped_scope = SpatialModelInput {
        scope: ContextScope::declared(id(99), id(2), id(3), id(4)),
        ..a
    };
    assert!(matches!(
        run(&policy, a, swapped_scope).unwrap().state(),
        SpatialPreservationState::Incomparable(IncomparableReason::BindingMismatch)
    ));
    let changed_context = SpatialModelInput {
        context: a.context.reconstructed([99; 32]),
        ..a
    };
    assert!(matches!(
        run(&policy, a, changed_context).unwrap().state(),
        SpatialPreservationState::Incomparable(IncomparableReason::BindingMismatch)
    ));
    let changed_geometry = SpatialModelInput {
        left: Some(rect(f, [0, 0], [1, 2])),
        ..a
    };
    assert!(matches!(
        run(&policy, a, changed_geometry).unwrap().state(),
        SpatialPreservationState::Incomparable(IncomparableReason::BindingMismatch)
    ));
}
