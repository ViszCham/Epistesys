//! Planned-API tests only. Given goals/authority never become checked by cost.
#[path = "../../lc631-analysis/tests/tlcx22_breakcost_support/mod.rs"]
mod support;
use lc631_analysis::{breakcost_measurement::*, creativity_measurement::*};
use lc631_runtime::{breakcost_selection as bc, joint_selection::*};
use lc631_substrate::resource::*;
use support::*;

macro_rules! fixture {
    ($f:ident, $hard:expr, $cap:expr, $ka:expr, $kb:expr, $body:block) => {
        let source = source();
        let mut raw = probe();
        raw.implementation_complete_candidate = $hard;
        let one = q(1, 1);
        let cases = [case(1, b"legacy-case", &raw, &one)];
        let rules = rules();
        let cancel = Cancellation::default();
        let (_issuer, live) = lease(cancel.clone());
        let view = live.view();
        let root = register(
            &source,
            binding(),
            &cases,
            &rules,
            AdapterKind::LegacyProjectionV1,
            $cap,
            &view,
        )
        .unwrap();
        let base = observe_baseline_current(&root, &view).unwrap();
        let ra = requests($ka);
        let rb = requests($kb);
        let fa = bind_candidate(&root, candidate(1), &ra, &view).unwrap();
        let fb = bind_candidate(&root, candidate(2), &rb, &view).unwrap();
        let oa = observe_breakcost_current(&root, &base, &fa, &view).unwrap();
        let ob = observe_breakcost_current(&root, &base, &fb, &view).unwrap();
        let $f = (
            &source, &cases, &root, &base, &fa, &fb, &oa, &ob, &view, &cancel,
        );
        $body
    };
}
fn pins() -> SelectionPins {
    let b = binding();
    SelectionPins {
        source: b.source,
        scope: b.scope,
        baseline: b.baseline,
        hard_invariants: b.hard_invariants,
        policy: b.policy,
        goal_profile: b.goal_profile,
        evidence_revision: b.evidence_revision,
    }
}
fn policy<'a>(kinds: &'a [AxisKind], p: SelectionPins) -> SelectionPolicy<'a> {
    SelectionPolicy {
        pins: p,
        axes: kinds,
        representative_order: None,
        declared_goal_solve_overlap: false,
        max_candidates: 2,
        max_canonical_bytes: 64,
    }
}
fn row<'a>(
    id: u64,
    p: SelectionPins,
    axes: &'a [AxisValue<'a>],
    z: &'a RationalInterval,
    gate: ReportedGate,
    key: &'a [u8],
    digest: lc631_substrate::Digest32,
) -> JointCandidate<'a> {
    JointCandidate {
        id: candidate(id).id,
        context: binding().context,
        pins: p,
        hard_gate: gate,
        map_back: ReportedGate::Pass,
        axes,
        total_loss: AxisValue::Exact(z),
        canonical_key: key,
        measurement_binding: digest,
    }
}

#[test]
fn s01_real_adapter_choice_flips_current_break_cost_winner_at_equal_given_goal() {
    for swap in [false, true] {
        let ka = if swap {
            AdapterKind::BoundedProjectionV2
        } else {
            AdapterKind::LegacyProjectionV1
        };
        let kb = if swap {
            AdapterKind::LegacyProjectionV1
        } else {
            AdapterKind::BoundedProjectionV2
        };
        fixture!(f, false, budget(), ka, kb, {
            let (source, cases, root, base, fa, fb, oa, ob, view, _) = f;
            let ea = inputs(source, root, cases, base, fa);
            let eb = inputs(source, root, cases, base, fb);
            let p = pins();
            let kinds = [AxisKind::Goal(d(99)), AxisKind::BreakCost];
            let policy = policy(&kinds, p);
            let z = RationalInterval::exact_ratio(0, 1).unwrap();
            let aa = [
                AxisValue::Exact(&z),
                bc::project_current(oa, ea, p, view).unwrap().value,
            ];
            let ab = [
                AxisValue::Exact(&z),
                bc::project_current(ob, eb, p, view).unwrap().value,
            ];
            let rows = [
                row(
                    1,
                    p,
                    &aa,
                    &z,
                    ReportedGate::Pass,
                    b"a",
                    oa.measurement_binding(),
                ),
                row(
                    2,
                    p,
                    &ab,
                    &z,
                    ReportedGate::Pass,
                    b"b",
                    ob.measurement_binding(),
                ),
            ];
            let owners = [
                bc::BreakCostRow {
                    owner: oa,
                    expected: ea,
                    row: 0,
                    axis: 1,
                },
                bc::BreakCostRow {
                    owner: ob,
                    expected: eb,
                    row: 1,
                    axis: 1,
                },
            ];
            let selected =
                bc::select_current(&policy, &rows, &owners, SearchCoverage::EvaluatedSet, view)
                    .unwrap();
            let report = selected.report_current(view).unwrap();
            assert_eq!(report.recommendation, Some(usize::from(swap)));
            assert_eq!(report.optimality, SelectionOptimality::BestAmongEvaluated);
            assert_cost(
                oa.historical_report(),
                if swap { 3 } else { 0 },
                if swap { 4 } else { 1 },
            );
            assert_cost(
                ob.historical_report(),
                if swap { 0 } else { 3 },
                if swap { 1 } else { 4 },
            );
        });
    }
}
#[test]
fn s02_hard_failed_owner_cannot_buy_pass_with_high_goal_or_soft_cost() {
    fixture!(
        f,
        true,
        budget(),
        AdapterKind::DirectLegacyDtoSerializeV1,
        AdapterKind::LegacyProjectionV1,
        {
            let (source, cases, root, base, fa, fb, oa, ob, view, _) = f;
            assert_no_scalar(oa.historical_report());
            assert_eq!(oa.historical_hard_status(), LocalHardStatus::Failed);
            let ea = inputs(source, root, cases, base, fa);
            let eb = inputs(source, root, cases, base, fb);
            let p = pins();
            let kinds = [AxisKind::Goal(d(99)), AxisKind::BreakCost];
            let policy = policy(&kinds, p);
            let z = RationalInterval::exact_ratio(0, 1).unwrap();
            let high = RationalInterval::exact_ratio(1, 1).unwrap();
            let aa = [
                AxisValue::Exact(&high),
                bc::project_current(oa, ea, p, view).unwrap().value,
            ];
            let ab = [
                AxisValue::Exact(&z),
                bc::project_current(ob, eb, p, view).unwrap().value,
            ];
            let owners = [
                bc::BreakCostRow {
                    owner: oa,
                    expected: ea,
                    row: 0,
                    axis: 1,
                },
                bc::BreakCostRow {
                    owner: ob,
                    expected: eb,
                    row: 1,
                    axis: 1,
                },
            ];
            let mut rows = [
                row(
                    1,
                    p,
                    &aa,
                    &z,
                    ReportedGate::Pass,
                    b"a",
                    oa.measurement_binding(),
                ),
                row(
                    2,
                    p,
                    &ab,
                    &z,
                    ReportedGate::Pass,
                    b"b",
                    ob.measurement_binding(),
                ),
            ];
            assert!(matches!(
                bc::select_current(&policy, &rows, &owners, SearchCoverage::EvaluatedSet, view),
                Err(bc::Error::HardGateMismatch)
            ));
            rows[0].hard_gate = ReportedGate::Fail;
            let selected =
                bc::select_current(&policy, &rows, &owners, SearchCoverage::EvaluatedSet, view)
                    .unwrap();
            let report = selected.report_current(view).unwrap();
            assert_eq!(report.recommendation, Some(1));
            assert!(!report.frontier.contains(&0));
        }
    );
}
#[test]
fn s03_external_reported_authority_fail_or_needs_evidence_is_never_overridden() {
    fixture!(
        f,
        false,
        budget(),
        AdapterKind::LegacyProjectionV1,
        AdapterKind::LegacyProjectionV1,
        {
            let (source, cases, root, base, fa, fb, oa, ob, view, _) = f;
            let ea = inputs(source, root, cases, base, fa);
            let eb = inputs(source, root, cases, base, fb);
            let p = pins();
            let kinds = [AxisKind::Goal(d(99)), AxisKind::BreakCost];
            let policy = policy(&kinds, p);
            let z = RationalInterval::exact_ratio(0, 1).unwrap();
            let aa = [
                AxisValue::Exact(&z),
                bc::project_current(oa, ea, p, view).unwrap().value,
            ];
            let ab = [
                AxisValue::Exact(&z),
                bc::project_current(ob, eb, p, view).unwrap().value,
            ];
            let owners = [
                bc::BreakCostRow {
                    owner: oa,
                    expected: ea,
                    row: 0,
                    axis: 1,
                },
                bc::BreakCostRow {
                    owner: ob,
                    expected: eb,
                    row: 1,
                    axis: 1,
                },
            ];
            for external in [ReportedGate::Fail, ReportedGate::NeedsEvidence] {
                let rows = [
                    row(1, p, &aa, &z, external, b"a", oa.measurement_binding()),
                    row(2, p, &ab, &z, external, b"b", ob.measurement_binding()),
                ];
                let selected =
                    bc::select_current(&policy, &rows, &owners, SearchCoverage::EvaluatedSet, view)
                        .unwrap();
                let report = selected.report_current(view).unwrap();
                assert_eq!(report.recommendation, None);
                assert!(report.candidates.iter().all(|r| r.hard_gate == external));
                assert_eq!(
                    report.recommendation_state,
                    RecommendationState::NoEligibleCandidate
                );
                assert!(report.dispositions.iter().all(|d| *d
                    == if external == ReportedGate::Fail {
                        Disposition::HardFailure
                    } else {
                        Disposition::NeedsEvidence
                    }));
            }
        }
    );
}
#[test]
fn s04_missing_copied_partial_foreign_and_cancelled_owners_cannot_return_cached_winner() {
    fixture!(
        f,
        false,
        budget(),
        AdapterKind::LegacyProjectionV1,
        AdapterKind::BoundedProjectionV2,
        {
            let (source, cases, root, base, fa, fb, oa, ob, view, cancel) = f;
            let ea = inputs(source, root, cases, base, fa);
            let eb = inputs(source, root, cases, base, fb);
            let p = pins();
            let kinds = [AxisKind::Goal(d(99)), AxisKind::BreakCost];
            let policy = policy(&kinds, p);
            let z = RationalInterval::exact_ratio(0, 1).unwrap();
            let aa = [
                AxisValue::Exact(&z),
                bc::project_current(oa, ea, p, view).unwrap().value,
            ];
            let ab = [
                AxisValue::Exact(&z),
                bc::project_current(ob, eb, p, view).unwrap().value,
            ];
            let rows = [
                row(
                    1,
                    p,
                    &aa,
                    &z,
                    ReportedGate::Pass,
                    b"a",
                    oa.measurement_binding(),
                ),
                row(
                    2,
                    p,
                    &ab,
                    &z,
                    ReportedGate::Pass,
                    b"b",
                    ob.measurement_binding(),
                ),
            ];
            let owners = [
                bc::BreakCostRow {
                    owner: oa,
                    expected: ea,
                    row: 0,
                    axis: 1,
                },
                bc::BreakCostRow {
                    owner: ob,
                    expected: eb,
                    row: 1,
                    axis: 1,
                },
            ];
            assert!(matches!(
                bc::select_current(&policy, &rows, &[], SearchCoverage::EvaluatedSet, view),
                Err(bc::Error::InvalidRows)
            ));
            let copy = oa.historical_report().clone();
            let detached = [
                AxisValue::Exact(&z),
                project_component_reported(&copy).unwrap().value,
            ];
            let copied_rows = [
                row(
                    1,
                    p,
                    &detached,
                    &z,
                    ReportedGate::Pass,
                    b"a",
                    oa.measurement_binding(),
                ),
                row(
                    2,
                    p,
                    &ab,
                    &z,
                    ReportedGate::Pass,
                    b"b",
                    ob.measurement_binding(),
                ),
            ];
            assert!(matches!(
                bc::select_current(
                    &policy,
                    &copied_rows,
                    &owners,
                    SearchCoverage::EvaluatedSet,
                    view
                ),
                Err(bc::Error::BindingMismatch)
            ));
            let duplicate = [
                bc::BreakCostRow {
                    owner: oa,
                    expected: ea,
                    row: 0,
                    axis: 1,
                },
                bc::BreakCostRow {
                    owner: ob,
                    expected: eb,
                    row: 0,
                    axis: 1,
                },
            ];
            assert!(matches!(
                bc::select_current(
                    &policy,
                    &rows,
                    &duplicate,
                    SearchCoverage::EvaluatedSet,
                    view
                ),
                Err(bc::Error::InvalidRows)
            ));
            let selected =
                bc::select_current(&policy, &rows, &owners, SearchCoverage::EvaluatedSet, view)
                    .unwrap();
            assert_eq!(
                selected.report_current(view).unwrap().recommendation,
                Some(0)
            );
            let (_i, foreign) = lease(Cancellation::default());
            let foreign = foreign.view();
            assert_eq!(view.run_id(), foreign.run_id());
            assert!(matches!(
                selected.report_current(&foreign),
                Err(bc::Error::ForeignReservation)
            ));
            cancel.cancel();
            assert!(matches!(
                selected.report_current(view),
                Err(bc::Error::Resource(ResourceError::Cancelled))
            ));
            assert_eq!(selected.historical_report().recommendation, Some(0));
        }
    );
    let mut cap = budget();
    cap.max_soft_checks_per_run = 1;
    fixture!(
        f,
        false,
        cap,
        AdapterKind::LegacyProjectionV1,
        AdapterKind::BoundedProjectionV2,
        {
            let (source, cases, root, base, fa, fb, oa, ob, view, _) = f;
            let ea = inputs(source, root, cases, base, fa);
            let eb = inputs(source, root, cases, base, fb);
            let p = pins();
            let kinds = [AxisKind::Goal(d(99)), AxisKind::BreakCost];
            let policy = policy(&kinds, p);
            let z = RationalInterval::exact_ratio(0, 1).unwrap();
            let aa = [
                AxisValue::Exact(&z),
                bc::project_current(oa, ea, p, view).unwrap().value,
            ];
            let ab = [
                AxisValue::Exact(&z),
                bc::project_current(ob, eb, p, view).unwrap().value,
            ];
            let rows = [
                row(
                    1,
                    p,
                    &aa,
                    &z,
                    ReportedGate::Pass,
                    b"a",
                    oa.measurement_binding(),
                ),
                row(
                    2,
                    p,
                    &ab,
                    &z,
                    ReportedGate::Pass,
                    b"b",
                    ob.measurement_binding(),
                ),
            ];
            let owners = [
                bc::BreakCostRow {
                    owner: oa,
                    expected: ea,
                    row: 0,
                    axis: 1,
                },
                bc::BreakCostRow {
                    owner: ob,
                    expected: eb,
                    row: 1,
                    axis: 1,
                },
            ];
            assert!(matches!(
                bc::select_current(&policy, &rows, &owners, SearchCoverage::EvaluatedSet, view),
                Err(bc::Error::IncompleteObservation)
            ));
        }
    );
}
#[test]
fn s05_missing_goal_and_strict_original_leaves_are_not_filled_by_local_break_cost() {
    fixture!(
        f,
        false,
        budget(),
        AdapterKind::LegacyProjectionV1,
        AdapterKind::BoundedProjectionV2,
        {
            let (source, cases, root, base, fa, fb, oa, ob, view, _) = f;
            let ea = inputs(source, root, cases, base, fa);
            let eb = inputs(source, root, cases, base, fb);
            // Missing goal observation, with the original goal-profile pins unchanged.
            let p = pins();
            let kinds = [AxisKind::BreakCost];
            let policy = policy(&kinds, p);
            let z = RationalInterval::exact_ratio(0, 1).unwrap();
            let aa = [bc::project_current(oa, ea, p, view).unwrap().value];
            let ab = [bc::project_current(ob, eb, p, view).unwrap().value];
            let rows = [
                row(
                    1,
                    p,
                    &aa,
                    &z,
                    ReportedGate::Pass,
                    b"a",
                    oa.measurement_binding(),
                ),
                row(
                    2,
                    p,
                    &ab,
                    &z,
                    ReportedGate::Pass,
                    b"b",
                    ob.measurement_binding(),
                ),
            ];
            let owners = [
                bc::BreakCostRow {
                    owner: oa,
                    expected: ea,
                    row: 0,
                    axis: 0,
                },
                bc::BreakCostRow {
                    owner: ob,
                    expected: eb,
                    row: 1,
                    axis: 0,
                },
            ];
            let selected =
                bc::select_current(&policy, &rows, &owners, SearchCoverage::EvaluatedSet, view)
                    .unwrap();
            let current = selected.report_current(view).unwrap();
            assert_eq!(current.recommendation, None);
            assert_eq!(
                current.recommendation_state,
                RecommendationState::NoGoalObservation
            );
            // Actual strict reported assessment rejects absent original leaves.
            // No formula-wide actual owner or TL/Preserve/Rebind/Reframe is fabricated.
            let report = oa.report_current(ea, view).unwrap();
            let reports = [report.clone()];
            for required in [
                &[ComponentMetric::BreakCost, ComponentMetric::Preserve][..],
                &[
                    ComponentMetric::BreakCost,
                    ComponentMetric::Rebind,
                    ComponentMetric::Reframe,
                ][..],
                &[
                    ComponentMetric::BreakCost,
                    ComponentMetric::ContextDistance,
                    ComponentMetric::Compatibility,
                ][..],
            ] {
                let strict = AssessmentPolicy {
                    scope: report.binding.scope,
                    baseline: report.binding.baseline,
                    required,
                };
                assert!(matches!(
                    CreativeAssessment::from_reports(&strict, &reports),
                    Err(MeasurementError::MissingMetric)
                ));
            }
        }
    );
    // A second root captures absence of the profile before observing a candidate.
    let source = source();
    let raw = probe();
    let one = q(1, 1);
    let cases = [case(1, b"legacy-case", &raw, &one)];
    let rules = rules();
    let (_issuer, live) = lease(Cancellation::default());
    let view = live.view();
    let mut no_goal = binding();
    no_goal.goal_profile = None;
    let root = register(
        &source,
        no_goal,
        &cases,
        &rules,
        AdapterKind::LegacyProjectionV1,
        budget(),
        &view,
    )
    .unwrap();
    let base = observe_baseline_current(&root, &view).unwrap();
    let req = requests(AdapterKind::LegacyProjectionV1);
    let frozen = bind_candidate(&root, candidate(1), &req, &view).unwrap();
    let owner = observe_breakcost_current(&root, &base, &frozen, &view).unwrap();
    let expected = inputs(&source, &root, &cases, &base, &frozen);
    let mut p = pins();
    p.goal_profile = None;
    let kinds = [AxisKind::BreakCost];
    let policy = policy(&kinds, p);
    let z = RationalInterval::exact_ratio(0, 1).unwrap();
    let axes = [bc::project_current(&owner, expected, p, &view)
        .unwrap()
        .value];
    let rows = [row(
        1,
        p,
        &axes,
        &z,
        ReportedGate::Pass,
        b"a",
        owner.measurement_binding(),
    )];
    let owners = [bc::BreakCostRow {
        owner: &owner,
        expected,
        row: 0,
        axis: 0,
    }];
    let selected =
        bc::select_current(&policy, &rows, &owners, SearchCoverage::EvaluatedSet, &view).unwrap();
    assert_eq!(selected.report_current(&view).unwrap().recommendation, None);
    assert_eq!(
        selected.report_current(&view).unwrap().recommendation_state,
        RecommendationState::NoGoalProfile
    );
}
