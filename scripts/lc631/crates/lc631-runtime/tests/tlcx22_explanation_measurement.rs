//! TEST-FIRST/API freeze. Expected unresolved import until separate export/body GO.
//! No local implementation, passing shim, ignored test or generated gold.
#[path = "tlcx22_explain_transfer_support/explanation_admission.rs"]
mod explanation_admission;
#[path = "tlcx22_explain_transfer_support/explanation_selection.rs"]
mod explanation_selection;
mod tlcx22_explain_transfer_support;
use lc631_analysis::creativity_measurement::{
    ComponentState, IndependenceStatus, InputBasis, SemanticValidation,
};
use lc631_core::{context::SourceRevision, measurement_numeric::RationalInterval};
use lc631_runtime::{explanation_measurement as e, joint_selection::*};
use lc631_substrate::resource::{Cancellation, Deadline, ResourceError};
use std::time::Duration;
use tlcx22_explain_transfer_support::{oracle as o, *};

#[test]
fn tlcx22_explain_negative_right_conclusion_wrong_derivation_is_completed_invalid_zero() {
    assert!(o::literal_entails(o::E_GAMMA, o::E_Q));
    assert!(!o::check(&[o::pq()], &o::q(), &o::e_trace(true, true)).unwrap());
    let f = Fixture::explain();
    let bad = f.projection(true, true);
    let cases = [f.case(b"q", &f.q, &bad)];
    let ri = root_input(&cases);
    let (_broker, _run, res) = run(Cancellation::default());
    let view = res.lease();
    let root = e::capture_explain(ri, &view).unwrap();
    assert_eq!(
        root.baseline_state(0),
        Some(e::CaseState::CompletedInvalid(
            e::InvalidDerivation::RuleMismatch
        ))
    );
    assert_eq!(
        root.baseline_score(0).unwrap(),
        &RationalInterval::exact_ratio(0, 1).unwrap()
    );
}

#[test]
fn tlcx22_explain_negative_every_step_and_final_goal_are_checked() {
    let f = Fixture::explain();
    let base = f.projection(false, false);
    let cases = [f.case(b"p", &f.p, &base)];
    let ri = root_input(&cases);
    let (_broker, _run, res) = run(Cancellation::default());
    let view = res.lease();
    let root = e::capture_explain(ri, &view).unwrap();
    // Last goal p is valid but step1's unused q claim chooses child0=p.
    let unused_wrong = vec![
        base[0],
        e::Step {
            conclusion: &f.q,
            rule: e::Rule::AndElim {
                prior: e::StepId(0),
                child: 0,
            },
        },
        base[1],
    ];
    for steps in [&unused_wrong[..], &f.projection(true, false)[..], &[][..]] {
        let responses = [response(b"p", steps)];
        let ci = input(1, &responses);
        let candidate = e::admit_explain(&root, ci, &view).unwrap();
        let gain = e::measure_explain(&candidate, &view).unwrap();
        assert!(matches!(
            gain.candidate_state(0),
            Some(e::CaseState::CompletedInvalid(_))
        ));
        assert_eq!(
            gain.report_current(
                &candidate,
                e::Expected {
                    root: ri,
                    candidate: ci
                },
                &view
            )
            .unwrap()
            .state,
            ComponentState::Measured(RationalInterval::exact_ratio(-1, 1).unwrap())
        );
    }
}

#[test]
fn tlcx22_explain_negative_forward_cycle_shape_and_foreign_atoms_are_invalid_input() {
    let f = Fixture::explain();
    let base = f.projection(false, false);
    let cases = [f.case(b"p", &f.p, &base)];
    let ri = root_input(&cases);
    let (_broker, _run, res) = run(Cancellation::default());
    let view = res.lease();
    let root = e::capture_explain(ri, &view).unwrap();
    let foreign = atom(99);
    let bad_cases = [
        vec![
            e::Step {
                conclusion: &f.p,
                rule: e::Rule::AndElim {
                    prior: e::StepId(1),
                    child: 0,
                },
            },
            base[0],
        ],
        vec![e::Step {
            conclusion: &f.p,
            rule: e::Rule::AndElim {
                prior: e::StepId(0),
                child: 0,
            },
        }],
        vec![
            base[0],
            e::Step {
                conclusion: &f.p,
                rule: e::Rule::AndElim {
                    prior: e::StepId(0),
                    child: 9,
                },
            },
        ],
        vec![e::Step {
            conclusion: &foreign,
            rule: e::Rule::Premise { index: 0 },
        }],
        vec![base[0]; 65],
    ];
    for steps in &bad_cases {
        let responses = [response(b"p", steps)];
        assert!(matches!(
            e::admit_explain(&root, input(1, &responses), &view),
            Err(e::Error::InvalidInput(_))
        ));
    }
    let inconsistent = theory(
        vec![
            atom(0),
            lc631_solvers::typed_query::BoolExpr::Not(Box::new(atom(0))),
        ],
        1,
    );
    let impossible = [e::ExplainCase {
        key: b"p",
        theory: &inconsistent,
        ..cases[0]
    }];
    let actual = e::capture_explain(root_input(&impossible), &view);
    // This inherited certificate still mentions q, which the new signature
    // does not declare. Keep that rejection separate from consistency.
    assert!(
        matches!(
            &actual,
            Err(e::Error::InvalidInput(e::InvalidInput::UndeclaredAtom))
        ),
        "error={:?}; root_created={}",
        actual.as_ref().err(),
        actual.is_ok()
    );
    let direct_p = [e::Step {
        conclusion: &f.p,
        rule: e::Rule::Premise { index: 0 },
    }];
    let contradiction_only = [e::ExplainCase {
        key: b"contradiction-only",
        theory: &inconsistent,
        goal: &f.p,
        weight: &f.one,
        baseline: e::Submission::Certificate(e::Certificate { steps: &direct_p }),
    }];
    let actual = e::capture_explain(root_input(&contradiction_only), &view);
    assert!(
        matches!(
            &actual,
            Err(e::Error::NoComparable(e::NoComparable::ContextInconsistent))
        ),
        "error={:?}; root_created={}",
        actual.as_ref().err(),
        actual.is_ok()
    );
}

#[test]
fn tlcx22_explain_positive_literal_half_negative_and_identity_gain_without_prose_score() {
    assert!(o::check(&[o::pq()], &o::p(), &o::e_trace(false, false)).unwrap());
    assert!(o::check(&[o::pq()], &o::q(), &o::e_trace(true, false)).unwrap());
    let f = Fixture::explain();
    let p = f.projection(false, false);
    let q_bad = f.projection(true, true);
    let q_good = f.projection(true, false);
    let cases = [f.case(b"p", &f.p, &p), f.case(b"q", &f.q, &q_bad)];
    let ri = root_input(&cases);
    let (_broker, _run, res) = run(Cancellation::default());
    let view = res.lease();
    let root = e::capture_explain(ri, &view).unwrap();
    for (n, steps, prose, expected) in [
        (1, &q_good, b"x".as_slice(), 1),
        (2, &q_good, &[b'x'; 4096][..], 1),
        (3, &q_bad, b"".as_slice(), 0),
    ] {
        let responses = [
            response(b"p", &p),
            e::ExplainResponse {
                prose,
                ..response(b"q", steps)
            },
        ];
        let ci = input(n, &responses);
        let candidate = e::admit_explain(&root, ci, &view).unwrap();
        let gain = e::measure_explain(&candidate, &view).unwrap();
        let report = gain
            .report_current(
                &candidate,
                e::Expected {
                    root: ri,
                    candidate: ci,
                },
                &view,
            )
            .unwrap();
        assert_eq!(
            report.state,
            ComponentState::Measured(RationalInterval::exact_ratio(expected, 2).unwrap())
        );
        assert_eq!(
            (report.basis, report.validation, report.independence),
            (
                InputBasis::Given,
                SemanticValidation::Unassessed,
                IndependenceStatus::SharedDependency
            )
        );
        assert!(!std::ptr::eq(
            root.baseline_node(0).unwrap(),
            gain.candidate_node(0).unwrap()
        ));
    }
    // Independent reversed pairing: both baselines pass, q candidate's proof fails.
    let reverse_cases = [f.case(b"p", &f.p, &p), f.case(b"q", &f.q, &q_good)];
    let mut reversed = root_input(&reverse_cases);
    reversed.binding.selection.baseline = d(190);
    reversed.binding.suite = d(191);
    reversed.binding.selection.scope =
        lc631_core::context::ContextScope::declared(d(192), d(2), d(3), d(4));
    let reverse_root = e::capture_explain(reversed, &view).unwrap();
    let responses = [response(b"p", &p), response(b"q", &q_bad)];
    let ci = input(4, &responses);
    let candidate = e::admit_explain(&reverse_root, ci, &view).unwrap();
    let gain = e::measure_explain(&candidate, &view).unwrap();
    assert_eq!(
        gain.historical_report().state,
        ComponentState::Measured(RationalInterval::exact_ratio(-1, 2).unwrap())
    );
}

#[test]
fn tlcx22_explain_missing_and_unfinished_keep_denominator_and_distinct_nodes() {
    let f = Fixture::explain();
    let p = f.projection(false, false);
    let q_bad = f.projection(true, true);
    let cases = [f.case(b"p", &f.p, &p), f.case(b"q", &f.q, &q_bad)];
    let mut ri = root_input(&cases);
    let (_broker, _run, res) = run(Cancellation::default());
    let view = res.lease();
    let root = e::capture_explain(ri, &view).unwrap();
    let responses = [response(b"p", &p)]; // q is absent, never dropped/reweighted.
    let ci = input(1, &responses);
    let candidate = e::admit_explain(&root, ci, &view).unwrap();
    let gain = e::measure_explain(&candidate, &view).unwrap();
    assert_eq!(gain.fixed_weight(), &q(2, 1));
    assert_eq!(
        gain.candidate_state(1),
        Some(e::CaseState::Unknown(e::Gap::MissingSubmission))
    );
    assert_eq!(
        gain.historical_report().state,
        ComponentState::Measured(RationalInterval::from_ratio(0, 1, 1, 2).unwrap())
    );
    ri.limits.max_case_work = 0;
    let unfinished = e::capture_explain(ri, &view).unwrap();
    let pending = e::admit_explain(&unfinished, ci, &view).unwrap();
    let unknown = e::measure_explain(&pending, &view).unwrap();
    assert_eq!(
        unknown.candidate_state(0),
        Some(e::CaseState::Unknown(e::Gap::WorkLimit))
    );
    assert_eq!(
        unknown.historical_report().state,
        ComponentState::Measured(RationalInterval::from_ratio(-1, 1, 1, 1).unwrap())
    );
    assert!(!std::ptr::eq(
        unfinished.baseline_node(0).unwrap(),
        unknown.candidate_node(0).unwrap()
    ));
}

#[test]
fn tlcx22_explain_current_expected_inputs_source_claim_and_nonrefund_work() {
    let f = Fixture::explain();
    let p = f.projection(false, false);
    let cases = [f.case(b"p", &f.p, &p)];
    let mut ri = root_input(&cases);
    ri.limits.max_run_work = 2_000_000;
    ri.limits.max_dispatches = 2;
    let responses = [response(b"p", &p)];
    let ci = input(1, &responses);
    let cancel = Cancellation::default();
    let (_broker, _run, res) = run(cancel.clone());
    let view = res.lease();
    let root = e::capture_explain(ri, &view).unwrap();
    let candidate = e::admit_explain(&root, ci, &view).unwrap();
    let expected = e::Expected {
        root: ri,
        candidate: ci,
    };
    let gain = e::measure_explain(&candidate, &view).unwrap();
    assert!(gain.report_current(&candidate, expected, &view).is_ok());
    for cut in 0..4 {
        let mut wrong = expected;
        let different_goal = [e::ExplainCase {
            goal: &f.q,
            ..cases[0]
        }];
        let different_gamma = theory(vec![f.q.clone()], 3);
        let different_theory = [e::ExplainCase {
            theory: &different_gamma,
            ..cases[0]
        }];
        match cut {
            0 => wrong.root.binding.selection.source = SourceRevision::of("foreign source"),
            1 => wrong.root.cases = &different_goal,
            2 => wrong.root.cases = &different_theory,
            3 => wrong.candidate.binding.target = d(201),
            _ => unreachable!(),
        }
        assert!(matches!(
            gain.report_current(&candidate, wrong, &view),
            Err(e::Error::ExpectedInputMismatch)
        ));
    }
    let (_other, _other_run, other_res) = run(Cancellation::default());
    let other_view = other_res.lease();
    assert!(matches!(
        gain.report_current(&candidate, expected, &other_view),
        Err(e::Error::ForeignReservation)
    ));
    assert_eq!(root.work_charged_current(&view).unwrap(), 2_000_000);
    drop(gain);
    assert_eq!(root.work_charged_current(&view).unwrap(), 2_000_000);
    assert!(matches!(
        e::measure_explain(&candidate, &view),
        Err(e::Error::WorkAdmissionExhausted)
    ));
    cancel.cancel();
    assert!(matches!(
        e::measure_explain(&candidate, &view),
        Err(e::Error::Resource(ResourceError::Cancelled))
    ));
    // Restore creates fresh owners, never reactivates cancelled old owners.
    let (_fresh, _fresh_run, fresh_res) = run(Cancellation::default());
    let fresh_view = fresh_res.lease();
    let fresh_root = e::capture_explain(ri, &fresh_view).unwrap();
    let fresh_candidate = e::admit_explain(&fresh_root, ci, &fresh_view).unwrap();
    let fresh_gain = e::measure_explain(&fresh_candidate, &fresh_view).unwrap();
    assert!(fresh_gain
        .report_current(&fresh_candidate, expected, &fresh_view)
        .is_ok());
    assert!(matches!(
        fresh_gain.report_current(&candidate, expected, &fresh_view),
        Err(e::Error::CandidateMismatch)
    ));
}

#[test]
fn tlcx22_explain_resource_admission_precedes_shape_and_expiry_stays_current_failure() {
    let f = Fixture::explain();
    let bad = [e::Step {
        conclusion: &f.p,
        rule: e::Rule::AndElim {
            prior: e::StepId(9),
            child: 0,
        },
    }];
    let cases = [f.case(b"p", &f.p, &bad)];
    let ri = root_input(&cases);
    let (_br, _run, tiny) = run_sized(
        ri.binding.selection.source.digest(),
        Cancellation::default(),
        1,
        64 * 1024,
        Deadline::after(Duration::from_secs(120)).unwrap(),
    );
    assert!(matches!(
        e::capture_explain(ri, &tiny.lease()),
        Err(e::Error::Resource(ResourceError::Exhausted(_)))
    ));
    let good = f.projection(false, false);
    let cases = [f.case(b"p", &f.p, &good)];
    let ri = root_input(&cases);
    let deadline = Deadline::after(Duration::from_secs(2)).unwrap();
    let (_br, _run, res) = run_sized(
        ri.binding.selection.source.digest(),
        Cancellation::default(),
        64 * 1024 * 1024,
        1024 * 1024,
        deadline,
    );
    let view = res.lease();
    let root = e::capture_explain(ri, &view).unwrap();
    let responses = [response(b"p", &good)];
    let ci = input(1, &responses);
    let candidate = e::admit_explain(&root, ci, &view).unwrap();
    let gain = e::measure_explain(&candidate, &view).unwrap();
    std::thread::sleep(deadline.remaining().unwrap_or(Duration::ZERO) + Duration::from_millis(2));
    assert!(matches!(
        gain.report_current(
            &candidate,
            e::Expected {
                root: ri,
                candidate: ci
            },
            &view
        ),
        Err(e::Error::Resource(ResourceError::DeadlineExceeded))
    ));
}
