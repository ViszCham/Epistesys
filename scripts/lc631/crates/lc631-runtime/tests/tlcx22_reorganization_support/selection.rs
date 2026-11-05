//! The future adapter must consume exact owner references around real comparison.
use super::support::*;
use lc631_runtime::joint_selection::{project_component_reported, ReportedComponentAxis};

#[test]
fn tlcx22_reorganization_equal_scalar_foreign_owner_rejected() {
    normal(|fx, view, _| {
        let b = fx.bindings([1, 1], false);
        let c = fx.bindings([1, 2], false);
        let uses = fx.root_uses();
        let baseline = fx.model(&b, &uses);
        let candidate = fx.model(&c, &uses);
        let probes = fx.probes([1, 3]);
        let maps = fx.maps([1, 2]);
        let plan =
            r::RebindPlan::capture_fixed(&baseline, &probes, fx.pins(), limits(), view).unwrap();
        let foreign_plan =
            r::RebindPlan::capture_fixed(&baseline, &probes, fx.pins(), limits(), view).unwrap();
        let p = r::PreparedRebindProgram::prepare(&plan, &candidate, &maps, CandidateId(2), view)
            .unwrap();
        let other_p =
            r::PreparedRebindProgram::prepare(&plan, &candidate, &maps, CandidateId(3), view)
                .unwrap();
        let q = r::RebindQueries::capture(&p, view).unwrap();
        let other_q = r::RebindQueries::capture(&other_p, view).unwrap();
        let owner = r::OwnedRebind::evaluate(&q, view).unwrap();
        let other_owner = r::OwnedRebind::evaluate(&other_q, view).unwrap();
        owner
            .with_current(view, |current| {
                let axis = project_component_reported(current.report()).unwrap();
                current.bind_axis(&plan, &p, &q, &axis).unwrap();
                let equal = ratio(3, 4);
                let detached = ReportedComponentAxis {
                    kind: AxisKind::Rebind,
                    value: AxisValue::Exact(&equal),
                    report: current.report(),
                };
                assert!(matches!(
                    current.bind_axis(&plan, &p, &q, &detached),
                    Err(r::Error::ForeignInterval)
                ));
                let copied = current.report().clone();
                let copied_axis = project_component_reported(&copied).unwrap();
                assert!(matches!(
                    current.bind_axis(&plan, &p, &q, &copied_axis),
                    Err(r::Error::ForeignReport)
                ));
                assert!(matches!(
                    current.bind_axis(&foreign_plan, &p, &q, &axis),
                    Err(r::Error::ForeignPlan)
                ));
                assert!(matches!(
                    current.bind_axis(&plan, &other_p, &q, &axis),
                    Err(r::Error::ForeignProgram)
                ));
                assert!(matches!(
                    current.bind_axis(&plan, &p, &other_q, &axis),
                    Err(r::Error::ForeignQueries)
                ));
                other_owner
                    .with_current(view, |other| {
                        let foreign_axis = project_component_reported(other.report()).unwrap();
                        assert!(matches!(
                            current.bind_axis(&plan, &p, &q, &foreign_axis),
                            Err(r::Error::ForeignReport)
                        ));
                    })
                    .unwrap();
                let relabelled = ReportedComponentAxis {
                    kind: AxisKind::Reframe,
                    value: AxisValue::Exact(axis.value.interval().unwrap()),
                    report: current.report(),
                };
                assert!(matches!(
                    current.bind_axis(&plan, &p, &q, &relabelled),
                    Err(r::Error::MetricMismatch)
                ));
            })
            .unwrap();
        // Reweighting or changing scope is a NEW plan, never this owner's plan.
        for pins in [
            SelectionPins {
                scope: ContextScope::declared(d(90), d(2), d(3), d(4)),
                ..fx.pins()
            },
            SelectionPins {
                baseline: d(91),
                ..fx.pins()
            },
            SelectionPins {
                policy: d(92),
                ..fx.pins()
            },
            SelectionPins {
                hard_invariants: d(93),
                ..fx.pins()
            },
        ] {
            let weights = fx.probes([3, 1]);
            let altered =
                r::RebindPlan::capture_fixed(&baseline, &weights, pins, limits(), view).unwrap();
            owner
                .with_current(view, |current| {
                    let axis = project_component_reported(current.report()).unwrap();
                    assert!(matches!(
                        current.bind_axis(&altered, &p, &q, &axis),
                        Err(r::Error::ForeignPlan)
                    ));
                })
                .unwrap();
        }
    });
}

#[test]
fn tlcx22_reorganization_current_consumer_cut() {
    normal(|fx, view, cancel| {
        let b = fx.bindings([1, 1], false);
        let c = fx.bindings([1, 2], false);
        let uses = fx.root_uses();
        let baseline = fx.model(&b, &uses);
        let candidate = fx.model(&c, &uses);
        let probes = fx.probes([1, 3]);
        let im = fx.maps([1, 1]);
        let nm = fx.maps([1, 2]);
        let plan =
            r::RebindPlan::capture_fixed(&baseline, &probes, fx.pins(), limits(), view).unwrap();
        let p0 =
            r::PreparedRebindProgram::prepare(&plan, &baseline, &im, CandidateId(1), view).unwrap();
        let p1 = r::PreparedRebindProgram::prepare(&plan, &candidate, &nm, CandidateId(2), view)
            .unwrap();
        let q0 = r::RebindQueries::capture(&p0, view).unwrap();
        let q1 = r::RebindQueries::capture(&p1, view).unwrap();
        let a = r::OwnedRebind::evaluate(&q0, view).unwrap();
        let b = r::OwnedRebind::evaluate(&q1, view).unwrap();
        let equal_program =
            r::PreparedRebindProgram::prepare(&plan, &baseline, &im, CandidateId(3), view).unwrap();
        let equal_queries = r::RebindQueries::capture(&equal_program, view).unwrap();
        let equal_owner = r::OwnedRebind::evaluate(&equal_queries, view).unwrap();
        a.with_current(view, |ca| {
            equal_owner
                .with_current(view, |ce| {
                    let ax = project_component_reported(ca.report()).unwrap();
                    let ex = project_component_reported(ce.report()).unwrap();
                    let ar = ca.bind_axis(&plan, &p0, &q0, &ax).unwrap();
                    let er = ce
                        .bind_axis(&plan, &equal_program, &equal_queries, &ex)
                        .unwrap();
                    assert_eq!(
                        r::compare_current(&ar, &er, view).unwrap(),
                        PairRelation::EquivalentInScope
                    );
                })
                .unwrap()
        })
        .unwrap();
        a.with_current(view, |ca| {
            b.with_current(view, |cb| {
                let ax = project_component_reported(ca.report()).unwrap();
                let bx = project_component_reported(cb.report()).unwrap();
                let ar = ca.bind_axis(&plan, &p0, &q0, &ax).unwrap();
                let br = cb.bind_axis(&plan, &p1, &q1, &bx).unwrap();
                assert_eq!(
                    r::compare_current(&ar, &br, view).unwrap(),
                    PairRelation::RightDominates
                );
                assert_eq!(
                    r::compare_current(&br, &ar, view).unwrap(),
                    PairRelation::LeftDominates
                );
                let decision = r::select_current(&[ar, br], view).unwrap();
                assert_eq!(decision.frontier(), &[CandidateId(2)]);
                assert_eq!(decision.recommendation(), None);
                assert_eq!(
                    decision.recommendation_state(),
                    RecommendationState::NoGoalProfile
                );
            })
            .unwrap()
        })
        .unwrap();
        // Cut the actual resource, after observing a nonempty consumed comparison.
        cancel.cancel();
        assert!(matches!(
            a.with_current(view, |_| ()),
            Err(r::Error::Resource(ResourceError::Cancelled))
        ));
        assert!(matches!(
            b.with_current(view, |_| ()),
            Err(r::Error::Resource(ResourceError::Cancelled))
        ));
    });
}

#[test]
fn tlcx22_reorganization_unknown_does_not_reuse_cached_winner() {
    normal(|fx, view, _| {
        let b = fx.bindings([1, 1], false);
        let c = fx.bindings([1, 2], false);
        let uses = fx.root_uses();
        let baseline = fx.model(&b, &uses);
        let candidate = fx.model(&c, &uses);
        let probes = fx.probes([1, 3]);
        let im = fx.maps([1, 1]);
        let nm = fx.maps([1, 2]);
        let mut cap = limits();
        cap.max_check_work = 1;
        let plan = r::RebindPlan::capture_fixed(&baseline, &probes, fx.pins(), cap, view).unwrap();
        let p0 =
            r::PreparedRebindProgram::prepare(&plan, &baseline, &im, CandidateId(1), view).unwrap();
        let p1 = r::PreparedRebindProgram::prepare(&plan, &candidate, &nm, CandidateId(2), view)
            .unwrap();
        let q0 = r::RebindQueries::capture(&p0, view).unwrap();
        let q1 = r::RebindQueries::capture(&p1, view).unwrap();
        let a = r::OwnedRebind::evaluate(&q0, view).unwrap();
        let b = r::OwnedRebind::evaluate(&q1, view).unwrap();
        a.with_current(view, |ca| {
            b.with_current(view, |cb| {
                assert!(ca.has_gaps() && cb.has_gaps());
                let ax = project_component_reported(ca.report()).unwrap();
                let bx = project_component_reported(cb.report()).unwrap();
                let ar = ca.bind_axis(&plan, &p0, &q0, &ax).unwrap();
                let br = cb.bind_axis(&plan, &p1, &q1, &bx).unwrap();
                assert_eq!(
                    r::compare_current(&ar, &br, view).unwrap(),
                    PairRelation::Unresolved
                );
                let result = r::select_current(&[ar, br], view).unwrap();
                assert_eq!(result.recommendation(), None);
                assert_ne!(
                    result.recommendation_state(),
                    RecommendationState::OperationalRepresentative
                );
            })
            .unwrap()
        })
        .unwrap();
    });
}

#[test]
fn tlcx22_reorganization_goal_profile_id_is_not_goal_observation() {
    normal(|fx, view, _| {
        let b = fx.bindings([1, 1], false);
        let uses = fx.root_uses();
        let baseline = fx.model(&b, &uses);
        let probes = fx.probes([1, 3]);
        let maps = fx.maps([1, 1]);
        let pins = SelectionPins {
            goal_profile: Some(d(88)),
            ..fx.pins()
        };
        let plan = r::RebindPlan::capture_fixed(&baseline, &probes, pins, limits(), view).unwrap();
        let p = r::PreparedRebindProgram::prepare(&plan, &baseline, &maps, CandidateId(1), view)
            .unwrap();
        let q = r::RebindQueries::capture(&p, view).unwrap();
        let owner = r::OwnedRebind::evaluate(&q, view).unwrap();
        owner
            .with_current(view, |current| {
                let axis = project_component_reported(current.report()).unwrap();
                let row = current.bind_axis(&plan, &p, &q, &axis).unwrap();
                let decision = r::select_current(&[row], view).unwrap();
                assert_eq!(decision.frontier(), &[CandidateId(1)]);
                assert_eq!(decision.recommendation(), None);
                assert_eq!(
                    decision.recommendation_state(),
                    RecommendationState::NoGoalObservation
                );
            })
            .unwrap();
    });
}

#[test]
fn tlcx22_reorganization_legacy_formula_compatibility() {
    use lc631_analysis::creativity_measurement::{
        AssessmentPolicy, CandidateSubject, ComponentAssessment, MeasurementError,
    };
    normal(|fx, view, _| {
        let b = fx.bindings([1, 1], false);
        let uses = fx.root_uses();
        let baseline = fx.model(&b, &uses);
        let probes = fx.probes([1, 3]);
        let maps = fx.maps([1, 1]);
        let plan =
            r::RebindPlan::capture_fixed(&baseline, &probes, fx.pins(), limits(), view).unwrap();
        let p = r::PreparedRebindProgram::prepare(&plan, &baseline, &maps, CandidateId(1), view)
            .unwrap();
        let q = r::RebindQueries::capture(&p, view).unwrap();
        let owner = r::OwnedRebind::evaluate(&q, view).unwrap();
        owner
            .with_current(view, |current| {
                let report = current.report();
                assert_point(report, 0, 1);
                assert_eq!(ComponentMetric::Rebind as u16, 0x0119);
                assert_eq!(ComponentMetric::Reframe as u16, 0x011a);
                assert_eq!(
                    project_component_reported(report).unwrap().kind,
                    AxisKind::Rebind
                );
                let refs = [report];
                let required = [
                    ComponentMetric::Preserve,
                    ComponentMetric::Rebind,
                    ComponentMetric::Reframe,
                    ComponentMetric::BreakCost,
                ];
                let policy = AssessmentPolicy {
                    scope: report.binding.scope,
                    baseline: report.binding.baseline,
                    required: &required,
                };
                assert!(matches!(
                    ComponentAssessment::from_reports(
                        &policy,
                        CandidateSubject::from_binding(report.binding),
                        &refs
                    ),
                    Err(MeasurementError::MissingMetric)
                ));
                // No absent Preserve/Reframe/Break/TL/goal is manufactured to close F2.
                // Complete 27-coefficient/F1/F2 legacy tests remain a central regression gate.
            })
            .unwrap();
    });
}
