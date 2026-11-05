//! P2 TEST-FIRST ONLY: strict cross-row actual-root identity, no production fix.
//! Goal=1, TL=0 and gates are explicitly conditional REPORTED reference inputs.
//! Gains, certificates, programs, current owners and intervals are actual producers.
mod tlcx22_explain_transfer_support;
mod tlcx22_root_join_support;
#[path = "tlcx22_explain_transfer_support/transfer_fixture.rs"]
mod transfer_fixture;
use lc631_analysis::creativity_measurement::{
    ComponentState, IndependenceStatus, InputBasis, SemanticValidation,
};
use lc631_core::measurement_numeric::RationalInterval;
use lc631_runtime::{explanation_measurement as e, joint_selection::*, transfer_measurement as t};
use lc631_substrate::resource::Cancellation;
use tlcx22_explain_transfer_support::*;
use tlcx22_root_join_support::{policy, reported_row, ConditionalReportedReferences};
use transfer_fixture as tf;

#[test]
fn tlcx22_explain_root_join_rejects_different_actual_baselines_despite_equal_pins() {
    explain_join(false);
}
#[test]
fn tlcx22_explain_root_join_rejects_equal_content_distinct_root_without_correspondence() {
    explain_join(true);
}
fn explain_join(equal_content: bool) {
    let f = Fixture::explain();
    let p = f.projection(false, false);
    let good_q = f.projection(true, false);
    let bad_q = f.projection(true, true);
    let cases_a = [f.case(b"p", &f.p, &p), f.case(b"q", &f.q, &good_q)];
    let cases_b = [f.case(b"p", &f.p, &p), f.case(b"q", &f.q, &bad_q)];
    let ri_a = root_input(&cases_a);
    let ri_b = if equal_content {
        ri_a
    } else {
        root_input(&cases_b)
    };
    assert_eq!(ri_a.binding, ri_b.binding);
    assert_eq!(ri_a.limits, ri_b.limits);
    assert_eq!(std::ptr::eq(ri_a.cases, ri_b.cases), equal_content);
    let (_broker, _run, reservation) = run(Cancellation::default());
    let view = reservation.lease();
    let root_a = e::capture_explain(ri_a, &view).unwrap();
    let root_b = e::capture_explain(ri_b, &view).unwrap();
    assert!(!std::ptr::eq(&root_a, &root_b));
    assert!(!std::ptr::eq(
        root_a.baseline_node(0).unwrap(),
        root_b.baseline_node(0).unwrap()
    ));
    let responses = [response(b"p", &p), response(b"q", &good_q)];
    let ci_a = input(1, &responses);
    let ci_b = input(2, &responses);
    let candidate_a = e::admit_explain(&root_a, ci_a, &view).unwrap();
    let candidate_b = e::admit_explain(&root_b, ci_b, &view).unwrap();
    let owner_a = e::measure_explain(&candidate_a, &view).unwrap();
    let owner_b = e::measure_explain(&candidate_b, &view).unwrap();
    let expected_a = e::Expected {
        root: ri_a,
        candidate: ci_a,
    };
    let expected_b = e::Expected {
        root: ri_b,
        candidate: ci_b,
    };
    let a = owner_a
        .report_current(&candidate_a, expected_a, &view)
        .unwrap();
    let b = owner_b
        .report_current(&candidate_b, expected_b, &view)
        .unwrap();
    assert_eq!(
        a.state,
        ComponentState::Measured(RationalInterval::exact_ratio(0, 1).unwrap())
    );
    assert_eq!(
        b.state,
        ComponentState::Measured(
            RationalInterval::exact_ratio(if equal_content { 0 } else { 1 }, 2).unwrap()
        )
    );
    for report in [a, b] {
        assert_eq!(
            (report.basis, report.validation, report.independence),
            (
                InputBasis::Given,
                SemanticValidation::Unassessed,
                IndependenceStatus::SharedDependency
            )
        );
    }
    // These two other axes are GIVEN REPORTED constants, not owner-verified goals/TL.
    let given = ConditionalReportedReferences::given_goal_one_tl_zero();
    let kinds = [AxisKind::Goal(d(8)), AxisKind::ExplainGain];
    let axes_a = [
        AxisValue::Exact(&given.goal),
        owner_a
            .axis_current(&candidate_a, expected_a, ri_a.binding.selection, &view)
            .unwrap()
            .value,
    ];
    let axes_b = [
        AxisValue::Exact(&given.goal),
        owner_b
            .axis_current(&candidate_b, expected_b, ri_b.binding.selection, &view)
            .unwrap()
            .value,
    ];
    let rows = [
        reported_row(
            ci_a.binding,
            ri_a.binding.selection,
            &axes_a,
            &given.total_loss,
            b"explain-root-a",
            owner_a.measurement_binding(),
        ),
        reported_row(
            ci_b.binding,
            ri_b.binding.selection,
            &axes_b,
            &given.total_loss,
            b"explain-root-b",
            owner_b.measurement_binding(),
        ),
    ];
    let policy = policy(ri_a.binding.selection, &kinds);
    let owners = [
        e::ExplainRow {
            owner: &owner_a,
            candidate: &candidate_a,
            expected: expected_a,
            row: 0,
            axis: 1,
        },
        e::ExplainRow {
            owner: &owner_b,
            candidate: &candidate_b,
            expected: expected_b,
            row: 1,
            axis: 1,
        },
    ];
    let result =
        e::select_explain_current(&policy, &rows, &owners, SearchCoverage::EvaluatedSet, &view);
    assert!(matches!(&result, Err(e::Error::BindingMismatch)),
        "strict actual-root join must reject; equal_content={equal_content}; error={:?}; accepted={}",
        result.as_ref().err(), result.is_ok());
}

#[test]
fn tlcx22_transfer_root_join_rejects_compose_vs_extract_actual_baselines() {
    transfer_join(false);
}
#[test]
fn tlcx22_transfer_root_join_rejects_equal_content_distinct_root_without_correspondence() {
    transfer_join(true);
}
fn transfer_join(equal_content: bool) {
    let f = Fixture::transfer();
    let cases = tf::cases(&f);
    let registry = tf::entries(&f);
    let mut ri_a = tf::root(&cases, &registry);
    if !equal_content {
        ri_a.baseline = t::Program::ComposeV1;
    }
    let ri_b = tf::root(&cases, &registry);
    assert_eq!(ri_a.binding, ri_b.binding);
    assert_eq!(ri_a.protocol, ri_b.protocol);
    assert_eq!(ri_a.limits, ri_b.limits);
    assert!(std::ptr::eq(ri_a.cases, ri_b.cases));
    assert!(std::ptr::eq(ri_a.registry, ri_b.registry));
    assert_eq!(ri_a.baseline == ri_b.baseline, equal_content);
    let (_broker, _run, reservation) = run(Cancellation::default());
    let view = reservation.lease();
    let root_a = t::capture_transfer(ri_a, &view).unwrap();
    let root_b = t::capture_transfer(ri_b, &view).unwrap();
    assert!(!std::ptr::eq(&root_a, &root_b));
    assert!(!std::ptr::eq(
        root_a.baseline_node(0).unwrap(),
        root_b.baseline_node(0).unwrap()
    ));
    let ci_a = tf::input(1, t::Program::ComposeV1);
    let ci_b = tf::input(2, t::Program::ComposeV1);
    let candidate_a = t::admit_transfer(&root_a, ci_a, &view).unwrap();
    let candidate_b = t::admit_transfer(&root_b, ci_b, &view).unwrap();
    let owner_a = t::execute_transfer(&candidate_a, &view).unwrap();
    let owner_b = t::execute_transfer(&candidate_b, &view).unwrap();
    let expected_a = t::Expected {
        root: ri_a,
        candidate: ci_a,
    };
    let expected_b = t::Expected {
        root: ri_b,
        candidate: ci_b,
    };
    let a = owner_a
        .report_current(&candidate_a, expected_a, &view)
        .unwrap();
    let b = owner_b
        .report_current(&candidate_b, expected_b, &view)
        .unwrap();
    assert_eq!(
        a.state,
        ComponentState::Measured(
            RationalInterval::exact_ratio(if equal_content { 3 } else { 0 }, 4).unwrap()
        )
    );
    assert_eq!(
        b.state,
        ComponentState::Measured(RationalInterval::exact_ratio(3, 4).unwrap())
    );
    for report in [a, b] {
        assert_eq!(
            (report.basis, report.validation, report.independence),
            (
                InputBasis::Given,
                SemanticValidation::Unassessed,
                IndependenceStatus::SharedDependency
            )
        );
    }
    let given = ConditionalReportedReferences::given_goal_one_tl_zero();
    let kinds = [AxisKind::Goal(d(8)), AxisKind::TransferGain];
    let axes_a = [
        AxisValue::Exact(&given.goal),
        owner_a
            .axis_current(&candidate_a, expected_a, ri_a.binding.selection, &view)
            .unwrap()
            .value,
    ];
    let axes_b = [
        AxisValue::Exact(&given.goal),
        owner_b
            .axis_current(&candidate_b, expected_b, ri_b.binding.selection, &view)
            .unwrap()
            .value,
    ];
    let rows = [
        reported_row(
            ci_a.binding,
            ri_a.binding.selection,
            &axes_a,
            &given.total_loss,
            b"transfer-root-a",
            owner_a.measurement_binding(),
        ),
        reported_row(
            ci_b.binding,
            ri_b.binding.selection,
            &axes_b,
            &given.total_loss,
            b"transfer-root-b",
            owner_b.measurement_binding(),
        ),
    ];
    let policy = policy(ri_a.binding.selection, &kinds);
    let owners = [
        t::TransferRow {
            owner: &owner_a,
            candidate: &candidate_a,
            expected: expected_a,
            row: 0,
            axis: 1,
        },
        t::TransferRow {
            owner: &owner_b,
            candidate: &candidate_b,
            expected: expected_b,
            row: 1,
            axis: 1,
        },
    ];
    let result =
        t::select_transfer_current(&policy, &rows, &owners, SearchCoverage::EvaluatedSet, &view);
    assert!(matches!(&result, Err(t::Error::BindingMismatch)),
        "strict actual-root join must reject; equal_content={equal_content}; error={:?}; accepted={}",
        result.as_ref().err(), result.is_ok());
}
